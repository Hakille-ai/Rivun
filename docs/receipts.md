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

Receipts make local and future distributed operation auditable without creating financial semantics.
