use tempfile::tempdir;
use std::fs::OpenOptions;
use std::io::Write;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;
use zap_net::durable_replay::DurableNonceStore;
use zap_net::ZapNetError;

const NONCE_LEN: usize = 12;

fn now_micros() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_micros() as u64
}

#[test]
fn stress_test_nonce_store_crash_restart_replay_flood() {
    let dir = tempdir().unwrap();
    let wal_path = dir.path().join("stress_nonces.wal");
    let node_id = Uuid::new_v4();
    let base_now = now_micros();
    let count = 5000;

    let mut initial_nonces = Vec::with_capacity(count);
    let mut new_nonces = Vec::with_capacity(count);

    // Phase 1: Write `count` nonces and simulate crash by dropping store
    {
        let mut store = DurableNonceStore::open(&wal_path, count * 2, 3_600_000_000).unwrap();
        for i in 0..count {
            let mut nonce = [0_u8; NONCE_LEN];
            nonce[0..8].copy_from_slice(&(i as u64).to_be_bytes());
            nonce[8..12].copy_from_slice(b"TEST");
            store.remember(node_id, nonce, base_now + (i as u64)).unwrap();
            initial_nonces.push(nonce);
        }
        // Drop store without clean shutdown (simulating abrupt process exit)
    }

    // Phase 2: Reopen from WAL (simulated process restart) and test 100% replay rejection
    {
        let mut store = DurableNonceStore::open(&wal_path, count * 2, 3_600_000_000).unwrap();
        let mut rejected = 0;
        for nonce in &initial_nonces {
            if store.contains(nonce) {
                let err = store.remember(node_id, *nonce, base_now + count as u64).unwrap_err();
                if matches!(err, ZapNetError::ReplayedDatagramNonce { .. }) {
                    rejected += 1;
                }
            }
        }
        assert_eq!(
            rejected, count,
            "100% of nonces must be rejected as replays after process restart! Rejected {} of {}",
            rejected, count
        );

        // Accept 5,000 new nonces
        for i in 0..count {
            let mut nonce = [0_u8; NONCE_LEN];
            nonce[0..8].copy_from_slice(&((count + i) as u64).to_be_bytes());
            nonce[8..12].copy_from_slice(b"NEW!");
            store.remember(node_id, nonce, base_now + count as u64 + (i as u64)).unwrap();
            new_nonces.push(nonce);
        }

        // Drop store again
    }

    // Phase 3: Reopen second time and verify both batches (10,000 total) are 100% rejected
    {
        let mut store = DurableNonceStore::open(&wal_path, count * 3, 3_600_000_000).unwrap();
        let mut rejected = 0;
        for nonce in initial_nonces.iter().chain(new_nonces.iter()) {
            if store.contains(nonce) {
                let err = store.remember(node_id, *nonce, base_now + count as u64 * 2).unwrap_err();
                if matches!(err, ZapNetError::ReplayedDatagramNonce { .. }) {
                    rejected += 1;
                }
            }
        }
        assert_eq!(
            rejected,
            count * 2,
            "100% of all accumulated nonces must be rejected across restarts!"
        );
    }
}

#[test]
fn stress_test_nonce_store_clock_jumps() {
    let dir = tempdir().unwrap();
    let wal_path = dir.path().join("clock_jumps.wal");
    let node_id = Uuid::new_v4();
    let base_time = now_micros();
    let max_age = 3_600_000_000_u64; // 1 hour in micros

    let nonce_valid = [1_u8; NONCE_LEN];
    let nonce_old = [2_u8; NONCE_LEN];

    {
        let mut store = DurableNonceStore::open(&wal_path, 100, max_age).unwrap();
        // Insert old nonce (2 hours ago) and valid nonce (10 seconds ago)
        store.remember(node_id, nonce_old, base_time.saturating_sub(2 * max_age)).unwrap();
        store.remember(node_id, nonce_valid, base_time.saturating_sub(10_000_000)).unwrap();
    }

    // Scenario A: Reopen store with clock within window
    {
        let store = DurableNonceStore::open(&wal_path, 100, max_age).unwrap();
        assert!(store.contains(&nonce_valid), "Valid nonce within max_age window must be retained");
        assert!(!store.contains(&nonce_old), "Old nonce outside max_age window must be pruned");
    }

    // Scenario B: Clock jump backward (system clock set back 1 hour)
    {
        let mut store = DurableNonceStore::open(&wal_path, 100, max_age).unwrap();
        assert!(store.contains(&nonce_valid));
        let err = store.remember(node_id, nonce_valid, base_time).unwrap_err();
        assert!(matches!(err, ZapNetError::ReplayedDatagramNonce { .. }));
    }
}

