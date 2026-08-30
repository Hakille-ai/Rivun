import { DocPage } from '../types';

export const DOMAIN_PACK_DOCS: DocPage[] = [
  {
    slug: ['domain-packs', 'architecture'],
    path: '/docs/domain-packs/architecture',
    title: 'Domain Pack Architecture & pack.toml Contract',
    description: 'Structure of .zpack bundles, pack.toml manifest contracts, safety defaults, and risk vocabularies.',
    section: '8. 7 Domain Packs & RivunStore',
    subSection: 'Packs Overview',
    headings: [
      { id: 'pack-structure', text: 'Pack Directory Structure', level: 2 },
      { id: 'pack-toml', text: 'pack.toml Manifest Schema', level: 2 },
      { id: 'risk-vocabularies', text: 'Risk Vocabularies & Safety Defaults', level: 2 },
    ],
    callouts: [
      {
        type: 'note',
        title: 'Four Risk Tiers',
        content: 'Actions are classified into 4 standardized risk tiers: LOW (auto-approve), MEDIUM (policy check), HIGH (Proof-of-Action quorum required), and CRITICAL (offline operator multi-sig required).',
      },
    ],
    rawContent: `
### Domain Pack Anatomy (\`.zpack\`)
A Domain Pack is a self-contained, cryptographically signed bundle containing:
- \`pack.toml\`: Metadata, risk definitions, capability dependencies, and driver mappings.
- \`schemas/\`: JSON / Binary schema definitions for domain messages.
- \`policies/\`: Declarative rule sets (\`Allow\`, \`Deny\`, \`RequirePoa\`).
- \`routes/\`: Subject routing tables.
- \`drivers/\`: Compiled WebAssembly action drivers (\`wasm32-wasip1\`).
- \`dashboards/\`: Embedded UI telemetry widget definitions.
    `,
  },
  {
    slug: ['domain-packs', 'lifecycle'],
    path: '/docs/domain-packs/lifecycle',
    title: 'Pack Lifecycle & CLI Commands',
    description: 'Step-by-step CLI guide: rivun pack init -> build -> sign -> verify -> install -> audit.',
    section: '8. 7 Domain Packs & RivunStore',
    subSection: 'Packs Overview',
    headings: [
      { id: 'lifecycle-steps', text: 'The 6 Lifecycle Steps', level: 2 },
      { id: 'cli-workflow', text: 'CLI Workflow Commands', level: 2 },
    ],
    rawContent: `
### The 6-Step Pack Lifecycle
1. \`rivun pack init <name>\`: Scaffolds a new domain pack with template files.
2. \`rivun pack build\`: Compiles WASM drivers and validates schemas.
3. \`rivun pack sign --key <keyfile>\`: Generates detached Ed25519 signature over BLAKE3 bundle digest.
4. \`rivun pack verify <bundle.zpack>\`: Validates internal checksums and author signature.
5. \`rivun pack install <bundle.zpack>\`: Deploys pack to local node registry.
6. \`rivun pack audit <bundle.zpack>\`: Runs static security analysis against capabilities and fuel limits.
    `,
  },
  {
    slug: ['domain-packs', 'agentic-dev'],
    path: '/docs/domain-packs/agentic-dev',
    title: 'Pack 1: Agentic Development (rivun-pack-agentic-dev)',
    description: 'Auditable coding agents, git repository access, code patch verification, and test execution.',
    section: '8. 7 Domain Packs & RivunStore',
    subSection: 'Foundation Packs',
    headings: [
      { id: 'pack-overview', text: 'Pack Overview', level: 2 },
      { id: 'capabilities', text: 'Declared Capabilities', level: 2 },
      { id: 'policy-rules', text: 'Safety Policy Rules', level: 2 },
    ],
    rawContent: `
Provides sandboxed execution for autonomous AI coding agents:
- Capabilities: \`repo.read\`, \`repo.patch\`, \`test.run\`, \`branch.create\`.
- Policy: Blocks direct pushes to \`main\`, requires 2-of-3 BFT consensus for destructive git resets.
    `,
  },
  {
    slug: ['domain-packs', 'smart-building'],
    path: '/docs/domain-packs/smart-building',
    title: 'Pack 2: Smart Building Automation (rivun-pack-smart-building)',
    description: 'HVAC control, access control, lighting, and environmental sensor monitoring with fail-closed safety.',
    section: '8. 7 Domain Packs & RivunStore',
    subSection: 'Foundation Packs',
    headings: [
      { id: 'pack-overview', text: 'Pack Overview', level: 2 },
      { id: 'fail-closed-safety', text: 'Fail-Closed Safety Gates', level: 2 },
    ],
    rawContent: `
Automates commercial building management systems (BMS) with hard temperature bounds (16°C to 28°C) and emergency fire alarm overrides.
    `,
  },
  {
    slug: ['domain-packs', 'cloud-ops'],
    path: '/docs/domain-packs/cloud-ops',
    title: 'Pack 3: Cloud & Infrastructure Ops (rivun-pack-cloud-ops)',
    description: 'Automated deployment pipelines, Kubernetes pod scaling, canary rollouts, and disaster recovery.',
    section: '8. 7 Domain Packs & RivunStore',
    subSection: 'Foundation Packs',
    headings: [
      { id: 'pack-overview', text: 'Pack Overview', level: 2 },
      { id: 'canary-guards', text: 'Canary Verification & Auto-Rollback', level: 2 },
    ],
    rawContent: `
Manages zero-downtime infrastructure operations with automated rollback safeguards if error rates exceed 0.5% during canary deployment.
    `,
  },
  {
    slug: ['domain-packs', 'industrial'],
    path: '/docs/domain-packs/industrial',
    title: 'Pack 4: Industrial Control & SCADA (rivun-pack-industrial)',
    description: 'Modbus/TCP register access, PLC valve actuation, conveyor control, and deterministic simulation gates.',
    section: '8. 7 Domain Packs & RivunStore',
    subSection: 'Foundation Packs',
    headings: [
      { id: 'pack-overview', text: 'Pack Overview', level: 2 },
      { id: 'simulation-gates', text: 'Pre-Execution Simulation Gates', level: 2 },
    ],
    rawContent: `
Interfaces with physical factory hardware, requiring digital twin simulation verification before opening pressure relief valves or adjusting motor RPM.
    `,
  },
  {
    slug: ['domain-packs', 'personal-ai'],
    path: '/docs/domain-packs/personal-ai',
    title: 'Pack 5: Personal AI Assistant (rivun-pack-personal-ai)',
    description: 'Calendar management, email triage, personal finance summaries, and human-in-the-loop approval gates.',
    section: '8. 7 Domain Packs & RivunStore',
    subSection: 'Foundation Packs',
    headings: [
      { id: 'pack-overview', text: 'Pack Overview', level: 2 },
      { id: 'human-approval', text: 'Human-in-the-Loop Prompts', level: 2 },
    ],
    rawContent: `
Empowers personal AI agents while strictly requiring local biometric or mobile confirmation for outgoing bank transfers or email sends.
    `,
  },
  {
    slug: ['domain-packs', 'healthcare'],
    path: '/docs/domain-packs/healthcare',
    title: 'Pack 6: Healthcare & Patient Care (rivun-pack-healthcare)',
    description: 'HIPAA-compliant patient vitals ingestion, medical device telemetry, and auditable care coordination.',
    section: '8. 7 Domain Packs & RivunStore',
    subSection: 'Foundation Packs',
    headings: [
      { id: 'pack-overview', text: 'Pack Overview', level: 2 },
      { id: 'hipaa-audit', text: 'Cryptographic HIPAA Audit Trails', level: 2 },
    ],
    rawContent: `
Ensures patient health information (PHI) is end-to-end encrypted with zero-knowledge blinded receipt commitments for compliance audits.
    `,
  },
  {
    slug: ['domain-packs', 'finance'],
    path: '/docs/domain-packs/finance',
    title: 'Pack 7: Financial Services & Trading (rivun-pack-finance)',
    description: 'Algorithmic trade proposals, risk checks, multi-signature threshold approval, and atomic settlement.',
    section: '8. 7 Domain Packs & RivunStore',
    subSection: 'Foundation Packs',
    headings: [
      { id: 'pack-overview', text: 'Pack Overview', level: 2 },
      { id: 'multi-sig-settlement', text: 'Multi-Signature Threshold Execution', level: 2 },
    ],
    rawContent: `
Enforces strict value-at-risk (VaR) position limits and requires 3-of-4 algorithmic risk validator signatures for trades exceeding $100,000.
    `,
  },
  {
    slug: ['domain-packs', 'rivunstore-publishing'],
    path: '/docs/domain-packs/rivunstore-publishing',
    title: 'RivunStore Bundle Publishing',
    description: 'Publishing signed domain packs and drivers to the decentralized RivunStore registry.',
    section: '8. 7 Domain Packs & RivunStore',
    subSection: 'RivunStore',
    headings: [
      { id: 'publishing-workflow', text: 'Publishing Workflow', level: 2 },
      { id: 'registry-signatures', text: 'Cryptographic Registry Index Signatures', level: 2 },
    ],
    rawContent: `
Authors publish packs by submitting signed \`.zpack\` archives to the RivunStore registry. The registry signs index manifests with Ed25519, allowing offline clients to verify authenticity.
    `,
  },
];
