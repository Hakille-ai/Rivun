# Rivun Developer Documentation Portal (`apps/docs-portal`) — Architectural Survey & Implementation Blueprint

**Document Version**: 1.0.0  
**Author**: Docs Portal Explorer  
**Target Repository**: `c:\Users\Stagiaire\Documents\Amadou PGC\Prs\Rivun`  
**Target Application**: `apps/docs-portal`  
**Status**: Ready for Implementation  

---

## 1. Executive Summary & Scope Definition

The **Rivun Developer Documentation Portal** (`apps/docs-portal`) is the definitive, production-grade technical knowledge platform for the Rivun ecosystem (the ZAP universal protocol). It serves software engineers, systems architects, security auditors, operators, and domain pack developers by delivering an exhaustive, Apple-grade, interactive documentation experience.

### Core Objectives
1. **Zero-Friction Discovery**: Instant client-side full-text search (<10ms latency) with `Cmd+K` / `Ctrl+K` keyboard orchestration, fuzzy search, categorized filters, and highlighted code matches.
2. **Apple-Grade Dark Aesthetic**: Refined dark glassmorphism styling, subtle micro-interactions, clean typography (Geist / SF Pro), responsive multi-level sidebar, dynamic breadcrumbs, and floating scroll-spy Table of Contents.
3. **Exhaustive Technical Coverage (A to Z)**: Complete, uncompromised documentation spanning all 26 workspace crates, 4 official SDKs (Rust, TypeScript, Python, Go), 7 Domain Packs, the core wire format (`@@rivun_HEADER@@`), Proof-of-Action consensus, WASM runtime sandbox, Rivun Cloud multi-tenant control plane, and the 7-Point Fleet Doctor diagnostics.
4. **Multi-Language Developer Experience**: Copyable multi-language code tabs (Rust, TypeScript, Python, Go, CLI) with unified syntax highlighting and clipboard feedback.
5. **Interactive Sandboxes & Visualizers**: Live in-browser wire frame encoder/decoder, Proof-of-Action quorum simulator ($T \le N$), PACT canonicalizer, and interactive REST API explorer.
6. **Zero Build Warnings/Errors**: Strict TypeScript type checking, static route validation, 0 broken internal links, and high performance.

---

## 2. Technology Stack & Architecture

### Recommended Technology Stack
Aligned with `apps/rivun-dashboard` and modern web standards:

| Layer | Technology | Rationale |
|---|---|---|
| **Framework** | Next.js 15.1+ (App Router) | High-performance Static Site Generation (SSG), dynamic nested layouts, SEO metadata, Server Components where applicable. |
| **Runtime / Library** | React 19 + TypeScript 5.7+ | Strict type-safety, React 19 actions/hooks, zero runtime type errors. |
| **Styling** | Tailwind CSS 3.4+ / 4 + PostCSS | Dark-mode glassmorphism tokens, Apple-inspired color palettes, fine-grained responsive utilities. |
| **Icons** | `lucide-react` (0.475+) | Clean, consistent, lightweight SVG iconography. |
| **Search Engine** | Custom Inverted Index + FlexSearch / Fuse.js | High-speed client-side full-text search without external cloud dependencies. |
| **Diagrams** | Mermaid.js / SVG Engine | In-browser client-side rendering of architecture flows, sequence diagrams, and state machines. |
| **Math / Formulas** | KaTeX (`rehype-katex` / `katex`) | Fast, pristine mathematical rendering for MMR $O(\log N)$ equations, BLAKE3 hashes, and BFT quorum bounds. |
| **Syntax Highlighting** | Prism.js / Shiki / custom highlighted code engine | Multi-language syntax highlighting for Rust, TypeScript, Python, Go, TOML, JSON, Bash, WAT. |

