# ZAP

**Universal low-latency protocol for typed message dispatch, signed by default.**

ZAP is a compact, signed, encrypted, low-latency messaging protocol implemented
in Rust. It moves **typed messages** between nodes — data, events, commands,
queries, responses, stream chunks, actions, and control messages — through a
unified wire format with end-to-end cryptographic provenance.

ZAP is a protocol, not a runtime: it is agnostic to AI models, application
frameworks, and hardware. It is used wherever messages must be verifiable,
auditable, and safe to dispatch — from factory safety systems to agent
gateways.

**Status:** pre-1.0, under active development. Interfaces can evolve; see
[Versioning](docs/versioning.md) before depending on public APIs.

## Highlights

- **Signed wire format.** Every frame carries an Ed25519 signature verified by
  the receiver. Identity is a deterministic function of the node public key.
- **Authenticated encrypted transport.** ChaCha20-Poly1305 datagrams over UDP
  with per-endpoint nonce prefixes, replay protection, and an optional
  restart-persistent nonce window.
- **Proof-of-Action.** Consensus-protected frames require a threshold of
  validator attestations before dispatch. A two-phase BFT consensus engine
  (proposal → pre-vote → pre-commit → certificate) is available for swarm
  coordination.
- **WASM sandboxing.** Untrusted drivers run inside Wasmtime with fuel, memory,
  time, output, and host-call bounds.
- **Auditable by design.** Append-only binary journals, signed action receipts,
  MMR accumulators, and blinded rollup commitments make execution history
  verifiable offline.
- **Multiple transports & integration surfaces.** Native HTTP, SSE, and
  WebSocket servers with a Model Context Protocol (MCP) gateway for agent
  integrations.
- **Multi-agent protocol.** Typed intents, sessions, delegation, capability
  negotiation, status/result contracts, and a cryptographic provenance chain
  across execution stages.
- **SDKs in 4 languages.** Rust, Go, TypeScript, and Python share the same
  protocol fixtures and canonical hashing rules.

## When to use ZAP

- You need typed messages with cryptographic provenance between nodes.
- Actions must pass deterministic policy before execution.
- High-risk operations need Proof-of-Action, simulation, explicit grants, or
  multi-party approval.
- Untrusted extensions must run in a resource-bounded sandbox.
- Operators need signed receipts, replay protection, and audit evidence.
- Gateways and SDKs must agree on stable protocol fixtures across languages.

## When not to use ZAP

- You only need a generic message broker, queue, or RPC framework (use MQTT,
  NATS, or gRPC).
- You need a database, hidden model memory store, financial ledger, or payment
  rail — ZAP is explicitly not one of these.
- An integration would bypass identity, policy, grants, PoA, or receipts for
  convenience.
- You cannot tolerate pre-1.0 API and CLI evolution.

## Repository layout

```
crates/       Workspace crates (23) — protocol, node, runtime, tooling
docs/         Protocol, security, operations, and guide documentation
examples/     Runnable examples, configs, domain packs, WASM drivers
fixtures/     Versioned JSON protocol fixtures shared by SDKs and tests
sdks/         Rust, Go, TypeScript, Python SDKs
tests/e2e/    Opaque-box 4-tier end-to-end suite (174 tests)
tools/        xtask and benchmark tooling
website/      Marketing and docs site (Next.js)
```

## Quickstart

### Prerequisites

