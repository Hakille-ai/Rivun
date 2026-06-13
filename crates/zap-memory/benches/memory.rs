use criterion::{Criterion, criterion_group, criterion_main};
use serde_json::Value;
use std::hint::black_box;
use tempfile::TempDir;
use zap_memory::{JsonlMemoryStore, MemoryPut, MemoryQuery, MemoryStore};

struct MemoryBench {
    _temp: TempDir,
    store: JsonlMemoryStore,
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
    let store = JsonlMemoryStore::open(temp.path().join("memory.jsonl"));
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

fn memory(c: &mut Criterion) {
    let bench = memory_fixture(64);
    let query = MemoryQuery {
        namespace: Some("default".to_string()),
        subject: Some("sensor.3".to_string()),
        content_type: Some("application/json".to_string()),
        include_tombstoned: false,
        limit: Some(4),
    };

    c.bench_function("memory_query_subject_64_records", |b| {
        b.iter(|| black_box(bench.store.query(black_box(&query)).unwrap()))
    });
    c.bench_function("memory_verify_jsonl_64_records", |b| {
        b.iter(|| black_box(bench.store.verify().unwrap()))
    });
}

criterion_group!(benches, memory);
criterion_main!(benches);
