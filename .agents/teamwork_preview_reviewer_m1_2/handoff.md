# Milestone 1 (R1: Durable Core & Replay Protection) — Code Quality & Correctness Review Report

## 1. Observation

### Summary of Commands Executed & Results

1. **Unit & Package Integration Tests**:
   - Command: `cargo test -p zap-net -p zap-node -p zap-journal -p zap-ledger`
   - Result: **PASSED** (19 in `zap-net`, 70 in `zap-node`, 6 in `zap-journal`, 21 in `zap-ledger`).

2. **Workspace Clippy & All-Targets Compilation**:
   - Command: `cargo clippy --workspace --all-targets -- -D warnings`
   - Result: **FAILED** (Exit code 1).
   - Verbatim Compiler Errors in `crates/zap-ledger/tests/m1_challenger_stress.rs`:
     ```text
     error: unused import: `ActionReceipt`
      --> crates\zap-ledger\tests\m1_challenger_stress.rs:7:5
       |
     7 |     ActionReceipt, ReceiptJournalStore, ReceiptReplicationRequest, ReceiptSegmentIndex,
       |     ^^^^^^^^^^^^^

     error[E0560]: struct `zap_core::ZapHeader` has no field named `sequence`
       --> crates\zap-ledger\tests\m1_challenger_stress.rs:25:13
        |
     25 |             sequence: 1,
        |             ^^^^^^^^ `zap_core::ZapHeader` does not have this field

     error[E0308]: mismatched types
       --> crates\zap-ledger\tests\m1_challenger_stress.rs:28:18
        |
     28 |         payload: format!("payload-{processed_at_micros}").into_bytes(),
        |                  ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ expected `Bytes`, found `Vec<u8>`

     error[E0063]: missing field `auth` in initializer of `zap_core::ZapFrame`
       --> crates\zap-ledger\tests\m1_challenger_stress.rs:19:17
        |
     19 |     let frame = ZapFrame {
        |                 ^^^^^^^^ missing `auth`

     error[E0599]: no method named `encode` found for struct `base64::engine::GeneralPurpose` in the current scope
        --> crates\zap-ledger\tests\m1_challenger_stress.rs:185:10
         |
     184 |       let other_pub_b64 = base64::engine::general_purpose::STANDARD_NO_PAD
     185 | |         .encode(other_key.verifying_key().to_bytes());
     ```

### Code Analysis Observations

1. **`crates/zap-ledger/tests/m1_challenger_stress.rs`**:
   - File exists in `crates/zap-ledger/tests/` and contains invalid Rust syntax/types relative to `zap-core` and `base64`.
   - Prevents `cargo clippy --workspace --all-targets -- -D warnings` and `cargo test -p zap-ledger --all-targets` from building.

2. **`crates/zap-net/src/durable_replay.rs` & `crates/zap-node/src/durable_replay.rs`**:
   - `DurableNonceStore::open()` and `DurableReplayStore::open()` stop reading WAL files upon encountering the first truncated/corrupted record (`while file.read_exact(&mut buf).is_ok()`).
   - The file is subsequently opened with `OpenOptions::new().append(true)` without truncating to the last valid record offset.
   - Any new nonces/fingerprints appended after a partial write are appended after the corrupted bytes. On subsequent restarts, recovery stops at the corrupt byte offset, abandoning all newly written replay records.
   - `DurableNonceStore::compact()` at line 124 writes `Uuid::nil().as_bytes()` into the record buffer instead of preserving the record's `node_id`.

3. **`crates/zap-node/src/durable_replay.rs`**:
   - Line 81: `ts + self.max_clock_skew_micros < now_micros` performs un-checked addition on `u64`. An adversarial frame timestamp `u64::MAX` triggers integer overflow panic in debug builds.

4. **`crates/zap-journal/src/lib.rs`**:
   - Lines 650-651: `let _ = self.seal_segment(last.sequence);` silently suppresses errors returned by `seal_segment()` during rotation.

---

## 2. Logic Chain

1. **Clippy & Build Integrity Requirement**:
   - Observation 1 shows that running `cargo clippy --workspace --all-targets -- -D warnings` fails due to compilation errors in `crates/zap-ledger/tests/m1_challenger_stress.rs`.
   - Project Acceptance Criteria explicitly mandates that `cargo clippy --workspace --all-targets -- -D warnings` runs cleanly with 0 warnings/errors across all targets in the workspace.
   - Therefore, the codebase currently fails acceptance criteria.

2. **Replay Store Durability & Edge-Case Error Handling**:
   - Observation 2 shows that when an unclean shutdown produces a partial record in `frames.wal` or `nonces.wal`, `open()` leaves the corrupt bytes intact and appends new records after them.
   - Upon the next node restart, `read_exact()` fails at the corrupted offset, discarding all nonces and frame fingerprints recorded after that unclean restart.
   - This creates a window where previously seen nonces/fingerprints can be replayed after a second restart, breaking the durable replay guarantee ("zero replay vulnerabilities across node restarts").

