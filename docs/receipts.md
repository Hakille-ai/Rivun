# Signed Receipts

ZAP can write signed action receipts for auditability. Receipts are not a billing, settlement, or payment system.

Enable JSONL receipts in node config:

```toml
[receipts]
path = "logs/actions.jsonl"
```

Each processed action appends one signed JSON object containing:

- receiver node id;
- source and target node ids;
- action name;
- BLAKE3 hash of the encoded frame;
- BLAKE3 hash of the payload;
- optional BLAKE3 hash of driver output;
- frame and processing timestamps;
- frame flags;
- optional Proof-of-Action summary.

The receipt signature is Ed25519 over a deterministic JSON payload with the domain prefix `ZAP-ACTION-RECEIPT-v1`. The signer is the node that processed the action.

Verify an append-only receipt log offline:

```bash
cargo run -p zap-cli -- receipts verify --path logs/actions.jsonl
cargo run -p zap-cli -- receipts verify --path logs/actions.jsonl --json
```

Verification parses each non-empty JSONL line, validates the signer identity,
and checks the Ed25519 signature. A tampered line fails with its line number.

Apply an offline retention cutoff after verification:

```bash
cargo run -p zap-cli -- receipts prune \
  --path logs/actions.jsonl \
  --before-processed-at-micros 1735689600000000 \
  --out logs/actions.retained.jsonl
```

`prune` keeps receipts whose `processed_at_micros` is greater than or equal to
the cutoff. It refuses to overwrite the output path unless `--force` is passed.

Merge verified receipt logs from multiple nodes or archive shards:

```bash
cargo run -p zap-cli -- receipts merge \
  logs/node-a.jsonl \
  logs/node-b.jsonl \
  --out logs/receipts.archive.jsonl
```

`merge` verifies every input log, keeps the first copy of each signed receipt,
and writes a deduplicated JSONL archive. The output path must be separate from
all input logs.

Receipts make local and future distributed operation auditable without creating financial semantics.