- Rust **1.93+** (edition 2024) — install via [rustup](https://rustup.rs)
- Docker (optional) — for containerized deployment

### Build and test

```bash
cargo build --workspace
cargo test --workspace --all-targets
```

### First steps

```bash
# Generate a node identity
cargo run -p zap-cli -- keygen --out .zap/node.key

# Send a typed action envelope (needs a configured peer, see below)
cargo run -p zap-cli -- send --config zap.toml --target <uuid> --action echo --payload hello

# Run a quick parse benchmark
cargo run -p zap-cli -- bench parse --iterations 100000
```

### Two-node demo

1. Generate keys for both nodes: `zap keygen --out .zap/node-a.key` and
   `zap keygen --out .zap/node-b.key`.
2. Copy `node_id` and `public_key` into
   [`examples/configs/node-a.toml`](examples/configs/node-a.toml) and
   [`examples/configs/node-b.toml`](examples/configs/node-b.toml), and set a
   shared 32-byte `transport_key` in both files.
3. Validate and run:

```bash
cargo run -p zap-cli -- check-config --config examples/configs/node-a.toml
cargo run -p zap-cli -- check-config --config examples/configs/node-b.toml

# Terminal 1
cargo run -p zap-cli -- run --config examples/configs/node-a.toml

# Terminal 2 — send an action and a typed event
cargo run -p zap-cli -- send --config examples/configs/node-b.toml \
  --target <node-a-uuid> --action echo --payload hello
cargo run -p zap-cli -- send --config examples/configs/node-b.toml \
  --target <node-a-uuid> --kind event --subject sensor.temperature \
  --payload '{"c":21.5}' --content-type application/json
```

> `zap send` binds to the `bind` address in its config so the receiver can
> enforce static peer addresses. Do not run `zap run` and `zap send` from the
> same config simultaneously.

### Programmatic examples

```bash
cargo run -p zap-examples --bin frame_basics      # frame creation, signing, verification
cargo run -p zap-examples --bin envelope_types    # ZENV envelopes and causal linking
cargo run -p zap-examples --bin memory_store      # append-only journal memory + audit
cargo run -p zap-examples --bin driver_manifest   # signed manifests + registry revocation
```

## Architecture

```
┌────────────────────────────────────────────────────────────────────────────┐
│                              ZAP Node (zap-node)                            │
│  policy │ capability │ router │ memory │ receipts │ observability          │
│  ───────────────────────────────┐                                          │
│         Node daemon             │   actors: udp_rx, gossip, mesh,           │
│  dispatch · receipts · PoA      │   consensus, execution                    │
│  ───────────────────────────────┘                                          │
│         │          │               │               │                       │
│  ┌──────▼───┐ ┌────▼─────┐ ┌───────▼───────┐ ┌─────▼──────────┐            │
│  │ Runtime  │ │ ZapStore │ │  Transport    │ │   Ledger       │            │
│  │ Wasmtime │ │ driver & │ │  (zap-net)    │ │   (zap-ledger) │            │
│  │ fuel/mem │ │ pack     │ │  ChaCha20     │ │   receipts     │            │
│  │ time/out │ │ registry │ │  Noise ·      │ │   MMR · batch  │            │
│  │ async    │ │ signed   │ │  replay       │ │   blinded      │            │
│  │ pipeline │ │ bundles  │ │  BFT ·        │ │   rollup cmts  │            │
│  │          │ │          │ │  gossip ·     │ │                │            │
│  │          │ │          │ │  mesh · PEX   │ │                │            │
│  └──────────┘ └──────────┘ └───────────────┘ └────────────────┘            │
│                                                                            │
│  ┌──────────────────────────────────────────────────────────────────────┐  │
│  │  Gateway (zap-gateway)    MCP stdio · HTTP REST · SSE · WebSocket     │  │
│  │                          provenance chain: intent → … → receipt       │  │
│  └──────────────────────────────────────────────────────────────────────┘  │
│                                                                            │
│  ┌─────────────────────────────────────────┐   ┌────────────────────────┐  │
│  │  Wire format (zap-core / zap-envelope)  │   │  Fleet (zap-telemetry) │  │
│  │  ZAP_ header │ ZENV │ ZSIG │ ZPOA       │   │  doctor · metrics      │  │
│  └─────────────────────────────────────────┘   │  incident · topology   │  │
│                                                └────────────────────────┘  │
└────────────────────────────────────────────────────────────────────────────┘
```

### Workspace crates

| Crate | Responsibility |
|---|---|
| **Core protocol** | |
| `zap-core` | ZAP-Wire v1 frame parsing/encoding: 64-byte `ZAP_` header, flags, auth and PoA trailers |
| `zap-envelope` | Universal `ZENV` payload envelopes: kind, subject, content type, metadata, body |
| `zap-crypto` | Node identity (Ed25519), key generation, frame signing/verification, PoA certificates |
| `zap-schema` | Typed message contracts for agent gateways and machine commands |
| `zap-pact` | PACT profile: signed action records, canonical hashes, revocations, bundles, dispute engine |
| **Node & execution** | |
| `zap-node` | Daemon core: config, peer verification, replay protection, receipts, capability-aware dispatch, actors |
| `zap-runtime` | Wasmtime sandboxed execution: ABI verification, fuel/memory/time/output limits, async pipeline, streaming |
| `zap-driver-sdk` | ABI helpers for WASM driver authors: async drivers, ring buffers, zero-copy IPC |
| `zap-machine` | Machine connections and profile contracts for industrial adapters |
| `zap-cli` | Operator CLI: 29 commands covering every workflow below |
| **Network** | |
| `zap-net` | Encrypted UDP transport, Noise handshake, durable anti-replay, BFT consensus, gossip, adaptive mesh |
| **Intelligence & policy** | |
| `zap-capability` | Capability ids, driver permission contracts, signed query/response |
| `zap-policy` | Deterministic policy decisions: allow/deny/PoA/grant/human/simulation |
| `zap-router` | Deterministic route tables and explainable route decisions |
| `zap-agent` | Agent protocol contracts: intents, sessions, delegation, negotiation, provenance chain, swarm |
| **Audit & storage** | |
| `zap-journal` | Append-only binary journal segments: hash chaining, sealing, indexes, crash recovery |
| `zap-ledger` | Signed action receipts, batch seals, incremental MMR, blinded rollup commitments |
| `zap-memory` | Append-only binary journal memory: body hashes, entries, tombstones, compaction |
| `zap-store` | Driver manifests, registry index, migrations, publications, install plans, bundles |
| `zap-pack` | Domain pack lifecycle: build, sign, verify, install, audit (shared with `zap-store`) |
| **Gateway & operations** | |
| `zap-gateway` | AI agent gateway: MCP server, HTTP REST, SSE, WebSocket, provenance chain engine |
| `zap-telemetry` | Fleet doctor, incident snapshots, Prometheus metrics, topology |
| `zap-ops` | Operations contracts: observability, governance, production configs |

## CLI

Full command reference for a node:

```bash
zap keygen            # generate a node identity key
zap run               # start the daemon
zap check-config      # validate a config
zap doctor            # operator readiness gate
zap fleet doctor      # multi-node health aggregation
zap send              # send a message or action to a peer
zap inspect           # decode a frame file
```

Identity & peer trust:

```bash
zap trust enroll / inspect
zap peer invite / accept / rotate / revoke
```

Protocol, policy & routing:

```bash
zap capability list / query / cache / inspect-manifest
zap discovery announce / query
zap route explain
zap policy evaluate
zap schema validate / inspect / export
```

Agents, PACT, packs & fixtures:

```bash
zap agent session / intent / status / result / delegate / negotiate / validate / schema
zap pact create / sign / verify / revoke / bundle / schema
zap pack init / build / sign / verify / install / audit / validate / inspect / list
zap fixtures verify --fixtures fixtures --sdk <sdk-path>
```

Drivers & registry:

```bash
zap driver-manifest create / verify
zap registry init / add / sign / verify-signature / resolve / pull / mirror
zap registry publication create / verify
zap registry plan create / verify
zap registry bundle export / pull-manifest / verify / import
zap registry revoke / deprecate / migration add / list
```

Audit & evidence:

```bash
zap receipts pull / verify / import-jsonl / export-jsonl / compact
zap memory put / get / query / tombstone / verify / prune / compact
zap memory import-jsonl / export-jsonl / export-evidence
zap incident snapshot
zap provenance verify
```

Consensus:

```bash
zap poa request / attest / validator-set create / verify / pull / apply
```

Gateway & simulation:

```bash
zap gateway start / status      # MCP stdio, HTTP REST, SSE, WebSocket
zap cluster up / status         # in-memory N-node cluster simulation
zap swarm bench / partition-test
zap bench parse
```

## SDKs

| SDK | Location | Notes |
|---|---|---|
| Rust (reference) | `sdks/rust` | Wraps canonical crates via path dependencies; network-free |
| Go | `sdks/go` | Control envelopes, UDP client, Ed25519, BLAKE3 |
| TypeScript | `sdks/typescript` | Node 24; `@noble` crypto; typecheck + declaration builds |
| Python | `sdks/python` | Dataclasses, stdlib UDP; optional `[crypto]` extra for hash/sign |

All SDKs consume the same versioned fixtures under `fixtures/` and reproduce
the same canonical BLAKE3 hashes and offline verification results. See
[SDKs](docs/sdks.md).

```bash
zap fixtures verify --fixtures fixtures --sdk sdks/rust --json
```

## Testing & benchmarks

```bash
cargo ci-fmt           # format check
cargo ci-test          # full workspace test suite
cargo ci-smoke         # end-to-end smoke: node + action + receipt
cargo ci-clippy        # lint with -D warnings
cargo ci-bench-smoke   # compile and run benchmarks in test mode
cargo ci-bench-full    # full Criterion run
```

The alias definitions live in [`.cargo/config.toml`](.cargo/config.toml) and
mirror the GitHub Actions workflows. The 4-tier end-to-end suite in
`tests/e2e` (174 tests) exercises all 15 roadmap features with real crypto and
no mocks — see [TEST_INFRA.md](TEST_INFRA.md).

| CI platform | Checks |
|---|---|
| Linux | Build, test, clippy, smoke, Docker validation |
| Windows | Build, test |
| Perf | Benchmark gates, regression detection, Pages publishing |

Benchmark history is published to
**[ZAP Benchmarks](https://hakille-ai.github.io/ZAP/)**.

## Security model

- **Identity:** Ed25519 node keys; the node UUID is derived from the public key.
- **Signatures:** every frame is fully verified (`ZAP_SIGN` is only a fast
  pre-filter hint).
- **Transport:** ChaCha20-Poly1305 authenticated encryption over UDP; Noise
  `NN_25519_ChaChaPoly_BLAKE2s` helpers; static peer table for deterministic
  trust.
- **Replay protection:** timestamp windows, in-memory fingerprint caches, and a
  restart-persistent nonce window.
- **Consensus gating:** `REQUIRES_CONSENSUS` frames need threshold PoA
  attestations before dispatch.
- **Sandboxing:** drivers start with no host capabilities; ABI v2 host imports
  are permissioned, bounded, and audited.
- **Audit:** signed receipts in hash-chained binary journals; batch seals, MMR
  inclusion/exclusion proofs, and blinded rollup commitments for offline
  verification.

See [Security Model](docs/security.md) and report vulnerabilities through
[SECURITY.md](SECURITY.md).

## Roadmap

| Phase | Status | Focus |
|---|---|---|
| 1 — Kernel Alpha | Implemented | Wire protocol, crypto, transport, WASM, CLI |
| 2 — Typed Agent Gateway | Implemented | Strict envelopes, policy gates, agent protocol, MCP gateway |
| 3 — SDKs & Driver Registry | Implemented | Signed manifests, ZapStore, domain packs, SDK conformance |
| 4 — Proof-of-Action Network | Implemented | Multi-validator PoA, BFT consensus, gossip, mesh, receipt audit |
| 5 — Core Interfaces | Partial | Capabilities, routing, memory, fleet doctor, gateway transports |
| 6 — 1.0 Readiness | Planned | Compatibility matrix, security audit, external adoption gates |

See [Roadmap](docs/roadmap.md) and
[Implementation Status](docs/roadmap-status.md).

## Documentation

| Document | Description |
|---|---|
| [Getting Started](docs/getting-started.md) | 5-minute developer onboarding |
| [Install](docs/install.md) | Source install, CLI build, Docker quickstart |
| [Protocol](docs/protocol.md) | ZAP-Wire v1 frame format and ZENV envelope specification |
| [Tutorial](docs/tutorial.md) | End-to-end factory telemetry & control |
| [Use Cases](docs/use-cases.md) | Application scenarios for the protocol |
| [FAQ](docs/faq.md) | Design, security, and protocol comparison questions |
| [Security Model](docs/security.md) | Threat model, crypto choices, defense-in-depth |
| [Gateway](docs/gateway.md) | MCP server, HTTP/SSE/WebSocket transports, provenance chain |
| [Network](docs/network.md) | BFT consensus, gossip, adaptive mesh, durable replay |
| [Ledger](docs/ledger.md) | Journals, batch seals, MMR, blinded rollup commitments |
| [Telemetry](docs/telemetry.md) | Fleet doctor, incident snapshots, Prometheus metrics, topology |
| [Swarm](docs/swarm.md) | Cluster simulation, swarm benchmarking, provenance verification |
| [Operations](docs/operations.md) | Operator workflows: doctor, receipts, incident runbooks |
| [Observability](docs/observability.md) | Metrics contract, health signals, alerting |
| [Runtime](docs/runtime.md) | WASM sandboxing: fuel, memory, time, output, host calls |
| [ZapStore](docs/zapstore.md) | Signed manifests, registry, versioning, revocation, bundles |
| [Domain Packs](docs/domain-packs.md) | Pack layout, lifecycle, CLI, risk model |
| [PACT Profile](docs/pact.md) | Signed action records, canonical hashes, bundles, disputes |
| [Agent Protocol](docs/agent-protocol.md) | Intents, sessions, delegation, negotiation, provenance, swarm |
| [SDKs](docs/sdks.md) | SDK surface, conformance matrix, fixtures |
| [Message Policy](docs/message-policy.md) | Deterministic allow/deny/require-PoA gates |
| [Capability, Router & Memory](docs/capability-router-memory.md) | Discovery, routing, auditable memory |
| [Receipts](docs/receipts.md) | Receipt ledger, peer pull, verification, pruning |
| [Discovery](docs/discovery.md) | Dynamic service discovery |
| [Machine Connections](docs/machine-connections.md) | `zap-machine` profiles and adapters |
| [Deployment](docs/deployment.md) | Docker, compose, hardening checklist |
| [RFC/ZEP Process](docs/rfc-process.md) | Proposal process for protocol & ecosystem contracts |
| [Versioning](docs/versioning.md) | Semver and wire compatibility rules |
| [Release Process](docs/release.md) | Release checklist and publishing workflow |
| [Roadmap](docs/roadmap.md) | Phased development plan |
| [Governance](docs/governance.md) | Roles, multi-sig, audit trail, break glass |

## Contributing

Contributions are welcome when they preserve the protocol's safety boundaries.
Please review:

- [CONTRIBUTING.md](CONTRIBUTING.md) — development workflow and PR guidelines
- [GOVERNANCE.md](GOVERNANCE.md) — project governance and decision-making
- [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md) — community standards
- [docs/rfc-process.md](docs/rfc-process.md) — how protocol changes are proposed

All public changes must preserve backward compatibility with the versioned
fixtures and SDK conformance matrix.

## License

Licensed under the [Apache License, Version 2.0](LICENSE). See [NOTICE](NOTICE)
for attributions.