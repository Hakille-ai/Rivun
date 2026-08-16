use std::fs;
use tempfile::tempdir;
use uuid::Uuid;
use zap_journal::{
    JournalOptions, JournalProfile, JournalQuery, JournalRecordInput, JournalStore, ZapJournalError,
};

fn record_input(i: u64) -> JournalRecordInput {
    JournalRecordInput {
        kind: if i.is_multiple_of(2) {
            "alpha".to_string()
        } else {
            "beta".to_string()
        },
        schema_version: 1,
        timestamp_micros: 100_000 + i * 10,
        id: Some(Uuid::new_v4()),
        namespace: Some("test_ns".to_string()),
        subject: Some(format!("sub_{}", i % 5)),
        content_type: Some("application/octet-stream".to_string()),
        source_node: Some(Uuid::new_v4()),
        target_node: Some(Uuid::new_v4()),
        tombstone_for: None,
        metadata: serde_json::json!({ "sequence": i }),
        payload: format!("payload-data-{i}").into_bytes(),
    }
}

fn calculate_blake3_hash(bytes: &[u8]) -> String {
    format!("blake3:{}", blake3::hash(bytes).to_hex())
}

#[test]
fn test_journal_rapid_rotation_stress() {
    let temp = tempdir().unwrap();
    let options = JournalOptions {
        max_segment_bytes: 64 * 1024,
        max_segment_count: Some(5),
        max_segment_records: Some(4),
    };
    let store = JournalStore::open(temp.path(), JournalProfile::Receipts).with_options(options);

    let total = 100;
    for i in 0..total {
        store.append(record_input(i), false).unwrap();
    }

    let report = store.verify().unwrap();
    assert!(
        report.segments <= 5,
        "Expected at most 5 segments due to max_segment_count, found {}",
        report.segments
    );

    // Verify records query returns active records
    let records = store.records().unwrap();
    assert!(!records.is_empty());

    // Verify hash chain continuity on active segments
    for record in &records {
        assert!(!record.entry_hash.is_empty());
        assert!(!record.payload_hash.is_empty());
    }

    assert!(report.verified);
}

#[test]
fn test_journal_manifest_hash_integrity_under_rotation() {
    let temp = tempdir().unwrap();
    let options = JournalOptions {
        max_segment_records: Some(3),
        ..Default::default()
    };
    let store = JournalStore::open(temp.path(), JournalProfile::Memory).with_options(options);

    for i in 0..12 {
        store.append(record_input(i), false).unwrap();
        if (i + 1) % 3 == 0 {
            let seq = i / 3;
            let manifest = store.seal_segment(seq).unwrap();
            let seg_path = temp.path().join(format!("{seq:020}.zjseg"));
            let seg_bytes = fs::read(&seg_path).unwrap();
            let calculated_hash = calculate_blake3_hash(&seg_bytes);
            assert_eq!(
                manifest.segment_hash, calculated_hash,
                "Manifest segment hash mismatch for sequence {seq}"
            );
        }
    }
}

#[test]
fn test_journal_tampered_record_detection() {
    let temp = tempdir().unwrap();
    let store = JournalStore::open(temp.path(), JournalProfile::Receipts);

    for i in 0..5 {
        store.append(record_input(i), false).unwrap();
    }

    let seg_path = temp.path().join("00000000000000000000.zjseg");
    let mut bytes = fs::read(&seg_path).unwrap();

    // Flip byte in payload area
    let offset = bytes.len() - 15;
    bytes[offset] ^= 0xAA;
    fs::write(&seg_path, bytes).unwrap();

    let verify_res = store.verify();
    assert!(
        matches!(verify_res, Err(ZapJournalError::InvalidEntryHash { .. })),
        "Expected InvalidEntryHash error on tampered segment, got {:?}",
        verify_res
    );
}

#[test]
fn test_journal_corrupted_index_rebuild() {
    let temp = tempdir().unwrap();
    let store = JournalStore::open(temp.path(), JournalProfile::Memory);

    for i in 0..10 {
        store.append(record_input(i), false).unwrap();
    }

    let idx_path = temp.path().join("00000000000000000000.zjidx");
    fs::write(&idx_path, b"corrupted index line data\n").unwrap();

    let query_res = store.query(&JournalQuery::default()).unwrap();
    assert_eq!(query_res.len(), 10);

    // Ensure index file was auto-rebuilt
    let idx_content = fs::read_to_string(&idx_path).unwrap();
    assert_eq!(idx_content.lines().count(), 10);
}

#[test]
fn test_journal_partial_tail_recovery() {
    let temp = tempdir().unwrap();
    let store = JournalStore::open(temp.path(), JournalProfile::Receipts);

    for i in 0..5 {
        store.append(record_input(i), false).unwrap();
    }

    let seg_path = temp.path().join("00000000000000000000.zjseg");
    let valid_len = fs::metadata(&seg_path).unwrap().len();

    // Append partial unclosed record header
    let mut file = fs::OpenOptions::new().append(true).open(&seg_path).unwrap();
    use std::io::Write;
    file.write_all(b"ZJRC\x01\x00incomplete header bytes")
        .unwrap();
    drop(file);

    let tail = store.recover_partial_tail().unwrap();
    assert!(tail.is_some(), "Expected partial tail to be recovered");
    let tail_info = tail.unwrap();
    assert_eq!(tail_info.offset, valid_len);

    let new_len = fs::metadata(&seg_path).unwrap().len();
    assert_eq!(new_len, valid_len);
}
