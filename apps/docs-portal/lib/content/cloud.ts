import { DocPage } from '../types';

export const CLOUD_DOCS: DocPage[] = [
  {
    slug: ['cloud', 'sovereign-architecture'],
    path: '/docs/cloud/sovereign-architecture',
    title: 'Sovereign Key Isolation Invariant',
    description: 'Zero-trust cryptographic architecture ensuring private keys never touch cloud servers.',
    section: '5. Rivun Cloud & Operator Station',
    subSection: 'Sovereign Architecture',
    headings: [
      { id: 'the-isolation-invariant', text: 'The Private Key Isolation Invariant', level: 2 },
      { id: 'cloud-role', text: 'Role of the Rivun Cloud SaaS Plane', level: 2 },
      { id: 'security-boundaries', text: 'Threat Model & Security Boundaries', level: 2 },
    ],
    callouts: [
      {
        type: 'invariant',
        title: 'Core Architectural Security Guarantee',
        content: 'Even in the event of a catastrophic compromise of the Rivun Cloud SaaS infrastructure, attackers CANNOT sign policies, forge receipts, or execute unauthorized actions on edge nodes because private keys exist only on air-gapped or local operator workstations.',
      },
    ],
    rawContent: `
### Sovereign Identity Model
In traditional enterprise SaaS, cloud providers store API tokens, master keys, and private certificates. If the cloud database is breached, the entire enterprise is compromised.

Rivun flips this model upside-down:
1. **Local Key Vault**: Ed25519 private keys are stored exclusively in \`~/.rivun/operator_keys/\` on the operator's local machine, encrypted with Argon2id + ChaCha20-Poly1305.
2. **Untrusted SaaS Broker**: Rivun Cloud acts merely as a distributed synchronization and telemetry relay. It receives **only cryptographic hashes, Merkle roots, and detached signatures**.
    `,
  },
  {
    slug: ['cloud', 'operator-workstation'],
    path: '/docs/cloud/operator-workstation',
    title: 'Operator Workstation (rivun-control)',
    description: 'Tauri desktop app and CLI managing ~/.rivun/operator_keys/ with local signing approval workflows.',
    section: '5. Rivun Cloud & Operator Station',
    subSection: 'Operator Workstation',
    headings: [
      { id: 'tauri-app', text: 'Desktop Workstation UI & Features', level: 2 },
      { id: 'key-management', text: 'Key Generation & Vault Storage', level: 2 },
      { id: 'signing-workflow', text: 'Human-in-the-Loop Signing Flow', level: 2 },
    ],
    rawContent: `
**\`rivun-control\`** is the sovereign command center for security operators and cluster administrators. Built as a native Rust + Tauri application, it provides:
- Visual diff viewer for staged security policies.
- Hardware security key (YubiKey / PKCS#11) and encrypted file vault support.
- Offline batch signing and cryptographic bundle generation.
    `,
  },
  {
    slug: ['cloud', 'policy-lifecycle'],
    path: '/docs/cloud/policy-lifecycle',
    title: 'Zero-Trust Policy Staging & Signing',
    description: '5-step lifecycle: Draft -> Staging -> Local Operator Signature -> Broadcast -> Atomic Tempfile Swap.',
    section: '5. Rivun Cloud & Operator Station',
    subSection: 'Policy Engine',
    headings: [
      { id: 'stage-1-draft', text: 'Step 1: Policy Draft in Dashboard', level: 2 },
      { id: 'stage-2-stage', text: 'Step 2: Staging on Cloud API', level: 2 },
      { id: 'stage-3-sign', text: 'Step 3: Offline Inspection & Ed25519 Signing', level: 2 },
      { id: 'stage-4-broadcast', text: 'Step 4: Push to Edge Nodes', level: 2 },
      { id: 'stage-5-atomic-swap', text: 'Step 5: Cryptographic Verification & Atomic File Swap', level: 2 },
    ],
    rawContent: `
### The 5-Step Policy Lifecycle
1. **Draft**: Policy author creates rules in TOML or Web UI.
2. **Staged**: Policy is uploaded to Cloud API with status \`staged\`.
3. **Signed**: Security operator inspects diff locally in \`rivun-control\` and signs using domain \`Rivun-POLICY-BUNDLE-v1\`.
4. **Broadcast**: Signed bundle is distributed to edge clusters via SSE / HTTPS.
5. **Atomic Swap**: Edge daemon \`rivun-cloud-bridge\` verifies signature against local trusted public key list, writes to temp file, and executes atomic file swap (\`tempfile::persist\`).
    `,
  },
  {
    slug: ['cloud', 'edge-bridge-daemon'],
    path: '/docs/cloud/edge-bridge-daemon',
    title: 'Edge Daemon (rivun-cloud-bridge)',
    description: 'Lightweight Rust sidecar for telemetry push, receipt batch synchronization, and client-side secret redaction.',
    section: '5. Rivun Cloud & Operator Station',
    subSection: 'Edge Bridge',
    headings: [
      { id: 'bridge-architecture', text: 'Edge Sidecar Architecture', level: 2 },
      { id: 'secret-redactor', text: 'Client-Side Secret Redaction (SecretRedactor)', level: 2 },
      { id: 'receipt-batching', text: 'Batch Receipt Push & Offline Buffering', level: 2 },
    ],
    rawContent: `
**\`rivun-cloud-bridge\`** runs as a lightweight daemon (under 12MB RAM) alongside edge nodes:
- Pushes 7-Point Fleet Doctor health telemetry every 15 seconds.
- Automatically scrubs credentials, private IP ranges, and proprietary payloads before transmitting logs using the compiled regex \`SecretRedactor\` engine.
- Buffers up to 100,000 receipts locally on disk if network connection is disrupted.
    `,
  },
  {
    slug: ['cloud', 'rest-sse-api'],
    path: '/docs/cloud/rest-sse-api',
    title: 'Multi-Tenant REST & SSE API',
    description: 'High-concurrency Axum 0.8 REST endpoints and real-time Server-Sent Events broker.',
    section: '5. Rivun Cloud & Operator Station',
    subSection: 'Cloud API',
    headings: [
      { id: 'api-architecture', text: 'Axum 0.8 Architecture', level: 2 },
      { id: 'rest-endpoints', text: 'Core REST Endpoints', level: 2 },
      { id: 'sse-streaming', text: 'Real-Time SSE Event Streams', level: 2 },
    ],
    rawContent: `
The **\`rivun-cloud-api\`** crate delivers a sub-millisecond asynchronous REST and SSE backend powered by Tokio and Axum 0.8:
- \`GET /v1/status\`: Cluster and cloud health status.
- \`GET /v1/orgs/{org_id}/nodes\`: Fleet node inventory and telemetry.
- \`GET /v1/orgs/{org_id}/receipts\`: Action receipt audit logs with MMR root proofs.
- \`GET /v1/orgs/{org_id}/events/stream\`: Real-time SSE stream of fleet alerts, node heartbeats, and policy sync events.
    `,
  },
  {
    slug: ['cloud', 'dashboard-integration'],
    path: '/docs/cloud/dashboard-integration',
    title: 'Rivun Dashboard Integration',
    description: 'Connecting the Next.js enterprise dashboard with the Rivun Cloud API and edge mesh.',
    section: '5. Rivun Cloud & Operator Station',
    subSection: 'Web Interface',
    headings: [
      { id: 'dashboard-features', text: 'Dashboard Features & Views', level: 2 },
      { id: 'live-telemetry', text: 'Live Fleet Visualization & Maps', level: 2 },
      { id: 'receipt-explorer', text: 'Cryptographic Receipt Explorer', level: 2 },
    ],
    rawContent: `
The **Rivun Dashboard** (\`apps/rivun-dashboard\`) provides an Apple-grade visual management plane with live node status maps, 7-Point Fleet Doctor radar charts, policy diff viewers, and MMR receipt proof trees.
    `,
  },
];
