# Roadmap

## Phase 1: Kernel Alpha

Implemented in this repository:

- strict ZAP-Wire header;
- frame signing and verification;
- encrypted UDP endpoint;
- static peer discovery;
- explicit capability discovery;
- deterministic route planning;
- auditable local memory store;
- sandboxed WASM execution;
- CLI, docs, tests, benches.

## Phase 2: Typed Agent Gateway

Foundation implemented:

- external agents and models emit strict typed `ZENV` messages instead of relying on an in-protocol natural-language compiler;
- `zap send --kind ... --subject ...` sends typed messages directly;
- `[message_policy]` rules can allow, deny, or require PoA for matching `kind` and `subject`;
- `zap send --requires-consensus` marks frames with `REQUIRES_CONSENSUS` and attaches PoA certificates through local validator keys or network validators;
- receiver-side policy rejects critical messages that arrive without required Proof-of-Action.

Next: publish SDK-friendly schemas for model gateways and richer policy authoring.

Progress added:

- `zap-schema` provides typed message contracts for agent gateways, machine
  commands, JSON payload validation, and optional node-side allowlists.
- `zap-policy` provides deterministic policy evaluation for `allow`, `deny`,
  `require_poa`, `require_grant`, `human_approval`, and `simulate_first`.
- `zap schema validate` and `zap policy evaluate` expose operator workflows for
  preflight validation before messages are signed and sent.

## Phase 3: SDKs and Driver Registry

Foundation implemented:

- `zap-store` defines signed driver manifests;
- `zap driver-manifest create` signs local WASM/WAT artifacts;
- `zap driver-manifest verify` checks manifest signature, hash, ABI, and action;
- local `registry.index.toml` files track active or revoked manifest versions;
- `zap registry revoke` marks unsafe manifest versions as revoked;
- optional operator signatures can approve registry indexes for deployment gates;
- `zap registry pull` fetches a peer registry index over signed control messages
  and can require an expected operator public key;
- `zap registry mirror` merges compatible signed peer indexes and preserves
  revocation priority for unsafe driver versions;
- `zap registry publication create/verify` records a signed publication
  statement over the canonical registry hash for release audit trails;
- `zap registry bundle export/verify/import` packages signed registries,
  publication metadata, manifests, and optional drivers for offline deployment;
- `zap-node` verifies configured manifests and registry entries before daemon startup;
- `zap-driver-sdk` provides minimal ABI helpers for driver authors.

Next: registry compatibility policy, semantic package version ranges, and
remote bundle distribution services.

## Phase 4: Proof-of-Action Network

Foundation implemented:

- `REQUIRES_CONSENSUS` frames can carry `ZPOA` trailers;
- validators sign a domain-separated digest of the signed frame;
- `zap-node` verifies configured validator public keys and threshold before dispatch;
- `zap send --requires-consensus` requires `--poa-validator-key` or `--poa-network` for consensus-protected frames.
- `zap send --poa-network` can collect attestations from configured validator peers with an operator-controlled timeout;
- portable PoA request/response JSON can be created with `zap poa request` and `zap poa attest`;
- signed versioned validator sets can be created, verified, applied to config,
  and loaded by `zap-node` through `[poa].validator_set`;
- `zap poa validator-set pull` requests signed validator sets from configured
  peers over `ZENV` control messages and verifies the nested authority
  signature before writing JSON.
- optional signed action receipts record processed actions for audit.
- `zap receipts verify` checks signed receipt JSONL logs offline.
- `zap receipts prune` applies verified timestamp-based retention to receipt logs.
- `zap receipts merge` builds deduplicated verified receipt archives from multiple logs.
- `zap receipts pull` requests signed receipts from configured peers over
  `ZENV` control messages, verifies the response, and writes mergeable JSONL.

Next: dynamic validator discovery, quorum policy hardening, and automated validator-set rollout.

## Phase 5: Future Core Interfaces

Foundation implemented:

- `zap-capability` defines capability ids, driver permission contracts, local
  advertisements, and signed query/response control subjects;
- `zap-router` provides deterministic route rules and `zap route explain`;
- `zap-node` applies routes before dispatch and can forward non-consensus
  messages by creating new signed frames;
- `zap-memory` provides append-only JSONL memory records, tombstones, pruning,
  body hash verification, and entry-to-entry hash-chain verification;
- `zap capability`, `zap route`, and `zap memory` expose operator workflows.
- `zap doctor` provides a score-based readiness gate over config validation,
  provenance, registry, receipts, PoA, memory, routing, and capability posture.
- capability advertisements can carry configured policy grants and
  requirements, with an optional gate requiring every advertised capability to
  have an explicit grant.
- remote capability query responses can be persisted in a hash-chained local
  cache and verified offline before operator review or deployment automation.
- peer routes can require a verified cached grant before forwarding messages to
  a remote node.

Next: live peer enrollment handshakes, distributed revocation propagation,
package distribution, fleet ops, and stream/mesh transport.

Progress added:

- `DriverPermissions` now includes scoped host permissions for event emission,
  local memory reads/writes, device-call requests, and per-call byte limits.
- `zap-runtime` exposes deny-by-default `zap` WASM imports and captures host
  calls as auditable runtime output.
- `zap-node` commits permitted `memory_write` host calls to the hash-chained
  memory store, preserving source node and frame hash.
- peer trust contracts add local machine-communication permissions for send,
  receive, route forwarding, PoA attestations, trust expiry, and transport-key
  rotation age.
- `zap trust enroll` and `zap trust inspect` provide operator workflows for
  peer onboarding and trust posture review before nodes run.
- `zap peer invite`, `zap peer accept`, `zap peer rotate`, and
  `zap peer revoke` provide signed offline machine enrollment, transport-key
  rotation, and local revocation workflows.
- `zap capability cache refresh` actively queries configured peers, appends
  signed advertisements to the verified JSONL cache, and reports skipped or
  failed peers for strict deployment gates.
