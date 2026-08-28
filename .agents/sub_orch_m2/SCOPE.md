# Scope: Milestone 2 (R2) - Merkle Mountain Range (MMR) & Compact Cryptographic Batch Receipts

## Architecture
Milestone 2 delivers high-throughput append-only cryptographic accumulators, compact multi-leaf proofs, exclusion proofs, batch sealing with Swarm Quorum multi-signatures, and Zero-Knowledge verifiable receipt rollups in `crates/rivun-ledger` and `crates/rivun-crypto`.

```
+-----------------------------------------------------------------------------------+
|                                rivun-ledger & rivun-crypto                            |
|                                                                                   |
|  +---------------------------+    +--------------------------------------------+  |
|  |       rivun-crypto          |    |                 rivun-ledger                 |  |
|  | - Blinded commitments     |    | - IncrementalMmr (O(log N) peak storage)   |  |
|  | - Batch threshold sigs    |    | - MmrBatchInclusionProof (dedup DAG)       |  |
|  | - Quorum multi-signatures |    | - MmrExclusionProof (non-membership)       |  |
|  +---------------------------+    | - ReceiptBatchSeal (Swarm multi-sig seal)  |  |
|               ^                   | - ZkReceiptBatchProof (ZK rollup proof)    |  |
|               |                   | - Disk persistence (.zmmr / journal sync)  |  |
|               +-------------------+--------------------------------------------+  |
+-----------------------------------------------------------------------------------+
```

## Feature Inventory Assignment
| # | Feature | Description | Milestone | Source |
|---|---------|-------------|-----------|--------|
| 4 | Incremental MMR Accumulator | Merkle Mountain Range $O(\log N)$ peak accumulator with disk persistence, peak-bagging root calculation | M2 | ORIGINAL_REQUEST §R2 |
| 5 | Compact Batch Receipts & Proofs | Batch inclusion proofs, exclusion/non-membership proofs, cryptographic batch sealing | M2 | ORIGINAL_REQUEST §R2 |
| 6 | ZK Verifiable Receipt Rollups | Blinded commitments and verifiable execution rollups proving correctness without exposing private payload contents | M2 | ORIGINAL_REQUEST §R2 |

## Detailed Module Deliverables
1. `crates/rivun-crypto`:
   - Blinded commitments: `BlindedReceiptCommitment` generation and verification with domain separated hashing.
   - Batch verification helpers and threshold multi-signature aggregation for Swarm Quorum seals.
2. `crates/rivun-ledger`:
   - `src/mmr.rs`:
     - `IncrementalMmr`: $O(\log N)$ memory peak accumulator, incremental leaf appending, peak-bagging root computation, disk persistence (`.zmmr` format / journal integration).
     - `MmrInclusionProof`: Single leaf inclusion proof.
     - `MmrBatchInclusionProof`: Deduplicated multi-leaf batch inclusion proof with compact sister DAG.
     - `MmrExclusionProof`: Non-membership proofs (BeforeRange, AfterRange, SequenceGap, HashBound).
   - `src/batch.rs`:
     - `ReceiptBatchSeal`, `SignedReceiptBatch`, `BatchValidatorSignature`, validation against MMR root, sequence range, and Swarm Quorum multi-signatures.
   - `src/zk.rs`:
     - `ZkReceiptBatchProof`, `ZkRollupPublicInputs`, `generate_rollup`, `verify` proving state transition and receipt correctness with zero knowledge of private payload bytes.
   - Integration with `ReceiptJournalStore`:
     - Auto-commit MMR peaks to `.zmmr` on segment rotation and batch sealing.

## Milestones & Status
| # | Sub-Milestone | Scope | Dependencies | Status |
|---|---------------|-------|-------------|--------|
| 1 | R2 Implementation | `crates/rivun-ledger`, `crates/rivun-crypto` | none | IN_PROGRESS |

## Interface Contracts
### `rivun-ledger` <-> `rivun-crypto`
- `IncrementalMmr::append_leaf(&mut self, leaf_hash: &MmrHash) -> u64`
- `IncrementalMmr::get_root(&self) -> MmrHash`
- `IncrementalMmr::bag_peaks(peaks: &[MmrHash]) -> MmrHash`
- `MmrBatchInclusionProof::verify(&self, root: &MmrHash) -> bool`
- `MmrExclusionProof::verify(&self, root: &MmrHash) -> bool`
- `ReceiptBatchSeal::verify_quorum(&self, validator_set: &PoaValidatorSet) -> Result<bool, LedgerError>`
- `ZkReceiptBatchProof::generate_rollup(receipts: &[SignedActionReceipt], ...) -> ZkReceiptBatchProof`
- `ZkReceiptBatchProof::verify(&self, root: &MmrHash) -> bool`

