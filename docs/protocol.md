# rivun Protocol

rivun is a universal low-latency protocol. It is independent of AI models, LLM providers, and application runtimes: any adapter can produce or consume rivun messages as long as it follows the wire and envelope contracts. rivun does not define billing, settlement, rewards, or financial rails.

The implementation is split into layers:

- **Wire**: signed binary frames with a strict fixed header.
- **Envelope**: typed `ZENV` payloads with kind, subject, content type, metadata, and body bytes.
- **Transport**: encrypted datagrams and peer addressing.
- **Node**: peer policy, replay protection, verification, receipts, and dispatch.
- **Runtime**: sandboxed WASM execution for local action handlers.
- **Adapters**: CLI, SDKs, devices, model runtimes, and application bridges.

## Wire Frame

Every rivun frame starts with a strict 64-byte big-endian header.

| Offset | Size | Field | Notes |
| --- | ---: | --- | --- |
| 0 | 4 | `MAGIC_NUMBER` | `0x5A41505F`, ASCII `@@rivun_HEADER@@` |
| 4 | 2 | `VERSION` | v1 is `0x0001` |
| 6 | 2 | `FLAGS` | encrypted, priority, consensus, signed, broadcast |
| 8 | 16 | `SOURCE_NODE` | UUID derived from the sender public key |
| 24 | 16 | `TARGET_NODE` | receiver UUID, or all zeroes for broadcast |
| 40 | 8 | `TIMESTAMP` | Unix microseconds |
| 48 | 8 | `@@rivun_HEADER@@LEN` | payload length, max 16 MiB in v1 |
| 56 | 8 | `@@rivun_HEADER@@SIGN` | fast signature hint, not a complete signature |

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

The Ed25519 signature transcript is the first 56 header bytes plus payload. `@@rivun_HEADER@@SIGN` is excluded to avoid self-referential signatures.

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

The wire payload can be a universal rivun envelope. Envelopes start with a 74-byte `ZENV` header and are parsed independently from transport and node policy:

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

The `rivun-envelope` API exposes owned constructors such as `ZapEnvelope::action(subject, body)`, `event`, `data`, `query`, and `response` using the default content type. Callers that need explicit media types can use `ZapEnvelope::new(kind, subject, content_type, body)` or `with_content_type(...)`. `ZapEnvelopeRef::parse(&[u8])` inspects encoded envelope bytes without copying the body.

## Control Subjects

Protocol extensions that do not require a new wire version use `ZENV` envelopes
with `kind = control` and versioned JSON bodies. Current control subjects
include:

| Subject | Content type | Purpose |
| --- | --- | --- |
| `rivun.capability.query` | `application/rivun-capability+json` | Ask a peer for advertised capabilities and grants |
| `rivun.capability.response` | `application/rivun-capability+json` | Return a signed peer capability advertisement |
| `rivun.capability.announce` | `application/rivun-capability+json` | Push an unsolicited signed capability advertisement |
| `rivun.discovery.announce` | `application/rivun-discovery+json` | Signed service and peer advertisement |
| `rivun.discovery.query` | `application/rivun-discovery+json` | Query a peer's services, peers, and learned announcements |
| `rivun.discovery.response` | `application/rivun-discovery+json` | Signed discovery response with services, peers, and known announcements |
| `rivun.poa.validator_set.request` | `application/rivun-poa-validator-set+json` | Request a signed versioned PoA validator set from a peer |
| `rivun.poa.validator_set.response` | `application/rivun-poa-validator-set+json` | Return a signed PoA validator set or an unavailable reason |
| `rivun.pact.verify` | `application/rivun-pact+json` | Exchange offline PACT verification requests or results |
| `rivun.pact.revoke` | `application/rivun-pact+json` | Exchange signed PACT revocation evidence |
| `rivun.pact.bundle` | `application/rivun-pact+json` | Exchange portable PACT bundles for offline verification |
| `rivun.registry.index.request` | `application/rivun-registry-index+json` | Request a peer's RivunStore registry index |
| `rivun.registry.index.response` | `application/rivun-registry-index+json` | Return a registry index or an unavailable reason |
| `rivun.registry.bundle.manifest.request` | `application/rivun-registry-bundle-manifest+json` | Request a peer's RivunStore bundle manifest |
| `rivun.registry.bundle.manifest.response` | `application/rivun-registry-bundle-manifest+json` | Return a bundle manifest or an unavailable reason |
| `rivun.receipts.request` | `application/rivun-receipts+json` | Request signed receipts from a peer receipt journal |
| `rivun.receipts.response` | `application/rivun-receipts+json` | Return verified signed receipts, with a truncation flag |
| `rivun.agent.intent` | `application/rivun-agent+json` | Typed agent intent (see [Agent Protocol](agent-protocol.md)) |
| `rivun.agent.session` | `application/rivun-agent+json` | Agent session lifecycle |
| `rivun.agent.delegation.request` | `application/rivun-agent+json` | Delegate scoped work to another agent |
| `rivun.agent.delegation.response` | `application/rivun-agent+json` | Accept, reject, or counter-offer a delegation |
| `rivun.agent.capability_negotiation.request` | `application/rivun-agent+json` | Negotiate required and optional capabilities |
| `rivun.agent.capability_negotiation.response` | `application/rivun-agent+json` | Capability negotiation outcome |
| `rivun.agent.status` | `application/rivun-agent+json` | Agent progress status |
| `rivun.agent.result` | `application/rivun-agent+json` | Terminal agent result |
| `rivun.agent.error` | `application/rivun-agent+json` | Structured agent error |
| `rivun.swarm.intent.propose` | `application/rivun-swarm+json` | Swarm consensus proposal |
| `rivun.swarm.intent.commit` | `application/rivun-swarm+json` | Swarm consensus commit certificate |
| `poa.attestation_request` | `application/rivun-poa+json` | Request a PoA attestation from a validator peer |
| `poa.attestation_response` | `application/rivun-poa+json` | Signed PoA attestation response |

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

PACT is a rivun-native profile for portable signed action records. A
`rivun.pact.record` message uses `kind = action`, subject `rivun.pact.record`, and
content type `application/rivun-pact+json`. Verification, revocation, and bundle
exchange use `kind = control` with `rivun.pact.verify`, `rivun.pact.revoke`, and
`rivun.pact.bundle`.

The canonical PACT signature payload contains only these ordered fields:
`pact_id`, `actor`, `target`, `intent`, `object`, `terms`, `consent`, `proof`,
`created_at_micros`, and `expires_at_micros`. Mutable audit fields such as
`status`, `hash`, `signature`, `verification`, `revocation`, and `timeline` are
excluded. Nested JSON object keys are sorted before hashing so Rust,
TypeScript, Python, and Go SDKs reproduce the same BLAKE3 digest.

PACT signatures reuse the existing rivun Ed25519 domain-message transcript with
domain `rivun-PACT-v1`. PACT execution evidence is recorded as optional receipt
metadata; it does not replace the signed receipt schema or introduce a
financial ledger.

## Encrypted UDP Datagram

`rivun-net` wraps encoded frames in an authenticated UDP datagram:

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

