import { DocPage } from '../types';

export const OPERATIONS_DOCS: DocPage[] = [
  {
    slug: ['operations', 'fleet-doctor'],
    path: '/docs/operations/fleet-doctor',
    title: '7-Point Fleet Doctor Diagnostic Suite',
    description: 'Comprehensive 7-point health check suite verifying network, storage, replay WAL, journal rotation, registry, certificate validity, and peer trust.',
    section: '9. Fleet Doctor & MMR Forensics',
    subSection: 'Diagnostics & Operations',
    headings: [
      { id: 'seven-checks', text: 'The 7 Diagnostic Checks', level: 2 },
      { id: 'check-1-network', text: '1. Network Reachability & Sockets', level: 2 },
      { id: 'check-2-storage', text: '2. Storage Mounts & Permissions', level: 2 },
      { id: 'check-3-replay', text: '3. Durable Replay WAL (ZAPFRM01)', level: 2 },
      { id: 'check-4-journal', text: '4. Receipt Journal Manifests (ZJSEG001)', level: 2 },
      { id: 'check-5-registry', text: '5. RivunStore Registry Signatures', level: 2 },
      { id: 'check-6-cert-quorum', text: '6. Node Key & PoA Quorum Threshold (T <= N)', level: 2 },
      { id: 'check-7-peer-trust', text: '7. Peer Trust & Quarantine Status', level: 2 },
      { id: 'running-doctor', text: 'Running Fleet Doctor from CLI', level: 2 },
    ],
    callouts: [
      {
        type: 'important',
        title: 'Diagnostic Fail-Safe',
        content: 'If any of checks 3, 4, 6, or 7 fail, the node automatically enters QUARANTINE mode, refusing to vote in consensus or execute state-mutating actions until resolved.',
      },
    ],
    rawContent: `
The **7-Point Fleet Doctor** provides continuous deep health inspections across every operational subsystem:

### The 7 Core Checks
1. **\`cluster_network_reachability\`**: Tests UDP bind sockets, MTU bounds, packet drop rates, and peer ping latencies (<50ms).
2. **\`storage_mounts_and_permissions\`**: Verifies read/write permissions, available disk space (>1GB), and POSIX lock semantics on \`~/.rivun/data/\`.
3. **\`durable_replay_store_wal\`**: Validates replay WAL integrity (\`ZAPFRM01\` magic header), sequence numbering, and clock skew tolerance ($\\pm 5000$ms).
4. **\`segment_rotation_and_manifest_signatures\`**: Inspects receipt journal rotation (\`ZJSEG001\`) and verifies \`SignedReceiptSegmentManifest\` signatures.
5. **\`rivun_store_index_and_signatures\`**: Validates offline RivunStore registry index manifests against trusted publisher keys.
6. **\`node_identity_key_and_poa_quorum\`**: Confirms local Ed25519 identity key validity, PACT signature threshold, and active PoA quorum availability ($T \\le N$).
7. **\`peer_trust_status\`**: Analyzes swarm topology for quarantined, revoked, or equivocation-banned nodes.
    `,
  },
  {
    slug: ['operations', 'incident-forensics'],
    path: '/docs/operations/incident-forensics',
    title: 'Incident Forensics & Secret Redaction',
    description: 'Capturing diagnostic bundles and scrubbing credentials with compiled regex SecretRedactor.',
    section: '9. Fleet Doctor & MMR Forensics',
    subSection: 'Diagnostics & Operations',
    headings: [
      { id: 'forensic-bundles', text: 'Forensic Snapshot Generation', level: 2 },
      { id: 'secret-redactor-rules', text: 'SecretRedactor Regex Rules', level: 2 },
    ],
    rawContent: `
When an anomaly occurs, operators run \`rivun doctor snapshot --out incident.zip\`. The internal **\`SecretRedactor\`** automatically scrubs API tokens, Ed25519 private keys, AWS credentials, and proprietary payload fields before outputting the diagnostic bundle.
    `,
  },
  {
    slug: ['operations', 'mmr-offline-verification'],
    path: '/docs/operations/mmr-offline-verification',
    title: 'MMR Offline Proof Verification',
    description: 'Mathematical verification of Merkle Mountain Range (MMR) inclusion and exclusion proofs without a network connection.',
    section: '9. Fleet Doctor & MMR Forensics',
    subSection: 'Diagnostics & Operations',
    headings: [
      { id: 'mmr-mathematics', text: 'MMR Peak-Bagging Mathematics', level: 2 },
      { id: 'inclusion-proofs', text: 'Verifying Inclusion Proofs ($O(\\log N)$)', level: 2 },
      { id: 'exclusion-proofs', text: 'Verifying Non-Membership (Exclusion Proofs)', level: 2 },
    ],
    rawContent: `
### Peak Bagging Equation
Given peak hashes $P_1, P_2, \\dots, P_k$ representing balanced subtrees of size $2^{h_i}$, the overall Merkle Mountain Range root is computed via right-to-left peak bagging:

$$\\text{Root} = \\text{BLAKE3}\\left(\\dots \\text{BLAKE3}\\left(\\text{BLAKE3}(P_k \\parallel P_{k-1}) \\parallel P_{k-2}\\right) \\dots \\parallel P_1\\right)$$

Inclusion proofs require at most $O(\\log N)$ hash evaluations, allowing third-party auditors to verify any historical action receipt with zero network access.
    `,
  },
  {
    slug: ['operations', 'provenance-reconstruction'],
    path: '/docs/operations/provenance-reconstruction',
    title: '7-Stage Causal Provenance Graph Reconstruction',
    description: 'Graph reconstruction linking Intent -> Negotiation -> Policy -> Consensus -> Driver -> PoA -> Receipt.',
    section: '9. Fleet Doctor & MMR Forensics',
    subSection: 'Diagnostics & Operations',
    headings: [
      { id: 'graph-reconstruction', text: 'Reconstructing the Causal DAG', level: 2 },
      { id: 'auditing-provenance', text: 'Auditing Provenance with rivun-cli', level: 2 },
    ],
    rawContent: `
The **7-Stage Causal Provenance Graph** guarantees complete non-repudiation for autonomous systems. Every receipt can be traced backward through each stage to identify exactly which AI agent initiated the intent, which policies evaluated it, and which validators signed the quorum certificate.
    `,
  },
];
