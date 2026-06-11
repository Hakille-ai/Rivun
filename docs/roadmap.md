# Roadmap

## Phase 1: Kernel Alpha

Implemented in this repository:

- strict ZAP-Wire header;
- frame signing and verification;
- encrypted UDP endpoint;
- static peer discovery;
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
- `zap-node` verifies configured manifests and registry entries before daemon startup;
- `zap-driver-sdk` provides minimal ABI helpers for driver authors.

Next: package distribution, registry signatures, and compatibility policy.

## Phase 4: Proof-of-Action Network

Foundation implemented:

- `REQUIRES_CONSENSUS` frames can carry `ZPOA` trailers;
- validators sign a domain-separated digest of the signed frame;
- `zap-node` verifies configured validator public keys and threshold before dispatch;
- `zap send --intent` requires `--poa-validator-key` for critical intent steps.
- `zap send --poa-network` can collect attestations from configured validator peers;
- portable PoA request/response JSON can be created with `zap poa request` and `zap poa attest`;
- optional signed action receipts record processed actions for audit.

Next: validator discovery, quorum policy hardening, receipt replication, and operator retention policy.
