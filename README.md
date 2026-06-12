<p align="center">
  <strong>⚡ ZAP</strong><br>
  <em>Universal Low-Latency Protocol for Typed Message Dispatch</em>
</p>

<p align="center">
  <a href="https://github.com/Hakille-ai/ZAP/actions"><img src="https://img.shields.io/github/actions/workflow/status/Hakille-ai/ZAP/ci.yml?branch=main&label=CI&style=flat-square" alt="CI"></a>
  <a href="https://github.com/Hakille-ai/ZAP/actions/workflows/perf.yml"><img src="https://img.shields.io/github/actions/workflow/status/Hakille-ai/ZAP/perf.yml?branch=main&label=Bench&style=flat-square" alt="Bench"></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/License-Apache_2.0-blue?style=flat-square" alt="License"></a>
  <img src="https://img.shields.io/badge/Rust-1.93+-orange?style=flat-square&logo=rust" alt="Rust 1.93+">
  <img src="https://img.shields.io/badge/Status-Pre--1.0_Alpha-yellow?style=flat-square" alt="Status">
</p>

---

ZAP is a compact, signed, encrypted, low-latency protocol implemented in Rust
for moving **typed messages** between nodes. While actions are a primary use
case, ZAP is not limited to action dispatch — it carries data, events, commands,
queries, responses, stream chunks, actions, and control messages through a
unified wire format.

ZAP is **protocol infrastructure**. It is independent of AI models, LLM
providers, and application runtimes. Receipts and Proof-of-Action support
auditability; they are not billing, settlement, rewards, or financial rails.

## ❓ Why ZAP?

1. **End-to-End Cryptographic Provenance**: Every message frame is signed by the sender node's identity and verified by the receiver, establishing full custody and identity tracking for distributed processes.
2. **Deterministic & Local Action Planning**: Natural language intents are parsed, checked against safety rules, and compiled into concrete actions entirely on-device, without relying on central LLMs.
3. **Consensus-Gated Operations**: High-risk actions (e.g. hardware control or factory safety systems) can require multi-node Proof-of-Action consensus (ZPOA) before dispatch.
4. **Sandboxed Edge Execution**: Execute untrusted custom device drivers inside a Wasmtime sandbox with strict instruction (fuel), memory, time, and permission boundaries.
5. **Durable Auditable Ledgers**: Nodes maintain append-only, BLAKE3 hash-chained memory stores and signed receipt logs, providing verifiable, tamper-evident audit trails.

## ✨ Key Features

| Feature | Description |
|---|---|
| **64-byte Wire Header** | Fixed-size `ZAP_` frame header with auth and PoA trailers — zero-copy parseable |
| **Universal Envelopes** | `ZENV` payload format with kind, subject, content type, metadata, and body |
| **Ed25519 Signatures** | Every frame is cryptographically signed and verified end-to-end |
| **Encrypted Transport** | ChaCha20-Poly1305 authenticated encryption over UDP with Noise helpers |
| **Replay Protection** | Nonce tracking and frame-level replay checks enabled by default |
| **WASM Sandboxing** | Wasmtime-based driver execution with fuel, memory, time, and output limits |
| **Signed Manifests** | Drivers are verified against SHA-256 hashes and Ed25519 author signatures |
| **Capability System** | Explicit capability advertisements, queries, grants, and policy enforcement |
| **Deterministic Routing** | Explainable route planning before local dispatch or peer forwarding |
| **Auditable Memory** | Append-only JSONL memory with body hashes, hash chains, and tombstones |
| **Proof-of-Action** | Multi-validator consensus for critical operations with configurable thresholds |
| **Intent Compiler** | Deterministic local compilation of natural-language intents to typed actions |
| **Receipt Ledger** | Signed, verifiable, prunable receipt logs for full operational audit trails |
| **Driver Registry** | Local ZapStore index with versioning, revocation, and operator signatures |

## 🏗 Architecture

