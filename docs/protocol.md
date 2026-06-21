# ZAP Protocol

ZAP is a universal low-latency protocol. It is independent of AI models, LLM providers, and application runtimes: any adapter can produce or consume ZAP messages as long as it follows the wire and envelope contracts. ZAP does not define billing, settlement, rewards, or financial rails.

The implementation is split into layers:

- **Wire**: signed binary frames with a strict fixed header.
- **Envelope**: typed `ZENV` payloads with kind, subject, content type, metadata, and body bytes.
- **Transport**: encrypted datagrams and peer addressing.
- **Node**: peer policy, replay protection, verification, receipts, and dispatch.
- **Runtime**: sandboxed WASM execution for local action handlers.
- **Adapters**: CLI, SDKs, devices, model runtimes, and application bridges.

## Wire Frame

Every ZAP frame starts with a strict 64-byte big-endian header.

| Offset | Size | Field | Notes |
| --- | ---: | --- | --- |
| 0 | 4 | `MAGIC_NUMBER` | `0x5A41505F`, ASCII `ZAP_` |
| 4 | 2 | `VERSION` | v1 is `0x0001` |
| 6 | 2 | `FLAGS` | encrypted, priority, consensus, signed, broadcast |
| 8 | 16 | `SOURCE_NODE` | UUID derived from the sender public key |
| 24 | 16 | `TARGET_NODE` | receiver UUID, or all zeroes for broadcast |
| 40 | 8 | `TIMESTAMP` | Unix microseconds |
| 48 | 8 | `ZAP_LEN` | payload length, max 16 MiB in v1 |
| 56 | 8 | `ZAP_SIGN` | fast signature hint, not a complete signature |

The encoded wire frame layout is:

```text
[64-byte header][payload][optional auth trailer]
```

The auth trailer is present when `SIGNED` is set:

```text
magic: "ZSIG"
algorithm: u16 = 1 (Ed25519)
signature_len: u16 = 64
signature: [u8; 64]
```

The Ed25519 signature transcript is the first 56 header bytes plus payload. `ZAP_SIGN` is excluded to avoid self-referential signatures.

When `REQUIRES_CONSENSUS` is set, a frame can carry a Proof-of-Action trailer after the auth trailer:

```text
magic: "ZPOA"
version: u16 = 1
threshold: u16
attestation_count: u16
reserved: u16 = 0
frame_digest: [u8; 32]
attestations:
  validator_node: [u8; 16]
  signature: [u8; 64]
```

The `frame_digest` is a domain-separated BLAKE3 digest of the signed frame without the PoA trailer. Validator signatures are Ed25519 signatures over that digest with a PoA-specific domain prefix.

## Universal Envelope

The wire payload can be a universal ZAP envelope. Envelopes start with a 74-byte `ZENV` header and are parsed independently from transport and node policy:

```text
magic: "ZENV"
version: u16 = 1
kind: u16
reserved: u16 = 0
id: [u8; 16]
correlation_id: [u8; 16], or UUID nil when absent
causation_id: [u8; 16], or UUID nil when absent
subject_len: u16
content_type_len: u16
metadata_len: u32
body_len: u64
subject: UTF-8 bytes
content_type: UTF-8 bytes
metadata: opaque bytes
body: opaque bytes
```

Envelope kinds in v1 are encoded as `u16`:

| Value | Kind |
| ---: | --- |
| 1 | `data` |
| 2 | `event` |
| 3 | `command` |
| 4 | `query` |
| 5 | `response` |
| 6 | `stream_chunk` |
| 7 | `action` |
| 8 | `control` |

The `zap-envelope` API exposes owned constructors such as `ZapEnvelope::action(subject, body)`, `event`, `data`, `query`, and `response` using the default content type. Callers that need explicit media types can use `ZapEnvelope::new(kind, subject, content_type, body)` or `with_content_type(...)`. `ZapEnvelopeRef::parse(&[u8])` inspects encoded envelope bytes without copying the body.

## Control Subjects

Protocol extensions that do not require a new wire version use `ZENV` envelopes
with `kind = control` and versioned JSON bodies. Current control subjects
include:

