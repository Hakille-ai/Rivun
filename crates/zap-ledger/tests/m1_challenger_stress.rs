use base64::Engine as _;
use bytes::Bytes;
use std::fs;
use tempfile::tempdir;
use uuid::Uuid;
use zap_core::{ZapFlags, ZapFrame};
use zap_crypto::{Keypair, sign_frame};
use zap_ledger::{
    ReceiptJournalStore, ReceiptReplicationRequest, ReceiptSegmentIndex, ReceiptSegmentManifest,
    SignedActionReceipt, SignedReceiptSegmentManifest, ZapLedgerError, hash_bytes,
};

fn make_receipt(
    node: &Keypair,
    source: &Keypair,
    processed_at_micros: u64,
    kind: &str,
    subject: &str,
) -> SignedActionReceipt {
    let unsigned = ZapFrame::with_timestamp(
        source.node_id(),
        node.node_id(),
        ZapFlags::SIGNED,
        processed_at_micros.saturating_sub(10),
        Bytes::from_static(b"payload"),
    )
    .unwrap();
    let frame = sign_frame(source, &unsigned).unwrap();
    SignedActionReceipt::new_message(
        node,
        &frame,
        kind,
        subject,
        Some(b"output_bytes"),
        processed_at_micros,
        None,
    )
    .unwrap()
}

#[test]
fn test_rapid_rotation_and_sealing_stress() {
    let temp = tempdir().unwrap();
    let node = Keypair::generate();
    let source = Keypair::generate();

    // Force rotation every 2 records
    let options = zap_journal::JournalOptions {
        max_segment_bytes: 64 * 1024,
        max_segment_count: None,
        max_segment_records: Some(2),
    };

    let store =
        ReceiptJournalStore::open_with_keypair(temp.path(), node.clone()).with_options(options);

    let total_records = 100;
    for i in 0..total_records {
        let receipt = make_receipt(
            &node,
            &source,
            1_000 + i * 10,
            if i.is_multiple_of(2) { "even" } else { "odd" },
            "sensor",
        );
        store.append(&receipt, false).unwrap();
        // Seal after every 2 records
        if (i + 1).is_multiple_of(2) {
            let seq = i / 2;
            let signed_manifest = store.rotate_and_seal_segment(seq).unwrap();
            signed_manifest.verify().unwrap();
        }
    }

    let all_receipts = store.all().unwrap();
    assert_eq!(all_receipts.len(), total_records as usize);

    // Verify index building across all 50 segments
    let index = store.build_and_verify_segment_index().unwrap();
    assert_eq!(index.entries.len(), 50);

    // Run query_fast across all segments
    let req = ReceiptReplicationRequest {
        after_processed_at_micros: Some(1_100),
        until_processed_at_micros: Some(1_500),
        kind: Some("even".to_string()),
        ..ReceiptReplicationRequest::default()
    };
    let fast_results = store.query_fast(&req).unwrap();
    let slow_results = store.query_with_limit(&req, 50).unwrap();

    assert_eq!(fast_results.len(), slow_results.len());
    assert_eq!(fast_results, slow_results);
}

#[test]
fn test_rapid_rotation_with_segment_pruning() {
    let temp = tempdir().unwrap();
    let node = Keypair::generate();
    let source = Keypair::generate();

    // Force rotation every 2 records, max 5 segments kept on disk
    let options = zap_journal::JournalOptions {
        max_segment_bytes: 64 * 1024,
        max_segment_count: Some(5),
        max_segment_records: Some(2),
    };

    let store =
        ReceiptJournalStore::open_with_keypair(temp.path(), node.clone()).with_options(options);

    for i in 0..30 {
        let receipt = make_receipt(&node, &source, 1_000 + i * 10, "prune_test", "sensor");
        store.append(&receipt, false).unwrap();
        if (i + 1).is_multiple_of(2) {
            let seq = i / 2;
            if store.dir().join(format!("{seq:020}.zjseg")).exists() {
                let _ = store.rotate_and_seal_segment(seq);
            }
        }
    }

    // Check how index building behaves when sequence 0 has been pruned!
    let index_res = store.build_and_verify_segment_index();
    println!("Index build result after pruning: {:?}", index_res);

    let req = ReceiptReplicationRequest {
        after_processed_at_micros: Some(1_200),
        ..ReceiptReplicationRequest::default()
    };
    let queried = store.query_fast(&req).unwrap();
    println!("Queried records after pruning: {}", queried.len());
}