### Application Directory Structure (`apps/docs-portal/`)
```text
apps/docs-portal/
├── app/
│   ├── layout.tsx                     # Root HTML layout, ThemeProvider, SearchModal provider
│   ├── page.tsx                       # Documentation Portal Home & Quick-Navigation Hub
│   ├── globals.css                    # Tailwind directives, dark theme tokens, glassmorphism CSS
│   ├── docs/
│   │   ├── layout.tsx                 # Documentation layout with Sidebar, Header, Breadcrumbs, TOC
│   │   ├── [...slug]/
│   │   │   └── page.tsx               # Dynamic documentation page renderer with SSG params
│   ├── sandbox/
│   │   ├── page.tsx                   # Interactive Wire Frame & Protocol Sandbox
│   │   └── poa-quorum/
│   │       └── page.tsx               # Interactive Proof-of-Action Quorum Calculator
│   ├── api-explorer/
│   │   └── page.tsx                   # Interactive Rivun Cloud REST & SSE API Explorer
│   └── search-index/
│       └── route.ts                   # Search index JSON API endpoint
├── components/
│   ├── layout/
│   │   ├── Header.tsx                 # Top navigation bar with search trigger, version picker, links
│   │   ├── Sidebar.tsx                # Multi-level hierarchical collapsible sidebar
│   │   ├── Breadcrumbs.tsx            # Dynamic breadcrumbs with JSON-LD schema
│   │   ├── TableOfContents.tsx        # Scroll-spy right-hand heading outline
│   │   └── Footer.tsx                 # Documentation footer & community links
│   ├── ui/
│   │   ├── CodeTabs.tsx               # Multi-language code tab switcher (Rust/TS/Py/Go/CLI)
│   │   ├── CodeBlock.tsx              # Syntax highlighted code block with copy button
│   │   ├── Callout.tsx                # Admonition box (NOTE, TIP, WARNING, SECURITY, INVARIANT)
│   │   ├── Mermaid.tsx                # Client-side Mermaid diagram renderer
│   │   ├── MathFormula.tsx            # KaTeX mathematical equation renderer
│   │   ├── SearchModal.tsx            # Cmd+K Full-Text Search Modal
│   │   ├── Badge.tsx                  # Version, risk level, and protocol badges
│   │   └── CardGrid.tsx               # Responsive documentation feature cards
│   └── interactive/
│       ├── WireFrameSandbox.tsx       # Live 64-byte header + ZENV + ZSIG/ZPOA encoder/decoder
│       ├── PoaQuorumSimulator.tsx     # Dynamic BFT consensus threshold validator (T <= N)
│       ├── PACTVisualizer.tsx         # Canonical PACT record hasher & signature verifier
│       └── ApiRequestTester.tsx       # Live REST request runner for Rivun Cloud API
├── content/                           # Structured documentation content modules
│   ├── getting-started/
│   ├── architecture/
│   ├── consensus/
│   ├── runtime/
│   ├── cloud/
│   ├── crates/                        # 26 crate API reference documents
│   ├── sdks/                          # 4 SDK developer manuals
│   ├── domain-packs/                  # 7 domain packs documentation
│   ├── operations/
│   └── sandboxes/
├── lib/
│   ├── docs-content.ts                # Content loader, slug resolver, metadata extractor
│   ├── search-index.ts                # Full-text inverted index builder and query evaluator
│   ├── navigation.ts                  # Sidebar navigation hierarchy tree & sitemap
│   └── types.ts                       # TypeScript interfaces for docs, navigation, search
├── public/
│   ├── search-index.json              # Precomputed client-side search index
│   └── images/                        # Architecture diagrams and assets
├── package.json
├── tsconfig.json
├── tailwind.config.ts
├── postcss.config.mjs
└── next.config.ts
```

---

## 3. Information Architecture & Navigation Tree

The documentation is organized into 10 structured sections covering every dimension of the Rivun ecosystem:

