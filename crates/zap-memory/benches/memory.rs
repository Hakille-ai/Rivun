use criterion::{BatchSize, Criterion, criterion_group, criterion_main};
use serde_json::Value;
use std::hint::black_box;
use tempfile::TempDir;
use zap_memory::{MemoryJournalStore, MemoryPut, MemoryQuery, MemoryStore};

struct MemoryBench {
    _temp: TempDir,
    store: MemoryJournalStore,
}

fn input(subject: String, body: Vec<u8>) -> MemoryPut {
    MemoryPut {
        namespace: "default".to_string(),
        subject,
        content_type: "application/json".to_string(),
        body,
        metadata: Value::Null,
        source_node: None,
        frame_hash: None,
    }
}

fn memory_fixture(records: usize) -> MemoryBench {
    let temp = tempfile::tempdir().unwrap();
    let store = MemoryJournalStore::open(temp.path().join("memory"));
    let mut tombstone_candidates = Vec::new();

    for index in 0..records {
        let body = format!(r#"{{"index":{index},"value":"benchmark"}}"#).into_bytes();
        let record = store
            .put(input(format!("sensor.{}", index % 8), body))
            .unwrap();
        if index % 16 == 0 {
            tombstone_candidates.push(record.id);
        }
    }
    for record_id in tombstone_candidates {
        store
            .tombstone(record_id, Some("benchmark tombstone".to_string()))
            .unwrap();
    }

    MemoryBench { _temp: temp, store }
}

fn append_records(store: &MemoryJournalStore, records: usize) {
    for index in 0..records {
        let body = format!(r#"{{"index":{index},"value":"append"}}"#).into_bytes();
        store
            .put(input(format!("sensor.{}", index % 32), body))
            .unwrap();
    }
}

fn bench_memory_size(c: &mut Criterion, records: usize) {
    let bench = memory_fixture(records);
    let query = MemoryQuery {
        namespace: Some("default".to_string()),
        subject: Some("sensor.3".to_string()),
        content_type: Some("application/json".to_string()),
        include_tombstoned: false,
        limit: Some(500),
    };

    c.bench_function(&format!("memory_query_subject_journal_{records}"), |b| {
        b.iter(|| black_box(bench.store.query(black_box(&query)).unwrap()))
    });
    c.bench_function(&format!("memory_verify_journal_{records}"), |b| {
        b.iter(|| black_box(bench.store.verify().unwrap()))
    });
}

fn memory(c: &mut Criterion) {
    c.bench_function("memory_append_journal_1000", |b| {
        b.iter_batched(
            || tempfile::tempdir().unwrap(),
            |temp| {
                let store = MemoryJournalStore::open(temp.path().join("memory"));
                append_records(&store, 1000);
                black_box(())
            },
            BatchSize::SmallInput,
        )
    });
    bench_memory_size(c, 1000);

    if std::env::var_os("ZAP_SCALE_BENCH").is_some() {
        bench_memory_size(c, 100_000);
        bench_memory_size(c, 1_000_000);
    }
}

criterion_group!(benches, memory);
criterion_main!(benches);
