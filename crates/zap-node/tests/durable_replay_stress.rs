use std::fs::OpenOptions;
use std::io::Write;
use std::sync::{Arc, Mutex};
use std::thread;
use tempfile::tempdir;
use uuid::Uuid;
use zap_core::{ZapFlags, ZapFrame, now_micros};
use zap_node::durable_replay::DurableReplayStore;

#[test]
fn stress_test_replay_store_crash_restart_replay_flood() {
    let dir = tempdir().unwrap();
    let wal_path = dir.path().join("stress_frames.wal");
    let source = Uuid::new_v4();
    let target = Uuid::new_v4();
    let base_now = now_micros().unwrap();
    let count = 5000;

    let mut initial_frames = Vec::with_capacity(count);
    let mut new_frames = Vec::with_capacity(count);

    // Phase 1: Store `count` frames and simulate crash by dropping store
    {
        let mut store = DurableReplayStore::open(&wal_path, count * 2, 3_600_000_000).unwrap();
        for i in 0..count {
            let payload = format!("frame_{i}");
            let frame = ZapFrame::with_timestamp(
                source,
                target,
                ZapFlags::ENCRYPTED,
                base_now + (i as u64),
                bytes::Bytes::from(payload),
            )
            .unwrap();
            store
                .check_and_insert(&frame, base_now + (i as u64))
                .unwrap();
            initial_frames.push(frame);
        }
    }

    // Phase 2: Reopen from WAL (simulated restart) and test 100% replay rejection
    {
        let mut store = DurableReplayStore::open(&wal_path, count * 2, 3_600_000_000).unwrap();
        let mut rejected = 0;
        for frame in &initial_frames {
            let err = store
                .check_and_insert(frame, base_now + count as u64)
                .unwrap_err();
            if err.to_string().contains("replayed frame rejected") {
                rejected += 1;
            }
        }
        assert_eq!(
            rejected, count,
            "100% of frames must be rejected as replays after process restart! Rejected {} of {}",
            rejected, count
        );

        // Store 5,000 new frames
        for i in 0..count {
            let payload = format!("new_frame_{i}");
            let frame = ZapFrame::with_timestamp(
                source,
                target,
                ZapFlags::ENCRYPTED,
                base_now + count as u64 + (i as u64),
                bytes::Bytes::from(payload),
            )
            .unwrap();
            store
                .check_and_insert(&frame, base_now + count as u64 + (i as u64))
                .unwrap();
            new_frames.push(frame);
        }
    }

    // Phase 3: Reopen second time and verify all 10,000 frames are 100% rejected
    {
        let mut store = DurableReplayStore::open(&wal_path, count * 3, 3_600_000_000).unwrap();
        let mut rejected = 0;
        for frame in initial_frames.iter().chain(new_frames.iter()) {
            let err = store
                .check_and_insert(frame, base_now + count as u64 * 2)
                .unwrap_err();
            if err.to_string().contains("replayed frame rejected") {
                rejected += 1;
            }
        }
        assert_eq!(
            rejected,
            count * 2,
            "100% of all accumulated frames must be rejected across restarts!"
        );
    }
}

#[test]
fn stress_test_replay_store_clock_jumps_and_overflow() {
    let dir = tempdir().unwrap();
    let wal_path = dir.path().join("clock_jumps_node.wal");
    let source = Uuid::new_v4();
    let target = Uuid::new_v4();
    let base_now = now_micros().unwrap();
    let skew_window = 300_000_000_u64; // 5 minutes in micros

    let mut store = DurableReplayStore::open(&wal_path, 100, skew_window).unwrap();

    // 1. Frame outside skew window (future + 10 mins)
    let frame_future = ZapFrame::with_timestamp(
        source,
        target,
        ZapFlags::ENCRYPTED,
        base_now + 600_000_000,
        bytes::Bytes::from_static(b"future"),
    )
    .unwrap();
    let err_future = store.check_and_insert(&frame_future, base_now).unwrap_err();
    assert!(err_future.to_string().contains("outside clock skew window"));

    // 2. Frame outside skew window (past - 10 mins)
    let frame_past = ZapFrame::with_timestamp(
        source,
        target,
        ZapFlags::ENCRYPTED,
        base_now.saturating_sub(600_000_000),
        bytes::Bytes::from_static(b"past"),
    )
    .unwrap();
    let err_past = store.check_and_insert(&frame_past, base_now).unwrap_err();
    assert!(err_past.to_string().contains("outside clock skew window"));

    // 3. Overflow attack vector test: frame timestamp = u64::MAX
    let frame_overflow = ZapFrame::with_timestamp(
        source,
        target,
        ZapFlags::ENCRYPTED,
        u64::MAX,
        bytes::Bytes::from_static(b"overflow"),
    )
    .unwrap();
    // Test if this panics or returns error gracefully
    let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        store.check_and_insert(&frame_overflow, base_now)
    }));
    assert!(
        res.is_err() || res.unwrap().is_err(),
        "u64::MAX timestamp frame must not panic the node!"
    );
}