```text
Rivun Documentation Portal
├── 1. Getting Started
│   ├── Overview & Value Proposition
│   ├── Installation & Prerequisites
│   ├── Quickstart: Local 3-Node Cluster
│   ├── Quickstart: Rust SDK
│   ├── Quickstart: TypeScript SDK
│   ├── Quickstart: Python SDK
│   └── Quickstart: Go SDK
├── 2. Architecture & Core Protocol
│   ├── Protocol Overview & Layering
│   ├── Wire Format (64-byte Header, ZAP_ Magic 0x5A41505F)
│   ├── Universal Envelope (74-byte Header, ZENV Magic, 8 Kinds)
│   ├── Cryptographic Signing (Ed25519, ZSIG Trailer, Transcripts)
│   ├── Encrypted UDP Datagrams (ZAPD Magic, ChaCha20-Poly1305, Nonces)
│   ├── Noise Protocol Handshake (Noise_NN_25519_ChaChaPoly_BLAKE2s)
│   └── Control Subject Catalog (Agent, PACT, Registry, Discovery)
├── 3. Consensus & Quorum Mesh
│   ├── Proof-of-Action (PoA) Consensus Model
│   ├── BFT Swarm Consensus Engine (Propose, Prevote, Precommit, Commit)
│   ├── Dynamic Threshold Signatures (T-of-N Quorum Quotas)
│   ├── Swarm Gossip Protocol (Epidemic Dissemination, PEX, Anti-Entropy)
│   ├── Network Partition & Failover Mesh (Phi Accrual Failure Detector)
│   └── Equivocation Detection & Dispute Proofs
├── 4. Sandboxed WASM Execution & Streaming
│   ├── Wasmtime Runtime & Sandboxing Architecture
│   ├── Driver ABI Specification (v1: alloc, dealloc, execute)
│   ├── Memory Limits, Fuel Metering & Timeout Controls
│   ├── Async Driver Pipelines & Tokio Task Chaining
│   ├── Lock-Free SPSC Ring-Buffers & Streaming I/O
│   └── Inter-Driver IPC Pipes & Deterministic Chaining
├── 5. Rivun Cloud SaaS & Operator Station
│   ├── Zero-Trust sovereign Architecture & Private Key Isolation Invariant
│   ├── Rivun Control Operator Workstation & Ed25519 Key Vault (~/.rivun/operator_keys)
│   ├── Zero-Trust Policy Staging & Offline Signing (Rivun-POLICY-BUNDLE-v1)
│   ├── Edge Daemon: rivun-cloud-bridge (Heartbeats, Receipts, Atomic Swap)
│   ├── Multi-Tenant SaaS API (Axum 0.8 REST & SSE Server)
│   └── Rivun Dashboard (Fleet, Policies, Ledger, Marketplace)
├── 6. 26 Crate API Reference
│   ├── rivun-core (Wire Header, Flags, Auth/Poa Trailers)
│   ├── rivun-crypto (Ed25519 Keypairs, Blinded Commitments, Signatures)
│   ├── rivun-envelope (ZENV Envelopes, 8 Kinds, Memory Parsers)
│   ├── rivun-agent (Agent Intents, Sessions, Delegations, 7-Stage Provenance)
│   ├── rivun-capability (Capability IDs, Grants, Driver Permissions)
│   ├── rivun-cli (Commands: cluster, swarm, receipts, doctor, pack)
│   ├── rivun-cloud-api (Axum 0.8 REST & SSE Server, DB, RBAC)
│   ├── rivun-cloud-bridge (Edge Sidecar, Ingestion, Atomic Policy Engine)
│   ├── rivun-driver-sdk (AsyncDriver, Pinned Buffers, Modbus/Ring-Buffers)
│   ├── rivun-gateway (HTTP & WebSocket API Gateway)
│   ├── rivun-journal (Append-Only Binary Segments ZJSEG001, Indexing)
│   ├── rivun-ledger (Signed Action Receipts, Batch Seals, MMR, Blinded ZK)
│   ├── rivun-machine (Deterministic State Machine Execution)
│   ├── rivun-memory (Memory Storage, Journals, Key-Value Indexing)
│   ├── rivun-net (UDP Datagrams, Gossip, Consensus, Mesh, Noise)
│   ├── rivun-node (Daemon Actor, Dispatch, Policy Enforcement)
│   ├── rivun-ops (Observability, Telemetry Exporter, Configs)
│   ├── rivun-pack (.zpack Archive Packaging, Signatures, Auditor)
│   ├── rivun-pact (PACT Portable Action Records, Verification, Disputes)
│   ├── rivun-policy (Rule Engine: Allow/Deny/PoA, Dispute Evaluator)
│   ├── rivun-router (Subject-Based Message Routing, Priority Queues)
│   ├── rivun-runtime (Wasmtime Execution, ABI v1, Async Engine, IPC)
│   ├── rivun-schema (JSON & Binary Schema Validation)
│   ├── rivun-store (RivunStore Driver Registry, Publications, Manifests)
│   ├── rivun-telemetry (Metrics, Prometheus, 7-Point Fleet Doctor)
│   └── rivun-control (Operator Workstation CLI & Desktop App)
├── 7. 4 SDK Developer Manuals
│   ├── Rust SDK (Integration, Builders, Transports, Examples)
│   ├── TypeScript SDK (Node.js/Browser, Noble Crypto, ZENV, PACT)
│   ├── Python SDK (Dataclasses, UDP Client, Crypto Verification)
│   ├── Go SDK (Protocol Types, BLAKE3, Ed25519, RivunStore)
│   └── Cross-SDK Conformance & Test Matrix (Shared JSON Fixtures)
├── 8. 7 Domain Packs & RivunStore
│   ├── Domain Pack Architecture & pack.toml Manifest Contract
│   ├── Risk Vocabularies & Safety Defaults (low, medium, high, critical)
│   ├── Pack Lifecycle: Init, Build, Sign, Verify, Install, Audit
│   ├── Pack 1: Agentic Development (rivun-pack-agentic-dev)
│   ├── Pack 2: Smart Building Automation (rivun-pack-smart-building)
│   ├── Pack 3: Cloud & Infrastructure Ops (rivun-pack-cloud-ops)
│   ├── Pack 4: Industrial Control & SCADA (rivun-pack-industrial)
│   ├── Pack 5: Personal AI Assistant (rivun-pack-personal-ai)
│   ├── Pack 6: Healthcare & Patient Care (rivun-pack-healthcare)
│   ├── Pack 7: Financial Services & Trading (rivun-pack-finance)
│   └── RivunStore Driver Manifests & Bundle Publishing
├── 9. Fleet Doctor, Incident Forensics & MMR Verifications
│   ├── 7-Point Fleet Doctor Diagnostic Suite:
│   │   ├── 1. Network Reachability & Sockets
│   │   ├── 2. Storage Mounts & Permissions
│   │   ├── 3. Durable Replay Protection WAL (ZAPFRM01)
│   │   ├── 4. Receipt Journal Segment Signatures (ZJSEG001)
│   │   ├── 5. RivunStore Registry Signatures
│   │   ├── 6. Node Identity Key & PoA Quorum Threshold
│   │   └── 7. Peer Trust & Quarantine Status
│   ├── Incident Forensics & Client-Side Secret Redaction (SecretRedactor)
│   ├── Merkle Mountain Range (MMR) Offline Proof Verification
│   └── 7-Stage Causal Provenance Graph Reconstruction
└── 10. Interactive Sandboxes & Tools
    ├── Live Wire Frame Sandbox (Header, ZENV, ZSIG, ZPOA)
    ├── Proof-of-Action Quorum Calculator (T <= N Validator Mesh)
    ├── PACT Record Canonicalizer & BLAKE3 Hasher
    └── Rivun Cloud REST API Live Explorer
```

