# rivun Ledger

The ledger stack makes rivun execution history verifiable in three complementary
ways:

1. **Append-only journal** (`rivun-journal`) — tamper-evident binary storage with
   entry hash chaining and signed segment sealing;
2. **Signed receipts + batch seals** (`rivun-ledger`) — per-action Ed25519
   receipts, batch root commitments, and validator quorum seals;
3. **MMR and blinded rollups** (`rivun-ledger::mmr`, `rivun-ledger::zk`) —
   cryptographic accumulation for inclusion/exclusion proofs and confidential
   audit commitments.

Receipts are **audit records, not financial records**. rivun has no ledger in the
payment sense of the word.

## Journal segments (`rivun-journal`)

`JournalStore` writes append-only binary segments:

- Segments use magic `ZJSEG001` and records use `ZJRC` (version 1).
- Every record carries a BLAKE3 payload hash and `entry_hash` that chains to
  the previous entry (`previous_entry_hash`).
- Sidecar indexes (`*.zjidx`) are rebuildable accelerators; segment manifests
  (`*.zjmanifest.json`) summarize one ordered segment (count, byte length,
  first/last timestamps, segment hash, first/last entry hashes, optional
  previous-segment hash).
- `seal_segment`/`rotate_and_seal` can produce signed segment manifests.
- `recover_partial_tail` repairs a crash-corrupted tail; `prune_old_segments`
  bounds disk usage; `verify` walks hashes and rebuilds indexes in place.
- Profiles: `Receipts` (1) and `Memory` (2); default max segment size 64 MB.

The CLI exposes journal operations through `rivun receipts ...` and
`rivun memory ...`; see [Receipts](receipts.md) and
[Capability, Router & Memory](capability-router-memory.md).

## Signed receipts (`rivun-ledger`)

`ActionReceipt` records the receiver node, source/target nodes, action name,
frame/payload/output BLAKE3 hashes, timestamps, flags, optional PoA summary,
and optional PACT reference. `SignedActionReceipt` signs a deterministic JSON
payload with domain `rivun-ACTION-RECEIPT-v1`; verification re-derives the signer
node id from the public key.

Verification is adaptive: scalar below 4 receipts, parallel rayon batches at
128+.

`ReceiptJournalStore` wraps `JournalStore`:

- `append` (with fsync policy), `query`/`query_fast`;
- `rotate_and_seal_segment`, `seal_segment_batch` (`*.zjseal.json`);
- `build_incremental_mmr`, `import_jsonl`/`export_jsonl`, `compact`;
- peer replication over `rivun.receipts.request/response` with cursor-style
  pagination (default limit 50, max 500).

## Batch seals (`rivun-ledger::batch`)

`ReceiptBatchSeal` commits to a contiguous receipt range: sequence range, MMR
root, and state transitions. `SignedReceiptBatch` adds validator signatures
(domain `rivun-RECEIPT-BATCH-SEAL-v1`, unpadded Base64 signatures) with
threshold quorum checks (`sign_with_validator`, `verify_quorum`). Batch seals
let auditors verify a range of receipts without re-verifying every signature.

## Merkle Mountain Range (`rivun-ledger::mmr`)

`IncrementalMmr` is an append-only accumulator (binary format `ZAPMMR01`)
supporting:

- `append_leaf`, `get_root`;
- `prove_inclusion` and `prove_batch_inclusion` (deduplicated sister-node
  DAGs);
- **exclusion proofs**: `prove_exclusion_before/after/gap/hash_bound`
  (non-membership of a hash at a position);
- `create_rollup_commitment` for ZK integration;
- `verify_proof` offline.

Domains: `MMR_LEAF_DOMAIN`, `MMR_NODE_DOMAIN`, `MMR_PEAK_BAG_DOMAIN`.

## Blinded rollup commitments (`rivun-ledger::zk`)

`BlindedReceiptCommitment` hides receipt payloads while committing to them:

```text
C = BLAKE3( domain || node_id || frame_hash || payload_hash || output_hash || salt )
```

- `ZkReceiptBatchProof::generate_rollup` builds a batch proof;
- `verify` checks the proof against the expected MMR root;
- auditors can later reveal hashes (not payloads) to prove specific receipts
  existed at rollup time.

This gives deployments **confidential auditability**: hash-level evidence
without exposing payload bytes.

## CLI quick reference

```bash
# Verify a receipt journal (optionally with provenance digests)
cargo run -p rivun-cli -- receipts verify --dir logs/receipts --provenance

# Pull and archive peer receipts
cargo run -p rivun-cli -- receipts pull --config rivun.toml --target <peer-node-id> --out-dir logs/peer-receipts

# Export/import JSONL archives and compact journals
cargo run -p rivun-cli -- receipts export-jsonl --dir logs/receipts --out logs/receipts.jsonl
cargo run -p rivun-cli -- receipts import-jsonl --in logs/receipts.jsonl --dir logs/receipts
cargo run -p rivun-cli -- receipts compact --dir logs/receipts --out logs/receipts.compacted
```

## Relationship to other components

- `rivun-node` appends a signed receipt per processed action
  (`[receipts].dir`).
- `rivun-gateway` exposes receipt queries (`rivun://ledger/receipts`) and its
  provenance chain terminates in the receipt stage.
- `rivun-telemetry`'s fleet doctor cross-verifies journal segment manifests and
  receipt log integrity.
- Receipts carry optional PACT references; see [PACT](pact.md).
