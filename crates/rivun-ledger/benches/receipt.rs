use bytes::Bytes;
use criterion::{BatchSize, Criterion, criterion_group, criterion_main};
use std::hint::black_box;
use tempfile::TempDir;
use uuid::Uuid;
use rivun_core::{RivunFlags, RivunFrame};
use rivun_crypto::{Keypair, sign_frame};
use rivun_ledger::{
    ReceiptJournalStore, ReceiptReplicationRequest, ReceiptReplicationResponse, SignedActionReceipt,
};

fn signed_frame(source: &Keypair, target: Uuid) -> RivunFrame {
    let unsigned = RivunFrame::with_timestamp(
        source.node_id(),
        target,
        RivunFlags::SIGNED,
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

struct ReceiptJournalBench {
    _temp: TempDir,
    journal: ReceiptJournalStore,
    request: ReceiptReplicationRequest,
}

fn make_receipts(node: &Keypair, source: &Keypair, records: usize) -> Vec<SignedActionReceipt> {
    (0..records)
        .map(|index| {
            receipt_at(
                node,
                source,
                1_000_000 + index as u64,
                if index % 2 == 0 { "echo" } else { "telemetry" },
            )
        })
        .collect()
}

fn receipt_journal_fixture(records: usize) -> ReceiptJournalBench {
    let node = Keypair::generate();
    let source = Keypair::generate();
    let temp = tempfile::tempdir().unwrap();
    let journal = ReceiptJournalStore::open(temp.path().join("receipts"));
    for receipt in make_receipts(&node, &source, records) {
        journal.append(&receipt, false).unwrap();
    }
    let request = ReceiptReplicationRequest {
        kind: Some("action".to_string()),
        subject: Some("echo".to_string()),
        source_node: Some(source.node_id()),
        target_node: Some(node.node_id()),
        limit: Some(500),
        ..ReceiptReplicationRequest::default()
    };

    ReceiptJournalBench {
        _temp: temp,
        journal,
        request,
    }
}

fn bench_receipt_journal_size(c: &mut Criterion, records: usize) {
    let bench = receipt_journal_fixture(records);
    c.bench_function(
        &format!("ledger_receipt_journal_pull_500_from_{records}"),
        |b| {
            b.iter(|| {
                black_box(
                    bench
                        .journal
                        .query(black_box(&bench.request))
                        .unwrap()
                        .len(),
                )
            })
        },
    );
    c.bench_function(&format!("ledger_receipt_journal_verify_{records}"), |b| {
        b.iter(|| black_box(bench.journal.verify().unwrap()))
    });
}

fn ledger(c: &mut Criterion) {
    let node = Keypair::generate();
    let source = Keypair::generate();
    let frame = signed_frame(&source, node.node_id());
    let receipt = SignedActionReceipt::new(&node, &frame, "echo", Some(b"ok"), 456, None).unwrap();
    let receipts = (0..512)
        .map(|index| {
            receipt_at(
                &node,
                &source,
                400 + index,
                if index % 4 == 0 { "echo" } else { "telemetry" },
            )
        })
        .collect::<Vec<_>>();
    let response_1 = ReceiptReplicationResponse::new(node.node_id(), receipts[..1].to_vec(), false);
    let response_8 = ReceiptReplicationResponse::new(node.node_id(), receipts[..8].to_vec(), false);
    let response_64 =
        ReceiptReplicationResponse::new(node.node_id(), receipts[..64].to_vec(), false);
    let response_256 =
        ReceiptReplicationResponse::new(node.node_id(), receipts[..256].to_vec(), false);
    let temp = tempfile::tempdir().unwrap();
    let journal = ReceiptJournalStore::open(temp.path().join("receipts"));
    for receipt in &receipts {
        journal.append(receipt, false).unwrap();
    }
    let request = ReceiptReplicationRequest {
        after_processed_at_micros: Some(420),
        kind: Some("action".to_string()),
        subject: Some("echo".to_string()),
        source_node: Some(source.node_id()),
        target_node: Some(node.node_id()),
        ..ReceiptReplicationRequest::default()
    };

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
    c.bench_function("ledger_receipt_replication_response_verify_1", |b| {
        b.iter(|| {
            response_1.verify().unwrap();
            black_box(())
        })
    });
    c.bench_function("ledger_receipt_replication_response_verify_8", |b| {
        b.iter(|| {
            response_8.verify().unwrap();
            black_box(())
        })
    });
    c.bench_function("ledger_receipt_replication_response_verify_64", |b| {
        b.iter(|| {
            response_64.verify().unwrap();
            black_box(())
        })
    });
    c.bench_function("ledger_receipt_replication_response_verify_256", |b| {
        b.iter(|| {
            response_256.verify().unwrap();
            black_box(())
        })
    });
    c.bench_function("ledger_receipt_journal_all_verify_512", |b| {
        b.iter(|| {
            black_box(journal.all().unwrap());
            black_box(())
        })
    });
    if std::env::var_os("RIVUN_JOURNAL_BENCH").is_some() {
        let append_receipts = make_receipts(&node, &source, 1000);
        c.bench_function("ledger_receipt_journal_append_1000", |b| {
            b.iter_batched(
                || tempfile::tempdir().unwrap(),
                |temp| {
                    let journal = ReceiptJournalStore::open(temp.path().join("receipts"));
                    for receipt in &append_receipts {
                        journal.append(receipt, false).unwrap();
                    }
                    black_box(())
                },
                BatchSize::SmallInput,
            )
        });
        bench_receipt_journal_size(c, 1000);
    }

    if std::env::var_os("RIVUN_SCALE_BENCH").is_some() {
        bench_receipt_journal_size(c, 100_000);
        bench_receipt_journal_size(c, 1_000_000);
    }
}

criterion_group!(benches, ledger);
criterion_main!(benches);