---

## 4. Instant Client-Side Full-Text Search Architecture

### Design & Mechanics
- **Sub-10ms Query Execution**: Runs entirely in WebAssembly / JavaScript in the browser. Zero network latency for keystrokes.
- **Index Generation**: During build time (`next build`), a static index script parses all markdown/content files, extracting:
  - `id`: unique URL path (e.g. `/docs/crates/rivun-core#rivunheader`)
  - `title`: document title
  - `section`: parent section category (e.g. `26 Crate Reference`)
  - `headings`: subheadings (H2, H3)
  - `content`: plain-text stripped content
  - `keywords`: crate names, struct names, CLI flags, wire magic constants (`ZAP_`, `ZENV`, `ZSIG`, `ZPOA`, `ZAPD`)
- **Search Modal (`SearchModal.tsx`)**:
  - Global hotkey listener: `Cmd+K` (Mac) / `Ctrl+K` (Windows/Linux) and `/` key.
  - Search input with clear button and loading state.
  - Filter chips: `All`, `Protocols`, `Crates`, `SDKs`, `Domain Packs`, `Cloud`, `CLI`.
  - Rich result cards displaying:
    - Section badge (e.g. `CRATE API`, `SDK MANUAL`, `PROTOCOL WIRE`)
    - Highlighted title with matched character bolding
    - Contextual text snippet preview with matched search terms highlighted
    - Keyboard navigation (`ArrowUp`, `ArrowDown`, `Enter` to open, `Escape` to close).

---

## 5. UI Component Library Specifications

### 1. Multi-Language Code Tabs (`CodeTabs.tsx`)
Allows developers to view equivalent code across multiple languages with 1 click:
- Languages supported: **Rust**, **TypeScript**, **Python**, **Go**, **CLI / Bash**.
- Active language preference persists in `localStorage` across documentation pages.
- Header bar with language icon badge, filename label, and instant copy button with "Copied!" checkmark feedback.

### 2. Admonition Callout Boxes (`Callout.tsx`)
Visually distinct, dark glassmorphic callouts with Lucide icons:
- `NOTE` (Blue / Info icon): General helpful context and architectural details.
- `TIP` (Green / Sparkles icon): Best practices and performance optimizations.
- `IMPORTANT` (Purple / AlertCircle icon): Key requirements and invariants.
- `WARNING` (Amber / AlertTriangle icon): Cautionary guidance and deprecations.
- `SECURITY` (Red / ShieldAlert icon): Security invariants, cryptographic boundaries, zero-trust rules.
- `INVARIANT` (Cyan / Lock icon): Formal mathematical guarantees (e.g., private keys never leave local workstation).

### 3. Client-Side Mermaid Renderer (`Mermaid.tsx`)
- Dynamically imports `mermaid` on the client side.
- Renders clean SVGs matching the dark Apple aesthetic with custom theme variables (slate backgrounds, neon cyan/emerald accents, crisp white text).
- Includes zoom, pan, and full-screen view controls for large distributed architecture diagrams.

### 4. Mathematical Formula Renderer (`MathFormula.tsx`)
- Powered by KaTeX for mathematical clarity.
- Renders inline equations ($T \le N$) and display block formulas:
$$\text{Digest} = \text{BLAKE3}\left(\text{"Rivun-POA-DIGEST-v1"} \parallel \text{frame\_without\_poa}\right)$$
$$\text{Receipt Root} = \text{MMR\_PEAK\_BAG}\left(P_1, P_2, \dots, P_k\right)$$

