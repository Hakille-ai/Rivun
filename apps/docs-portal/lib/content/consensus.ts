import { DocPage } from '../types';

export const CONSENSUS_DOCS: DocPage[] = [
  {
    slug: ['consensus', 'poa-model'],
    path: '/docs/consensus/poa-model',
    title: 'Proof-of-Action (PoA) Consensus Model',
    description: 'Cryptographic action verification without proof-of-work energy waste or token staking friction.',
    section: '3. Consensus & Quorum Mesh',
    subSection: 'Consensus Engine',
    headings: [
      { id: 'why-poa', text: 'Why Proof-of-Action?', level: 2 },
      { id: 'poa-lifecycle', text: 'Action Verification Lifecycle', level: 2 },
      { id: 'quorum-thresholds', text: 'Threshold Mathematical Bounds', level: 2 },
    ],
    callouts: [
      {
        type: 'important',
        title: 'BFT Quorum Mathematical Guarantee',
        content: 'For a swarm of N validators, the Byzantine Fault Tolerant quorum threshold is T = floor(2N/3) + 1. The swarm tolerates up to F = floor((N-1)/3) faulty or malicious nodes.',
      },
    ],
    rawContent: `
### The Proof-of-Action (PoA) Paradigm
Traditional blockchains rely on **Proof-of-Work (PoW)** which wastes immense energy, or **Proof-of-Stake (PoS)** which creates plutocratic staking centralization.

**Proof-of-Action (PoA)** directly verifies the **cryptographic intent, capability authorization, simulation outcome, and execution correctness** of discrete actions before state commitment.

$$\\text{Quorum Threshold: } T = \\left\\lfloor \\frac{2N}{3} \\right\\rfloor + 1$$
$$\\text{Fault Tolerance: } F = \\left\\lfloor \\frac{N - 1}{3} \\right\\rfloor$$
    `,
  },
  {
    slug: ['consensus', 'bft-consensus'],
    path: '/docs/consensus/bft-consensus',
    title: 'BFT Swarm Consensus Engine',
    description: 'Two-phase commit state machine: Propose, Prevote, Precommit, and Commit phases.',
    section: '3. Consensus & Quorum Mesh',
    subSection: 'Consensus Engine',
    headings: [
      { id: 'state-machine-phases', text: '2-Phase State Machine Phases', level: 2 },
      { id: 'phase-1-propose', text: '1. Proposal Dissemination', level: 2 },
      { id: 'phase-2-prevote', text: '2. Prevote Collection', level: 2 },
      { id: 'phase-3-precommit', text: '3. Precommit Certificate', level: 2 },
      { id: 'phase-4-commit', text: '4. Final Execution & Receipt Commit', level: 2 },
    ],
    rawContent: `
### The 4 Consensus States
1. **PROPOSE**: Leader broadcasts proposal containing action hash and execution gas budget.
2. **PREVOTE**: Validators verify policy rules, dry-run WASM bytecode, and sign a \`SwarmVote::Prevote\`.
3. **PRECOMMIT**: Upon collecting $\\ge T$ prevotes, leader assembles a \`PrecommitCertificate\` and requests final signatures.
4. **COMMIT**: Once $\\ge T$ precommits arrive, nodes execute action, write receipt to MMR, and attach \`ZPOA\` trailer.
    `,
  },
  {
    slug: ['consensus', 'threshold-signatures'],
    path: '/docs/consensus/threshold-signatures',
    title: 'Dynamic Threshold Signatures (ZPOA)',
    description: '40 + 80*K byte ZPOA trailer format, multi-validator attestation certificates, and aggregate verification.',
    section: '3. Consensus & Quorum Mesh',
    subSection: 'Cryptographic Quorum',
    headings: [
      { id: 'zpoa-trailer-layout', text: 'ZPOA Binary Layout', level: 2 },
      { id: 'attestation-records', text: 'Validator Attestation Records', level: 2 },
      { id: 'aggregate-verification', text: 'Batch Signature Verification', level: 2 },
    ],
    rawContent: `
### ZPOA Binary Layout ($40 + 80 \\times K$ Bytes)

| Offset | Field | Size | Description |
|---|---|---|---|
| \`0x00\` | \`magic\` | 4 bytes | Fixed \`0x5A504F41\` (\`ZPOA\`) |
| \`0x04\` | \`version\` | 2 bytes | Version (\`0x0001\`) |
| \`0x06\` | \`threshold\` | 2 bytes | Required threshold $T$ |
| \`0x08\` | \`attestation_count\` | 2 bytes | Count $K \\ge T$ |
| \`0x0A\` | \`reserved\` | 2 bytes | Reserved \`0x0000\` |
| \`0x0C\` | \`frame_digest\` | 32 bytes | BLAKE3 frame digest |
| \`0x2C\` | \`attestations\` | $80 \\times K$ bytes | Array of (16-byte UUID + 64-byte Ed25519 signature) |
    `,
  },
  {
    slug: ['consensus', 'gossip-protocol'],
    path: '/docs/consensus/gossip-protocol',
    title: 'Swarm Gossip & Anti-Entropy',
    description: 'Epidemic dissemination with k-fanout, bloom filter deduplication cache, peer exchange (PEX), and vector clock reconciliation.',
    section: '3. Consensus & Quorum Mesh',
    subSection: 'Networking Mesh',
    headings: [
      { id: 'epidemic-dissemination', text: 'Epidemic Dissemination ($k$-fanout)', level: 2 },
      { id: 'bloom-cache', text: 'Bloom Filter Deduplication Cache', level: 2 },
      { id: 'vector-clocks', text: 'Vector Clock Anti-Entropy', level: 2 },
    ],
    rawContent: `
Rivun employs a robust **Epidemic Gossip Mesh** with configurable fanout ($k=3$), ensuring message propagation in $O(\\log N)$ rounds across thousands of geo-distributed edge nodes.
    `,
  },
  {
    slug: ['consensus', 'mesh-failover'],
    path: '/docs/consensus/mesh-failover',
    title: 'Network Partition & Failover Mesh',
    description: 'Phi Accrual failure detection, heartbeat jitter, split-brain partition mitigation, dynamic 2-hop relay routing.',
    section: '3. Consensus & Quorum Mesh',
    subSection: 'Resilience',
    headings: [
      { id: 'phi-accrual', text: 'Phi Accrual Failure Detector', level: 2 },
      { id: 'partition-mitigation', text: 'Partition Isolation & Safe Halting', level: 2 },
      { id: 'relay-routing', text: '2-Hop Relay Routing', level: 2 },
    ],
    rawContent: `
The **Phi Accrual Failure Detector** continuously monitors peer heartbeat intervals, computing a dynamic suspicion score $\\Phi$. If $\\Phi > 8.0$, peers are marked suspect; if $\\Phi > 12.0$, automatic failover reroutes traffic via 2-hop relay nodes.
    `,
  },
  {
    slug: ['consensus', 'slashing-disputes'],
    path: '/docs/consensus/slashing-disputes',
    title: 'Equivocation Detection & Slashing',
    description: 'Automated cryptographic proof of double-voting, Byzantine validator quarantine, and PACT escrow slashing.',
    section: '3. Consensus & Quorum Mesh',
    subSection: 'Dispute Resolution',
    headings: [
      { id: 'equivocation-proofs', text: 'Equivocation Proof Construction', level: 2 },
      { id: 'quarantine-action', text: 'Automated Quarantine & Eviction', level: 2 },
      { id: 'escrow-slashing', text: 'PACT Escrow Slashing Execution', level: 2 },
    ],
    rawContent: `
If a validator signs two conflicting proposals at the same height or epoch, any peer can submit an **Equivocation Proof** containing both signed frames. The network verifies both signatures against the validator public key and executes immediate quarantine and escrow penalty.
    `,
  },
];
