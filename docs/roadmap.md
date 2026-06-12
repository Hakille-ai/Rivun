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

## Phase 2: Cognitive Interpreter

Foundation implemented:

- `zap-intent` maps local text or structured JSON intent into typed ZAP action steps;
- `zap compile-intent --explain` prints an auditable plan with rule metadata;
- `zap send --intent` sends each compiled step as a signed universal envelope;
- optional JSON intent policies can allow, deny, or require PoA for matching steps;
- emergency-stop safety intents mark frames with `REQUIRES_CONSENSUS`.

Next: expand the grammar and support local model backends behind the same policy gate.

## Phase 3: SDKs and Driver Registry

Foundation implemented:

- `zap-store` defines signed driver manifests;
- `zap driver-manifest create` signs local WASM/WAT artifacts;
- `zap driver-manifest verify` checks manifest signature, hash, ABI, and action;
- local `registry.index.toml` files track active or revoked manifest versions;
- `zap registry revoke` marks unsafe manifest versions as revoked;
- optional operator signatures can approve registry indexes for deployment gates;
- `zap-node` verifies configured manifests and registry entries before daemon startup;
- `zap-driver-sdk` provides minimal ABI helpers for driver authors.

Next: package distribution, registry compatibility policy, and remote index publishing.

## Phase 4: Proof-of-Action Network

Foundation implemented:

- `REQUIRES_CONSENSUS` frames can carry `ZPOA` trailers;
- validators sign a domain-separated digest of the signed frame;
- `zap-node` verifies configured validator public keys and threshold before dispatch;
- `zap send --intent` requires `--poa-validator-key` for critical intent steps.
- `zap send --poa-network` can collect attestations from configured validator peers with an operator-controlled timeout;
- portable PoA request/response JSON can be created with `zap poa request` and `zap poa attest`;
- optional signed action receipts record processed actions for audit.
- `zap receipts verify` checks signed receipt JSONL logs offline.
- `zap receipts prune` applies verified timestamp-based retention to receipt logs.
- `zap receipts merge` builds deduplicated verified receipt archives from multiple logs.

Next: dynamic validator discovery, quorum policy hardening, and remote receipt replication.

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

Next: active cache refresh workflows, richer peer trust policy, and carefully
scoped WASM host imports for memory access.
