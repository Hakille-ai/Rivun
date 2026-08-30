import { DocPage } from '../types';

export const GETTING_STARTED_DOCS: DocPage[] = [
  {
    slug: ['getting-started', 'overview'],
    path: '/docs/getting-started/overview',
    title: 'Overview & Value Proposition',
    description: 'Introduction to Rivun (ZAP) — the zero-trust, high-throughput autonomous automation and consensus protocol.',
    section: '1. Getting Started',
    subSection: 'Fundamentals',
    headings: [
      { id: 'what-is-rivun', text: 'What is Rivun?', level: 2 },
      { id: 'core-tenets', text: 'Core Architecture Tenets', level: 2 },
      { id: 'the-zap-ecosystem', text: 'The ZAP Protocol Ecosystem', level: 2 },
      { id: 'high-level-flow', text: 'High-Level Action Flow', level: 2 },
    ],
    callouts: [
      {
        type: 'invariant',
        title: 'Zero-Trust Sovereign Invariant',
        content: 'Private keys NEVER leave local operator workstations or edge nodes. The cloud SaaS plane operates strictly as an untrusted metadata and relay coordinator.',
      },
    ],
    multiLangSnippets: [
      {
        id: 'quick-connect',
        snippets: {
          rust: {
            title: 'main.rs',
            code: `use rivun_core::{RivunFrame, RivunFlags};\nuse rivun_crypto::Keypair;\n\n#[tokio::main]\nasync fn main() -> Result<(), Box<dyn std::error::Error>> {\n    let keypair = Keypair::generate();\n    let frame = RivunFrame::builder()\n        .flags(RivunFlags::SIGNED | RivunFlags::REQUIRES_CONSENSUS)\n        .source(keypair.node_id())\n        .target([0u8; 16]) // Broadcast\n        .payload(b"ZAP_ACTION_HELLO")\n        .sign(&keypair)?;\n    println!("Created signed frame: len={}", frame.encoded_len());\n    Ok(())\n}`,
          },
          typescript: {
            title: 'index.ts',
            code: `import { RivunClient, RivunFlags } from '@rivun/sdk';\n\nconst client = new RivunClient({ endpoint: 'udp://127.0.0.1:9001' });\nawait client.connect();\n\nconst receipt = await client.dispatchAction({\n  subject: 'agent.action.execute',\n  payload: { action: 'HEATER_ON', room: 'lab-4' },\n  flags: RivunFlags.SIGNED | RivunFlags.REQUIRES_CONSENSUS,\n});\nconsole.log('Action receipt verified:', receipt.receiptHash);`,
          },
          python: {
            title: 'client.py',
            code: `from rivun_sdk import RivunClient, RivunFlags\n\nclient = RivunClient(host="127.0.0.1", port=9001)\nclient.connect()\n\nreceipt = client.dispatch_action(\n    subject="agent.action.execute",\n    payload={"action": "HEATER_ON", "room": "lab-4"},\n    flags=RivunFlags.SIGNED | RivunFlags.REQUIRES_CONSENSUS\n)\nprint(f"Receipt Hash: {receipt.receipt_hash}")`,
          },
          go: {
            title: 'main.go',
            code: `package main\n\nimport (\n\t"fmt"\n\t"github.com/rivun/rivun/sdks/go/pkg/rivun"\n)\n\nfunc main() {\n\tclient, err := rivun.NewClient("127.0.0.1:9001")\n\tif err != nil {\n\t\tpanic(err)\n\t}\n\treceipt, err := client.DispatchAction("agent.action.execute", []byte(\`{"action":"HEATER_ON"}\`))\n\tif err != nil {\n\t\tpanic(err)\n\t}\n\tfmt.Printf("Receipt Root: %x\\n", receipt.RootHash)\n}`,
          },
          bash: {
            title: 'CLI',
            code: `# Install and verify CLI\nrivun --version\n\n# Generate operator keypair\nrivun keygen --out ~/.rivun/operator_key.json\n\n# Launch 3-node in-memory cluster\nrivun cluster up --nodes 3\n\n# Check cluster health\nrivun doctor cluster`,
          },
        },
      },
    ],
    rawContent: `
Rivun (ZAP protocol) is the world's first **zero-trust, cryptographically verified orchestration and consensus runtime** built specifically for autonomous AI agents, industrial edge nodes, and multi-party decentralized automation.

### Key Capabilities
- **Deterministic WASM Sandboxing**: Run untrusted action drivers with fuel metering, memory limits (16MB), and strict epoch timeouts via Wasmtime.
- **Proof-of-Action (PoA) BFT Consensus**: Achieve sub-millisecond BFT quorum consensus ($T \le N$) without proof-of-work energy waste or token staking friction.
- **Durable Merkle Mountain Range (MMR) Receipts**: Every physical or digital action produces an Ed25519-signed action receipt anchored into an append-only cryptographic MMR log.
- **Zero-Trust Sovereign Operator Station**: Private keys remain isolated on local operator workstations; policy changes require explicit human-in-the-loop offline signatures before deployment.
- **Universal Multi-Language Support**: Official first-class SDKs for **Rust**, **TypeScript**, **Python**, and **Go** with 100% bit-for-bit wire compatibility verified via shared test fixtures.
    `,
  },
  {
    slug: ['getting-started', 'installation'],
    path: '/docs/getting-started/installation',
    title: 'Installation & Prerequisites',
    description: 'System requirements, binary installation, Cargo installation, and environment configuration.',
    section: '1. Getting Started',
    subSection: 'Setup',
    headings: [
      { id: 'system-requirements', text: 'System Requirements', level: 2 },
      { id: 'installing-cli', text: 'Installing the Rivun CLI', level: 2 },
      { id: 'installing-sdks', text: 'Installing Language SDKs', level: 2 },
      { id: 'verifying-installation', text: 'Verifying Your Installation', level: 2 },
    ],
    callouts: [
      {
        type: 'tip',
        title: 'Rust Toolchain Recommendation',
        content: 'For building WASM drivers and Rust SDK projects, use Rust 1.80+ with the wasm32-wasip1 target installed via `rustup target add wasm32-wasip1`.',
      },
    ],
    multiLangSnippets: [
      {
        id: 'install-all',
        snippets: {
          bash: {
            title: 'CLI & Cargo',
            code: `# Install precompiled binary via curl\ncurl -proto '=https' --tlsv1.2 -sSf https://get.rivun.dev | sh\n\n# Or install from source with Cargo\ncargo install --path crates/rivun-cli\n\n# Add WASM target for driver compilation\nrustup target add wasm32-wasip1`,
          },
          rust: {
            title: 'Cargo.toml',
            code: `[dependencies]\nrivun-core = "0.1.0"\nrivun-crypto = "0.1.0"\nrivun-envelope = "0.1.0"\nrivun-net = "0.1.0"\nrivun-ledger = "0.1.0"`,
          },
          typescript: {
            title: 'package.json',
            code: `npm install @rivun/sdk @noble/ed25519 @noble/hashes`,
          },
          python: {
            title: 'pip',
            code: `pip install rivun-sdk\n# Or with cryptographic acceleration:\npip install "rivun-sdk[crypto]"`,
          },
          go: {
            title: 'go.mod',
            code: `go get github.com/rivun/rivun/sdks/go`,
          },
        },
      },
    ],
    rawContent: `
### System Requirements
Rivun nodes and SDKs are cross-platform and support:
- **Operating Systems**: Linux (x86_64, aarch64), macOS (Apple Silicon / Intel), Windows 10/11 (x86_64).
- **Memory**: Minimum 512 MB RAM per node (2 GB recommended for high-throughput journals).
- **Storage**: Minimum 100 MB disk space for binaries + WAL storage.
- **Networking**: UDP ports (default \`9001-9003\`) and HTTP/SSE ports (default \`8080\`).

### Environment Variables
| Variable | Description | Default |
|---|---|---|
| \`RIVUN_HOME\` | Configuration directory | \`~/.rivun\` |
| \`RIVUN_LOG\` | Logging level (\`trace\`, \`debug\`, \`info\`, \`warn\`, \`error\`) | \`info\` |
| \`RIVUN_BIND_ADDR\` | Local UDP bind interface | \`0.0.0.0:9001\` |
| \`RIVUN_PEERS\` | Comma-separated list of seed peers | \`""\` |
    `,
  },
  {
    slug: ['getting-started', 'cluster-quickstart'],
    path: '/docs/getting-started/cluster-quickstart',
    title: 'Local 3-Node Cluster Quickstart',
    description: 'Bootstrap a 3-node Proof-of-Action BFT consensus cluster in 30 seconds using the Rivun CLI.',
    section: '1. Getting Started',
    subSection: 'Quickstart',
    headings: [
      { id: 'cluster-init', text: 'Initializing Cluster Nodes', level: 2 },
      { id: 'launching-nodes', text: 'Starting the Swarm', level: 2 },
      { id: 'verifying-consensus', text: 'Testing Consensus & Action Receipts', level: 2 },
      { id: 'inspecting-receipts', text: 'Inspecting the Merkle Mountain Range (MMR)', level: 2 },
    ],
    callouts: [
      {
        type: 'important',
        title: 'Quorum Threshold (T <= N)',
        content: 'In a 3-node cluster, the BFT quorum threshold is T = 2. Actions requiring consensus succeed as long as at least 2 nodes attest to the action digest.',
      },
    ],
    rawContent: `
### 1. Launch 3-Node In-Memory Cluster
The \`rivun cluster up\` command spawns 3 isolated daemon actors with automatic loopback networking, ephemeral journals, and pre-funded validator keys:

\`\`\`bash
rivun cluster up --nodes 3 --daemon
\`\`\`

Expected output:
\`\`\`text
[+] Node 1 started: ID=d8f1e09a-4c22-4819-bf91-30912384a101 (UDP :9001) [LEADER]
[+] Node 2 started: ID=a13c907b-8910-412e-9d21-998811223344 (UDP :9002) [VALIDATOR]
[+] Node 3 started: ID=ef449911-3322-4455-6677-8899aabbccdd (UDP :9003) [VALIDATOR]
[✓] BFT Swarm Mesh Formed: N=3, Quorum Threshold T=2
\`\`\`

### 2. Check Swarm Health
Inspect live mesh topology, round-trip ping latencies, and vector clock anti-entropy status:

\`\`\`bash
rivun cluster status
\`\`\`

### 3. Dispatch Consensus Action Frame
Send a signed action requiring Proof-of-Action quorum:

\`\`\`bash
rivun action dispatch \\
  --subject "hvac.temperature.set" \\
  --payload '{"zone": "datacenter-1", "target_temp_c": 21.5}' \\
  --require-poa
\`\`\`

The cluster collects 2-of-3 validator signatures, executes the sandboxed WASM driver, attaches the \`ZPOA\` trailer, appends the action receipt to the local \`.zjseg\` journal, and anchors the leaf into the \`.zmmr\` Merkle Mountain Range.
    `,
  },
  {
    slug: ['getting-started', 'rust-quickstart'],
    path: '/docs/getting-started/rust-quickstart',
    title: 'Quickstart: Rust SDK',
    description: 'Build high-performance, memory-safe Rivun applications using the native Rust SDK.',
    section: '1. Getting Started',
    subSection: 'SDK Quickstarts',
    headings: [
      { id: 'cargo-setup', text: 'Cargo Dependencies', level: 2 },
      { id: 'keypair-and-frame', text: 'Generating Keys & Building Frames', level: 2 },
      { id: 'sending-datagrams', text: 'Sending Encrypted Datagrams', level: 2 },
      { id: 'verifying-receipts', text: 'Verifying Action Receipts & MMR Proofs', level: 2 },
    ],
    callouts: [
      {
        type: 'tip',
        title: 'Zero-Copy Parsing',
        content: 'Use `RivunFrame::from_bytes(&buf)` and `RivunEnvelopeRef::from_bytes(&buf)` for sub-microsecond zero-copy deserialization directly from network buffers.',
      },
    ],
    multiLangSnippets: [
      {
        id: 'rust-full-sample',
        snippets: {
          rust: {
            title: 'src/main.rs',
            code: `use rivun_core::{RivunFrame, RivunFlags, now_micros};\nuse rivun_crypto::{Keypair, sign_frame, verify_frame};\nuse rivun_envelope::{RivunEnvelope, RivunMessageKind};\n\n#[tokio::main]\nasync fn main() -> Result<(), Box<dyn std::error::Error>> {\n    // 1. Generate Ed25519 node identity keypair\n    let keypair = Keypair::generate();\n    let node_id = keypair.node_id();\n    println!("Local Node ID: {}", node_id);\n\n    // 2. Build structured ZENV envelope\n    let envelope = RivunEnvelope::builder()\n        .kind(RivunMessageKind::Action)\n        .subject("scada.valve.emergency_shutoff")\n        .content_type("application/json")\n        .body(br#"{"valve_id": "V-102", "pressure_psi": 840.5}"#)\n        .build()?;\n\n    // 3. Encapsulate inside 64-byte wire frame with signature\n    let mut frame = RivunFrame::builder()\n        .flags(RivunFlags::SIGNED | RivunFlags::REQUIRES_CONSENSUS)\n        .source(node_id)\n        .target([0u8; 16]) // Broadcast to all swarm validators\n        .timestamp(now_micros())\n        .payload(envelope.as_bytes())\n        .build()?;\n\n    // 4. Attach Ed25519 ZSIG authentication trailer\n    sign_frame(&mut frame, &keypair)?;\n\n    // 5. Verify cryptographic validity\n    assert!(verify_frame(&frame, &keypair.public_key())?);\n    println!("Frame successfully signed and verified! Total len: {} bytes", frame.encoded_len());\n\n    Ok(())\n}`,
          },
        },
      },
    ],
    rawContent: `
The **Rust SDK** provides zero-overhead, bare-metal protocol primitives, SIMD-accelerated BLAKE3 hashing, constant-time Ed25519 signing via \`ed25519-dalek\`, and native Tokio async networking.
    `,
  },
  {
    slug: ['getting-started', 'typescript-quickstart'],
    path: '/docs/getting-started/typescript-quickstart',
    title: 'Quickstart: TypeScript SDK',
    description: 'Integrate browser and Node.js applications with the Rivun protocol using TypeScript.',
    section: '1. Getting Started',
    subSection: 'SDK Quickstarts',
    headings: [
      { id: 'npm-install', text: 'Installation', level: 2 },
      { id: 'node-client', text: 'Initializing Client & Ed25519 Keypair', level: 2 },
      { id: 'zenv-builder', text: 'Building ZENV Envelopes', level: 2 },
      { id: 'pact-verification', text: 'Verifying PACT Receipts in Browser', level: 2 },
    ],
    multiLangSnippets: [
      {
        id: 'ts-quickstart',
        snippets: {
          typescript: {
            title: 'src/index.ts',
            code: `import { \n  RivunKeypair, \n  RivunEnvelope, \n  RivunMessageKind, \n  RivunWireFrame, \n  RivunFlags \n} from '@rivun/sdk';\n\n// 1. Generate or import Ed25519 keypair\nconst keypair = RivunKeypair.generate();\nconsole.log('Node ID:', keypair.nodeId);\n\n// 2. Construct Universal Envelope (ZENV)\nconst envelope = RivunEnvelope.create({\n  kind: RivunMessageKind.Action,\n  subject: 'robotics.arm.move',\n  contentType: 'application/json',\n  body: JSON.stringify({ x: 120.5, y: 45.2, z: 88.0, speed: 0.8 }),\n});\n\n// 3. Encode into 64-byte wire frame with ZSIG trailer\nconst wireFrame = RivunWireFrame.encode({\n  flags: RivunFlags.SIGNED | RivunFlags.PRIORITY,\n  sourceNode: keypair.nodeId,\n  targetNode: '00000000-0000-0000-0000-000000000000',\n  payload: envelope.toBytes(),\n  keypair,\n});\n\nconsole.log('Encoded Wire Frame Bytes:', wireFrame.byteLength);`,
          },
        },
      },
    ],
    rawContent: `
The **TypeScript SDK** runs identically in Node.js (v18+), Bun, Deno, and modern web browsers with pure WebCrypto and \`@noble/ed25519\` cryptography.
    `,
  },
  {
    slug: ['getting-started', 'python-quickstart'],
    path: '/docs/getting-started/python-quickstart',
    title: 'Quickstart: Python SDK',
    description: 'Python 3.10+ SDK with dataclasses, type annotations, and standard UDP transport.',
    section: '1. Getting Started',
    subSection: 'SDK Quickstarts',
    headings: [
      { id: 'pip-install', text: 'Installation', level: 2 },
      { id: 'python-code', text: 'Python Client Example', level: 2 },
      { id: 'pact-signing', text: 'Signing PACT Records', level: 2 },
    ],
    multiLangSnippets: [
      {
        id: 'py-sample',
        snippets: {
          python: {
            title: 'quickstart.py',
            code: `from rivun_sdk import Keypair, RivunEnvelope, RivunMessageKind, RivunFrame, RivunFlags\nimport json\n\n# 1. Generate Ed25519 keypair\nkeypair = Keypair.generate()\nprint(f"Node UUID: {keypair.node_id}")\n\n# 2. Build ZENV envelope\nenvelope = RivunEnvelope(\n    kind=RivunMessageKind.ACTION,\n    subject="ai.agent.intent.propose",\n    content_type="application/json",\n    body=json.dumps({"task": "audit_repo", "target": "crates/rivun-core"}).encode("utf-8")\n)\n\n# 3. Create 64-byte wire frame with signature\nframe = RivunFrame.builder() \\\n    .flags(RivunFlags.SIGNED | RivunFlags.REQUIRES_CONSENSUS) \\\n    .source(keypair.node_id) \\\n    .payload(envelope.encode()) \\\n    .sign(keypair)\n\nprint(f"Encoded frame: {len(frame.bytes())} bytes")`,
          },
        },
      },
    ],
    rawContent: `
The **Python SDK** is built for AI agent frameworks (LangChain, AutoGen, CrewAI), data science pipelines, and edge Python scripts.
    `,
  },
  {
    slug: ['getting-started', 'go-quickstart'],
    path: '/docs/getting-started/go-quickstart',
    title: 'Quickstart: Go SDK',
    description: 'Idiomatic Go client with BLAKE3 hashing, Ed25519 verification, and binary serialization.',
    section: '1. Getting Started',
    subSection: 'SDK Quickstarts',
    headings: [
      { id: 'go-get', text: 'Go Module Setup', level: 2 },
      { id: 'go-example', text: 'Encoding & Signing Frames in Go', level: 2 },
    ],
    multiLangSnippets: [
      {
        id: 'go-sample',
        snippets: {
          go: {
            title: 'main.go',
            code: `package main\n\nimport (\n\t"fmt"\n\t"github.com/rivun/rivun/sdks/go/pkg/rivun"\n)\n\nfunc main() {\n\t// Generate keypair\n\tkp, err := rivun.GenerateKeypair()\n\tif err != nil {\n\t\tpanic(err)\n\t}\n\tfmt.Printf("Node ID: %s\\n", kp.NodeID())\n\n\t// Create envelope\n\tenv := rivun.NewEnvelope(\n\t\trivun.KindAction,\n\t\t"sensor.telemetry.batch",\n\t\t"application/json",\n\t\t[]byte(\`{"temp": 24.2, "humidity": 55.1}\`),\n\t)\n\n\t// Build wire frame\n\tframe, err := rivun.NewFrameBuilder().\n\t\tSetFlags(rivun.FlagSigned | rivun.FlagPriority).\n\t\tSetSource(kp.NodeIDBytes()).\n\t\tSetPayload(env.Encode()).\n\t\tSign(kp)\n\tif err != nil {\n\t\tpanic(err)\n\t}\n\n\tfmt.Printf("Frame generated: %d bytes\\n", len(frame.Bytes()))\n}`,
          },
        },
      },
    ],
    rawContent: `
The **Go SDK** delivers high-concurrency microservices integration, Kubernetes operators, and industrial gateway controllers.
    `,
  },
];
