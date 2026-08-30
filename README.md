# Rivun

**Universal low-latency protocol for typed message dispatch, signed by default.**

Rivun is a compact, signed, encrypted, low-latency messaging protocol implemented
in Rust. It moves **typed messages** between nodes — data, events, commands,
queries, responses, stream chunks, actions, and control messages — through a
unified wire format with end-to-end cryptographic provenance.

Rivun is a protocol, not a runtime: it is agnostic to AI models, application
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

## When to use Rivun

- You need typed messages with cryptographic provenance between nodes.
- Actions must pass deterministic policy before execution.
- High-risk operations need Proof-of-Action, simulation, explicit grants, or
  multi-party approval.
- Untrusted extensions must run in a resource-bounded sandbox.
- Operators need signed receipts, replay protection, and audit evidence.
- Gateways and SDKs must agree on stable protocol fixtures across languages.

## When not to use Rivun

- You only need a generic message broker, queue, or RPC framework (use MQTT,
  NATS, or gRPC).
- You need a database, hidden model memory store, financial ledger, or payment
  rail — Rivun is explicitly not one of these.
- An integration would bypass identity, policy, grants, PoA, or receipts for
  convenience.
- You cannot tolerate pre-1.0 API and CLI evolution.

## Repository layout

```
apps/
  rivun-dashboard/    Next.js 16 / React 19 / Tailwind dark-mode enterprise SaaS UI
  rivun-control/      Local operator workstation & Ed25519 secure key vault (Tauri/CLI)
crates/               Workspace crates (26) — protocol, node, runtime, cloud bridge, cloud API
docs/                 Protocol, security, operations, and cloud architecture guides
examples/             Runnable examples, configs, domain packs, WASM drivers
fixtures/             Versioned JSON protocol fixtures shared by SDKs and tests
sdks/                 Rust, Go, TypeScript, Python SDKs
tests/e2e/            Opaque-box 4-tier end-to-end suite (174 tests)
tools/                xtask and benchmark tooling
website/              Marketing and docs site (Next.js)
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
cargo run -p Rivun-cli -- keygen --out .Rivun/node.key

# Send a typed action envelope (needs a configured peer, see below)
cargo run -p Rivun-cli -- send --config Rivun.toml --target <uuid> --action echo --payload hello

# Run a quick parse benchmark
cargo run -p Rivun-cli -- bench parse --iterations 100000
```

### Two-node demo

1. Generate keys for both nodes: `Rivun keygen --out .Rivun/node-a.key` and
   `Rivun keygen --out .Rivun/node-b.key`.
2. Copy `node_id` and `public_key` into
   [`examples/configs/node-a.toml`](examples/configs/node-a.toml) and
   [`examples/configs/node-b.toml`](examples/configs/node-b.toml), and set a
   shared 32-byte `transport_key` in both files.
3. Validate and run:

```bash
cargo run -p Rivun-cli -- check-config --config examples/configs/node-a.toml
cargo run -p Rivun-cli -- check-config --config examples/configs/node-b.toml

# Terminal 1
cargo run -p Rivun-cli -- run --config examples/configs/node-a.toml

# Terminal 2 — send an action and a typed event
cargo run -p Rivun-cli -- send --config examples/configs/node-b.toml \
  --target <node-a-uuid> --action echo --payload hello
cargo run -p Rivun-cli -- send --config examples/configs/node-b.toml \
  --target <node-a-uuid> --kind event --subject sensor.temperature \
  --payload '{"c":21.5}' --content-type application/json
```

> `Rivun send` binds to the `bind` address in its config so the receiver can
> enforce static peer addresses. Do not run `Rivun run` and `Rivun send` from the
> same config simultaneously.

### Programmatic examples

```bash
cargo run -p Rivun-examples --bin frame_basics      # frame creation, signing, verification
cargo run -p Rivun-examples --bin envelope_types    # ZENV envelopes and causal linking
cargo run -p Rivun-examples --bin memory_store      # append-only journal memory + audit
cargo run -p Rivun-examples --bin driver_manifest   # signed manifests + registry revocation
```

## Architecture