### 5. Interactive Sandboxes (`interactive/`)
- **Wire Frame Sandbox**: Interactive visualizer where users can toggle flags (`ENCRYPTED`, `PRIORITY`, `REQUIRES_CONSENSUS`, `SIGNED`, `BROADCAST`), enter source/target UUIDs, payload string/JSON, and view the exact 64-byte hex header + payload + auth trailer in real-time.
- **Proof-of-Action Quorum Calculator**: Sliders for Total Validators ($N$) and Threshold ($T$), toggle active/faulty/byzantine nodes, and visually inspect whether quorum is achieved ($T \le N_{\text{healthy}}$).
- **PACT Visualizer**: Form to build PACT action records, auto-sort JSON keys deterministically, compute canonical BLAKE3 hash, and generate detached Ed25519 signatures.
- **API Explorer**: Swagger/Postman-style interactive testing interface for Rivun Cloud endpoints (`/v1/status`, `/v1/orgs/{org}/nodes`, `/v1/orgs/{org}/receipts`, `/v1/orgs/{org}/policies`, `/v1/registry/packs`).

---

## 6. Comprehensive Documentation Content Inventory (A to Z)

### Section 1: Getting Started & SDK Quickstarts
- **Quickstart Overview**: Architecture summary, 5-minute setup roadmap.
- **CLI Tooling**: Installation of `rivun-cli`, generating keys (`keygen`), launching 3-node in-memory cluster (`cluster up --nodes 3`), viewing live status (`cluster status`).
- **Rust SDK Quickstart**: Adding `rivun-sdk` dependency, constructing `RivunFrame`, encoding `ZENV` envelopes, signing with `Keypair`.
- **TypeScript SDK Quickstart**: `npm install @rivun/sdk`, initializing Node.js client, building control envelopes, verifying Ed25519 signatures with `@noble/ed25519`.
- **Python SDK Quickstart**: `pip install rivun-sdk`, creating dataclass envelopes, sending encrypted UDP datagrams, verifying PACT records.
- **Go SDK Quickstart**: `go get github.com/rivun/rivun/sdks/go`, encoding binary frames, computing BLAKE3 hashes, parsing RivunStore payloads.

### Section 2: Architecture & Core Protocol Specifications
- **Fixed 64-Byte Wire Header**:
  - Byte 0-3: `MAGIC_NUMBER` = `0x5A41505F` (`ZAP_`)
  - Byte 4-5: `VERSION` = `0x0001` (v1)
  - Byte 6-7: `FLAGS` (bitfield: `ENCRYPTED=0x01`, `PRIORITY=0x02`, `REQUIRES_CONSENSUS=0x04`, `SIGNED=0x08`, `BROADCAST=0x10`)
  - Byte 8-23: `SOURCE_NODE` (16-byte UUID derived from Ed25519 public key)
  - Byte 24-39: `TARGET_NODE` (16-byte UUID, or all zeros for broadcast)
  - Byte 40-47: `TIMESTAMP` (u64 Unix microseconds)
  - Byte 48-55: `ZAP_LEN` (u64 payload length, max 16 MiB)
  - Byte 56-63: `ZAP_SIGN` (8-byte fast signature hint)
- **Authentication Trailer (`ZSIG`)**:
  - 72-byte trailer: magic `ZSIG`, algorithm `u16 = 1` (Ed25519 dalek), signature length `u16 = 64`, signature `[u8; 64]`.
  - Signature transcript: First 56 header bytes (excluding `ZAP_SIGN`) + full payload bytes.
- **Proof-of-Action Trailer (`ZPOA`)**:
  - Magic `ZPOA`, version `u16 = 1`, threshold `u16`, attestation count `u16`, reserved `u16 = 0`, 32-byte frame digest, followed by $k$ attestations (16-byte validator UUID + 64-byte Ed25519 signature).
- **Universal Envelope (`ZENV`)**:
  - 74-byte header: magic `ZENV`, version `u16 = 1`, kind `u16` (1: Data, 2: Event, 3: Command, 4: Query, 5: Response, 6: StreamChunk, 7: Action, 8: Control), ID (16-byte UUID), Correlation ID (16-byte UUID), Causation ID (16-byte UUID), length fields for subject, content-type, metadata, and body.
- **Encrypted UDP Datagram (`ZAPD`)**:
  - Magic `ZAPD`, version `u8 = 1`, reserved `[u8; 3]`, source node UUID (16 bytes), target node UUID (16 bytes), 12-byte nonce (4-byte random prefix + 8-byte big-endian counter), ChaCha20-Poly1305 AEAD ciphertext.
- **Noise Handshake Protocol**:
  - `Noise_NN_25519_ChaChaPoly_BLAKE2s` handshake protocol, ephemeral key derivation, and session key exchange.

