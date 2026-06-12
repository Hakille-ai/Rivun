use serde_json::json;
use tempfile::tempdir;
use uuid::Uuid;
use zap_memory::{JsonlMemoryStore, MemoryPut, MemoryQuery, MemoryStore};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== ZAP Hash-Chained Memory Store ===");

    // 1. Open a local memory store backed by a JSONL file
    // We use a temporary directory for demonstration purposes.
    let dir = tempdir()?;
    let memory_file = dir.path().join("audit_memory.jsonl");
    let store = JsonlMemoryStore::open(&memory_file);

    println!("\nMemory store initialized at: {}", memory_file.display());

    // 2. Put records into the store
    // Every record contains content (body), structured metadata (json), and optional network frame contexts
    let record1 = store.put(MemoryPut {
        namespace: "telemetry".to_string(),
        subject: "sensor.temperature".to_string(),
        content_type: "application/json".to_string(),
        body: serde_json::to_vec(&json!({ "temperature_c": 21.5 }))?,
        metadata: json!({ "sensor_id": "sensor-west-1", "accuracy": "high" }),
        source_node: Some(Uuid::new_v4()),
        frame_hash: Some("blake3:0102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f20".to_string()),
    })?;

    println!("\n1. First Record Saved:");
    println!("  ID: {}", record1.id);
    println!("  Namespace: {}", record1.namespace);
    println!("  Subject: {}", record1.subject);
    println!("  Body Hash: {}", record1.body_hash);
    println!("  Previous Entry Hash: {:?}", record1.previous_entry_hash);
    println!("  Entry Hash: {:?}", record1.entry_hash);

    // Write a second record to establish the cryptographic hash chain
    let record2 = store.put(MemoryPut {
        namespace: "telemetry".to_string(),
        subject: "sensor.humidity".to_string(),
        content_type: "application/json".to_string(),
        body: serde_json::to_vec(&json!({ "humidity_pct": 48.0 }))?,
        metadata: json!({ "sensor_id": "sensor-west-1" }),
        source_node: Some(Uuid::new_v4()),
        frame_hash: None,
    })?;

    println!("\n2. Second Record Saved (Linked to First):");
    println!("  ID: {}", record2.id);
    println!("  Previous Entry Hash (matches first's entry hash):");
    println!("    {:?}", record2.previous_entry_hash);
    println!("  Entry Hash: {:?}", record2.entry_hash);

    // 3. Query records
    println!("\n3. Querying Memory Store for namespace='telemetry':");
    let query = MemoryQuery {
        namespace: Some("telemetry".to_string()),
        ..MemoryQuery::default()
    };
    let results = store.query(&query)?;
    println!("  Query returned {} records:", results.len());
    for record in &results {
        println!("    - ID: {}, Subject: {}", record.id, record.subject);
    }

    // 4. Verify/Audit memory integrity
    // This parses all JSON lines, recalculates individual blake3 entry/body hashes,
    // and checks the chronological link of the hash chain.
    println!("\n4. Performing Cryptographic Integrity Audit:");
    let audit_report = store.verify()?;
    println!("  Audit Status: verified = {}", audit_report.verified);
    println!("  Total entries analyzed: {}", audit_report.entries);
    println!("  Total records found:    {}", audit_report.records);
    println!("  Total tombstones found: {}", audit_report.tombstones);

    // 5. Delete records using tombstones
    // ZAP uses soft-deletion (appending a tombstone record) to preserve the historical audit trail.
    println!("\nTombstoning record 1 (ID: {})...", record1.id);
    store.tombstone(record1.id, Some("Sensor recalibrated".to_string()))?;

    // Try to query again (should not return record1 by default)
    let active_records = store.query(&MemoryQuery {
        namespace: Some("telemetry".to_string()),
        include_tombstoned: false, // Default
        ..MemoryQuery::default()
    })?;
    println!("\nQuerying after tombstone (include_tombstoned = false):");
    println!("  Active records: {}", active_records.len());
    for r in &active_records {
        println!("    - ID: {}, Subject: {}", r.id, r.subject);
    }

    // Query including tombstoned records (history view)
    let all_records = store.query(&MemoryQuery {
        namespace: Some("telemetry".to_string()),
        include_tombstoned: true,
        ..MemoryQuery::default()
    })?;
    println!("\nQuerying after tombstone (include_tombstoned = true):");
    println!("  Historical records: {}", all_records.len());
    for r in &all_records {
        println!("    - ID: {}, Subject: {} (Tombstoned)", r.id, r.subject);
    }

    // 6. Final verification report includes the tombstone in the chain
    let final_audit = store.verify()?;
    println!("\nFinal Cryptographic Integrity Audit:");
    println!("  Audit Status: verified = {}", final_audit.verified);
    println!("  Total entries:     {}", final_audit.entries);
    println!("  Total records:     {}", final_audit.records);
    println!("  Total tombstones:  {}", final_audit.tombstones);

    Ok(())
}