```
┌────────────────────────────────────────────────────────────────────────────┐
│                              Rivun Node (Rivun-node)                            │
│  policy │ capability │ router │ memory │ receipts │ observability          │
│  ───────────────────────────────┐                                          │
│         Node daemon             │   actors: udp_rx, gossip, mesh,           │
│  dispatch · receipts · PoA      │   consensus, execution                    │
│  ───────────────────────────────┘                                          │
│         │          │               │               │                       │
│  ┌──────▼───┐ ┌────▼─────┐ ┌───────▼───────┐ ┌─────▼──────────┐            │
│  │ Runtime  │ │ RivunStore │ │  Transport    │ │   Ledger       │            │
│  │ Wasmtime │ │ driver & │ │  (Rivun-net)    │ │   (Rivun-ledger) │            │
│  │ fuel/mem │ │ pack     │ │  ChaCha20     │ │   receipts     │            │
│  │ time/out │ │ registry │ │  Noise ·      │ │   MMR · batch  │            │
│  │ async    │ │ signed   │ │  replay       │ │   blinded      │            │
│  │ pipeline │ │ bundles  │ │  BFT ·        │ │   rollup cmts  │            │
│  │          │ │          │ │  gossip ·     │ │                │            │
│  │          │ │          │ │  mesh · PEX   │ │                │            │
│  └──────────┘ └──────────┘ └───────────────┘ └────────────────┘            │
│                                                                            │
│  ┌──────────────────────────────────────────────────────────────────────┐  │
│  │  Gateway (Rivun-gateway)    MCP stdio · HTTP REST · SSE · WebSocket     │  │
│  │                          provenance chain: intent → … → receipt       │  │
│  └──────────────────────────────────────────────────────────────────────┘  │
│                                                                            │
│  ┌─────────────────────────────────────────┐   ┌────────────────────────┐  │
│  │  Wire format (Rivun-core / Rivun-envelope)  │   │  Fleet (Rivun-telemetry) │  │
│  │  @@rivun_HEADER@@ header │ ZENV │ ZSIG │ ZPOA       │   │  doctor · metrics      │  │
│  └─────────────────────────────────────────┘   │  incident · topology   │  │
│                                                └────────────────────────┘  │
└────────────────────────────────────────────────────────────────────────────┘
```

### Workspace crates

| Crate | Responsibility |
|---|---|
| **Core protocol** | |
| `Rivun-core` | Rivun-Wire v1 frame parsing/encoding: 64-byte `@@rivun_HEADER@@` header, flags, auth and PoA trailers |
| `Rivun-envelope` | Universal `ZENV` payload envelopes: kind, subject, content type, metadata, body |
| `Rivun-crypto` | Node identity (Ed25519), key generation, frame signing/verification, PoA certificates |
| `Rivun-schema` | Typed message contracts for agent gateways and machine commands |
| `Rivun-pact` | PACT profile: signed action records, canonical hashes, revocations, bundles, dispute engine |
| **Node & execution** | |
| `Rivun-node` | Daemon core: config, peer verification, replay protection, receipts, capability-aware dispatch, actors |
| `Rivun-runtime` | Wasmtime sandboxed execution: ABI verification, fuel/memory/time/output limits, async pipeline, streaming |
| `Rivun-driver-sdk` | ABI helpers for WASM driver authors: async drivers, ring buffers, zero-copy IPC |
| `Rivun-machine` | Machine connections and profile contracts for industrial adapters |
| `Rivun-cli` | Operator CLI: 29 commands covering every workflow below |
| **Network** | |
| `Rivun-net` | Encrypted UDP transport, Noise handshake, durable anti-replay, BFT consensus, gossip, adaptive mesh |
| **Intelligence & policy** | |
| `Rivun-capability` | Capability ids, driver permission contracts, signed query/response |
| `Rivun-policy` | Deterministic policy decisions: allow/deny/PoA/grant/human/simulation |
| `Rivun-router` | Deterministic route tables and explainable route decisions |
| `Rivun-agent` | Agent protocol contracts: intents, sessions, delegation, negotiation, provenance chain, swarm |
| **Audit & storage** | |
| `Rivun-journal` | Append-only binary journal segments: hash chaining, sealing, indexes, crash recovery |
| `Rivun-ledger` | Signed action receipts, batch seals, incremental MMR, blinded rollup commitments |
| `Rivun-memory` | Append-only binary journal memory: body hashes, entries, tombstones, compaction |
| `Rivun-store` | Driver manifests, registry index, migrations, publications, install plans, bundles |
| `Rivun-pack` | Domain pack lifecycle: build, sign, verify, install, audit (shared with `Rivun-store`) |
| **Gateway & operations** | |
| `Rivun-gateway` | AI agent gateway: MCP server, HTTP REST, SSE, WebSocket, provenance chain engine |
| `Rivun-telemetry` | Fleet doctor, incident snapshots, Prometheus metrics, topology |
| `Rivun-ops` | Operations contracts: observability, governance, production configs |
| **Cloud & SaaS control plane** | |
| `rivun-cloud-bridge` | Edge daemon: telemetry/receipt ingestion, policy polling, Ed25519 signature checks, atomic swap |
| `rivun-cloud-api` | Multi-tenant Axum SaaS backend: REST API, Server-Sent Events (SSE), RBAC, pack registry |
| `rivun-control` | Local operator workstation & Ed25519 key vault: offline signing for staged policy bundles |