```
┌─────────────────────────────────────────────────────────────────────┐
│                          ZAP Node                                   │
│                                                                     │
│  ┌──────────┐  ┌──────────┐  ┌───────────┐  ┌───────────────────┐  │
│  │  Intent   │  │ Capabil- │  │  Router   │  │     Memory        │  │
│  │ Compiler  │  │   ity    │  │           │  │  (JSONL + Hash)   │  │
│  └─────┬─────┘  └────┬─────┘  └─────┬─────┘  └───────────────────┘  │
│        │              │              │                                │
│  ┌─────▼──────────────▼──────────────▼──────┐                        │
│  │              Node Daemon                  │                        │
│  │  ┌────────┐ ┌─────────┐ ┌──────────────┐ │                        │
│  │  │Dispatch│ │Receipts │ │  PoA Verify  │ │                        │
│  │  └───┬────┘ └─────────┘ └──────────────┘ │                        │
│  └──────┼────────────────────────────────────┘                        │
│         │                                                             │
│  ┌──────▼───────┐    ┌───────────────┐    ┌─────────────────────┐    │
│  │   Runtime    │    │   ZapStore    │    │     Transport       │    │
│  │  (Wasmtime)  │    │  (Manifests)  │    │  (Encrypted UDP)    │    │
│  │  ┌─────────┐ │    │  ┌─────────┐  │    │  ┌──────────────┐  │    │
│  │  │ Driver  │ │    │  │Manifest │  │    │  │ChaCha20+Noise│  │    │
│  │  │  WASM   │ │    │  │Registry │  │    │  │  Peer Table  │  │    │
│  │  └─────────┘ │    │  └─────────┘  │    │  │Replay Protect│  │    │
│  └──────────────┘    └───────────────┘    │  └──────────────┘  │    │
│                                           └─────────────────────┘    │
│                                                                      │
│  ┌─────────────────────────────────────────────────────────────┐     │
│  │                    ZAP Wire Protocol                         │     │
│  │  ┌────────────┐  ┌──────────┐  ┌────────────┐  ┌─────────┐ │     │
│  │  │ ZAP_ Frame │  │   ZENV   │  │ Auth Trail │  │  PoA    │ │     │
│  │  │  (64 B)    │  │ Envelope │  │  (Ed25519) │  │ Trailer │ │     │
│  │  └────────────┘  └──────────┘  └────────────┘  └─────────┘ │     │
│  └─────────────────────────────────────────────────────────────┘     │
└──────────────────────────────────────────────────────────────────────┘
```

## 📦 Workspace Crates

ZAP is organized as a Rust workspace of 14 focused crates (~15,600 lines of Rust):

### Core Protocol

| Crate | Description |
|---|---|
| `zap-core` | ZAP-Wire v1 frame parsing/encoding: fixed 64-byte `ZAP_` header, bitflags, auth trailers, PoA trailers |
| `zap-envelope` | Universal `ZENV` payload envelopes — kind, subject, content type, metadata, and body |
| `zap-crypto` | Node identity (Ed25519), key generation, full-frame signing, verification, and PoA certificates |
| `zap-net` | Encrypted UDP endpoint, static peer table, ChaCha20-Poly1305 encryption, Noise helpers, nonce replay checks |

### Execution & Dispatch

| Crate | Description |
|---|---|
| `zap-node` | Daemon core: TOML config, peer verification, replay protection, receipts, capability-aware dispatch, routing |
| `zap-runtime` | Wasmtime sandboxed execution: ABI verification, fuel metering, memory limits, time bounds, output caps |
| `zap-driver-sdk` | Minimal ABI helpers for WASM driver authors |
| `zap-cli` | Operator CLI: `keygen`, `run`, `send`, `inspect`, `doctor`, `compile-intent`, `registry`, `capability`, `route`, `memory`, `receipts`, `poa` |

### Intelligence & Policy

| Crate | Description |
|---|---|
| `zap-intent` | Deterministic local intent compiler — maps natural language to typed ZAP action steps with policy gates |
| `zap-capability` | Capability identifiers, driver permission contracts, local advertisements, signed query/response |
| `zap-router` | Deterministic route tables, explainable route decisions, peer grant requirements |

### Audit & Storage

