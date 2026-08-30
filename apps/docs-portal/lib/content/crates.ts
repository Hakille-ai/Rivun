import { DocPage } from '../types';

export const CRATE_DOCS: DocPage[] = [
  {
    slug: ['crates', 'rivun-core'],
    path: '/docs/crates/rivun-core',
    title: 'rivun-core — Core Wire Protocol & Framing',
    description: 'Root binary protocol types, fixed 64-byte frame header, bitflags, auth and PoA trailers.',
    section: '6. 26 Crate API Reference',
    subSection: 'Core Crates',
    headings: [
      { id: 'overview', text: 'Crate Overview', level: 2 },
      { id: 'key-structs', text: 'Primary Types & Enums', level: 2 },
      { id: 'code-example', text: 'Usage Example', level: 2 },
    ],
    callouts: [
      {
        type: 'note',
        title: 'Zero Workspace Dependencies',
        content: 'rivun-core is the foundational leaf crate in the workspace. It contains zero internal workspace dependencies and compiles in milliseconds.',
      },
    ],
    multiLangSnippets: [
      {
        id: 'rivun-core-example',
        snippets: {
          rust: {
            title: 'main.rs',
            code: `use rivun_core::{RivunFrame, RivunHeader, RivunFlags, now_micros};\n\nlet frame = RivunFrame::builder()\n    .flags(RivunFlags::SIGNED | RivunFlags::PRIORITY)\n    .source([1u8; 16])\n    .target([2u8; 16])\n    .timestamp(now_micros())\n    .payload(b"HELLO_RIVUN")\n    .build()?;\n\nassert_eq!(frame.header().magic, [0x5A, 0x41, 0x50, 0x5F]);`,
          },
        },
      },
    ],
    rawContent: `
### Key Structs & Traits
- \`RivunHeader\`: 64-byte big-endian header (\`magic\`, \`version\`, \`flags\`, \`source_node\`, \`target_node\`, \`timestamp_micros\`, \`rivun_len\`, \`rivun_sign\`).
- \`RivunFrame\`: Memory-managed or zero-copy sliced wire frame container.
- \`RivunFlags\`: Bitmask supporting \`ENCRYPTED (0x01)\`, \`PRIORITY (0x02)\`, \`REQUIRES_CONSENSUS (0x04)\`, \`SIGNED (0x08)\`, \`BROADCAST (0x10)\`.
- \`AuthTrailer\`: 72-byte Ed25519 authentication trailer (\`ZSIG\`).
- \`PoaTrailer\`: Variable-length Proof-of-Action quorum attestation trailer (\`ZPOA\`).
    `,
  },
  {
    slug: ['crates', 'rivun-crypto'],
    path: '/docs/crates/rivun-crypto',
    title: 'rivun-crypto — Cryptographic Primitives & Signatures',
    description: 'Ed25519 signing/verification, BLAKE3 domain separation, node ID derivation (UUID v8), blinded commitments.',
    section: '6. 26 Crate API Reference',
    subSection: 'Core Crates',
    headings: [
      { id: 'overview', text: 'Crate Overview', level: 2 },
      { id: 'key-structs', text: 'Primary Types & Functions', level: 2 },
      { id: 'code-example', text: 'Usage Example', level: 2 },
    ],
    multiLangSnippets: [
      {
        id: 'crypto-example',
        snippets: {
          rust: {
            title: 'main.rs',
            code: `use rivun_crypto::{Keypair, sign_frame, verify_frame, node_id_from_public_key};\nuse rivun_core::RivunFrame;\n\nlet keypair = Keypair::generate();\nlet node_id = node_id_from_public_key(&keypair.public_key());\n\nlet mut frame = RivunFrame::builder().source(node_id).payload(b"SECURE_DATA").build()?;\nsign_frame(&mut frame, &keypair)?;\nassert!(verify_frame(&frame, &keypair.public_key())?);`,
          },
        },
      },
    ],
    rawContent: `
### Key Structs & Functions
- \`Keypair\`: Ed25519 secret and public keypair wrapper.
- \`PublicKey\`: 32-byte Ed25519 public key.
- \`node_id_from_public_key(pk: &PublicKey) -> [u8; 16]\`: Derives UUID v8 using domain \`Rivun-NODE-ID-v1\`.
- \`sign_frame(frame, keypair)\`: Attaches \`ZSIG\` trailer.
- \`verify_frame(frame, pk)\`: Verifies detached signature across header and payload bytes.
- \`BlindedReceiptCommitment\`: Zero-knowledge blinded receipt hashing.
    `,
  },
  {
    slug: ['crates', 'rivun-envelope'],
    path: '/docs/crates/rivun-envelope',
    title: 'rivun-envelope — Universal 74-Byte ZENV Messaging',
    description: 'Universal 74-byte zero-copy ZENV envelope parser, builder, 8 message kinds, and correlation tracking.',
    section: '6. 26 Crate API Reference',
    subSection: 'Core Crates',
    headings: [
      { id: 'overview', text: 'Crate Overview', level: 2 },
      { id: 'message-kinds', text: 'RivunMessageKind Enum', level: 2 },
      { id: 'code-example', text: 'Usage Example', level: 2 },
    ],
    rawContent: `
### Key Structs & Enums
- \`RivunEnvelope\`: Owned container for ZENV messages.
- \`RivunEnvelopeRef<'a>\`: Zero-copy borrowing parser.
- \`RivunMessageKind\`: \`Data (1)\`, \`Event (2)\`, \`Command (3)\`, \`Query (4)\`, \`Response (5)\`, \`StreamChunk (6)\`, \`Action (7)\`, \`Control (8)\`.
- \`ZenvHeader\`: 74-byte packed binary header.
    `,
  },
  {
    slug: ['crates', 'rivun-agent'],
    path: '/docs/crates/rivun-agent',
    title: 'rivun-agent — Agent Protocols & 7-Stage Provenance',
    description: 'Autonomous AI agent contracts, intents, sessions, delegations, and 7-stage causal provenance chain engine.',
    section: '6. 26 Crate API Reference',
    subSection: 'Agent Protocols',
    headings: [
      { id: 'overview', text: 'Crate Overview', level: 2 },
      { id: 'provenance-engine', text: '7-Stage Causal Provenance Chain', level: 2 },
      { id: 'key-structs', text: 'Primary Types', level: 2 },
    ],
    rawContent: `
### 7-Stage Causal Provenance Chain
The \`ProvenanceChainBuilder\` links:
1. \`Intent\`: Human/Agent initial proposal.
2. \`Negotiation\`: Multi-agent capability handshake.
3. \`Policy\`: Policy engine evaluation result.
4. \`Consensus\`: Precommit quorum certificate.
5. \`Driver\`: WASM sandbox execution record.
6. \`PoA\`: Multi-validator threshold attestation.
7. \`Receipt\`: MMR leaf commitment root.
    `,
  },
  {
    slug: ['crates', 'rivun-capability'],
    path: '/docs/crates/rivun-capability',
    title: 'rivun-capability — Hierarchical Permission Trees',
    description: 'Capability identifiers, permission matrices, driver declarations, and cached grant trees.',
    section: '6. 26 Crate API Reference',
    subSection: 'Security & Access',
    headings: [
      { id: 'overview', text: 'Crate Overview', level: 2 },
      { id: 'capability-id', text: 'CapabilityId Hierarchies', level: 2 },
    ],
    rawContent: `
Provides hierarchical dot-separated capabilities (e.g. \`scada.hvac.read\`, \`scada.hvac.write\`, \`scada.hvac.*\`) with dynamic grant token verification and permission caching.
    `,
  },
  {
    slug: ['crates', 'rivun-cli'],
    path: '/docs/crates/rivun-cli',
    title: 'rivun-cli — Unified Command-Line Interface',
    description: 'Operator CLI for cluster orchestration, keygen, doctor, journal inspection, and pack packaging.',
    section: '6. 26 Crate API Reference',
    subSection: 'Tooling',
    headings: [
      { id: 'overview', text: 'Crate Overview', level: 2 },
      { id: 'cli-commands', text: 'Top-Level Commands', level: 2 },
    ],
    rawContent: `
### Commands
- \`rivun keygen\`: Generate Ed25519 keypairs.
- \`rivun cluster up\`: Launch local in-memory or UDP clusters.
- \`rivun doctor\`: Run 7-Point Fleet Doctor diagnostics.
- \`rivun receipts verify\`: Offline MMR proof validator.
- \`rivun pack init / build / sign / audit\`: Domain pack lifecycle.
    `,
  },
  {
    slug: ['crates', 'rivun-cloud-api'],
    path: '/docs/crates/rivun-cloud-api',
    title: 'rivun-cloud-api — Multi-Tenant SaaS Server',
    description: 'Axum 0.8 REST API and Server-Sent Events broker for fleet observability, receipts, and policy staging.',
    section: '6. 26 Crate API Reference',
    subSection: 'Cloud & SaaS',
    headings: [
      { id: 'overview', text: 'Crate Overview', level: 2 },
      { id: 'routes', text: 'REST & SSE Endpoints', level: 2 },
    ],
    rawContent: `
Axum 0.8 async server supporting database storage, multi-tenant organization boundaries, receipt search, and real-time SSE stream broadcasting.
    `,
  },
  {
    slug: ['crates', 'rivun-cloud-bridge'],
    path: '/docs/crates/rivun-cloud-bridge',
    title: 'rivun-cloud-bridge — Edge Telemetry & Policy Sync Sidecar',
    description: 'Edge daemon pushing telemetry and receipts to Rivun Cloud and pulling signed policy bundles with atomic file swap.',
    section: '6. 26 Crate API Reference',
    subSection: 'Cloud & SaaS',
    headings: [
      { id: 'overview', text: 'Crate Overview', level: 2 },
      { id: 'atomic-engine', text: 'Atomic Policy Swap Engine', level: 2 },
    ],
    rawContent: `
Lightweight sidecar daemon integrating \`SecretRedactor\`, receipt spooling, and offline signature validation for staged policies.
    `,
  },
  {
    slug: ['crates', 'rivun-driver-sdk'],
    path: '/docs/crates/rivun-driver-sdk',
    title: 'rivun-driver-sdk — Rust Guest WASM Driver SDK',
    description: 'Ergonomic macros, buffer wrappers, and ABI v1 helpers for authoring WebAssembly action drivers.',
    section: '6. 26 Crate API Reference',
    subSection: 'WASM Runtime',
    headings: [
      { id: 'overview', text: 'Crate Overview', level: 2 },
      { id: 'macro-usage', text: 'rivun_driver! Macro & PackedResult', level: 2 },
    ],
    rawContent: `
Provides zero-boilerplate helpers for compiling Rust crates to \`wasm32-wasip1\` drivers adhering to Driver ABI v1.
    `,
  },
  {
    slug: ['crates', 'rivun-gateway'],
    path: '/docs/crates/rivun-gateway',
    title: 'rivun-gateway — Agent Gateway & MCP Protocol Server',
    description: 'Model Context Protocol (MCP) server over stdio/SSE/WebSocket and HTTP REST bridge to the Rivun mesh.',
    section: '6. 26 Crate API Reference',
    subSection: 'Agent Protocols',
    headings: [
      { id: 'overview', text: 'Crate Overview', level: 2 },
      { id: 'mcp-integration', text: 'Model Context Protocol (MCP) Tools', level: 2 },
    ],
    rawContent: `
Bridges LLM agents (Claude, ChatGPT, Local Models) directly to Rivun capabilities via standard MCP tool calls and WebSocket connections.
    `,
  },
  {
    slug: ['crates', 'rivun-journal'],
    path: '/docs/crates/rivun-journal',
    title: 'rivun-journal — Segmented Append-Only Binary Storage',
    description: 'High-throughput append-only disk storage, binary index files, segment rotation (ZJSEG001), and recovery.',
    section: '6. 26 Crate API Reference',
    subSection: 'Storage & Ledger',
    headings: [
      { id: 'overview', text: 'Crate Overview', level: 2 },
      { id: 'segment-format', text: 'ZJSEG001 Segment Format', level: 2 },
    ],
    rawContent: `
Manages durable WAL segments on disk with CRC32/BLAKE3 checksums, 64MB automatic rotation boundaries, and crash-safe indexing.
    `,
  },
  {
    slug: ['crates', 'rivun-ledger'],
    path: '/docs/crates/rivun-ledger',
    title: 'rivun-ledger — Action Receipts & Merkle Mountain Ranges',
    description: 'Signed action receipts, Merkle Mountain Range (MMR) accumulator, inclusion/exclusion proofs, batch seals.',
    section: '6. 26 Crate API Reference',
    subSection: 'Storage & Ledger',
    headings: [
      { id: 'overview', text: 'Crate Overview', level: 2 },
      { id: 'mmr-accumulator', text: 'Incremental Merkle Mountain Range (MMR)', level: 2 },
    ],
    rawContent: `
### Key Structs
- \`ActionReceipt\`: Cryptographic record of action execution.
- \`SignedActionReceipt\`: Ed25519-certified action receipt.
- \`IncrementalMmr\`: Append-only Merkle Mountain Range with $O(\\log N)$ peak bagging and inclusion proofs.
- \`MmrExclusionProof\`: Non-membership verification.
    `,
  },
  {
    slug: ['crates', 'rivun-machine'],
    path: '/docs/crates/rivun-machine',
    title: 'rivun-machine — Industrial Machine Connection Primitives',
    description: 'Hardware-neutral device profiles, Modbus/Serial/TCP protocol adapters, and deterministic machine state.',
    section: '6. 26 Crate API Reference',
    subSection: 'Industrial & Edge',
    headings: [
      { id: 'overview', text: 'Crate Overview', level: 2 },
      { id: 'device-profiles', text: 'DeviceProfile & ProtocolAdapter', level: 2 },
    ],
    rawContent: `
Provides deterministic abstractions over physical PLC, SCADA, and IoT hardware controllers with built-in simulation fallback modes.
    `,
  },
  {
    slug: ['crates', 'rivun-memory'],
    path: '/docs/crates/rivun-memory',
    title: 'rivun-memory — Auditable Hash-Chained Memory Journal',
    description: 'Auditable local binary memory journal, hash-chained entry trees, tombstones, and cryptographic verification.',
    section: '6. 26 Crate API Reference',
    subSection: 'Storage & Ledger',
    headings: [
      { id: 'overview', text: 'Crate Overview', level: 2 },
      { id: 'memory-store', text: 'MemoryJournalStore & Verification', level: 2 },
    ],
    rawContent: `
Tamper-evident agent memory storage where every read, write, and tombstone deletion is cryptographically hash-chained.
    `,
  },
  {
    slug: ['crates', 'rivun-net'],
    path: '/docs/crates/rivun-net',
    title: 'rivun-net — Encrypted UDP, BFT Consensus & Gossip Mesh',
    description: 'ChaCha20-Poly1305 UDP transport (ZAPD), BFT consensus engine, gossip mesh, and failure detection.',
    section: '6. 26 Crate API Reference',
    subSection: 'Networking & Mesh',
    headings: [
      { id: 'overview', text: 'Crate Overview', level: 2 },
      { id: 'transport-mesh', text: 'UdpTransport & SwarmMeshTopology', level: 2 },
    ],
    rawContent: `
Handles point-to-point AEAD UDP framing, peer discovery, vector clock reconciliation, and BFT consensus round coordination.
    `,
  },
  {
    slug: ['crates', 'rivun-node'],
    path: '/docs/crates/rivun-node',
    title: 'rivun-node — Node Daemon Actor & Dispatch Runtime',
    description: 'Main node daemon runtime, configuration validation, peer trust management, and message dispatch loop.',
    section: '6. 26 Crate API Reference',
    subSection: 'Node Core',
    headings: [
      { id: 'overview', text: 'Crate Overview', level: 2 },
      { id: 'daemon-loop', text: 'NodeDaemon & Message Dispatch Loop', level: 2 },
    ],
    rawContent: `
The central daemon orchestrating the networking stack, policy enforcer, WASM runtime, and journal storage into a unified actor.
    `,
  },
  {
    slug: ['crates', 'rivun-ops'],
    path: '/docs/crates/rivun-ops',
    title: 'rivun-ops — Governance, Approvals & Audit Trails',
    description: 'Operational governance contracts, multi-signature quorums, release manifests, and audit trails.',
    section: '6. 26 Crate API Reference',
    subSection: 'Operations',
    headings: [
      { id: 'overview', text: 'Crate Overview', level: 2 },
      { id: 'governance', text: 'ApprovalPolicy & Multi-Sig Quorums', level: 2 },
    ],
    rawContent: `
Formalizes operational approval workflows for sensitive actions like firmware upgrades and cluster reconfigurations.
    `,
  },
  {
    slug: ['crates', 'rivun-pack'],
    path: '/docs/crates/rivun-pack',
    title: 'rivun-pack — .zpack Archive Bundling & Auditing',
    description: 'Domain Pack bundle compiler, pack.toml manifest parser, detached signatures, and security auditing.',
    section: '6. 26 Crate API Reference',
    subSection: 'Domain Packs',
    headings: [
      { id: 'overview', text: 'Crate Overview', level: 2 },
      { id: 'bundle-pipeline', text: 'DomainPackBundle & audit_bundle()', level: 2 },
    ],
    rawContent: `
Compiles policies, WASM drivers, schemas, and routes into compressed \`.zpack\` archives with cryptographic author signatures.
    `,
  },
  {
    slug: ['crates', 'rivun-pact'],
    path: '/docs/crates/rivun-pact',
    title: 'rivun-pact — Multi-Party Conditional Contracts & Escrow',
    description: 'PACT portable action records, escrow deposits, threshold arbitration, slashing engine, and dispute proofs.',
    section: '6. 26 Crate API Reference',
    subSection: 'Agent Protocols',
    headings: [
      { id: 'overview', text: 'Crate Overview', level: 2 },
      { id: 'pact-engine', text: 'RivunPact & DisputeEngine', level: 2 },
    ],
    rawContent: `
Enables autonomous agents to negotiate binding multi-party contracts with automated escrow release or threshold arbitration rulings.
    `,
  },
  {
    slug: ['crates', 'rivun-policy'],
    path: '/docs/crates/rivun-policy',
    title: 'rivun-policy — Deterministic Rule Engine & Gates',
    description: 'Conditional rule evaluation (Allow/Deny/PoA/Grant), break-glass overrides, and dispute evaluators.',
    section: '6. 26 Crate API Reference',
    subSection: 'Security & Access',
    headings: [
      { id: 'overview', text: 'Crate Overview', level: 2 },
      { id: 'policy-rules', text: 'PolicySet, PolicyRule & Decision Gates', level: 2 },
    ],
    rawContent: `
Evaluates subject patterns, payload predicates, time-of-day windows, and risk categories to determine action authorization in sub-microsecond speeds.
    `,
  },
  {
    slug: ['crates', 'rivun-router'],
    path: '/docs/crates/rivun-router',
    title: 'rivun-router — Deterministic Message Routing & Priority Queues',
    description: 'Subject-based message routing, capability-based dispatch, priority queues, and routing explanations.',
    section: '6. 26 Crate API Reference',
    subSection: 'Networking & Mesh',
    headings: [
      { id: 'overview', text: 'Crate Overview', level: 2 },
      { id: 'route-tables', text: 'RouteTable & PriorityQueue', level: 2 },
    ],
    rawContent: `
Fast prefix-tree pattern matching engine that resolves target node endpoints and local handler channels for incoming envelopes.
    `,
  },
  {
    slug: ['crates', 'rivun-runtime'],
    path: '/docs/crates/rivun-runtime',
    title: 'rivun-runtime — Wasmtime Execution & Ring-Buffers',
    description: 'Wasmtime runtime host, Driver ABI v1 executor, SPSC ring-buffers, driver pipelines, and Modbus streaming.',
    section: '6. 26 Crate API Reference',
    subSection: 'WASM Runtime',
    headings: [
      { id: 'overview', text: 'Crate Overview', level: 2 },
      { id: 'runtime-engine', text: 'AsyncWasmExecutor & DriverPipeline', level: 2 },
    ],
    rawContent: `
The host execution environment managing WebAssembly module compilation, instance pooling, fuel tracking, and lock-free ring-buffer I/O.
    `,
  },
  {
    slug: ['crates', 'rivun-schema'],
    path: '/docs/crates/rivun-schema',
    title: 'rivun-schema — ZENV Schema & Contract Validation',
    description: 'Typed JSON and binary message contracts for ZENV envelopes, schema validation, and field constraints.',
    section: '6. 26 Crate API Reference',
    subSection: 'Core Crates',
    headings: [
      { id: 'overview', text: 'Crate Overview', level: 2 },
      { id: 'message-contracts', text: 'MessageContract & Schema Validation', level: 2 },
    ],
    rawContent: `
Enforces strict schema constraints on message payloads before passing them to drivers or ledger storage.
    `,
  },
  {
    slug: ['crates', 'rivun-store'],
    path: '/docs/crates/rivun-store',
    title: 'rivun-store — Offline Signed Driver Registry',
    description: 'Driver registry, semantic version resolution, offline installation plans, and publication manifests.',
    section: '6. 26 Crate API Reference',
    subSection: 'Domain Packs',
    headings: [
      { id: 'overview', text: 'Crate Overview', level: 2 },
      { id: 'registry-types', text: 'DriverRegistry, DriverManifest & InstallPlan', level: 2 },
    ],
    rawContent: `
Maintains an offline-capable, cryptographically verified registry of WASM drivers and domain pack bundles with dependency solving.
    `,
  },
  {
    slug: ['crates', 'rivun-telemetry'],
    path: '/docs/crates/rivun-telemetry',
    title: 'rivun-telemetry — 7-Point Fleet Doctor Diagnostics',
    description: 'Prometheus metrics, OpenTelemetry tracing, fleet topology aggregation, 7-Point Fleet Doctor, incident forensics.',
    section: '6. 26 Crate API Reference',
    subSection: 'Operations',
    headings: [
      { id: 'overview', text: 'Crate Overview', level: 2 },
      { id: 'fleet-doctor', text: 'FleetDoctor 7-Point Diagnostic Engine', level: 2 },
    ],
    rawContent: `
Implements the comprehensive 7-point health check suite, secret-scrubbing forensic snapshots, and Prometheus metric exporters.
    `,
  },
  {
    slug: ['crates', 'rivun-control'],
    path: '/docs/crates/rivun-control',
    title: 'rivun-control — Sovereign Operator Desktop Workstation',
    description: 'Operator workstation desktop app (Tauri) and local Ed25519 key vault for staging and signing policies.',
    section: '6. 26 Crate API Reference',
    subSection: 'Tooling',
    headings: [
      { id: 'overview', text: 'Crate Overview', level: 2 },
      { id: 'key-vault', text: 'OperatorVault & Offline Signing Commands', level: 2 },
    ],
    rawContent: `
Native Tauri application providing sovereign key isolation, policy diff review, offline signature generation, and cloud synchronization.
    `,
  },
];