## Rivun Cloud & Operator Workstation

Rivun Cloud extends the low-level protocol into an enterprise multi-tenant control plane under a strict **Zero-Trust Sovereign Invariant**:

- **No Private Key Ingestion**: Private Ed25519 signing keys never leave local operator workstations (`Rivun Control`).
- **Cryptographic Staging**: Policies are staged in the Cloud, inspected via side-by-side visual diffs, and signed with `Rivun Control` before edge nodes apply them via atomic filesystem rename.
- **Enterprise Dark UI**: Next.js 16 / React 19 dashboard with live SSE streaming, 7-point Doctor diagnostic badges, interactive 7-stage causal provenance graphs ($H_{\text{intent}} \to \dots \to H_{\text{root}}$), and air-gapped CLI verification modals.

See [Rivun Cloud Architecture Guide](docs/cloud.md) for complete details.

Full command reference for a node:

```bash
Rivun keygen            # generate a node identity key
Rivun run               # start the daemon
Rivun check-config      # validate a config
Rivun doctor            # operator readiness gate
Rivun fleet doctor      # multi-node health aggregation
Rivun send              # send a message or action to a peer
Rivun inspect           # decode a frame file
```

Identity & peer trust:

```bash
Rivun trust enroll / inspect
Rivun peer invite / accept / rotate / revoke
```

Protocol, policy & routing:

```bash
Rivun capability list / query / cache / inspect-manifest
Rivun discovery announce / query
Rivun route explain
Rivun policy evaluate
Rivun schema validate / inspect / export
```

Agents, PACT, packs & fixtures:

```bash
Rivun agent session / intent / status / result / delegate / negotiate / validate / schema
Rivun pact create / sign / verify / revoke / bundle / schema
Rivun pack init / build / sign / verify / install / audit / validate / inspect / list
Rivun fixtures verify --fixtures fixtures --sdk <sdk-path>
```

Drivers & registry:

```bash
Rivun driver-manifest create / verify
Rivun registry init / add / sign / verify-signature / resolve / pull / mirror
Rivun registry publication create / verify
Rivun registry plan create / verify
Rivun registry bundle export / pull-manifest / verify / import
Rivun registry revoke / deprecate / migration add / list
```

Audit & evidence:

```bash
Rivun receipts pull / verify / import-jsonl / export-jsonl / compact
Rivun memory put / get / query / tombstone / verify / prune / compact
Rivun memory import-jsonl / export-jsonl / export-evidence
Rivun incident snapshot
Rivun provenance verify
```

Consensus:

```bash
Rivun poa request / attest / validator-set create / verify / pull / apply
```

Gateway & simulation:

```bash
Rivun gateway start / status      # MCP stdio, HTTP REST, SSE, WebSocket
Rivun cluster up / status         # in-memory N-node cluster simulation
Rivun swarm bench / partition-test
Rivun bench parse
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
Rivun fixtures verify --fixtures fixtures --sdk sdks/rust --json
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
**[Rivun Benchmarks](https://hakille-ai.github.io/Rivun/)**.

## Security model

- **Identity:** Ed25519 node keys; the node UUID is derived from the public key.
- **Signatures:** every frame is fully verified (`@@rivun_HEADER@@SIGN` is only a fast
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
| 3 — SDKs & Driver Registry | Implemented | Signed manifests, RivunStore, domain packs, SDK conformance |
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
| [Protocol](docs/protocol.md) | Rivun-Wire v1 frame format and ZENV envelope specification |
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
| [RivunStore](docs/RivunStore.md) | Signed manifests, registry, versioning, revocation, bundles |
| [Domain Packs](docs/domain-packs.md) | Pack layout, lifecycle, CLI, risk model |
| [PACT Profile](docs/pact.md) | Signed action records, canonical hashes, bundles, disputes |
| [Agent Protocol](docs/agent-protocol.md) | Intents, sessions, delegation, negotiation, provenance, swarm |
| [SDKs](docs/sdks.md) | SDK surface, conformance matrix, fixtures |
| [Message Policy](docs/message-policy.md) | Deterministic allow/deny/require-PoA gates |
| [Capability, Router & Memory](docs/capability-router-memory.md) | Discovery, routing, auditable memory |
| [Receipts](docs/receipts.md) | Receipt ledger, peer pull, verification, pruning |
| [Discovery](docs/discovery.md) | Dynamic service discovery |
| [Machine Connections](docs/machine-connections.md) | `Rivun-machine` profiles and adapters |
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