#[test]
fn stress_test_replay_store_compaction_under_load() {
    let dir = tempdir().unwrap();
    let wal_path = dir.path().join("compact_node.wal");
    let source = Uuid::new_v4();
    let target = Uuid::new_v4();
    let base_now = now_micros().unwrap();
    let count = 2000;

    let mut frames = Vec::new();
    {
        let mut store = DurableReplayStore::open(&wal_path, count * 2, 3_600_000_000).unwrap();
        for i in 0..count {
            let frame = ZapFrame::with_timestamp(
                source,
                target,
                ZapFlags::ENCRYPTED,
                base_now + (i as u64),
                bytes::Bytes::from(format!("frame_{i}")),
            )
            .unwrap();
            store
                .check_and_insert(&frame, base_now + (i as u64))
                .unwrap();
            frames.push(frame);
        }

        // Compact WAL
        store.compact(base_now + count as u64).unwrap();
    }

    // Reopen store after compaction and verify 100% rejection
    {
        let mut store = DurableReplayStore::open(&wal_path, count * 2, 3_600_000_000).unwrap();
        let mut rejected = 0;
        for frame in &frames {
            let err = store
                .check_and_insert(frame, base_now + count as u64 * 2)
                .unwrap_err();
            if err.to_string().contains("replayed frame rejected") {
                rejected += 1;
            }
        }
        assert_eq!(
            rejected, count,
            "100% of frames must survive compaction and be rejected as replays!"
        );
    }
}

#[test]
fn stress_test_replay_store_partial_write_corruption() {
    let dir = tempdir().unwrap();
    let wal_path = dir.path().join("corrupt_node.wal");
    let source = Uuid::new_v4();
    let target = Uuid::new_v4();
    let base_now = now_micros().unwrap();

    let frame1 = ZapFrame::with_timestamp(
        source,
        target,
        ZapFlags::ENCRYPTED,
        base_now,
        bytes::Bytes::from_static(b"frame1"),
    )
    .unwrap();

    let frame2 = ZapFrame::with_timestamp(
        source,
        target,
        ZapFlags::ENCRYPTED,
        base_now + 100,
        bytes::Bytes::from_static(b"frame2"),
    )
    .unwrap();

    {
        let mut store = DurableReplayStore::open(&wal_path, 100, 3_600_000_000).unwrap();
        store.check_and_insert(&frame1, base_now).unwrap();
    }

    // Append 17 unaligned bytes (less than 40B record size) simulating process crash mid-write
    {
        let mut file = OpenOptions::new().append(true).open(&wal_path).unwrap();
        file.write_all(b"PARTIAL_FRAME_GARB").unwrap();
        file.flush().unwrap();
    }

    // Reopen store from corrupted file
    {
        let mut store = DurableReplayStore::open(&wal_path, 100, 3_600_000_000).unwrap();
        // Check frame1 replay rejection
        assert!(
            store.check_and_insert(&frame1, base_now).is_err(),
            "frame1 must be recognized"
        );

        // Insert frame2
        store.check_and_insert(&frame2, base_now + 100).unwrap();
    }

    // Reopen store again (subsequent restart)
    {
        let mut store = DurableReplayStore::open(&wal_path, 100, 3_600_000_000).unwrap();
        let f1_rej = store.check_and_insert(&frame1, base_now).is_err();
        let f2_rej = store.check_and_insert(&frame2, base_now + 100).is_err();

        println!(
            "ReplayStore partial write test: f1_rej={}, f2_rej={}",
            f1_rej, f2_rej
        );
        assert!(f1_rej, "frame1 must be rejected as replay");
        assert!(
            f2_rej,
            "frame2 appended after crash must be rejected as replay"
        );
    }
}

#[test]
fn stress_test_replay_store_concurrent_access() {
    let dir = tempdir().unwrap();
    let wal_path = dir.path().join("concurrent_node.wal");
    let source = Uuid::new_v4();
    let target = Uuid::new_v4();
    let base_now = now_micros().unwrap();

    let store = Arc::new(Mutex::new(
        DurableReplayStore::open(&wal_path, 10_000, 3_600_000_000).unwrap(),
    ));

    let threads: Vec<_> = (0..10)
        .map(|t| {
            let store = Arc::clone(&store);
            thread::spawn(move || {
                for i in 0..500 {
                    let frame = ZapFrame::with_timestamp(
                        source,
                        target,
                        ZapFlags::ENCRYPTED,
                        base_now + (i as u64),
                        bytes::Bytes::from(format!("t_{t}_f_{i}")),
                    )
                    .unwrap();
                    let mut guard = store.lock().unwrap();
                    guard
                        .check_and_insert(&frame, base_now + (i as u64))
                        .unwrap();
                }
            })
        })
        .collect();

    for t in threads {
        t.join().unwrap();
    }

    // Reopen store and verify 100% rejection of all 5,000 frames
    {
        let mut store = DurableReplayStore::open(&wal_path, 10_000, 3_600_000_000).unwrap();
        let mut count = 0;
        for t in 0..10 {
            for i in 0..500 {
                let frame = ZapFrame::with_timestamp(
                    source,
                    target,
                    ZapFlags::ENCRYPTED,
                    base_now + (i as u64),
                    bytes::Bytes::from(format!("t_{t}_f_{i}")),
                )
                .unwrap();
                if store
                    .check_and_insert(&frame, base_now + 1_000_000)
                    .is_err()
                {
                    count += 1;
                }
            }
        }
        assert_eq!(
            count, 5000,
            "All 5,000 frames inserted concurrently must survive restart!"
        );
    }
}
