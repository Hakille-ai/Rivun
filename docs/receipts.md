# Signed Receipts

ZAP can write signed action receipts for auditability. Receipts are not a billing, settlement, or payment system.

Enable the binary receipt journal in node config:

```toml
[receipts]
dir = "logs/receipts"
```

Each processed action appends one signed receipt record to append-only binary
segments (`*.zjseg`) with rebuildable sidecar indexes (`*.zjidx`) and manifests
(`*.zjmanifest.json`). The signed receipt payload contains:

- receiver node id;
- source and target node ids;
- action name;
- BLAKE3 hash of the encoded frame;
- BLAKE3 hash of the payload;
- optional BLAKE3 hash of driver output;
- frame and processing timestamps;
- frame flags;
- optional Proof-of-Action summary;
- optional PACT reference for verified `zap.pact.record` messages.

The receipt signature is Ed25519 over a deterministic JSON payload with the domain prefix `ZAP-ACTION-RECEIPT-v1`. The signer is the node that processed the action.

Verify an append-only receipt journal offline:

```bash
cargo run -p zap-cli -- receipts verify --dir logs/receipts
cargo run -p zap-cli -- receipts verify --dir logs/receipts --json
```

Verification walks the binary segments, checks the BLAKE3 journal hash chain,
rebuildable index consistency, signer identity, and Ed25519 signature. JSONL is
kept only for import/export/debug paths.

## PACT References

When the processed message is a signed `zap.pact.record` envelope with
`content_type = application/zap-pact+json`, `zap-node` verifies the PACT body
before writing the receipt. The receipt then includes an optional `pact` object
with the PACT id, intent, canonical hash, status, optional policy decision,
optional PoA summary, and optional output hash.

This is an extension of the existing receipt schema, not a replacement for it.
The receipt signature still covers the full receipt payload, including the PACT
reference when it is present. PACT evidence remains audit metadata; receipts
remain execution records and are not financial records.

Import or export legacy JSONL archives explicitly:

```bash
cargo run -p zap-cli -- receipts import-jsonl \
  --in logs/actions.legacy.jsonl \
  --dir logs/receipts
cargo run -p zap-cli -- receipts export-jsonl \
  --dir logs/receipts \
  --out logs/actions.archive.jsonl
```

Compact a journal into a fresh binary directory after verification:

```bash
cargo run -p zap-cli -- receipts compact \
  --dir logs/receipts \
  --out logs/receipts.compacted
```

`import-jsonl`, `export-jsonl`, and `compact` refuse to overwrite their output
unless `--force` is passed.

Pull signed receipts from a configured peer over signed `ZENV` control
messages:

```bash
cargo run -p zap-cli -- receipts pull \
  --config zap.toml \
  --target <peer-node-id> \
  --after-processed-at-micros 1735689600000000 \
  --until-processed-at-micros 1735776000000000 \
  --limit 100 \
  --out-dir logs/peer-receipts \
  --json
```

`pull` sends `zap.receipts.request`, verifies the signed
`zap.receipts.response`, verifies every nested receipt signature, and writes a
binary journal that can be passed to `receipts verify`, `export-jsonl`, or
`compact`.
Requests can filter by processed timestamp, kind, subject, source node, and
target node. Responses include a `truncated` flag when more matching receipts
exist than the requested limit and may include `next_after_processed_at_micros`
so clients can resume a bounded pull without duplicating the last page.

The journal keeps index files as accelerators, never as the cryptographic
source of truth:

- `*.zjseg` stores append-only binary records with payload bytes and BLAKE3
  entry hash chaining.
- `*.zjidx` stores rebuildable selection data by time, kind, subject, source,
  target, id, namespace, and segment offset.
- `*.zjmanifest.json` summarizes one ordered segment with receipt
  count, byte length, first/last processing timestamps, segment hash, first/last
  receipt hashes, and optional previous-segment hash.

Corrupt indexes can be rebuilt from segments; corrupt segments fail
verification.

## Cryptographic accumulation

Beyond plain verification, the ledger layer (`zap-ledger`) accumulates the
receipt stream into verifiable commitments:

- **Batch seals** — `SignedReceiptBatch` commits to a contiguous receipt
  range (sequence range, MMR root, state transitions) with validator quorum
  signatures, so auditors can validate a range without re-verifying every
  signature.
- **Incremental MMR** — an append-only Merkle Mountain Range (`ZAPMMR01`
  binary format) with `prove_inclusion`, `prove_batch_inclusion`, and
  **exclusion proofs** (non-membership), buildable from the journal via
  `ReceiptJournalStore::build_incremental_mmr`.
- **Blinded rollups** — `ZkReceiptBatchProof` commits to blinded receipt hashes
  (`BLAKE3(domain || node_id || frame_hash || payload_hash || output_hash ||
  salt)`), enabling confidential auditability: hash-level evidence without
  exposing payload bytes.

`ReceiptJournalStore` can seal segment batches (`*.zjseal.json`) during
rotation. See [Ledger](ledger.md) for the full API surface and threat
properties.

Receipts make local and future distributed operation auditable without creating financial semantics.
