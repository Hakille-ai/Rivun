# rivun PACT Profile

PACT is a rivun-native profile for portable signed action records. It captures
who requested an action, what they intended, the target and object, consent,
terms, proof, status, revocation evidence, and offline verification metadata.

PACT does not add a parallel API server, database, payment rail, or new wire
format. A PACT record is ordinary `application/rivun-pact+json` carried inside
the existing `ZENV` envelope and signed with the existing rivun Ed25519
domain-message helpers.

## Message Subjects

| Subject | ZENV kind | Content type | Purpose |
| --- | --- | --- | --- |
| `rivun.pact.record` | `action` | `application/rivun-pact+json` | Portable signed action record |
| `rivun.pact.verify` | `control` | `application/rivun-pact+json` | Verification request or result exchange |
| `rivun.pact.revoke` | `control` | `application/rivun-pact+json` | Signed revocation evidence exchange |
| `rivun.pact.bundle` | `control` | `application/rivun-pact+json` | Offline bundle exchange |

`rivun.pact.record` is an action because it can participate in normal policy,
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

PACT signatures use the rivun domain-message transcript:

```text
domain || 0x00 || canonical_signing_payload
```

The PACT signature domain is:

```text
rivun-PACT-v1
```

Revocation evidence uses:

```text
rivun-PACT-REVOCATION-v1
```

## CLI Workflow

Create an unsigned record:

```bash
cargo run -p rivun-cli -- pact create \
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

Sign it with an existing rivun node key:

```bash
cargo run -p rivun-cli -- pact sign \
  --input pact-unsigned.json \
  --key .rivun/node.key \
  --out pact-signed.json
```

Verify offline:

```bash
cargo run -p rivun-cli -- pact verify \
  --input pact-signed.json \
  --now-micros 1893457000000000 \
  --json
```

Revoke with signed evidence:

```bash
cargo run -p rivun-cli -- pact revoke \
  --input pact-signed.json \
  --revoked-by ops.lead \
  --reason "operator stop" \
  --key .rivun/node.key \
  --revoked-at-micros 1893457000000000 \
  --out pact-revoked.json
```

Export and verify a portable bundle:

```bash
cargo run -p rivun-cli -- pact bundle export \
  --pact pact-signed.json \
  --out pact-bundle.json

cargo run -p rivun-cli -- pact bundle verify \
  --bundle pact-bundle.json \
  --now-micros 1893457000000000 \
  --json
```

Export the JSON schema:

```bash
cargo run -p rivun-cli -- pact schema --out pact.schema.json
```

## Dispute and Escrow

`rivun-pact::dispute` adds a deterministic mediation layer over signed PACT
records for multi-party workflows:

- `EscrowPact` — a PACT locked in escrow with `PactState` transitions
  (`Locked → Settled / Disputed / Slashed`);
- `DisputeCase` — an opened dispute with evidence (`DisputeEvidence`) and
  arbitration votes;
- `DisputeEngine` — `create_escrow_pact`, `settle_normal`,
  `execute_timeout_slash`, `open_dispute`, `submit_arbitration_vote`;
- `RulingOutcome` — `ReleaseToRecipient`, `SlashRefundToSender`,
  `SplitEqual`.

Quorum counting is explicit (`verify_quorum`), so a 2-of-3 arbitration panel
can settle an escrow deterministically and offline. Slashing is applied
automatically when an enforced timeout expires.

### Durable dispute state

`DisputeEngine` can persist and restore its complete escrow/dispute state with
`save_to_path` and `load_from_path`. The store writes a fsynced temporary file
before replacing the snapshot, and refuses to restore a file with an invalid
version, checksum, participant relation, escrow allocation, arbitration
configuration, or final-ruling quorum. Applications should save after each
state transition; automatic cross-node replication and signature verification
of arbitration votes remain separate deployment responsibilities.

Escrow and disputes are **protocol evidence**, not a payment rail: the ruling
outcome is recorded in the signed PACT timeline and can be referenced by
receipts, but no value moves through rivun itself.

## Receipts

When a node processes a signed `rivun.pact.record` with
`application/rivun-pact+json`, it verifies the PACT body before writing the
receipt reference. The existing signed receipt schema is not replaced. Instead,
the receipt gets an optional `pact` object containing:

- `pact_id`
- `intent`
- canonical PACT `hash`
- PACT `status`
- optional policy decision
- optional PoA summary
- optional output hash

This keeps PACT evidence tied to normal rivun audit logs while preserving the
receipt ledger as the authoritative execution record.

## Fixtures and SDKs

Shared fixtures:

- `fixtures/pact-record-v1.json`
- `fixtures/pact-bundle-v1.json`
- `fixtures/protocol/signed-pact-record-frame-v1.json`

Validate all fixtures:

```bash
cargo run -p rivun-cli -- fixtures verify --fixtures fixtures
```

Validate SDK coverage:

```bash
cargo run -p rivun-cli -- fixtures verify --fixtures fixtures --sdk sdks/typescript --json
cargo run -p rivun-cli -- fixtures verify --fixtures fixtures --sdk sdks/python --json
cargo run -p rivun-cli -- fixtures verify --fixtures fixtures --sdk sdks/go --json
cargo run -p rivun-cli -- fixtures verify --fixtures fixtures --sdk sdks/rust --json
```

The official Rust, TypeScript, Python, and Go SDKs load the same PACT fixture
and reproduce the same canonical BLAKE3 hash. Rust, TypeScript, and Go verify
the fixture signature directly in their standard test paths. Python verifies
when installed with the optional `crypto` dependencies.

## Boundaries

PACT is protocol evidence, not a financial ledger. The Pactara idea of
portable value maps to `terms` in rivun so operators can describe limits,
conditions, or value-like context without creating settlement semantics.

Revocation is signed protocol evidence in records, bundles, and receipts. It is
not a global centralized registry. Operators can still layer registries,
policy, or peer distribution above the profile when a deployment needs that.

