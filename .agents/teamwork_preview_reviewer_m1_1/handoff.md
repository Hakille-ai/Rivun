# Milestone 1 (R1: Durable Core & Replay Protection) — Review Handoff Report

## 1. Observation

### Implementation & Verification Findings
I performed an independent code quality, correctness, and security inspection of the Milestone 1 implementation across `crates/zap-net`, `crates/zap-node`, `crates/zap-journal`, and `crates/zap-ledger`.

#### Tested Targets & Test Suite Execution
1. Executed `cargo test -p zap-net -p zap-node -p zap-journal -p zap-ledger`:
   - `zap-journal`: 6 passed, 0 failed.
   - `zap-net`: 19 passed, 0 failed.
   - `zap-ledger`: 21 passed, 0 failed (unit tests) + 6 passed, 0 failed (`m1_challenger_stress.rs`).
   - `zap-node`: 70 passed, 0 failed.
   - Total: 122 tests passed across target crates with 0 failures, 0 ignored.
2. Executed `cargo clippy -p zap-net -p zap-node -p zap-journal -p zap-ledger --all-targets -- -D warnings`:
   - Clean compilation with zero warnings across all targets.

#### Architectural Verification
- **Durable Replay Protection (`crates/zap-net/src/durable_replay.rs` & `crates/zap-node/src/durable_replay.rs`)**:
  - `DurableNonceStore` (magic `b"ZAPNONC1"`) persists 36-byte records (`timestamp_micros`, `node_id`, `nonce`) with immediate `sync_data()`. Re-hydration populates `seen` and `order` collections.
  - `DurableReplayStore` (magic `b"ZAPFRM01"`) persists 40-byte records (`timestamp_micros`, `source_node`, `fingerprint`) in `zap-node`. `ReplayGuard` delegates frame checks to the durable store, preventing replay attacks across node restarts.
- **Segment Rotation & Sealing (`crates/zap-journal/src/lib.rs`)**:
  - `JournalStore` enforces limits (`max_segment_bytes` & `max_segment_records`). `seal_segment()` computes BLAKE3 hashes of segment contents, writes `.zjmanifest.json`, and rotates sequence numbers seamlessly.
- **Signed Manifests & Fast Index Queries (`crates/zap-ledger/src/lib.rs`)**:
  - `ReceiptJournalStore` signs segment manifests with the node's Ed25519 keypair under domain `b"ZAP-RECEIPT-SEGMENT-MANIFEST-v1"` to create `.zjmanifest.json.sig` files.
  - `build_and_verify_segment_index()` verifies signature chains and sequence continuity. `query_fast()` prunes non-matching segment ranges prior to record scanning, achieving high-performance indexed reads.

---

## 2. Logic Chain

1. **Replay Protection Persistency**:
   - The test `durable_nonce_store_persists_nonces_across_restarts` in `zap-net` and `durable_replay_store_persists_fingerprints_across_restart` in `zap-node` write nonces/frames to WAL files, simulate process restarts by opening new store instances on existing files, and verify that replayed items are correctly rejected.
   - Code inspection confirmed real disk operations (`fs::OpenOptions`, `sync_data()`, `read_exact`) with no mock memory shortcuts.

2. **Manifest Signing & Cryptographic Security**:
   - `SignedReceiptSegmentManifest::sign()` creates Ed25519 signatures over `ReceiptSegmentManifest` JSON representations using domain separation.
   - Modifying signature bytes or public key material in tests (`test_signature_and_manifest_tampering`) triggers explicit `InvalidSignature` and `SegmentManifestSignerNodeMismatch` errors.

3. **Query Optimization Correctness**:
   - `query_fast()` computes candidate sequences via `ReceiptSegmentIndex::candidate_segments()`, passing only matching sequences to `query_filtered()`. Stress testing confirmed output parity between `query_fast()` and `query_with_limit()`.

4. **Integrity & Code Quality**:
   - Checked for integrity violations: no hardcoded outputs, fake implementations, or bypassed verification steps were present.

---

## 3. Caveats

- No caveats. All core requirements for Milestone 1 are completely satisfied and verified.

---

## 4. Conclusion

**Verdict**: **APPROVE**

Milestone 1 (R1: Durable Core & Replay Protection) passes code quality, correctness, and security review. The durable replay store, segment rotation, manifest signing, and fast indexed query engine are fully implemented, verified, and free of defects or integrity issues.

---

## 5. Verification Method

To independently verify this review:

1. **Run Crates Unit & Integration Test Suite**:
   ```powershell
   cargo test -p zap-net -p zap-node -p zap-journal -p zap-ledger
   ```
2. **Run Linter / Clippy Check**:
   ```powershell
   cargo clippy -p zap-net -p zap-node -p zap-journal -p zap-ledger --all-targets -- -D warnings
   ```
3. **Verify Replay Protection across Restarts**:
   Inspect `crates/zap-net/src/durable_replay.rs` and `crates/zap-node/src/durable_replay.rs`.
4. **Verify Signed Segment Manifests & Fast Index Query Performance**:
   Inspect `crates/zap-ledger/src/lib.rs` and `crates/zap-journal/src/lib.rs`.
