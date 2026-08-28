# Security Model

rivun v1 treats identity, transport confidentiality, and execution isolation as separate layers.

## Identity

Each node has an Ed25519 keypair. The node UUID is deterministically derived from the public key with a domain-separated BLAKE3 hash and UUID version 8 bits.

`@@rivun_HEADER@@SIGN` is only an 8-byte hint. It is useful for fast filtering, but the verifier must validate the full 64-byte Ed25519 signature in the auth trailer.

## Transport

`rivun-net` encrypts UDP datagrams with ChaCha20-Poly1305. The clear envelope contains routing metadata and nonce; the entire envelope is authenticated as AEAD associated data. Static peer discovery is used in v1 because it is auditable and deterministic.

Transport nonces are 96 bits: a random 32-bit prefix generated when an endpoint binds, followed by a monotonic 64-bit counter. This prevents nonce reuse across normal process restarts even when a static transport key is reused. If the counter is ever exhausted, sending fails and the transport key must be rotated.

Receivers keep a bounded in-memory nonce cache per peer and reject repeated
datagram nonces before returning inbound frames to higher layers. `rivun-node`
sizes this cache with `security.replay_cache_capacity`.

Anti-replay can survive restarts:

- `rivun-net::durable_replay::DurableNonceStore` persists transport nonces in a
  write-ahead log (`ZAPNONC1` records), compacted atomically, closing the
  restart replay window for datagrams;
- `rivun-node::durable_replay::DurableReplayStore` persists frame fingerprints
  (with clock-skew checks) across restarts at the daemon level.

The crate also includes a Noise `NN_25519_ChaChaPoly_BLAKE2s` helper for future dynamic session bootstrap.

## Peer Trust

Transport keys prove that a datagram came from a configured peer, but transport
trust is not the same as operational permission. Each peer can carry a local
trust contract:

```toml
[trust]
require_peer_expiry = true
max_transport_key_age_micros = 2592000000000

[peers.trust]
status = "trusted"
allow_send = true
allow_receive = true
allow_forward = false
allow_poa_attestation = true
expires_at_micros = 1765000000000000
```

`rivun-node` rejects revoked peers during config validation, warns for
quarantined or restricted peers, blocks outbound CLI sends when
`allow_send=false`, rejects inbound frames when `allow_receive=false`, and
prevents routes from targeting peers where forwarding is disabled. Optional
expiry and transport-key age gates let operators force peer re-enrollment and
key rotation on a schedule.

`rivun peer invite` creates a signed offline enrollment document for a node's
advertised address, public key, transport key, key epoch, optional expiry, and
labels. `rivun peer accept` verifies the domain-separated Ed25519 signature and
node id before emitting a peer block or updated config. `rivun peer rotate` bumps
a peer transport-key epoch, and `rivun peer revoke` sets trust to `revoked` while
disabling send, receive, forwarding, and PoA-attestation permissions.

## Anti-Replay

`rivun-node` validates timestamp freshness before dispatching an action. By default, frames outside a five-minute clock-skew window are rejected. The node also keeps an in-memory BLAKE3 fingerprint cache of recently accepted frames per process and rejects exact frame replays.

The policy is configured with:

```toml
[security]
max_clock_skew_micros = 300000000
replay_cache_capacity = 4096
```

Set `replay_cache_capacity = 0` only for specialized tests where frame replay and transport nonce replay detection must be disabled.

For production checks, run:

```bash
rivun check-config --strict --config rivun.toml
```

Strict mode exits non-zero when validation emits safety warnings, such as
unsigned frame acceptance, disabled replay detection, missing signed driver
manifests, missing PoA validators, reused peer transport keys, or overly broad
Unix key-file permissions.

## Proof-of-Action

Frames marked `REQUIRES_CONSENSUS` must carry a PoA certificate before `rivun-node` dispatches them. The certificate contains a digest of the signed frame, a threshold, and Ed25519 attestations from configured validators.

Node config controls the verifier set:

```toml
[poa]
required_threshold = 1

[[poa.validators]]
node_id = "..."
public_key = "..."
```

For distributed operations, `[poa]` can instead point at a signed, versioned
validator-set JSON file:

```toml
[poa]
required_threshold = 2
validator_set = "poa-validators.v4.json"
validator_set_authority = "operator-public-key"
```

`rivun poa validator-set create` signs the set with an authority key, `verify`
checks the domain-separated signature and validator identities, and `apply`
writes the config fields above. At startup, `rivun-node` verifies the set, rejects
expired or not-yet-valid sets, and uses the stricter of the local threshold and
the set threshold. `rivun poa validator-set pull` can fetch this signed JSON from
a configured peer over `ZENV` control messages; operators should still pin the
expected authority public key so the nested set signature is checked against the
intended authority, not just the responding peer.

`rivun send --requires-consensus` refuses to emit consensus-protected frames
unless a local `--poa-validator-key` is supplied or `--poa-network` can collect
a configured validator quorum.

Receiver-side `[message_policy]` rules can require PoA for matching typed
messages. For example, a node can require every `action` with subject
`safety.*` to carry `REQUIRES_CONSENSUS` and a valid certificate before routing
or driver execution.

`rivun send --poa-network` can request attestations from configured validator
peers. Responses are accepted only when the validator public key matches config,
the response signature verifies, and the response digest equals the requested
signed-frame digest. The sender waits up to `--poa-timeout-ms` for the required
threshold; the default is 2000 ms.

## Swarm, Gossip, and BFT Consensus

When deployments enable the swarm subsystems (`rivun-net::consensus`,
`rivun-net::gossip`, `rivun-net::mesh`), the trust model extends as follows:

- **BFT consensus** — `BftConsensusEngine` requires `2n/3 + 1` quorum; pre-votes
  and pre-commits are Ed25519-signed; equivocation produces a proof and slashes
  the offender; commit certificates are compact binary (`ZSC1`) with signer
  bitmasks. This is a domain-specific coordination mechanism, not a blockchain.
- **Gossip** — `GossipEnvelope` (`ZGSP`) messages are signed; message ids are
  BLAKE3 digests; hop damping (max 16) bounds amplification; PEX and
  anti-entropy sync carry only hashes and ranges until verified.
- **Mesh** — `ZapRelayEnvelope` (`ZRLY`) relays are encrypted and hop-limited
  (max 2) so partitions cannot be used to bypass peer trust.
- **Partition safety** — phi-accrual failure detection and `PartitionStatus`
  classify `Normal`/`DegradedMinority`/`Isolated`; the 2/3 quorum rule means a
  minority subset can never commit.

The swarm protocol subjects are `rivun.swarm.intent.propose` and
`rivun.swarm.intent.commit` (see [Network](network.md)).

## Signed Receipts

When `[receipts].dir` is configured, `rivun-node` appends one Ed25519-signed receipt after each processed action to the binary receipt journal. Receipts contain hashes, action metadata, and optional PoA summaries. They are audit records only, not financial records.

Operators can pull receipts from configured peers with signed `ZENV` control
messages. `rivun receipts pull` accepts bounded filters, verifies the peer's
signed response frame, verifies every nested receipt signature, and writes a
binary journal that remains compatible with offline `verify`, `export-jsonl`,
and `compact` workflows.

## PACT Signed Action Records

PACT records are signed protocol evidence carried as
`application/rivun-pact+json` in `ZENV` envelopes. They do not introduce a new
identity system or signature stack: PACT hashing uses BLAKE3 and PACT
signatures use the same Ed25519 domain-message transcript as other rivun
evidence. The PACT signature domain is `rivun-PACT-v1`.

The canonical PACT signing payload excludes mutable fields such as status,
hash, signature, verification results, revocation evidence, and timeline
entries. Nested JSON keys are sorted before hashing so official SDKs reproduce
the same digest offline.

When a node receives `rivun.pact.record`, it verifies the PACT body before
attaching a PACT reference to the signed receipt. Revocation is represented as
signed protocol evidence in records and bundles, not as a central global
registry.

## Capability Discovery, Routing, and Memory

Capability discovery uses signed `ZENV` control messages with subjects
`rivun.capability.query` and `rivun.capability.response`. Responses describe local
drivers, configured memory access, and declared host permissions. They do not
grant authority by themselves; enforcement still comes from node config, signed
RivunStore manifests, registry policy, and runtime checks.