### Section 3: Consensus & Quorum Mesh
- **Proof-of-Action (PoA) Paradigm**: Cryptographic action verification without waste of Proof-of-Work or financial stake centralization.
- **BFT Quorum Protocol**: Propose, Prevote, Precommit, Commit state machine with 2/3+ quorum rules ($T \le N$).
- **Dynamic Threshold Signatures**: Aggregation and multi-validator certificate generation.
- **Swarm Gossip Engine**: Epidemic dissemination with k-fanout, bloom filter deduplication cache, peer exchange (PEX), and vector clock anti-entropy synchronization.
- **Failure Detection & Resilient Mesh**: Phi Accrual failure detection, heartbeat jitter, split-brain partition mitigation, dynamic 2-hop relay routing.

### Section 4: Sandboxed WASM Execution & Streaming Runtime
- **Wasmtime Host Architecture**: Isolation guarantees, capability-based host call security.
- **Driver ABI v1**:
  - `memory` export
  - `rivun_alloc(len: i32) -> i32`
  - `rivun_dealloc(ptr: i32, len: i32)`
  - `rivun_execute(action_ptr: i32, action_len: i32, payload_ptr: i32, payload_len: i32) -> i64` (encodes `(result_ptr << 32) | result_len`).
- **Resource Constraints**: Strict WebAssembly fuel metering, maximum memory allocations (default 16MB), timeout timers (default 1000ms).
- **Async Execution Pipeline**: Tokio asynchronous driver scheduler, pinned ring-buffers, lock-free SPSC queues, Modbus/TCP industrial streaming.
- **Zero-Copy Driver IPC**: Inter-driver chaining (Perception $\to$ Policy $\to$ Actuator) with aggregate fuel budgeting.

### Section 5: Rivun Cloud SaaS & Zero-Trust Operator Station
- **Private Key Isolation Invariant**: Sovereign identity model where SaaS control plane NEVER sees, stores, or handles private keys.
- **Operator Station (`rivun-control`)**: Local Tauri desktop app and CLI managing `~/.rivun/operator_keys/` with offline human-in-the-loop signing.
- **Policy Staging & Signing Lifecycle**:
  - Step 1: Policy drafted in Dashboard / TOML.
  - Step 2: Staged on Cloud API with status `staged`.
  - Step 3: Operator inspects diff locally in `rivun-control` and signs with domain `Rivun-POLICY-BUNDLE-v1`.
  - Step 4: Signed bundle submitted to Cloud API and broadcast to edge nodes.
  - Step 5: `rivun-cloud-bridge` on edge nodes verifies signature against local trusted whitelist, writes to tempfile, and executes atomic file swap (`tempfile::persist`).
- **Edge Sidecar Daemon (`rivun-cloud-bridge`)**: Heartbeat telemetry, receipt batch pusher, incident reporter with `SecretRedactor` client-side data scrubbing.
- **Multi-Tenant REST & SSE API (`rivun-cloud-api`)**: Axum 0.8 REST API and real-time Server-Sent Events broker.

### Section 6: 26 Crate API Reference
Complete architectural and programming reference for all 26 workspace crates:

| # | Crate | Primary Types & Interfaces | Key Responsibilities |
|---|---|---|---|
| 1 | `rivun-core` | `RivunHeader`, `RivunFrame`, `RivunFlags`, `AuthTrailer`, `PoaTrailer`, `PoaAttestation` | 64-byte wire frame parsing, binary encoding, magic constants. |
| 2 | `rivun-crypto` | `Keypair`, `PublicKey`, `NodeId`, `BlindedReceiptCommitment` | Ed25519 signatures, BLAKE3 domains, blinded commitments. |
| 3 | `rivun-envelope` | `RivunEnvelope`, `RivunEnvelopeRef`, `RivunMessageKind` (8 kinds) | 74-byte universal envelope codec, correlation/causation tracking. |
| 4 | `rivun-agent` | `AgentId`, `AgentIntent`, `AgentSession`, `DelegationRequest`, `ProvenanceStage` | High-level agent protocol, 7-stage causal provenance chain. |
| 5 | `rivun-capability` | `CapabilityId`, `CapabilityStore`, `DriverPermissions`, `CapabilityGrants` | Capability identifiers, permission definitions, host call gates. |
| 6 | `rivun-cli` | `ClusterCommand`, `SwarmCommand`, `DoctorCommand`, `ReceiptsCommand` | Unified operator and developer command-line interface. |
| 7 | `rivun-cloud-api` | `CloudDatabase`, `EventBroker`, `AppState`, REST/SSE route handlers | Axum 0.8 multi-tenant SaaS REST API and live SSE broker. |
| 8 | `rivun-cloud-bridge` | `CloudBridgeDaemon`, `CloudBridgeClient`, `PolicyVerifier`, `BridgeConfig` | Edge node sidecar daemon, telemetry pusher, atomic policy sync. |
| 9 | `rivun-driver-sdk` | `AsyncDriver`, `SpscRingBuffer`, `ModbusConnection`, `PinnedBuffer` | Asynchronous driver SDK, streaming I/O, pinned memory buffers. |
| 10 | `rivun-gateway` | `GatewayServer`, `HttpAdapter`, `WebSocketAdapter`, `FrameRouter` | Edge HTTP/WebSocket gateway to Rivun mesh. |
| 11 | `rivun-journal` | `JournalStore`, `JournalRecord`, `SegmentManifest`, `JournalProfile` | Append-only binary journal segments (`ZJSEG001`), indexing, recovery. |
| 12 | `rivun-ledger` | `ActionReceipt`, `SignedActionReceipt`, `IncrementalMmr`, `ZkReceiptBatchProof` | Signed receipts, batch seals, Merkle Mountain Ranges, blinded ZK rollups. |
| 13 | `rivun-machine` | `StateMachine`, `StateTransition`, `DeterministicExecutor` | Deterministic finite state machine execution. |
| 14 | `rivun-memory` | `MemoryStore`, `MemoryJournal`, `VectorIndex`, `MemoryRecord` | Memory storage, key-value journals, vector embeddings. |
| 15 | `rivun-net` | `UdpTransport`, `SwarmGossipEngine`, `BftConsensusEngine`, `SwarmMeshTopology` | Encrypted UDP datagrams (`ZAPD`), gossip, BFT consensus, mesh. |
| 16 | `rivun-node` | `NodeDaemon`, `Dispatcher`, `PolicyEnforcer`, `ReceiptLogger` | Core node execution daemon actor and message dispatcher. |
| 17 | `rivun-ops` | `MetricsCollector`, `PrometheusExporter`, `LoggingConfig` | Observability, metrics collection, production operations. |
| 18 | `rivun-pack` | `DomainPackBundle`, `DomainPackBundleSignature`, `audit_bundle` | `.zpack` bundle compiler, manifest validator, signature verifier. |
| 19 | `rivun-pact` | `RivunPact`, `RivunPactProof`, `RivunPactVerification`, `DisputeEngine` | PACT portable action records, offline verification, dispute solver. |
| 20 | `rivun-policy` | `PolicyEngine`, `PolicyRule`, `DisputeEvaluator`, `ActionPolicy` | Conditional rule evaluator (Allow/Deny/PoA/Grant), dispute engine. |
| 21 | `rivun-router` | `MessageRouter`, `RouteTable`, `CapabilityRoute`, `PriorityQueue` | Subject-based message routing, capability-based dispatch. |
| 22 | `rivun-runtime` | `AsyncWasmExecutor`, `DriverPipeline`, `StreamingBufferPool`, `IpcPipe` | Wasmtime runtime, ABI v1, async execution pipeline, IPC pipes. |
| 23 | `rivun-schema` | `SchemaValidator`, `JsonSchema`, `BinaryCodec` | JSON schema and binary protocol schema validation. |
| 24 | `rivun-store` | `DriverRegistry`, `DomainPackRegistry`, `DriverManifest`, `InstallPlan` | RivunStore driver registry, publication manifests, offline bundles. |
| 25 | `rivun-telemetry` | `FleetDoctor`, `FleetDoctorReport`, `FleetDoctorCheck`, `FleetTopology` | Telemetry metrics, 7-Point Fleet Doctor diagnostics engine. |
| 26 | `rivun-control` | `KeyVault`, `OperatorSigner`, `CloudSync`, `TauriApp` | Local operator workstation desktop app & Ed25519 key vault. |

### Section 7: 4 SDK Developer Manuals
- **Rust Developer Manual**: Cargo setup, idiomatic builders, async Tokio clients, custom WASM driver authoring, receipt verification.
- **TypeScript Developer Manual**: Node.js and browser integration, `@noble/hashes` & `@noble/ed25519` cryptographic signing, ZENV envelope codecs, PACT canonical verification.
- **Python Developer Manual**: Dataclasses, typing, stdlib UDP socket transport, cryptographic verification with `crypto` extra, receipt signing messages.
- **Go Developer Manual**: Go package imports, struct serialization, canonical BLAKE3 hashing with `lukechampine.com/blake3`, standard `crypto/ed25519` verification, RivunStore types.
- **Conformance & Test Matrix**: Reference to 11 shared JSON fixtures in `fixtures/`, proving bit-for-bit equivalence across all 4 SDKs.

### Section 8: 7 Domain Packs & RivunStore Publishing
- **Domain Pack Structure**: `pack.toml`, `README.md`, `schemas/`, `policies/`, `routes/`, `drivers/`, `dashboards/`, `tests/`.
- **7 Foundation Packs**:
  1. `agentic-dev`: Auditable coding agents, `repo.read`, `repo.patch`, `test.run`.
  2. `smart-building`: Smart building sensors and actuators, fail-closed safety policy.
  3. `cloud-ops`: Deployment and incident automation, rollback safeguards.
  4. `industrial`: Industrial control and SCADA with simulation gates and Proof-of-Action.
  5. `personal-ai`: Personal assistant actions with human-in-the-loop approval gates.
  6. `healthcare`: Privacy-first patient care coordination with strict HIPAA audit trails.
  7. `finance`: Trade proposal, risk check, multi-signature approval, execution, and settlement flows.
