use bytes::Bytes;
use criterion::{Criterion, criterion_group, criterion_main};
use std::hint::black_box;
use uuid::Uuid;
use zap_core::{ZapFlags, ZapFrame};
use zap_crypto::{Keypair, sign_frame};
use zap_ledger::{ReceiptReplicationRequest, ReceiptReplicationResponse, SignedActionReceipt};

fn signed_frame(source: &Keypair, target: Uuid) -> ZapFrame {
    let unsigned = ZapFrame::with_timestamp(
        source.node_id(),
        target,
        ZapFlags::SIGNED,
        123,
        Bytes::from_static(b"benchmark-payload"),
    )
    .unwrap();
    sign_frame(source, &unsigned).unwrap()
}

fn receipt_at(
    node: &Keypair,
    source: &Keypair,
    processed_at_micros: u64,
    subject: &str,
) -> SignedActionReceipt {
    let frame = signed_frame(source, node.node_id());
    SignedActionReceipt::new_message(
        node,
        &frame,
        "action",
        subject,
        Some(b"ok"),
        processed_at_micros,
        None,
    )
    .unwrap()
}

fn ledger(c: &mut Criterion) {
    let node = Keypair::generate();
    let source = Keypair::generate();
    let frame = signed_frame(&source, node.node_id());
    let receipt = SignedActionReceipt::new(&node, &frame, "echo", Some(b"ok"), 456, None).unwrap();
    let receipts = (0..64)
        .map(|index| {
            receipt_at(
                &node,
                &source,
                400 + index,
                if index % 4 == 0 { "echo" } else { "telemetry" },
            )
        })
        .collect::<Vec<_>>();
    let request = ReceiptReplicationRequest {
        after_processed_at_micros: Some(420),
        kind: Some("action".to_string()),
        subject: Some("echo".to_string()),
        source_node: Some(source.node_id()),
        target_node: Some(node.node_id()),
        ..ReceiptReplicationRequest::default()
    };
    let response = ReceiptReplicationResponse::new(node.node_id(), receipts[..8].to_vec(), false);

    c.bench_function("ledger_sign_action_receipt", |b| {
        b.iter(|| {
            black_box(
                SignedActionReceipt::new(&node, black_box(&frame), "echo", Some(b"ok"), 456, None)
                    .unwrap(),
            )
        })
    });
    c.bench_function("ledger_verify_action_receipt", |b| {
        b.iter(|| {
            receipt.verify().unwrap();
            black_box(())
        })
    });
    c.bench_function("ledger_receipt_replication_filter_64", |b| {
        b.iter(|| {
            black_box(
                receipts
                    .iter()
                    .filter(|receipt| request.matches(black_box(receipt)))
                    .count(),
            )
        })
    });
    c.bench_function("ledger_receipt_replication_response_verify_8", |b| {
        b.iter(|| {
            response.verify().unwrap();
            black_box(())
        })
    });
}

criterion_group!(benches, ledger);
criterion_main!(benches);