#[test]
fn stress_test_nonce_store_compaction_under_load() {
    let dir = tempdir().unwrap();
    let wal_path = dir.path().join("compact.wal");
    let node_id = Uuid::new_v4();
    let base_now = now_micros();
    let count = 2000;

    let mut nonces = Vec::new();
    {
        let mut store = DurableNonceStore::open(&wal_path, count * 2, 3_600_000_000).unwrap();
        for i in 0..count {
            let mut nonce = [0_u8; NONCE_LEN];
            nonce[0..8].copy_from_slice(&(i as u64).to_be_bytes());
            nonce[8..12].copy_from_slice(b"CMP!");
            store.remember(node_id, nonce, base_now + (i as u64)).unwrap();
            nonces.push(nonce);
        }

        // Perform explicit WAL compaction
        store.compact(base_now + count as u64).unwrap();
    }

    // Reopen store after compaction (simulating process restart post-compaction)
    {
        let mut store = DurableNonceStore::open(&wal_path, count * 2, 3_600_000_000).unwrap();
        let mut rejected = 0;
        for nonce in &nonces {
            if store.contains(nonce) {
                let err = store.remember(node_id, *nonce, base_now + count as u64 * 2).unwrap_err();
                if matches!(err, ZapNetError::ReplayedDatagramNonce { .. }) {
                    rejected += 1;
                }
            }
        }
        assert_eq!(
            rejected, count,
            "100% of nonces must survive compaction and be rejected as replays!"
        );
    }
}

#[test]
fn stress_test_nonce_store_partial_write_corruption() {
    let dir = tempdir().unwrap();
    let wal_path = dir.path().join("corrupt.wal");
    let node_id = Uuid::new_v4();
    let base_now = now_micros();

    let nonce1 = [1_u8; NONCE_LEN];
    let nonce2 = [2_u8; NONCE_LEN];

    {
        let mut store = DurableNonceStore::open(&wal_path, 100, 3_600_000_000).unwrap();
        store.remember(node_id, nonce1, base_now).unwrap();
    }

    // Simulate partial record write due to crash: append 15 unaligned bytes (less than 36B record length)
    {
        let mut file = OpenOptions::new().append(true).open(&wal_path).unwrap();
        file.write_all(b"TRUNCATED_GARB1").unwrap();
        file.flush().unwrap();
    }

    // Reopen store from partially corrupted file
    {
        let mut store = DurableNonceStore::open(&wal_path, 100, 3_600_000_000).unwrap();
        assert!(store.contains(&nonce1), "nonce1 should be preserved despite trailing garbage");

        // Write nonce2 into store
        store.remember(node_id, nonce2, base_now + 100).unwrap();
        assert!(store.contains(&nonce2));
    }

    // Reopen store again (simulating subsequent restart)
    {
        let store = DurableNonceStore::open(&wal_path, 100, 3_600_000_000).unwrap();
        let has_n1 = store.contains(&nonce1);
        let has_n2 = store.contains(&nonce2);

        println!("Partial write test: has_n1={}, has_n2={}", has_n1, has_n2);
        assert!(has_n1, "nonce1 must be preserved");
        assert!(has_n2, "nonce2 appended after corruption must be preserved");
    }
}

#[test]
fn stress_test_nonce_store_concurrent_access() {
    let dir = tempdir().unwrap();
    let wal_path = dir.path().join("concurrent.wal");
    let node_id = Uuid::new_v4();
    let base_now = now_micros();

    let store = Arc::new(Mutex::new(
        DurableNonceStore::open(&wal_path, 10_000, 3_600_000_000).unwrap(),
    ));

    let threads: Vec<_> = (0..10)
        .map(|t| {
            let store = Arc::clone(&store);
            thread::spawn(move || {
                for i in 0..500 {
                    let mut nonce = [0_u8; NONCE_LEN];
                    nonce[0..4].copy_from_slice(&(t as u32).to_be_bytes());
                    nonce[4..12].copy_from_slice(&(i as u64).to_be_bytes());
                    let mut guard = store.lock().unwrap();
                    guard.remember(node_id, nonce, base_now + (i as u64)).unwrap();
                }
            })
        })
        .collect();

    for t in threads {
        t.join().unwrap();
    }

    // Reopen store after multi-threaded flood and verify all 5,000 nonces are present and rejected
    {
        let mut store = DurableNonceStore::open(&wal_path, 10_000, 3_600_000_000).unwrap();
        let mut count = 0;
        for t in 0..10 {
            for i in 0..500 {
                let mut nonce = [0_u8; NONCE_LEN];
                nonce[0..4].copy_from_slice(&(t as u32).to_be_bytes());
                nonce[4..12].copy_from_slice(&(i as u64).to_be_bytes());
                if store.contains(&nonce) {
                    let err = store.remember(node_id, nonce, base_now + 1_000_000).unwrap_err();
                    if matches!(err, ZapNetError::ReplayedDatagramNonce { .. }) {
                        count += 1;
                    }
                }
            }
        }
        assert_eq!(count, 5000, "All 5,000 nonces inserted concurrently must survive restart!");
    }
}