| Crate | Description |
|---|---|
| `zap-ledger` | Signed receipt records for action audit trails |
| `zap-memory` | Append-only JSONL memory: body hashes, entry hash chains, tombstones, pruning, verification |
| `zap-store` | ZapStore driver manifests: SHA-256 hashes, Ed25519 signatures, local registry with versioning and revocation |

## 🚀 Quickstart

### Prerequisites

- **Rust 1.93+** (edition 2024) — install via [rustup](https://rustup.rs)
- **Docker** (optional) — for containerized deployment

### Build & Test

```bash
# Run the full test suite
cargo test --workspace --all-targets

# Generate a node identity key
cargo run -p zap-cli -- keygen --out .zap/node.key

# Compile a natural-language intent into typed actions
cargo run -p zap-cli -- compile-intent "Ajuster la température à 20" --explain

# Run a quick parse benchmark (100k iterations)
cargo run -p zap-cli -- bench parse --iterations 100000
```

> **Note:** `zap keygen` refuses to overwrite an existing key unless `--force`
> is provided.

### Two-Node Local Demo

1. **Generate keys** for each node
2. **Configure peers** — copy `node_id`, `public_key`, and a shared 32-byte
   `transport_key` into TOML configs based on
   [`node-a.toml`](examples/configs/node-a.toml) and
   [`node-b.toml`](examples/configs/node-b.toml)
3. **Validate and run:**

```bash
# Validate configs
cargo run -p zap-cli -- check-config --config examples/configs/node-a.toml
cargo run -p zap-cli -- check-config --config examples/configs/node-b.toml

# Start node A
cargo run -p zap-cli -- run --config examples/configs/node-a.toml

# From another terminal — send an action
cargo run -p zap-cli -- send --config examples/configs/node-b.toml \
  --target <node-a-uuid> --action echo --payload hello

# Send a universal event envelope
cargo run -p zap-cli -- send --config examples/configs/node-b.toml \
  --target <node-a-uuid> --kind event --subject sensor.temperature \
  --payload '{"c":21.5}' --content-type application/json
```

> `zap send` binds to the `bind` address in its config so the receiver can
> enforce static peer addresses. Do not run `zap run` and `zap send` from the
> same config simultaneously.

## 💡 Programmatic Examples

ZAP provides compile-ready Rust code examples under the `examples/` directory. You can build and run them via cargo:

```bash
# Run ZAP binary frame creation, signing, and verification example
cargo run -p zap-examples --bin frame_basics

# Run ZENV Universal Payload Envelope construction and causal linking
cargo run -p zap-examples --bin envelope_types

# Run natural language intent parsing and rule-based safety policy checks
cargo run -p zap-examples --bin intent_compiler

# Run append-only JSONL memory store and BLAKE3 hash chain audits
cargo run -p zap-examples --bin memory_store

# Run driver manifest creation, signing, and local registry revocation
cargo run -p zap-examples --bin driver_manifest
```

Refer to the source files in `examples/src/bin/` to see how to import and use the APIs in your own projects.

## 🐳 Docker Deployment

### Build

```bash
docker build -t zap:local .
```

### Run with Compose

```bash
mkdir -p .zap/container
docker compose run --rm node keygen --out /var/lib/zap/node.key
docker compose up --build
```

The container runs as a **non-root user** (UID 10001), exposes **UDP 7000**,
uses a **read-only root filesystem**, drops all capabilities, limits PIDs to
128, and stores node state under `/var/lib/zap`.

See [Deployment](docs/deployment.md) for production hardening notes.

## 🛠 CLI Reference

### Configuration & Diagnostics

```bash
zap doctor --config zap.toml                          # Readiness gate with scoring
zap doctor --config zap.toml --json --strict          # Machine-readable strict mode
zap check-config --strict --config zap.toml           # Validate config
```

### Sending Messages

```bash
# Action envelope → WASM driver
zap send --config zap.toml --target <uuid> --action echo --payload hello

# Binary payload from file
zap send --config zap.toml --target <uuid> --action upload \
  --payload-file payload.bin --binary-payload

# Event envelope with metadata
zap send --config zap.toml --target <uuid> --kind event \
  --subject sensor.temperature --payload '{"c":21.5}' \
  --content-type application/json --metadata '{"source":"sim"}'

# Intent-based sending with policy
zap send --config zap.toml --target <uuid> \
  --intent "Ajuster la température à 20" --policy policy.json
```

### Driver Manifests & Registry

```bash
# Create and verify a signed manifest
zap driver-manifest create --driver echo.wat --action echo \
  --author-key .zap/node.key --out echo.manifest.toml
zap driver-manifest verify --driver echo.wat --manifest echo.manifest.toml

# Manage a local registry
zap registry init --out registry.index.toml
zap registry add --registry registry.index.toml --manifest echo.manifest.toml
zap registry verify --registry registry.index.toml --manifest echo.manifest.toml
zap registry revoke --registry registry.index.toml \
  --action echo --version 0.1.0 --reason "bad release"
zap registry sign --registry registry.index.toml --operator-key .zap/node.key
zap registry verify-signature --registry registry.index.toml
```

### Capabilities, Routing & Memory

```bash
# Capability discovery
zap capability list --config zap.toml --json
zap capability query --config zap.toml --target <uuid> \
  --cache .zap/capabilities.jsonl --json
zap capability cache verify --path .zap/capabilities.jsonl

# Route planning
zap route explain --config zap.toml --kind action --subject echo --json

# Local memory store
zap memory put --path .zap/memory.jsonl --subject note --payload hello
zap memory verify --path .zap/memory.jsonl
```

### Proof-of-Action & Receipts

```bash
# PoA workflow
zap send --config zap.toml --target <uuid> \
  --intent "déclencher arrêt urgence robot" --poa-network --poa-timeout-ms 5000
zap poa request --frame critical-frame.bin \
  --requester-key .zap/node.key --threshold 1 > poa-request.json
zap poa attest --request poa-request.json \
  --validator-key .zap/validator.key > poa-response.json

# Receipt audit
zap receipts verify --path logs/actions.jsonl
zap receipts prune --path logs/actions.jsonl \
  --before-processed-at-micros 1735689600000000 --out logs/retained.jsonl
zap receipts merge logs/node-a.jsonl logs/node-b.jsonl \
  --out logs/receipts.archive.jsonl

# Frame inspection
zap inspect frame.bin --verify-with-public-key <base64-public-key>
```

## 🔒 Security Model

ZAP is designed with **defense-in-depth** from the ground up:

- **Ed25519 identity** — every node has a unique cryptographic identity
- **Full-frame signatures** — `ZAP_SIGN` is an 8-byte optimization hint; full
  verification is always enforced
- **Authenticated encryption** — ChaCha20-Poly1305 over UDP datagrams
- **Replay protection** — nonce tracking and frame-level replay checks enabled
  by default
- **WASM sandboxing** — drivers have no host capabilities; future APIs will
  require explicit grants
- **Capability model** — discovered capabilities are descriptive only and do not
  grant authority
- **Hash-chain memory** — local memory records are verifiable JSONL audit data,
  not hidden model state
- **Consensus gating** — frames marked `REQUIRES_CONSENSUS` require PoA
  certificates before dispatch

> Please report vulnerabilities privately. See [SECURITY.md](SECURITY.md) and
> the [Security Model](docs/security.md) documentation.

## ⚙️ Development

### Required CI Checks

```bash
cargo ci-fmt           # Format check
cargo ci-test          # Full workspace test suite
cargo ci-smoke         # End-to-end smoke: launches node, sends action, verifies receipt
cargo ci-bench-smoke   # Compile and run benchmarks in test mode
cargo ci-clippy        # Lint with -D warnings
```

These aliases are defined in [`.cargo/config.toml`](.cargo/config.toml) and
mirror the [GitHub Actions workflows](.github/workflows/).

### Benchmarks

```bash
cargo ci-bench-full                                                    # Full Criterion run
cargo ci-bench-compare --base target/bench-results/base.json \
  --head target/bench-results/head.json                                # Regression check
```

Pull requests compare base and head commits on the same runner and fail when
critical regressions exceed the thresholds in
[`bench-thresholds.toml`](tools/bench-thresholds.toml). Pushes to `main`
publish benchmark history to GitHub Pages:
**[ZAP Benchmarks](https://hakille-ai.github.io/ZAP/)**.

### CI Matrix

| Platform | Checks |
|---|---|
| **Linux** | Build, test, clippy, smoke, Docker validation |
| **Windows** | Build, test |
| **Perf** | Benchmark gates, regression detection, Pages publishing |

## 📋 Project Status

ZAP is **pre-1.0** and under active development. The codebase is structured
like a production system from the start:

- ✅ Strict binary parsing with property-based tests
- ✅ Cryptographic verification on every frame
- ✅ Encrypted transport with replay protection
- ✅ Sandboxed WASM execution with resource limits
- ✅ CLI tooling for all operator workflows
- ✅ Comprehensive test suite and benchmark harnesses
- ✅ Docker packaging with security hardening
- ✅ Full operator and protocol documentation

Compatibility is taken seriously even before 1.0. See
[Versioning](docs/versioning.md) before changing public APIs, CLI behavior, or
wire formats.

## 🗺 Roadmap

| Phase | Status | Focus |
|---|---|---|
| **1 — Kernel Alpha** | ✅ Implemented | Wire protocol, crypto, transport, WASM, CLI |
| **2 — Cognitive Interpreter** | ✅ Foundation | Intent compiler, policy gates, explain mode |
| **3 — SDKs & Driver Registry** | ✅ Foundation | Signed manifests, ZapStore, revocation |
| **4 — Proof-of-Action Network** | ✅ Foundation | Multi-validator PoA, receipts, audit |
| **5 — Future Core Interfaces** | ✅ Foundation | Capabilities, routing, memory, doctor |

See the full [Roadmap](docs/roadmap.md) for detailed status and next steps.

## 📚 Documentation

| Document | Description |
|---|---|
| [Protocol](docs/protocol.md) | ZAP-Wire v1 frame format and ZENV envelope specification |
| [Security Model](docs/security.md) | Threat model, crypto choices, and defense-in-depth design |
| [Use Cases](docs/use-cases.md) | Real-world application scenarios for the ZAP protocol |
| [Getting Started](docs/getting-started.md) | Step-by-step developer onboarding and cluster setup |
| [End-to-End Tutorial](docs/tutorial.md) | Full guide detailing WASM drivers, intents, policies, and Proof-of-Action |
| [FAQ](docs/faq.md) | Frequently asked questions about design, security, and protocol comparisons |
| [Deployment](docs/deployment.md) | Production configuration, Docker, and hardening guide |
| [Operations](docs/operations.md) | Operator workflows: doctor, receipts, monitoring |
| [Runtime](docs/runtime.md) | WASM sandboxing: fuel, memory, time, and output limits |
| [ZapStore](docs/zapstore.md) | Signed manifests, registry, versioning, and revocation |
| [Capability, Router & Memory](docs/capability-router-memory.md) | Discovery, routing, and auditable memory |
| [Intent Compiler](docs/intent.md) | Natural-language intent to typed action compilation |
| [Signed Receipts](docs/receipts.md) | Receipt ledger, verification, pruning, and merging |
| [Versioning](docs/versioning.md) | Semantic versioning and wire compatibility rules |
| [Release Process](docs/release.md) | Release checklist and publishing workflow |
| [Roadmap](docs/roadmap.md) | Phased development plan and current status |
| [PDF Requirements Trace](docs/pdf-requirements.md) | Requirements traceability matrix |

## 🤝 Contributing

Contributions are welcome when they preserve the protocol's safety boundaries.
Please review these documents before submitting:

- [CONTRIBUTING.md](CONTRIBUTING.md) — development workflow and PR guidelines
- [GOVERNANCE.md](GOVERNANCE.md) — project governance and decision-making
- [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md) — community standards

## 📄 License

Licensed under the [Apache License, Version 2.0](LICENSE). See [NOTICE](NOTICE)
for attributions.

---

<p align="center">
  <sub>Built with 🦀 Rust · Made by <a href="https://github.com/Hakille-ai">Hakille AI</a></sub>
</p>