#[test]
fn test_signature_and_manifest_tampering() {
    let temp = tempdir().unwrap();
    let node = Keypair::generate();
    let source = Keypair::generate();

    let options = zap_journal::JournalOptions {
        max_segment_records: Some(5),
        ..Default::default()
    };

    let store =
        ReceiptJournalStore::open_with_keypair(temp.path(), node.clone()).with_options(options);

    for i in 0..10 {
        let receipt = make_receipt(&node, &source, 1_000 + i * 10, "tamper", "sensor");
        store.append(&receipt, false).unwrap();
    }
    let manifest0 = store.rotate_and_seal_segment(0).unwrap();
    let manifest1 = store.rotate_and_seal_segment(1).unwrap();
    manifest0.verify().unwrap();
    manifest1.verify().unwrap();

    // 1. Signature Byte Corruption
    let sig_path = store.signed_manifest_path(0);
    let content = fs::read_to_string(&sig_path).unwrap();
    let mut signed_json: serde_json::Value = serde_json::from_str(&content).unwrap();

    let orig_sig = signed_json["signature"].as_str().unwrap().to_string();
    let mut bad_sig = orig_sig.clone();
    // Flip a character in base64 string
    let last_char = bad_sig.pop().unwrap();
    bad_sig.push(if last_char == 'A' { 'B' } else { 'A' });
    signed_json["signature"] = serde_json::Value::String(bad_sig);
    fs::write(
        &sig_path,
        serde_json::to_string_pretty(&signed_json).unwrap(),
    )
    .unwrap();

    let verify_res = store.load_signed_manifest(0);
    assert!(
        matches!(
            verify_res,
            Err(ZapLedgerError::InvalidSignature) | Err(ZapLedgerError::Base64(_))
        ),
        "Expected InvalidSignature or Base64 error, got: {:?}",
        verify_res
    );

    // Restore valid signature
    signed_json["signature"] = serde_json::Value::String(orig_sig);
    fs::write(
        &sig_path,
        serde_json::to_string_pretty(&signed_json).unwrap(),
    )
    .unwrap();

    // 2. Signer Public Key Tampering (Replacing with another key)
    let other_key = Keypair::generate();
    let other_pub_b64 = base64::engine::general_purpose::STANDARD_NO_PAD
        .encode(other_key.verifying_key().to_bytes());
    signed_json["signer_public_key"] = serde_json::Value::String(other_pub_b64);
    fs::write(
        &sig_path,
        serde_json::to_string_pretty(&signed_json).unwrap(),
    )
    .unwrap();

    let verify_res = store.load_signed_manifest(0);
    assert!(
        matches!(
            verify_res,
            Err(ZapLedgerError::SegmentManifestSignerNodeMismatch { .. })
                | Err(ZapLedgerError::InvalidSignature)
        ),
        "Expected node mismatch or invalid signature error, got: {:?}",
        verify_res
    );

    // 3. Segment Content Tampering
    let seg_path = store.dir().join("00000000000000000000.zjseg");
    let mut seg_bytes = fs::read(&seg_path).unwrap();
    let len = seg_bytes.len();
    seg_bytes[len - 10] ^= 0xFF;
    fs::write(&seg_path, seg_bytes).unwrap();

    let read_res = store.read_segment_receipts(0);
    assert!(
        read_res.is_err(),
        "Expected segment read error after tampering segment file, got Ok"
    );
}