Configured capability grants are policy assertions attached to advertisements,
not ambient authority. `rivun-node` rejects grants for capabilities it does not
actually advertise, and deployments can set
`capability_policy.require_grants_for_advertised = true` to require explicit
grant coverage for every advertised capability.

`rivun capability query --cache` stores signed peer responses in a local
hash-chained JSONL cache. `rivun capability cache refresh --config rivun.toml
--strict` actively refreshes configured peers into `[capability_cache].path`
and reports any skipped or failed peer before route gates depend on cached
grants. Cache verification checks entry hashes, chain continuity, peer/ad node
identity consistency, duplicate entry ids, and grants that reference missing
advertised capabilities.

Routes can declare `requires_peer_grant`. During config validation, `rivun-node`
verifies the capability cache and requires the target peer's latest cached
advertisement to grant the requested capability before the node can start with
that route.

Routes are deterministic config entries evaluated after frame verification,
PoA validation, timestamp freshness, and replay checks. Forwarded routes create
new signed frames from the routing node. Consensus-protected frames are not
forwarded in v1 because the original PoA certificate is bound to the original
signed frame.

`rivun-memory` stores local memory as append-only binary journal records with
BLAKE3 body hashes, entry-to-entry hash chaining, disk indexes, and tombstones.
`rivun memory verify` recalculates stored hashes, validates the append-only chain,
and rejects orphaned tombstones. The v1 memory store is local audit data, not a
remote database and not a hidden model state channel.

## Gateway Security

`rivun-gateway` exposes the node through MCP (stdio/HTTP), REST, SSE, and
WebSocket transports. The trust boundary is preserved:

- optional bearer-token authentication on HTTP endpoints (`--auth-token`);
- every mutation is executed through the node pipeline: signatures, freshness,
  replay checks, policy, PoA, and receipts still apply;
- frame/payload size limits are enforced on every transport (4 MB default);
- the provenance chain is root-signed; `MissingStep` or
  `StepVerificationFailed` rejects digests and surfaces as gateway errors;
- MCP stdio mode is intended for trusted local agent runtimes.

See [Gateway](gateway.md) for the transports and the provenance chain.

## Runtime Isolation

WASM drivers receive no host imports by default. Network, filesystem, clock, and environment capabilities are still denied as broad ambient authority.

Signed RivunStore manifests bind a driver action to a BLAKE3 artifact hash, ABI version, declared permissions, and author Ed25519 identity. `rivun-node` verifies the manifest signature and hash before compiling the driver. Local registry indexes can also carry an operator signature; set `[registry] require_signature = true` to reject unsigned or tampered indexes during config validation and daemon startup. `rivun registry resolve --abi-req` lets installers constrain integer ABI ranges, while `rivun registry migration add` records signed upgrade metadata that install plans carry forward for rollout gates. `rivun registry deprecate` marks migration-only releases that automatic resolution skips, while `rivun registry revoke` blocks unsafe releases. `rivun registry pull --operator-public-key` fetches indexes over signed peer control frames while still requiring the nested registry operator signature, so transport authenticity and deployment approval remain separate checks. `rivun registry mirror` keeps that model across multiple peers: it fails on conflicting driver identity fields, lets revocations override active entries, and emits an unsigned merged index that must be reviewed and re-signed before deployment. `rivun registry publication create` then signs the canonical hash of that approved registry, giving release audits a stable record of the exact index that was published. `rivun registry bundle pull-manifest` discovers a peer's published bundle manifest over signed control frames, while `rivun registry bundle verify/import` remains the final gate that recomputes registry, publication, manifest, and driver hashes and only copies listed safe relative paths. ABI v2 foundations expose only scoped `rivun` imports for event emission, local memory interaction, and device-call requests. Those imports are inert unless explicitly granted, bounded by `max_host_call_bytes`, and still subject to node config gates such as `[memory] allow_driver_write = true`.

Current enforced limits:

- max linear memory;
- one instance, one memory, one table;
- fuel budget;
- output byte limit;
- wall-clock epoch interruption.
- host call byte limit.

