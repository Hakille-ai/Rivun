import { DocPage } from '../types';

export const ARCHITECTURE_DOCS: DocPage[] = [
  {
    slug: ['architecture', 'overview'],
    path: '/docs/architecture/overview',
    title: 'Protocol Architecture & Layering',
    description: 'Deep dive into the 5-layer Rivun/ZAP protocol architecture from wire frames to multi-party PACT contracts.',
    section: '2. Architecture & Core Protocol',
    subSection: 'Protocol Core',
    headings: [
      { id: 'architectural-layers', text: 'Architectural Layer Model', level: 2 },
      { id: 'layer-1-wire', text: 'Layer 1: Wire & Cryptographic Framing', level: 2 },
      { id: 'layer-2-envelope', text: 'Layer 2: Universal Messaging Envelope (ZENV)', level: 2 },
      { id: 'layer-3-consensus', text: 'Layer 3: BFT Consensus & Quorum Mesh', level: 2 },
      { id: 'layer-4-runtime', text: 'Layer 4: Sandboxed WASM & Streaming Runtime', level: 2 },
      { id: 'layer-5-sovereign', text: 'Layer 5: Sovereign Multi-Party PACT Contracts', level: 2 },
    ],
    callouts: [
      {
        type: 'invariant',
        title: 'Cryptographic Provenance Invariant',
        content: 'Every action must form an unbroken cryptographic link: H(intent) -> H(negotiation) -> H(policy) -> H(consensus) -> H(driver) -> H(poa) -> H(receipt) -> H(root).',
      },
    ],
    rawContent: `
Rivun establishes a strict, mathematically verifiable 5-layer protocol stack designed for high throughput (100k+ ops/sec) and sub-millisecond wire dispatch.

### The 5-Layer Stack

\`\`\`text
+-------------------------------------------------------------+
| Layer 5: Autonomous Multi-Party Contracts (PACT)            |
| (Escrow, Threshold Arbitration, Dispute Proofs, Slashing)   |
+-------------------------------------------------------------+
| Layer 4: Sandboxed Runtime & Zero-Copy Streaming            |
| (Wasmtime ABI v1, Fuel Metering, SPSC Ring Buffers, Modbus) |
+-------------------------------------------------------------+
| Layer 3: Consensus & Epidemic Swarm Mesh                    |
| (Proof-of-Action BFT, Vector Clocks, Phi Accrual Detection) |
+-------------------------------------------------------------+
| Layer 2: Universal Messaging Envelope (ZENV)                |
| (74-byte Zero-Copy Header, 8 Message Kinds, Causation IDs)  |
+-------------------------------------------------------------+
| Layer 1: Wire & Cryptographic Framing (RivunWire)           |
| (64-byte Header, Ed25519 ZSIG, ZPOA, ChaCha20-Poly1305)     |
+-------------------------------------------------------------+
\`\`\`
    `,
  },
  {
    slug: ['architecture', 'wire-format'],
    path: '/docs/architecture/wire-format',
    title: 'Fixed 64-Byte Wire Header Format',
    description: 'Detailed binary layout, byte offsets, bitflags, and parsing invariants of the 64-byte RivunHeader.',
    section: '2. Architecture & Core Protocol',
    subSection: 'Protocol Core',
    headings: [
      { id: 'header-layout', text: 'Binary Header Layout', level: 2 },
      { id: 'field-specifications', text: 'Field Specifications', level: 2 },
      { id: 'bitflag-definitions', text: 'RivunFlags Bitmask Table', level: 2 },
      { id: 'trailers-layout', text: 'Authentication & PoA Trailers', level: 2 },
    ],
    callouts: [
      {
        type: 'important',
        title: 'Endianness and Magic Value',
        content: 'All multibyte integers in the 64-byte wire header are Big-Endian. The magic bytes are strictly 0x5A41505F (ASCII "ZAP_").',
      },
    ],
    rawContent: `
### Binary Layout Specification (64-Byte RivunHeader)

| Byte Range | Field Name | Type | Description |
|---|---|---|---|
| \`00 - 03\` | \`magic\` | \`[u8; 4]\` | Fixed \`0x5A41505F\` (ASCII \`ZAP_\`) |
| \`04 - 05\` | \`version\` | \`u16\` | Protocol version (\`0x0001\` = v1) |
| \`06 - 07\` | \`flags\` | \`u16\` | Bitfield flags (\`RivunFlags\`) |
| \`08 - 23\` | \`source_node\` | \`[u8; 16]\` | 16-byte UUID of sender node |
| \`24 - 39\` | \`target_node\` | \`[u8; 16]\` | 16-byte UUID of recipient (\`00...00\` for broadcast) |
| \`40 - 47\` | \`timestamp_micros\` | \`u64\` | Unix timestamp in microseconds |
| \`48 - 55\` | \`rivun_len\` | \`u64\` | Payload length in bytes (max 16 MiB) |
| \`56 - 63\` | \`rivun_sign\` | \`[u8; 8]\` | 8-byte fast signature hint |

### Bitflag Definitions (\`RivunFlags\`)
- \`ENCRYPTED = 0x0001\` (bit 0): Payload is encrypted with AEAD ChaCha20-Poly1305.
- \`PRIORITY = 0x0002\` (bit 1): Urgent routing bypasses normal queue buffers.
- \`REQUIRES_CONSENSUS = 0x0004\` (bit 2): Swarm must reach BFT quorum ($T \\le N$).
- \`SIGNED = 0x0008\` (bit 3): Frame has attached \`ZSIG\` authentication trailer.
- \`BROADCAST = 0x0010\` (bit 4): Frame addressed to entire mesh network.
    `,
  },
  {
    slug: ['architecture', 'universal-envelope'],
    path: '/docs/architecture/universal-envelope',
    title: 'Universal Envelope (ZENV)',
    description: '74-byte zero-copy ZENV envelope layout, 8 typed message kinds, correlation tracking, and serialization.',
    section: '2. Architecture & Core Protocol',
    subSection: 'Protocol Core',
    headings: [
      { id: 'zenv-header', text: '74-Byte ZENV Binary Header', level: 2 },
      { id: 'message-kinds', text: 'The 8 Typed Message Kinds', level: 2 },
      { id: 'causation-tracking', text: 'Correlation & Causation Tracking', level: 2 },
    ],
    callouts: [
      {
        type: 'tip',
        title: 'Zero-Copy Slicing',
        content: 'ZENV is designed for zero-copy deserialization: header fields can be read by casting raw byte slices without heap allocation.',
      },
    ],
    rawContent: `
The **Universal Envelope** (\`ZENV\`) carries structured messages across 8 discrete message kinds:

### Message Kinds (\`RivunMessageKind\`)
1. \`Data (0x0001)\`: Raw binary sensor readings or telemetry snapshots.
2. \`Event (0x0002)\`: Broadcast notification of a state change in the mesh.
3. \`Command (0x0003)\`: Instruction directing a node to execute an operation.
4. \`Query (0x0004)\`: Read-only request for node or ledger state.
5. \`Response (0x0005)\`: Direct reply to a prior Query or Command.
6. \`StreamChunk (0x0006)\`: Sequenced fragment of a zero-copy streaming ring buffer.
7. \`Action (0x0007)\`: State-mutating operational action subject to policy and PoA.
8. \`Control (0x0008)\`: Low-level mesh management, heartbeat, or peer exchange.
    `,
  },
  {
    slug: ['architecture', 'cryptography'],
    path: '/docs/architecture/cryptography',
    title: 'Cryptographic Signing & Transcripts',
    description: 'Ed25519 signing specifications, BLAKE3 domain separation transcripts, and blinded commitments.',
    section: '2. Architecture & Core Protocol',
    subSection: 'Cryptography',
    headings: [
      { id: 'domain-separators', text: 'BLAKE3 Domain Separator Catalog', level: 2 },
      { id: 'zsig-trailer', text: 'ZSIG Authentication Trailer', level: 2 },
      { id: 'blinded-commitments', text: 'Blinded Commitments & Zero-Knowledge Hashing', level: 2 },
    ],
    callouts: [
      {
        type: 'security',
        title: 'Transcript Domain Separation',
        content: 'All BLAKE3 hashes MUST prepend their designated domain separator to eliminate cross-protocol collision attacks.',
      },
    ],
    rawContent: `
### Domain Separator Strings

| Domain Separator | Purpose |
|---|---|
| \`Rivun-NODE-ID-v1\` | Derives 16-byte UUIDv8 from Ed25519 Public Key |
| \`Rivun-SIGN-HINT-v1\` | Computes 8-byte fast rejection hint |
| \`Rivun-POA-DIGEST-v1\` | Computes frame digest for validator signing |
| \`Rivun-POA-SIGNATURE-v1\` | Transcript prefix for validator PoA attestation |
| \`Rivun-POA-VALIDATOR-SET-v1\` | Transcript for validator quorum reconfiguration |
| \`Rivun-BLINDED-COMMITMENT-v1\` | Blinds action intent with local salt |
| \`Rivun-BLINDED-RECEIPT-v1\` | Blinds receipt hashes for private ZK audits |
| \`Rivun-BATCH-SEAL-v1\` | MMR peak root batch seal signature |
| \`Rivun-PROVENANCE-CHAIN-v1\` | Complete 7-stage causal provenance chain root |
| \`ZAP-PACT-v1\` | PACT deterministic JSON contract signature |
| \`Rivun-POLICY-BUNDLE-v1\` | Operator offline policy bundle signature |
    `,
  },
  {
    slug: ['architecture', 'encrypted-udp'],
    path: '/docs/architecture/encrypted-udp',
    title: 'Encrypted UDP Datagrams (ZAPD)',
    description: 'ChaCha20-Poly1305 AEAD authenticated UDP datagrams, 12-byte nonces, and durable replay protection.',
    section: '2. Architecture & Core Protocol',
    subSection: 'Transport',
    headings: [
      { id: 'zapd-framing', text: 'ZAPD Datagram Layout', level: 2 },
      { id: 'nonce-construction', text: '12-Byte Nonce Construction', level: 2 },
      { id: 'replay-protection', text: 'Durable WAL Replay Protection (ZAPFRM01)', level: 2 },
    ],
    rawContent: `
Every point-to-point UDP packet in Rivun is encapsulated in a **ZAPD** datagram with AEAD ChaCha20-Poly1305 encryption.

### Nonce Structure (12 Bytes)
- Bytes 0-3: 4-byte random session prefix (generated during handshake).
- Bytes 4-11: 8-byte monotonically increasing big-endian packet counter.

### Replay Guard Invariant
Nodes maintain a hybrid in-memory sliding window + disk-backed WAL (\`ZAPFRM01\`) recording observed nonces. Packets with duplicated or stale nonces (>5000ms clock skew) are immediately dropped before decryption.
    `,
  },
  {
    slug: ['architecture', 'noise-handshake'],
    path: '/docs/architecture/noise-handshake',
    title: 'Noise Handshake Protocol',
    description: 'Implementation of Noise_NN_25519_ChaChaPoly_BLAKE2s for mutual peer authentication and key exchange.',
    section: '2. Architecture & Core Protocol',
    subSection: 'Transport',
    headings: [
      { id: 'noise-pattern', text: 'Noise Pattern Specification', level: 2 },
      { id: 'ephemeral-keys', text: 'Ephemeral Key Derivation', level: 2 },
      { id: 'session-rekeying', text: 'Session Rekeying & Forward Secrecy', level: 2 },
    ],
    rawContent: `
Rivun nodes establish secure point-to-point sessions using the \`Noise_NN_25519_ChaChaPoly_BLAKE2s\` pattern, providing forward secrecy and authenticated symmetric session keys without centralized certificate authorities.
    `,
  },
  {
    slug: ['architecture', 'subject-catalog'],
    path: '/docs/architecture/subject-catalog',
    title: 'Control Subject Catalog',
    description: 'Standardized hierarchical subject namespaces for Agent, PACT, Driver, Ledger, and Discovery subjects.',
    section: '2. Architecture & Core Protocol',
    subSection: 'Catalog',
    headings: [
      { id: 'namespace-rules', text: 'Namespace Hierarchy Rules', level: 2 },
      { id: 'agent-subjects', text: 'Agent & Provenance Subjects', level: 2 },
      { id: 'pact-subjects', text: 'PACT & Dispute Subjects', level: 2 },
      { id: 'system-subjects', text: 'System & Discovery Subjects', level: 2 },
    ],
    rawContent: `
### Subject Namespace Hierarchy
- \`agent.intent.propose\`: Initial agent intent proposal.
- \`agent.negotiation.request\`: Capability negotiation between agents.
- \`pact.create\`: Creation of a multi-party conditional contract.
- \`pact.dispute.raise\`: Formal dispute escalation.
- \`scada.driver.execute\`: Industrial actuator execution request.
- \`node.discovery.announce\`: Swarm peer advertisement.
- \`ledger.receipt.commit\`: Signed action receipt broadcast.
    `,
  },
];
