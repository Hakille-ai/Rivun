# Milestone 1 Forensic Audit Report — Handoff

## 1. Observation

### Forensic Audit Overview
- **Work Product**: Milestone 1 implementation (`zap-net`, `zap-node`, `zap-journal`, `zap-ledger`).
- **Integrity Profile**: General Project (Development Mode per `ORIGINAL_REQUEST.md`).
- **Verdict**: **INTEGRITY VIOLATION**

### Phase 1: Source Code & Static Analysis Results
1. **Hardcoded Test Nonces & Result Stubs**: **PASS**
   - Investigated `crates/zap-net/src/durable_replay.rs` and `crates/zap-node/src/durable_replay.rs`. Nonces and frame fingerprints are formatted using binary WAL layouts (`ZAPNONC1` and `ZAPFRM01`), persisted with atomic writes and `sync_data()`, and parsed dynamically upon node restart.
2. **Facade & Dummy Cryptography Detection**: **PASS**
   - Investigated `crates/zap-journal/src/lib.rs` and `crates/zap-ledger/src/lib.rs`. Journal framing uses real BLAKE3 hash chaining (`compute_entry_hash`). Segment manifests and receipts are signed and verified using Ed25519 keypairs via `ed25519_dalek`.
3. **Pre-populated Artifact Check**: **PASS**
   - No pre-populated log or verification artifacts exist prior to audit execution.

### Phase 2: Behavioral & Build Verification Results

1. **`cargo test` Verification**: **FAIL**
   Command:
   ```powershell
   cargo test -p zap-net -p zap-node -p zap-journal -p zap-ledger
   ```
   **Output / Failure Log**:
   ```text
   Running tests\m1_journal_stress.rs (target\debug\deps\m1_journal_stress-5b61d2909076e707.exe)

   running 5 tests
   test test_journal_tampered_record_detection ... ok
   test test_journal_partial_tail_recovery ... ok
   test test_journal_corrupted_index_rebuild ... ok
   test test_journal_manifest_hash_integrity_under_rotation ... ok
   test test_journal_rapid_rotation_stress ... FAILED

   failures:

   ---- test_journal_rapid_rotation_stress stdout ----

   thread 'test_journal_rapid_rotation_stress' (17820) panicked at crates\zap-journal\tests\m1_journal_stress.rs:45:33:
   called `Result::unwrap()` on an `Err` value: HashChainMismatch { path: "C:\\Users\\STAGIA~1\\AppData\\Local\\Temp\\.tmpRp0ez1\\00000000000000000020.zjseg", offset: 33 }
   note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace

   failures:
       test_journal_rapid_rotation_stress

   test result: FAILED. 4 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.38s
   ```

2. **`cargo clippy` Verification**: **FAIL**
   Command:
   ```powershell
   cargo clippy -p zap-net -p zap-node -p zap-journal -p zap-ledger --all-targets -- -D warnings
   ```
   **Output / Failure Log**:
   ```text
   error: manual implementation of `.is_multiple_of()`
     --> crates\zap-journal\tests\m1_journal_stress.rs:11:18
      |
   11 |         kind: if i % 2 == 0 { "alpha".to_string() } else { "beta".to_string() },
      |                  ^^^^^^^^^^ help: replace with: `i.is_multiple_of(2)`
      |
      = help: for further information visit https://rust-lang.github.io/rust-clippy/rust-1.95.0/index.html#manual_is_multiple_of
      = note: `-D clippy::manual-is-multiple-of` implied by `-D warnings`
      = help: to override `-D warnings` add `#[allow(clippy::manual_is_multiple_of)]`

   error: could not compile `zap-journal` (test "m1_journal_stress") due to 1 previous error
   ```

---

## 2. Logic Chain

1. **Test Failure Root Cause**:
   - In `crates/zap-journal/src/lib.rs`, `JournalStore::scan_records()` verifies the entry hash chain across segments on disk by checking that each record's `previous_entry_hash` matches `hash_or_none(previous_hash)`.
   - At the start of `scan_records()`, `previous_hash` is initialized to `None` (which expects `blake3:0000...`).
   - When segment pruning (`max_segment_count`) removes older segments (e.g. segments 0..14), the earliest remaining segment on disk (segment 15) has a first record whose `previous_entry_hash` refers to the last entry of the pruned segment 14.
   - Because segment 14 is missing from disk, `scan_records()` starts at segment 15 with `previous_hash = None`, causing `scan_records()` to fail with `ZapJournalError::HashChainMismatch`.
   - `store.verify()` relies on `scan_records()`, so calling `store.verify()` on a pruned journal fails.

2. **Clippy Failure Root Cause**:
   - `crates/zap-journal/tests/m1_journal_stress.rs` contains `i % 2 == 0` on line 11, which triggers `clippy::manual_is_multiple_of` under `-D warnings` on Rust 1.95 toolchain.

3. **Integrity Auditor Standard**:
   - Acceptance criteria require 100% passing tests (`cargo test --workspace --all-targets`) and 0 clippy warnings (`cargo clippy --workspace --all-targets -- -D warnings`).
   - Any test or clippy failure invalidates project completion claims and requires an **INTEGRITY VIOLATION** verdict.

---

## 3. Caveats

- No caveats. The test and clippy failures were directly observed and reproduced via empirical command execution.

---

## 4. Conclusion

**Verdict**: **INTEGRITY VIOLATION**

Milestone 1 cannot be certified due to:
1. Behavioral test failure in `zap-journal`: `test_journal_rapid_rotation_stress` panics with `HashChainMismatch` when `JournalStore::verify()` is invoked on a journal with pruned segments.
2. Code quality failure in `zap-journal`: `cargo clippy` fails with compiler error on `m1_journal_stress.rs:11`.

---

## 5. Verification Method

To independently reproduce the failure:

1. Run `cargo test` on `zap-journal`:
   ```powershell
   cargo test -p zap-journal --test m1_journal_stress
   ```
   Observed result: `test_journal_rapid_rotation_stress` fails with `HashChainMismatch`.

2. Run `cargo clippy` on `zap-journal`:
   ```powershell
   cargo clippy -p zap-journal --all-targets -- -D warnings
   ```
   Observed result: Compilation error `clippy::manual_is_multiple_of` in `m1_journal_stress.rs`.