3. **Compact Data Fidelity**:
   - Observation 2 also shows that `DurableNonceStore::compact()` replaces `node_id` with `Uuid::nil()`.
   - Overwriting origin node IDs during compaction corrupts audit trails for active nonces.

4. **Cryptographic Manifest Verification**:
   - Observation from `crates/zap-ledger/src/lib.rs` (lines 821–893, 949–1050) shows that `SignedReceiptSegmentManifest` properly signs and verifies BLAKE3 segment hashes, Ed25519 node public keys, and sequence chains (`ReceiptSegmentChainMismatch`).
   - The cryptographic verification logic itself is sound, but it cannot be verified in integration stress tests (`m1_challenger_stress.rs`) until compiler errors are fixed.

---

## 3. Caveats

- The core unit tests in `zap-net`, `zap-node`, `zap-journal`, and `zap-ledger` pass cleanly when target filtering excludes integration test targets.
- Cryptographic signature generation and verification for receipts and manifests in `zap-ledger` are correctly implemented using Ed25519-dalek with domain separation (`ZAP-RECEIPT-SEGMENT-MANIFEST-v1`).

---

## 4. Conclusion

**Verdict**: **`REQUEST_CHANGES`**

### Findings Summary

#### [Critical] Finding 1: Integration Test Compilation Failure in `zap-ledger` Breaks `cargo clippy`
- **Location**: `crates/zap-ledger/tests/m1_challenger_stress.rs` (lines 7, 19, 25, 28, 185)
- **Why**: File contains 5 compilation errors (`unused import`, `ZapHeader` missing `sequence` field, `Vec<u8>` vs `Bytes` mismatch, missing `auth` in `ZapFrame`, missing `use base64::Engine;`).
- **Impact**: `cargo clippy --workspace --all-targets -- -D warnings` fails.
- **Suggested Fix**: Update `m1_challenger_stress.rs` to match the current `zap-core` and `base64` API interfaces (use `ZapFrame::with_timestamp`, import `base64::Engine`, remove unused imports).

#### [Major] Finding 2: Corrupted WAL Tail Recovery Bypasses Replay Protection Across Node Restarts
- **Location**: `crates/zap-net/src/durable_replay.rs` (lines 46-62) & `crates/zap-node/src/durable_replay.rs` (lines 38-57)
- **Why**: Partial record writes during unclean process shutdown are not truncated upon `open()`. Appending new records after corrupt bytes causes subsequent restarts to fail parsing at the corrupt offset, losing all subsequent replay protections.
- **Suggested Fix**: Track the byte offset of the last valid record during `open()`. If `read_exact()` fails before EOF, truncate the file to `last_valid_offset` (`file.set_len(last_valid_offset)`) before opening for append.

#### [Major] Finding 3: `DurableNonceStore::compact()` Overwrites Node IDs with `Uuid::nil()`
- **Location**: `crates/zap-net/src/durable_replay.rs` (line 124)
- **Why**: Overwrites the 16-byte `node_id` field with `Uuid::nil()` during compaction.
- **Suggested Fix**: Retain node IDs in `self.order` queue (e.g. `VecDeque<([u8; 12], Uuid, u64)>`) so compaction preserves original node IDs.

#### [Minor] Finding 4: Integer Overflow Risk in Clock Skew Check
- **Location**: `crates/zap-node/src/durable_replay.rs` (line 81)
- **Why**: `ts + self.max_clock_skew_micros < now_micros` overflows on `u64::MAX`.
- **Suggested Fix**: Use `saturating_add` or checked subtraction (`now_micros.saturating_sub(ts) > max_clock_skew_micros`).

#### [Minor] Finding 5: Silent Error Suppression in `JournalStore::current_segment()`
- **Location**: `crates/zap-journal/src/lib.rs` (lines 650-651)
- **Why**: `let _ = self.seal_segment(last.sequence);` ignores sealing failures.
- **Suggested Fix**: Log or handle errors returned by `seal_segment()`.

---

## 5. Verification Method

To verify resolution of these findings after fixes:

1. **Run Full Workspace Clippy**:
   ```powershell
   cargo clippy --workspace --all-targets -- -D warnings
   ```
   *Expected result*: Exit code 0 with zero warnings/errors.

2. **Run All Milestone 1 Unit & Integration Tests**:
   ```powershell
   cargo test -p zap-net -p zap-node -p zap-journal -p zap-ledger --all-targets
   ```
   *Expected result*: All tests (including `m1_challenger_stress.rs`) pass cleanly.

3. **Verify WAL Corruption Recovery**:
   Append invalid garbage bytes to `frames.wal` / `nonces.wal`, insert new nonces, reopen the store, and verify newly inserted nonces are still remembered.
