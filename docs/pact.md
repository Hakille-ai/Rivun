# ZAP PACT Profile

PACT is a ZAP-native profile for portable signed action records. It captures
who requested an action, what they intended, the target and object, consent,
terms, proof, status, revocation evidence, and offline verification metadata.

PACT does not add a parallel API server, database, payment rail, or new wire
format. A PACT record is ordinary `application/zap-pact+json` carried inside
the existing `ZENV` envelope and signed with the existing ZAP Ed25519
domain-message helpers.

## Message Subjects

| Subject | ZENV kind | Content type | Purpose |
| --- | --- | --- | --- |
| `zap.pact.record` | `action` | `application/zap-pact+json` | Portable signed action record |
| `zap.pact.verify` | `control` | `application/zap-pact+json` | Verification request or result exchange |
| `zap.pact.revoke` | `control` | `application/zap-pact+json` | Signed revocation evidence exchange |
| `zap.pact.bundle` | `control` | `application/zap-pact+json` | Offline bundle exchange |

`zap.pact.record` is an action because it can participate in normal policy,
PoA, routing, dispatch, and receipt flows. Verification, revocation, and bundle
exchange are control messages because they coordinate protocol evidence rather
than request device or driver execution.

## Canonical Signature Payload

The signed payload is a fixed ordered JSON object containing only immutable
fields:

1. `pact_id`
2. `actor`
3. `target`
4. `intent`
5. `object`
6. `terms`
7. `consent`
8. `proof`
9. `created_at_micros`
10. `expires_at_micros`

Mutable fields are excluded from the signature payload: `status`, `hash`,
`signature`, `verification`, `revocation`, and `timeline`.

Nested JSON object keys are sorted before hashing. This lets Rust,
TypeScript, Python, and Go reproduce the same BLAKE3 digest for the same PACT
record. The hash format is `blake3:<64 lowercase hex characters>`.

PACT signatures use the ZAP domain-message transcript:

```text
domain || 0x00 || canonical_signing_payload
```

The PACT signature domain is:

```text
ZAP-PACT-v1
```

Revocation evidence uses:

```text
ZAP-PACT-REVOCATION-v1
```

## CLI Workflow

Create an unsigned record:

```bash
cargo run -p zap-cli -- pact create \
  --pact-id 33333333-3333-4333-8333-333333333333 \
  --actor agent.alpha \
  --target driver.valve \
  --intent valve.open \
  --object '{"valve":"v-7","zone":"north"}' \
  --terms '{"max_runtime_ms":5000,"requires_receipt":true}' \
  --consent '{"operator":"ops.lead","approved":true}' \
  --proof '{"kind":"policy","decision":"allow"}' \
  --created-at-micros 1893456000000000 \
  --expires-at-micros 1893459600000000 \
  --out pact-unsigned.json
```

Sign it with an existing ZAP node key:

```bash
cargo run -p zap-cli -- pact sign \
  --input pact-unsigned.json \
  --key .zap/node.key \
  --out pact-signed.json
```

Verify offline:

```bash
cargo run -p zap-cli -- pact verify \
  --input pact-signed.json \
  --now-micros 1893457000000000 \
  --json
```

Revoke with signed evidence:

```bash
cargo run -p zap-cli -- pact revoke \
  --input pact-signed.json \
  --revoked-by ops.lead \
  --reason "operator stop" \
  --key .zap/node.key \
  --revoked-at-micros 1893457000000000 \
  --out pact-revoked.json
```

Export and verify a portable bundle:

```bash
cargo run -p zap-cli -- pact bundle export \
  --pact pact-signed.json \
  --out pact-bundle.json

cargo run -p zap-cli -- pact bundle verify \
  --bundle pact-bundle.json \
  --now-micros 1893457000000000 \
  --json
```

Export the JSON schema:

```bash
cargo run -p zap-cli -- pact schema --out pact.schema.json
```

## Receipts

When a node processes a signed `zap.pact.record` with
`application/zap-pact+json`, it verifies the PACT body before writing the
receipt reference. The existing signed receipt schema is not replaced. Instead,
the receipt gets an optional `pact` object containing:

- `pact_id`
- `intent`
- canonical PACT `hash`
- PACT `status`
- optional policy decision
- optional PoA summary
- optional output hash

This keeps PACT evidence tied to normal ZAP audit logs while preserving the
receipt ledger as the authoritative execution record.

## Fixtures and SDKs

Shared fixtures:

- `fixtures/pact-record-v1.json`
- `fixtures/pact-bundle-v1.json`
- `fixtures/protocol/signed-pact-record-frame-v1.json`

Validate all fixtures:

```bash
cargo run -p zap-cli -- fixtures verify --fixtures fixtures
```

Validate SDK coverage:

```bash
cargo run -p zap-cli -- fixtures verify --fixtures fixtures --sdk sdks/typescript --json
cargo run -p zap-cli -- fixtures verify --fixtures fixtures --sdk sdks/python --json
cargo run -p zap-cli -- fixtures verify --fixtures fixtures --sdk sdks/go --json
cargo run -p zap-cli -- fixtures verify --fixtures fixtures --sdk sdks/rust --json
```

The official Rust, TypeScript, Python, and Go SDKs load the same PACT fixture
and reproduce the same canonical BLAKE3 hash. Rust, TypeScript, and Go verify
the fixture signature directly in their standard test paths. Python verifies
when installed with the optional `crypto` dependencies.

## Boundaries

PACT is protocol evidence, not a financial ledger. The Pactara idea of
portable value maps to `terms` in ZAP so operators can describe limits,
conditions, or value-like context without creating settlement semantics.

Revocation is signed protocol evidence in records, bundles, and receipts. It is
not a global centralized registry. Operators can still layer registries,
policy, or peer distribution above the profile when a deployment needs that.
