import { NavSection, NavItem } from './types';

export const DOCS_NAVIGATION: NavSection[] = [
  {
    title: '1. Getting Started',
    icon: 'Rocket',
    items: [
      { title: 'Overview & Value Proposition', href: '/docs/getting-started/overview' },
      { title: 'Installation & CLI Tooling', href: '/docs/getting-started/installation' },
      { title: 'Local 3-Node Cluster', href: '/docs/getting-started/cluster-quickstart' },
      { title: 'Quickstart: Rust SDK', href: '/docs/getting-started/rust-quickstart', badge: 'Rust' },
      { title: 'Quickstart: TypeScript SDK', href: '/docs/getting-started/typescript-quickstart', badge: 'TS' },
      { title: 'Quickstart: Python SDK', href: '/docs/getting-started/python-quickstart', badge: 'Py' },
      { title: 'Quickstart: Go SDK', href: '/docs/getting-started/go-quickstart', badge: 'Go' },
    ],
  },
  {
    title: '2. Architecture & Core Protocol',
    icon: 'Layers',
    items: [
      { title: 'Protocol Overview & Layering', href: '/docs/architecture/overview' },
      { title: 'Fixed 64-byte Wire Header', href: '/docs/architecture/wire-format', badge: '0x5A41505F' },
      { title: 'Universal Envelope (ZENV)', href: '/docs/architecture/universal-envelope', badge: '74-byte' },
      { title: 'Cryptographic Signing & Transcripts', href: '/docs/architecture/cryptography' },
      { title: 'Encrypted UDP Datagrams (ZAPD)', href: '/docs/architecture/encrypted-udp', badge: 'AEAD' },
      { title: 'Noise Handshake Protocol', href: '/docs/architecture/noise-handshake' },
      { title: 'Control Subject Catalog', href: '/docs/architecture/subject-catalog' },
    ],
  },
  {
    title: '3. Consensus & Quorum Mesh',
    icon: 'Cpu',
    items: [
      { title: 'Proof-of-Action (PoA) Paradigm', href: '/docs/consensus/poa-model' },
      { title: 'BFT Swarm Consensus Engine', href: '/docs/consensus/bft-consensus', badge: 'T <= N' },
      { title: 'Dynamic Threshold Signatures (ZPOA)', href: '/docs/consensus/threshold-signatures' },
      { title: 'Swarm Gossip & Anti-Entropy', href: '/docs/consensus/gossip-protocol' },
      { title: 'Network Partition & Failover Mesh', href: '/docs/consensus/mesh-failover' },
      { title: 'Equivocation Detection & Slashing', href: '/docs/consensus/slashing-disputes' },
    ],
  },
  {
    title: '4. Sandboxed WASM & Streaming',
    icon: 'Boxes',
    items: [
      { title: 'Wasmtime Host Sandboxing', href: '/docs/runtime/wasm-sandboxing' },
      { title: 'Driver ABI v1 Specification', href: '/docs/runtime/driver-abi' },
      { title: 'Resource Constraints & Fuel Metering', href: '/docs/runtime/resource-metering' },
      { title: 'Async Tokio Driver Pipelines', href: '/docs/runtime/async-pipelines' },
      { title: 'Lock-Free SPSC Ring-Buffers', href: '/docs/runtime/spsc-ringbuffers', badge: 'Zero-Copy' },
      { title: 'Inter-Driver Zero-Copy IPC', href: '/docs/runtime/inter-driver-ipc' },
    ],
  },
  {
    title: '5. Rivun Cloud & Operator Station',
    icon: 'Cloud',
    items: [
      { title: 'Sovereign Key Isolation Invariant', href: '/docs/cloud/sovereign-architecture', badge: 'Zero-Trust' },
      { title: 'Operator Workstation (rivun-control)', href: '/docs/cloud/operator-workstation' },
      { title: 'Zero-Trust Policy Staging & Signing', href: '/docs/cloud/policy-lifecycle' },
      { title: 'Edge Daemon (rivun-cloud-bridge)', href: '/docs/cloud/edge-bridge-daemon' },
      { title: 'Multi-Tenant REST & SSE API', href: '/docs/cloud/rest-sse-api' },
      { title: 'Rivun Dashboard Integration', href: '/docs/cloud/dashboard-integration' },
    ],
  },
  {
    title: '6. 26 Crate API Reference',
    icon: 'Package',
    items: [
      { title: 'rivun-core (Wire Protocol)', href: '/docs/crates/rivun-core' },
      { title: 'rivun-crypto (Ed25519 & BLAKE3)', href: '/docs/crates/rivun-crypto' },
      { title: 'rivun-envelope (ZENV Envelopes)', href: '/docs/crates/rivun-envelope' },
      { title: 'rivun-agent (Provenance & Intents)', href: '/docs/crates/rivun-agent' },
      { title: 'rivun-capability (Permission Trees)', href: '/docs/crates/rivun-capability' },
      { title: 'rivun-cli (CLI Tooling)', href: '/docs/crates/rivun-cli' },
      { title: 'rivun-cloud-api (Axum 0.8 Server)', href: '/docs/crates/rivun-cloud-api' },
      { title: 'rivun-cloud-bridge (Edge Sidecar)', href: '/docs/crates/rivun-cloud-bridge' },
      { title: 'rivun-driver-sdk (WASM Driver SDK)', href: '/docs/crates/rivun-driver-sdk' },
      { title: 'rivun-gateway (MCP & WebSocket Gateway)', href: '/docs/crates/rivun-gateway' },
      { title: 'rivun-journal (ZJSEG001 Disk Log)', href: '/docs/crates/rivun-journal' },
      { title: 'rivun-ledger (Receipts & MMR)', href: '/docs/crates/rivun-ledger' },
      { title: 'rivun-machine (Device Adapters)', href: '/docs/crates/rivun-machine' },
      { title: 'rivun-memory (Hash-Chained Journal)', href: '/docs/crates/rivun-memory' },
      { title: 'rivun-net (UDP, Gossip & BFT)', href: '/docs/crates/rivun-net' },
      { title: 'rivun-node (Daemon Actor & Mesh)', href: '/docs/crates/rivun-node' },
      { title: 'rivun-ops (Governance & Multi-Sig)', href: '/docs/crates/rivun-ops' },
      { title: 'rivun-pack (.zpack Bundler & Audit)', href: '/docs/crates/rivun-pack' },
      { title: 'rivun-pact (Conditional Contracts)', href: '/docs/crates/rivun-pact' },
      { title: 'rivun-policy (Rule Engine & Gates)', href: '/docs/crates/rivun-policy' },
      { title: 'rivun-router (Deterministic Routing)', href: '/docs/crates/rivun-router' },
      { title: 'rivun-runtime (Wasmtime & Ring-Buffers)', href: '/docs/crates/rivun-runtime' },
      { title: 'rivun-schema (ZENV Schema Validation)', href: '/docs/crates/rivun-schema' },
      { title: 'rivun-store (Driver Registry)', href: '/docs/crates/rivun-store' },
      { title: 'rivun-telemetry (7-Point Fleet Doctor)', href: '/docs/crates/rivun-telemetry' },
      { title: 'rivun-control (Tauri Workstation)', href: '/docs/crates/rivun-control' },
    ],
  },
  {
    title: '7. 4 SDK Developer Manuals',
    icon: 'Code',
    items: [
      { title: 'Rust SDK Developer Manual', href: '/docs/sdks/rust', badge: 'Rust' },
      { title: 'TypeScript SDK Developer Manual', href: '/docs/sdks/typescript', badge: 'TS' },
      { title: 'Python SDK Developer Manual', href: '/docs/sdks/python', badge: 'Python' },
      { title: 'Go SDK Developer Manual', href: '/docs/sdks/go', badge: 'Go' },
      { title: 'Cross-SDK Conformance Matrix', href: '/docs/sdks/conformance-matrix', badge: '11 Fixtures' },
    ],
  },
  {
    title: '8. 7 Domain Packs & RivunStore',
    icon: 'Store',
    items: [
      { title: 'Domain Pack Architecture & Manifests', href: '/docs/domain-packs/architecture' },
      { title: 'Pack Lifecycle & CLI Commands', href: '/docs/domain-packs/lifecycle' },
      { title: 'Pack 1: Agentic Development', href: '/docs/domain-packs/agentic-dev' },
      { title: 'Pack 2: Smart Building Automation', href: '/docs/domain-packs/smart-building' },
      { title: 'Pack 3: Cloud & Infrastructure Ops', href: '/docs/domain-packs/cloud-ops' },
      { title: 'Pack 4: Industrial Control & SCADA', href: '/docs/domain-packs/industrial' },
      { title: 'Pack 5: Personal AI Assistant', href: '/docs/domain-packs/personal-ai' },
      { title: 'Pack 6: Healthcare & Patient Care', href: '/docs/domain-packs/healthcare' },
      { title: 'Pack 7: Financial Services & Trading', href: '/docs/domain-packs/finance' },
      { title: 'RivunStore Bundle Publishing', href: '/docs/domain-packs/rivunstore-publishing' },
    ],
  },
  {
    title: '9. Fleet Doctor & MMR Forensics',
    icon: 'ShieldCheck',
    items: [
      { title: '7-Point Fleet Doctor Diagnostics', href: '/docs/operations/fleet-doctor', badge: '7 Checks' },
      { title: 'Incident Forensics & Secret Redaction', href: '/docs/operations/incident-forensics' },
      { title: 'MMR Offline Proof Verification', href: '/docs/operations/mmr-offline-verification' },
      { title: '7-Stage Causal Provenance Graph', href: '/docs/operations/provenance-reconstruction' },
    ],
  },
  {
    title: '10. Interactive Sandboxes & Tools',
    icon: 'PlayCircle',
    items: [
      { title: 'Live Wire Frame Sandbox', href: '/sandbox', badge: 'Live Tool' },
      { title: 'Proof-of-Action Quorum Calculator', href: '/sandbox/poa-quorum', badge: 'Live Tool' },
      { title: 'PACT Record Canonicalizer', href: '/sandbox/pact', badge: 'Live Tool' },
      { title: 'Rivun Cloud REST API Explorer', href: '/api-explorer', badge: 'Live Tool' },
    ],
  },
];

export function getAllNavItems(): NavItem[] {
  return DOCS_NAVIGATION.flatMap((section) => section.items);
}

export function findPrevNextNav(currentPath: string): { prev?: NavItem; next?: NavItem } {
  const allItems = getAllNavItems();
  const currentIndex = allItems.findIndex((item) => item.href === currentPath);
  if (currentIndex === -1) {
    return {};
  }
  return {
    prev: currentIndex > 0 ? allItems[currentIndex - 1] : undefined,
    next: currentIndex < allItems.length - 1 ? allItems[currentIndex + 1] : undefined,
  };
}
