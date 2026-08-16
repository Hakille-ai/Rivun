# Milestone 1 (R1: Receipt Journal Segment Rotation & Manifest Signing) — Challenger Handoff Report

## Verdict: REJECT

---

## 1. Observation

Adversarial stress testing of Milestone 1 Receipt Journal Segment Rotation & Manifest Signing was conducted in `zap-journal` and `zap-ledger`. Two test harnesses were created and executed:
- `crates/zap-ledger/tests/m1_challenger_stress.rs`
- `crates/zap-journal/tests/m1_journal_stress.rs`

### Confirmed Empirical Failures & Bugs

#### Bug 1: `HashChainMismatch` panic in `zap-journal` upon segment pruning
- **Command Executed**: `cargo test -p zap-journal --test m1_journal_stress -- --nocapture`
- **Output / Failure Log**:
  ```text
  thread 'test_journal_rapid_rotation_stress' (20828) panicked at crates\zap-journal\tests\m1_journal_stress.rs:45:33:
  called `Result::unwrap()` on an `Err` value: HashChainMismatch { path: "C:\\Users\\...\\00000000000000000020.zjseg", offset: 33 }
  ```
- **Code Inspection**: In `crates/zap-journal/src/lib.rs` (lines 766–788):
  ```rust
  fn scan_records<F>(&self, allow_partial_tail: bool, callback: &mut F) -> Result<()> {
      let mut previous_hash = None;
      for segment in self.segments()? {
          scan_segment(..., &mut |record| {
              if record.previous_entry_hash != hash_or_none(previous_hash.as_deref()) {
                  return Err(ZapJournalError::HashChainMismatch { ... });
              }
              ...
          })?;
      }
  }
  ```
  When `max_segment_count` prunes old segments (e.g., sequence 0..19), the remaining segments start at sequence 20. The first record of sequence 20 points to the last record of sequence 19 in `previous_entry_hash`. Because `scan_records()` hardcodes `previous_hash = None`, it compares against `"none"` and returns `HashChainMismatch` on the first record of sequence 20.

#### Bug 2: `build_and_verify_segment_index()` hardcodes sequence 0 and breaks under pruning
- **Command Executed**: `cargo test --test m1_challenger_stress -- --nocapture`
- **Output / Log**:
  ```text
  Index build result after pruning: Ok(ReceiptSegmentIndex { schema_version: 1, node_id: ..., entries: [] })
  ```
- **Code Inspection**: In `crates/zap-ledger/src/lib.rs` (lines 535–541):
  ```rust
  pub fn build_and_verify_segment_index(&self) -> Result<ReceiptSegmentIndex> {
      let node_id = self.keypair.as_ref().map(|k| k.node_id()).unwrap_or_default();
      let mut manifests = Vec::new();
      let mut sequence = 0_u64;
      while self.signed_manifest_path(sequence).exists() {
          let signed = self.load_signed_manifest(sequence)?;
          manifests.push(signed);
          sequence += 1;
      }
      ReceiptSegmentIndex::from_manifests(node_id, &manifests)
  }
  ```
  When sequence 0 is deleted due to `max_segment_count` pruning, `signed_manifest_path(0).exists()` evaluates to `false`. The loop immediately terminates at `sequence = 0`, producing an empty index (`entries: []`). Consequently, `query_fast` silently fails to use the signed segment index after segment pruning and degrades into full table scans.

#### Bug 3: Automatic segment rotation in `JournalStore` skips `.zjmanifest.json.sig` generation
- **Code Inspection**: In `crates/zap-journal/src/lib.rs` (lines 641–665), `current_segment()` triggers auto-rotation and calls `seal_segment(last.sequence)`. This writes unsigned `.zjmanifest.json`, but `JournalStore` has no reference to `Keypair`, so `.zjmanifest.json.sig` is never created during auto-rotations triggered by `append()`. Unless callers manually call `rotate_and_seal_segment(seq)` on `ReceiptJournalStore`, auto-rotated segments remain unsigned on disk.

#### Bug 4: `ReceiptSegmentManifest::from_receipts` receives random `Uuid::new_v4()` for `segment_id`
- **Code Inspection**: In `crates/zap-ledger/src/lib.rs` (lines 505–511):
  ```rust
  let segment_id = Uuid::new_v4();
  let manifest = ReceiptSegmentManifest::from_receipts(
      segment_id,
      sequence,
      &receipts,
      previous_segment_hash,
  )?;
  ```
  Generating a new random UUID when signing the manifest decouples `manifest.segment_id` from the actual segment header `id` created when the `.zjseg` file was opened.

---

## 2. Logic Chain

1. **Rapid Segment Rotation & Pruning Stress**:
   - Creating stores configured with `max_segment_count = Some(N)` causes `JournalStore` to delete older `.zjseg`, `.zjidx`, `.zjmanifest.json`, and `.zjmanifest.json.sig` files once segment count exceeds `N`.
   - When sequence 0 is pruned, any subsequent call to `JournalStore::records()`, `JournalStore::verify()`, or `ReceiptJournalStore::all()` fails with `ZapJournalError::HashChainMismatch` because `scan_records` assumes the first segment processed must have `previous_entry_hash == "none"`.
   - Simultaneously, `build_and_verify_segment_index()` in `zap-ledger` relies on `signed_manifest_path(sequence)` starting at `sequence = 0_u64`. When sequence 0 is missing, the loop yields 0 entries, effectively disabling index-accelerated queries (`query_fast`) after segment pruning.

2. **Cryptographic Signature & Tampering Verification**:
   - Signature tampering (flipping bytes in base64 signature) correctly triggers `ZapLedgerError::InvalidSignature`.
   - Public key substitution (replacing `signer_public_key` with another keypair) correctly triggers `SegmentManifestSignerNodeMismatch`.
   - Segment file payload modification correctly breaks `read_segment_receipts()` and hash comparisons.
   - Hash chain corruption (`previous_segment_hash` mismatch) correctly triggers `ReceiptSegmentChainMismatch`.
   - Partial tail truncation (`recover_partial_tail()`) and index recovery (`rebuild_indexes()`) operate correctly on unpruned stores.

---

## 3. Caveats

- Unpruned stores (`max_segment_count: None`) pass basic rotation and manifest verification tests.
- However, segment rotation with bounded disk usage (`max_segment_count`) is an explicit requirement of Milestone 1 (R1). The failure modes under segment pruning prevent approving this milestone.

---

## 4. Conclusion

Milestone 1 Receipt Journal Segment Rotation & Manifest Signing is **REJECTED** due to:
1. `HashChainMismatch` runtime panic in `zap-journal` when querying pruned stores.
2. `build_and_verify_segment_index()` returning empty indexes when sequence 0 is pruned.
3. Unsigned `.zjmanifest.json.sig` files during automatic `append()` rotations.
4. Mismatched random `segment_id` in `rotate_and_seal_segment`.

The worker must remediate these issues before Milestone 1 can be accepted.

---

## 5. Verification Method

To independently reproduce and verify these findings:

1. **Run the Ledger Stress Test Suite**:
   ```powershell
   cargo test --test m1_challenger_stress -- --nocapture
   ```
   *Observe `test_rapid_rotation_with_segment_pruning` returning `entries: []` for index build.*

2. **Run the Journal Stress Test Suite**:
   ```powershell
   cargo test -p zap-journal --test m1_journal_stress -- --nocapture
   ```
   *Observe `test_journal_rapid_rotation_stress` failing with `HashChainMismatch` when `records()` is called after segment pruning.*