- **Pack Lifecycle & RivunStore**: `rivun pack init` $\to$ `build` $\to$ `sign` $\to$ `verify` $\to$ `install` $\to$ `audit`.

### Section 9: Fleet Doctor Diagnostics, Incident Forensics & MMR Verifications
- **7-Point Fleet Doctor Diagnostic Checks**:
  1. `network` (`cluster_network_reachability`): UDP bind port reachable, socket open, active peer count.
  2. `storage` (`storage_mounts_and_permissions`): Receipt and memory directories exist with read/write permissions.
  3. `replay_guard` (`durable_replay_store_wal`): Durable replay WAL active (`ZAPFRM01` magic), clock skew validation.
  4. `journal` (`segment_rotation_and_manifest_signatures`): Receipt journal segment rotation (`ZJSEG001`) and `SignedReceiptSegmentManifest` cryptographic verification.
  5. `pack_registry` (`rivun_store_index_and_signatures`): RivunStore registry index presence with valid cryptographic signature.
  6. `certificate_validity` (`node_identity_key_and_poa_quorum`): Node Ed25519 identity keypair validity, PACT signature threshold, and PoA quorum availability ($T \le N$).
  7. `peer_trust` (`peer_trust_status`): Peer trust verification, detecting quarantined/revoked/banned nodes in fleet topology.
- **Incident Forensics**: Diagnostic dump capture, client-side data scrubbing with `SecretRedactor`.
- **MMR Offline Mathematical Proof**: Inclusion proofs, exclusion proofs, peak-bagging root calculation, independent receipt verification (`rivun receipts verify --offline`).

### Section 10: Interactive Sandboxes & Protocol Playgrounds
- **Live Wire Frame Sandbox**: Visual frame builder, byte offset inspector, real-time hexadecimal output.
- **Proof-of-Action Quorum Simulator**: Interactive mesh topology where users can simulate node failures and network partitions.
- **PACT Canonicalizer**: Paste raw JSON, inspect sorted keys, view exact BLAKE3 digest and domain-separated signing transcript.
- **API Explorer**: Interactive REST testing sandbox for Rivun Cloud endpoints with instant schema validation.

---

## 7. Build System, Scripts & Verification Setup

### Package Configuration (`apps/docs-portal/package.json`)
```json
{
  "name": "docs-portal",
  "version": "0.1.0",
  "private": true,
  "scripts": {
    "dev": "next dev --port 3001",
    "build": "next build",
    "start": "next start --port 3001",
    "lint": "next lint",
    "typecheck": "tsc --noEmit"
  },
  "dependencies": {
    "clsx": "^2.1.1",
    "lucide-react": "^0.475.0",
    "next": "^15.1.7",
    "react": "^19.0.0",
    "react-dom": "^19.0.0",
    "tailwind-merge": "^3.0.1"
  },
  "devDependencies": {
    "@types/node": "^22.13.4",
    "@types/react": "^19.0.8",
    "@types/react-dom": "^19.0.3",
    "autoprefixer": "^10.5.4",
    "postcss": "^8.5.2",
    "tailwindcss": "^3.4.17",
    "typescript": "^5.7.3"
  }
}
```

### Verification Criteria
- `npm run build` succeeds with **0 errors and 0 warnings**.
- `npm run typecheck` passes with **0 TypeScript diagnostic errors**.
- All static routes, dynamic docs slugs (`/docs/[...slug]`), and interactive tools render seamlessly.
- Client-side search index builds completely during static generation.

---

## 8. Implementation Strategy & Next Steps

1. **Scaffold `apps/docs-portal`**: Create configuration files (`package.json`, `tsconfig.json`, `tailwind.config.ts`, `postcss.config.mjs`, `next.config.ts`).
2. **Build Component Foundation**: Implement design system primitives, multi-language `CodeTabs`, `Callout`, `Breadcrumbs`, `Sidebar`, `TableOfContents`, and `SearchModal`.
3. **Assemble Complete Documentation Content Tree**: Populate structured content modules for all 10 documentation sections (Getting Started, Wire/Envelopes, Consensus, WASM, Cloud SaaS, 26 Crate APIs, 4 SDKs, 7 Domain Packs, Fleet Doctor, Sandboxes).
4. **Implement Interactive Tools**: Build in-browser Wire Frame Sandbox, PoA Quorum Simulator, PACT Canonicalizer, and API Explorer.
5. **Build and Validate**: Run `npm run build` and `npm run typecheck`, verify 0 errors, and validate complete link integrity.