#[test]
fn test_manifest_chain_tampering() {
    let node = Keypair::generate();
    let source = Keypair::generate();

    let r1 = vec![make_receipt(&node, &source, 1000, "kind", "sub")];
    let m1 = SignedReceiptSegmentManifest::sign(
        &node,
        ReceiptSegmentManifest::from_receipts(Uuid::new_v4(), 0, &r1, None).unwrap(),
    )
    .unwrap();

    let r2 = vec![make_receipt(&node, &source, 2000, "kind", "sub")];
    let m2 = SignedReceiptSegmentManifest::sign(
        &node,
        ReceiptSegmentManifest::from_receipts(
            Uuid::new_v4(),
            1,
            &r2,
            Some(m1.manifest.segment_hash.clone()),
        )
        .unwrap(),
    )
    .unwrap();

    let index = ReceiptSegmentIndex::from_manifests(node.node_id(), &[m1, m2]).unwrap();
    index.validate().unwrap();

    // Corrupt chain hash in sequence 1
    let mut bad_index = index.clone();
    bad_index.entries[1].previous_segment_hash = Some(hash_bytes(b"bogus segment hash"));
    assert!(matches!(
        bad_index.validate(),
        Err(ZapLedgerError::ReceiptSegmentChainMismatch { .. })
    ));
}

#[test]
fn test_query_fast_correctness_and_boundary_conditions() {
    let temp = tempdir().unwrap();
    let node = Keypair::generate();
    let source = Keypair::generate();

    let store = ReceiptJournalStore::open_with_keypair(temp.path(), node.clone());

    for i in 0..50 {
        let receipt = make_receipt(
            &node,
            &source,
            1_000 + i * 100,
            if i.is_multiple_of(3) { "alpha" } else { "beta" },
            "target_subject",
        );
        store.append(&receipt, false).unwrap();
    }
    store.rotate_and_seal_segment(0).unwrap();

    // Boundary check 1: Exact after timestamp match
    let req = ReceiptReplicationRequest {
        after_processed_at_micros: Some(2_000),
        until_processed_at_micros: Some(3_000),
        ..ReceiptReplicationRequest::default()
    };
    let results = store.query_fast(&req).unwrap();
    assert!(
        results.iter().all(
            |r| r.receipt.processed_at_micros > 2_000 && r.receipt.processed_at_micros <= 3_000
        )
    );

    // Boundary check 2: Limit enforcement
    let req_limit = ReceiptReplicationRequest {
        limit: Some(5),
        ..ReceiptReplicationRequest::default()
    };
    let results_limit = store.query_fast(&req_limit).unwrap();
    assert_eq!(results_limit.len(), 5);
}

#[test]
fn test_corruption_recovery_and_tail_truncation() {
    let temp = tempdir().unwrap();
    let node = Keypair::generate();
    let source = Keypair::generate();

    let store = ReceiptJournalStore::open_with_keypair(temp.path(), node.clone());

    for i in 0..10 {
        let receipt = make_receipt(&node, &source, 1_000 + i * 10, "tail_test", "sub");
        store.append(&receipt, false).unwrap();
    }

    let seg_path = store.dir().join("00000000000000000000.zjseg");
    let orig_len = fs::metadata(&seg_path).unwrap().len();

    // Append corrupted garbage at the end (partial record)
    let mut file = fs::OpenOptions::new().append(true).open(&seg_path).unwrap();
    use std::io::Write;
    file.write_all(b"ZJRC\x01\x00\x00\x00corrupted garbage tail bytes")
        .unwrap();
    drop(file);

    assert!(fs::metadata(&seg_path).unwrap().len() > orig_len);

    let recovered = store.recover_partial_tail().unwrap();
    assert!(recovered, "Expected partial tail recovery to succeed");

    assert_eq!(fs::metadata(&seg_path).unwrap().len(), orig_len);
    assert!(store.verify().unwrap().verified);
}