| Subject | Content type | Purpose |
| --- | --- | --- |
| `zap.capability.query` | `application/zap-capability+json` | Ask a peer for advertised capabilities and grants |
| `zap.capability.response` | `application/zap-capability+json` | Return a signed peer capability advertisement |
| `zap.poa.validator_set.request` | `application/zap-poa-validator-set+json` | Request a signed versioned PoA validator set from a peer |
| `zap.poa.validator_set.response` | `application/zap-poa-validator-set+json` | Return a signed PoA validator set or an unavailable reason |
| `zap.pact.verify` | `application/zap-pact+json` | Exchange offline PACT verification requests or results |
| `zap.pact.revoke` | `application/zap-pact+json` | Exchange signed PACT revocation evidence |
| `zap.pact.bundle` | `application/zap-pact+json` | Exchange portable PACT bundles for offline verification |
| `zap.registry.index.request` | `application/zap-registry-index+json` | Request a peer's ZapStore registry index |
| `zap.registry.index.response` | `application/zap-registry-index+json` | Return a registry index or an unavailable reason |
| `zap.registry.bundle.manifest.request` | `application/zap-registry-bundle-manifest+json` | Request a peer's ZapStore bundle manifest |
| `zap.registry.bundle.manifest.response` | `application/zap-registry-bundle-manifest+json` | Return a bundle manifest or an unavailable reason |
| `zap.receipts.request` | `application/zap-receipts+json` | Request signed receipts from a peer receipt journal |
| `zap.receipts.response` | `application/zap-receipts+json` | Return verified signed receipts, with a truncation flag |

PoA validator-set requests can include a minimum epoch. Responses are signed as
normal frames and carry a nested signed validator-set document. Receivers should
verify the response frame, the nested validator-set signature, the expected
authority, and the epoch before applying it to config.

Registry index requests can set `require_signature = true`. Responses are signed
as normal frames and may carry a nested `DriverRegistry` document. Receivers
should verify the response frame and, for production, require an operator public
key so the pulled index is both peer-authenticated and operator-approved.
Multi-source mirroring reuses the same request/response subjects once per peer
and merges only compatible entries; it is not a separate wire protocol.

Registry bundle manifest requests can require publication metadata and driver
artifact entries. Responses are signed as normal frames and carry a
`RegistryBundleManifest` with registry, publication, manifest, and driver
checksums. Receivers should treat the manifest as discovery metadata, then
verify downloaded bundle files with `registry bundle verify` before import.

Receipt replication requests can filter by `after_processed_at_micros`,
`until_processed_at_micros`, `kind`, `subject`, `source_node`, and
`target_node`, and include a bounded `limit`. Responses contain signed receipt
objects, a `truncated` flag, and optionally `next_after_processed_at_micros`
for cursor-style bounded pulls. Receivers must verify the response frame
signature and each nested receipt signature before archiving or merging.

## PACT Profile

PACT is a ZAP-native profile for portable signed action records. A
`zap.pact.record` message uses `kind = action`, subject `zap.pact.record`, and
content type `application/zap-pact+json`. Verification, revocation, and bundle
exchange use `kind = control` with `zap.pact.verify`, `zap.pact.revoke`, and
`zap.pact.bundle`.

The canonical PACT signature payload contains only these ordered fields:
`pact_id`, `actor`, `target`, `intent`, `object`, `terms`, `consent`, `proof`,
`created_at_micros`, and `expires_at_micros`. Mutable audit fields such as
`status`, `hash`, `signature`, `verification`, `revocation`, and `timeline` are
excluded. Nested JSON object keys are sorted before hashing so Rust,
TypeScript, Python, and Go SDKs reproduce the same BLAKE3 digest.

PACT signatures reuse the existing ZAP Ed25519 domain-message transcript with
domain `ZAP-PACT-v1`. PACT execution evidence is recorded as optional receipt
metadata; it does not replace the signed receipt schema or introduce a
financial ledger.

## Encrypted UDP Datagram

`zap-net` wraps encoded frames in an authenticated UDP datagram:

```text
magic: "ZAPD"
version: u8 = 1
reserved: 3 zero bytes
source_node: 16 bytes
target_node: 16 bytes
nonce: 12 bytes
ciphertext: ChaCha20-Poly1305(frame bytes)
```

The datagram header is AEAD associated data. Reserved bytes must be zero in v1 and are rejected otherwise. The nonce is a random 32-bit endpoint prefix followed by a big-endian 64-bit counter.

For direct messages, the datagram `target_node` and inner frame `TARGET_NODE` both equal the receiving node id. For broadcast, each encrypted datagram still targets one concrete receiving peer for routing and key selection, while the inner frame sets `TARGET_NODE` to UUID nil and `BROADCAST` in `FLAGS`.
