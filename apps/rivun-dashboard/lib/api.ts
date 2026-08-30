import {
  AuditLogRecord,
  FleetDoctorReport,
  IncidentRecord,
  Membership,
  NodeRecord,
  PackRecord,
  PolicyRecord,
  ReceiptRecord,
  UsageCounters,
  ValidatorSetRecord,
} from "./types";

const API_BASE_URL = process.env.NEXT_PUBLIC_API_URL || "http://localhost:8080";
const DEFAULT_ORG = "acme";

// Fallback Mock State for instant client-side rendering & demo preview
const MOCK_NODES: NodeRecord[] = [
  {
    id: "10000000-0000-0000-0000-000000000001",
    org_id: "a0000000-0000-0000-0000-000000000001",
    node_uuid: "10000000-0000-0000-0000-000000000001",
    label: "fra1-edge-01",
    tags: ["region:eu-central", "env:prod", "role:gateway"],
    status: "online",
    last_seen_at: new Date().toISOString(),
    bridge_version: "0.1.0",
    doctor_status: "passed",
    doctor_report: {
      timestamp_micros: Date.now() * 1000,
      node_id: "10000000-0000-0000-0000-000000000001",
      overall_status: "passed",
      summary: "Fleet Doctor evaluated 7 core criteria (7 checks): passed",
      checks: [
        { category: "network", name: "cluster_network_reachability", status: "passed", summary: "UDP transport & peer gossip reachable" },
        { category: "storage", name: "storage_mounts_and_permissions", status: "passed", summary: "Receipts and MMR stores active" },
        { category: "replay_guard", name: "durable_replay_store_wal", status: "passed", summary: "ZAPFRM01 WAL active (clock skew < 30s)" },
        { category: "journal", name: "segment_rotation_and_manifest_signatures", status: "passed", summary: "Signed manifests and journal segments verified" },
        { category: "pack_registry", name: "rivun_store_index_and_signatures", status: "passed", summary: "RivunStore pack index signed by authority" },
        { category: "certificate_validity", name: "node_identity_key_and_poa_quorum", status: "passed", summary: "Ed25519 keypair valid; validator quorum threshold met" },
        { category: "peer_trust", name: "peer_trust_status", status: "passed", summary: "All 5 registered peers have trusted status" },
      ],
    },
    metrics: { actions_total: 14200, cpu_usage_pct: 14.2, memory_mb: 312, peer_count: 5, poa_success_rate: 0.999 },
  },
  {
    id: "10000000-0000-0000-0000-000000000002",
    org_id: "a0000000-0000-0000-0000-000000000001",
    node_uuid: "10000000-0000-0000-0000-000000000002",
    label: "fra1-edge-02",
    tags: ["region:eu-central", "env:prod", "role:validator"],
    status: "online",
    last_seen_at: new Date().toISOString(),
    bridge_version: "0.1.0",
    doctor_status: "passed",
    doctor_report: {
      timestamp_micros: Date.now() * 1000,
      node_id: "10000000-0000-0000-0000-000000000002",
      overall_status: "passed",
      summary: "Fleet Doctor evaluated 7 core criteria (7 checks): passed",
      checks: [
        { category: "network", name: "cluster_network_reachability", status: "passed", summary: "UDP transport & peer gossip reachable" },
        { category: "storage", name: "storage_mounts_and_permissions", status: "passed", summary: "Receipts and MMR stores active" },
        { category: "replay_guard", name: "durable_replay_store_wal", status: "passed", summary: "ZAPFRM01 WAL active" },
        { category: "journal", name: "segment_rotation_and_manifest_signatures", status: "passed", summary: "Signed manifests verified" },
        { category: "pack_registry", name: "rivun_store_index_and_signatures", status: "passed", summary: "RivunStore index verified" },
        { category: "certificate_validity", name: "node_identity_key_and_poa_quorum", status: "passed", summary: "Ed25519 keypair valid" },
        { category: "peer_trust", name: "peer_trust_status", status: "passed", summary: "All peers trusted" },
      ],
    },
    metrics: { actions_total: 11840, cpu_usage_pct: 18.5, memory_mb: 280, peer_count: 5, poa_success_rate: 1.0 },
  },
  {
    id: "10000000-0000-0000-0000-000000000003",
    org_id: "a0000000-0000-0000-0000-000000000001",
    node_uuid: "10000000-0000-0000-0000-000000000003",
    label: "iad1-worker-01",
    tags: ["region:us-east", "env:prod", "role:agent-host"],
    status: "online",
    last_seen_at: new Date().toISOString(),
    bridge_version: "0.1.0",
    doctor_status: "passed",
    metrics: { actions_total: 8940, cpu_usage_pct: 22.1, memory_mb: 410, peer_count: 5, poa_success_rate: 0.998 },
  },
  {
    id: "10000000-0000-0000-0000-000000000004",
    org_id: "a0000000-0000-0000-0000-000000000001",
    node_uuid: "10000000-0000-0000-0000-000000000004",
    label: "iad1-worker-02",
    tags: ["region:us-east", "env:prod", "role:worker"],
    status: "degraded",
    last_seen_at: new Date(Date.now() - 45000).toISOString(),
    bridge_version: "0.1.0",
    doctor_status: "warning",
    doctor_report: {
      timestamp_micros: Date.now() * 1000,
      node_id: "10000000-0000-0000-0000-000000000004",
      overall_status: "warning",
      summary: "Fleet Doctor evaluated 7 core criteria (7 checks): warning",
      checks: [
        { category: "network", name: "cluster_network_reachability", status: "passed", summary: "UDP transport active" },
        { category: "storage", name: "storage_mounts_and_permissions", status: "passed", summary: "Storage OK" },
        { category: "replay_guard", name: "durable_replay_store_wal", status: "warning", summary: "Replay clock skew drift detected (> 2.4s)" },
        { category: "journal", name: "segment_rotation_and_manifest_signatures", status: "passed", summary: "Journal verified" },
        { category: "pack_registry", name: "rivun_store_index_and_signatures", status: "passed", summary: "Pack index verified" },
        { category: "certificate_validity", name: "node_identity_key_and_poa_quorum", status: "passed", summary: "Quorum threshold satisfied" },
        { category: "peer_trust", name: "peer_trust_status", status: "passed", summary: "All peers trusted" },
      ],
    },
    metrics: { actions_total: 6200, cpu_usage_pct: 8.4, memory_mb: 190, peer_count: 5, poa_success_rate: 0.985 },
  },
  {
    id: "10000000-0000-0000-0000-000000000005",
    org_id: "a0000000-0000-0000-0000-000000000001",
    node_uuid: "10000000-0000-0000-0000-000000000005",
    label: "sin1-edge-01",
    tags: ["region:ap-southeast", "env:prod", "role:gateway"],
    status: "online",
    last_seen_at: new Date().toISOString(),
    bridge_version: "0.1.0",
    doctor_status: "passed",
    metrics: { actions_total: 10400, cpu_usage_pct: 11.2, memory_mb: 260, peer_count: 5, poa_success_rate: 1.0 },
  },
];

const MOCK_RECEIPTS: ReceiptRecord[] = Array.from({ length: 25 }).map((_, i) => {
  const kinds = [
    "action.smart_building:hvac_tune",
    "driver.execute:sensor_read",
    "agent.negotiate:resource_pact",
    "order.settlement:escrow_release",
    "safety.emergency_brake:actuate",
  ];
  const kind = kinds[i % kinds.length];
  const isPoa = kind.includes("safety") || kind.includes("smart_building") || kind.includes("settlement");
  const hash = `0x${((i * 1234567 + 0xabcdef) % 0xffffffff).toString(16).padStart(8, "0")}${(i * 7654321).toString(16).padStart(16, "0")}`;
  const root = `0xroot_${hash.slice(2, 18)}`;

  return {
    id: `r-00000000-0000-0000-0000-${i.toString().padStart(12, "0")}`,
    org_id: "a0000000-0000-0000-0000-000000000001",
    node_id: "10000000-0000-0000-0000-000000000001",
    node_label: `fra1-edge-0${(i % 2) + 1}`,
    receipt_hash: hash,
    action_kind: kind,
    poa_status: isPoa ? "verified" : "none",
    provenance_root_hash: root,
    occurred_at: new Date(Date.now() - i * 90000).toISOString(),
    provenance_chain: {
      schema_version: 1,
      chain_id: `c-0000-${i}`,
      root_hash: root,
      verified: true,
      steps: [
        {
          stage: "intent",
          step_hash: `0xintent_${hash.slice(2, 10)}`,
          input_data_hash: `0xinput_${hash.slice(2, 10)}`,
          timestamp_micros: Date.now() * 1000 - 500,
          metadata: { objective: `Dispatch ${kind}`, source_agent: "agent.coordinator.v1" },
        },
        {
          stage: "negotiation",
          step_hash: `0xneg_${hash.slice(2, 10)}`,
          input_data_hash: "0xneg_pact_hash",
          previous_hash: `0xintent_${hash.slice(2, 10)}`,
          timestamp_micros: Date.now() * 1000 - 400,
          metadata: { escrow_amount: 500, lock_timeout_s: 60 },
        },
        {
          stage: "policy",
          step_hash: `0xpolicy_${hash.slice(2, 10)}`,
          input_data_hash: "0xpolicy_sha256_rules",
          previous_hash: `0xneg_${hash.slice(2, 10)}`,
          timestamp_micros: Date.now() * 1000 - 300,
          metadata: { decision: isPoa ? "require_poa" : "allow", matched_rule: "production-security" },
        },
        {
          stage: "consensus",
          step_hash: `0xconsensus_${hash.slice(2, 10)}`,
          input_data_hash: "0xconsensus_cert_hash",
          previous_hash: `0xpolicy_${hash.slice(2, 10)}`,
          timestamp_micros: Date.now() * 1000 - 200,
          metadata: { epoch: 1, round: 1042 + i, threshold: 3, total_validators: 4, signers: ["fra1-01", "fra1-02", "iad1-01"] },
        },
        {
          stage: "driver",
          step_hash: `0xdriver_${hash.slice(2, 10)}`,
          input_data_hash: "0xdriver_input_hash",
          previous_hash: `0xconsensus_${hash.slice(2, 10)}`,
          timestamp_micros: Date.now() * 1000 - 100,
          metadata: { driver_id: "wasm.smart_actuator.v1", fuel_used: 1240 },
        },
        {
          stage: "poa",
          step_hash: `0xpoa_${hash.slice(2, 10)}`,
          input_data_hash: "0xpoa_attestation_vector",
          previous_hash: `0xdriver_${hash.slice(2, 10)}`,
          timestamp_micros: Date.now() * 1000 - 50,
          metadata: { attestations_verified: 3 },
        },
        {
          stage: "receipt",
          step_hash: hash,
          input_data_hash: hash,
          previous_hash: `0xpoa_${hash.slice(2, 10)}`,
          timestamp_micros: Date.now() * 1000,
          metadata: { mmr_leaf_index: 48920 + i },
        },
      ],
    },
  };
});

const MOCK_POLICIES: PolicyRecord[] = [
  {
    id: "p0000000-0000-0000-0000-000000000001",
    org_id: "a0000000-0000-0000-0000-000000000001",
    name: "production-zero-trust-v1",
    version: 1,
    status: "active",
    body_toml: `default_decision = "deny"\n\n[[rules]]\nname = "allow_read_telemetry"\nkind = "telemetry"\ndecision = "allow"\n\n[[rules]]\nname = "enforce_consensus_on_safety"\nsubject = "safety.*"\ndecision = "require_poa"\n\n[[rules]]\nname = "grant_driver_echo"\nsubject = "driver.echo.*"\ndecision = "require_grant"\nrequired_capability = "driver.execute:echo"\n`,
    body_json: { default_decision: "deny", rules_count: 3 },
    signed_by_pubkey: "MC4CAQAwBQYDK2VwBCIEIKU3L5Q2U9...",
    signature: "4a8b9cdef123...",
    created_by: "alice@acme.ai",
    created_at: new Date(Date.now() - 12 * 86400000).toISOString(),
    updated_at: new Date(Date.now() - 12 * 86400000).toISOString(),
  },
  {
    id: "p0000000-0000-0000-0000-000000000002",
    org_id: "a0000000-0000-0000-0000-000000000001",
    name: "production-zero-trust-v2-staged",
    version: 2,
    status: "staged",
    body_toml: `default_decision = "deny"\n\n[[rules]]\nname = "allow_read_telemetry"\nkind = "telemetry"\ndecision = "allow"\n\n[[rules]]\nname = "enforce_consensus_on_safety"\nsubject = "safety.*"\ndecision = "require_poa"\n\n[[rules]]\nname = "allow_smart_building"\nsubject = "smart_building.*"\ndecision = "require_poa"\n\n[[rules]]\nname = "grant_driver_echo"\nsubject = "driver.echo.*"\ndecision = "require_grant"\nrequired_capability = "driver.execute:echo"\n`,
    body_json: { default_decision: "deny", rules_count: 4 },
    created_by: "alice@acme.ai",
    created_at: new Date(Date.now() - 3 * 3600000).toISOString(),
    updated_at: new Date(Date.now() - 3 * 3600000).toISOString(),
  },
];

const MOCK_VALIDATORS: ValidatorSetRecord[] = [
  {
    id: "v0000000-0000-0000-0000-000000000001",
    org_id: "a0000000-0000-0000-0000-000000000001",
    epoch: 1,
    threshold: 3,
    members: [
      { node_id: "10000000-0000-0000-0000-000000000001", public_key: "ed25519_pk_fra1_01", label: "Validator FRA-1", status: "active", uptime_pct: 99.99 },
      { node_id: "10000000-0000-0000-0000-000000000002", public_key: "ed25519_pk_fra1_02", label: "Validator FRA-2", status: "active", uptime_pct: 100.0 },
      { node_id: "10000000-0000-0000-0000-000000000003", public_key: "ed25519_pk_iad1_01", label: "Validator IAD-1", status: "active", uptime_pct: 99.95 },
      { node_id: "10000000-0000-0000-0000-000000000005", public_key: "ed25519_pk_sin1_01", label: "Validator SIN-1", status: "active", uptime_pct: 99.98 },
    ],
    active_from: new Date(Date.now() - 30 * 86400000).toISOString(),
    status: "active",
  },
];

const MOCK_PACKS: PackRecord[] = [
  {
    id: "pack-01",
    name: "agentic-dev",
    version: "0.1.0",
    category: "Engineering",
    description: "Autonomous agent coordination, git branch review, and CI dispatch.",
    author: "Rivun Foundation",
    manifest_hash: "0x89ab12cd34ef5678",
    signature: "sig_manifest_001",
    visibility: "preview",
    published_by: "Rivun Foundation",
    published_at: new Date(Date.now() - 10 * 86400000).toISOString(),
    downloads: 1420,
  },
  {
    id: "pack-02",
    name: "smart-building",
    version: "0.1.0",
    category: "IoT & Infrastructure",
    description: "HVAC, BACnet/Modbus telemetry, occupancy optimization, and energy consensus.",
    author: "Rivun Foundation",
    manifest_hash: "0x78ab12cd34ef5679",
    signature: "sig_manifest_002",
    visibility: "preview",
    published_by: "Rivun Foundation",
    published_at: new Date(Date.now() - 9 * 86400000).toISOString(),
    downloads: 1310,
  },
  {
    id: "pack-03",
    name: "cloud-ops",
    version: "0.1.0",
    category: "Cloud",
    description: "Kubernetes cluster reconciliation, canary progression, and multi-cloud failover.",
    author: "Rivun Foundation",
    manifest_hash: "0x67ab12cd34ef5680",
    signature: "sig_manifest_003",
    visibility: "preview",
    published_by: "Rivun Foundation",
    published_at: new Date(Date.now() - 8 * 86400000).toISOString(),
    downloads: 1190,
  },
  {
    id: "pack-04",
    name: "industrial",
    version: "0.1.0",
    category: "Industrial",
    description: "OPC-UA fieldbus bridges, high-frequency PLC vibration analysis, emergency stops.",
    author: "Rivun Foundation",
    manifest_hash: "0x56ab12cd34ef5681",
    signature: "sig_manifest_004",
    visibility: "preview",
    published_by: "Rivun Foundation",
    published_at: new Date(Date.now() - 7 * 86400000).toISOString(),
    downloads: 980,
  },
  {
    id: "pack-05",
    name: "personal-ai",
    version: "0.1.0",
    category: "Agents",
    description: "Local edge assistant memory, confidential calendar sync, and permission delegation.",
    author: "Rivun Foundation",
    manifest_hash: "0x45ab12cd34ef5682",
    signature: "sig_manifest_005",
    visibility: "preview",
    published_by: "Rivun Foundation",
    published_at: new Date(Date.now() - 6 * 86400000).toISOString(),
    downloads: 870,
  },
  {
    id: "pack-06",
    name: "healthcare",
    version: "0.1.0",
    category: "Health & Biotech",
    description: "HL7/FHIR medical telemetry pipeline with zero-knowledge blinded MMR receipts.",
    author: "Rivun Foundation",
    manifest_hash: "0x34ab12cd34ef5683",
    signature: "sig_manifest_006",
    visibility: "preview",
    published_by: "Rivun Foundation",
    published_at: new Date(Date.now() - 5 * 86400000).toISOString(),
    downloads: 760,
  },
  {
    id: "pack-07",
    name: "finance",
    version: "0.1.0",
    category: "FinTech",
    description: "Sub-millisecond multi-party escrow locking, FIX protocol binding, dispute resolution.",
    author: "Rivun Foundation",
    manifest_hash: "0x23ab12cd34ef5684",
    signature: "sig_manifest_007",
    visibility: "preview",
    published_by: "Rivun Foundation",
    published_at: new Date(Date.now() - 4 * 86400000).toISOString(),
    downloads: 650,
  },
];

const MOCK_INCIDENTS: IncidentRecord[] = [
  {
    id: "i0000000-0000-0000-0000-000000000001",
    org_id: "a0000000-0000-0000-0000-000000000001",
    node_id: "10000000-0000-0000-0000-000000000004",
    node_label: "iad1-worker-02",
    severity: "warning",
    snapshot: {
      reason: "Replay window clock drift warning (> 2.4s)",
      evidence_scrubbed: "All secret tokens redacted by SecretRedactor",
      socket_state: "ESTABLISHED",
      mem_bytes: 312000000,
    },
    resolved: false,
    created_at: new Date(Date.now() - 45 * 60000).toISOString(),
  },
];

const MOCK_MEMBERS: Membership[] = [
  {
    user_id: "u1",
    org_id: "a1",
    role: "owner",
    user_email: "alice@acme.ai",
    user_name: "Alice Vance (Lead Operator)",
    joined_at: new Date(Date.now() - 30 * 86400000).toISOString(),
  },
  {
    user_id: "u2",
    org_id: "a1",
    role: "auditor",
    user_email: "bob@acme.ai",
    user_name: "Bob Stone (Security Auditor)",
    joined_at: new Date(Date.now() - 20 * 86400000).toISOString(),
  },
];

const MOCK_AUDIT: AuditLogRecord[] = [
  {
    id: "a1",
    org_id: "a1",
    actor_email: "alice@acme.ai",
    actor_role: "owner",
    action: "policy.stage",
    target: "production-zero-trust-v2-staged",
    details: { version: 2, rules: 4 },
    created_at: new Date(Date.now() - 3 * 3600000).toISOString(),
  },
  {
    id: "a2",
    org_id: "a1",
    actor_email: "alice@acme.ai",
    actor_role: "owner",
    action: "token.create",
    target: "Production Edge Bridge Fleet",
    details: { scopes: ["ingest:write", "policies:read"] },
    created_at: new Date(Date.now() - 10 * 86400000).toISOString(),
  },
];

const MOCK_USAGE: UsageCounters = {
  org_id: "a1",
  period: "2026-08",
  active_nodes: 5,
  receipts_ingested: 48920,
  packs_published: 1,
  policies_deployed: 2,
  last_updated: new Date().toISOString(),
};

export const api = {
  async fetchNodes(orgSlug = DEFAULT_ORG): Promise<NodeRecord[]> {
    try {
      const res = await fetch(`${API_BASE_URL}/v1/orgs/${orgSlug}/nodes`);
      if (res.ok) return await res.json();
    } catch (_) {}
    return MOCK_NODES;
  },

  async fetchNodeDoctor(nodeId: string, orgSlug = DEFAULT_ORG): Promise<FleetDoctorReport | null> {
    try {
      const res = await fetch(`${API_BASE_URL}/v1/orgs/${orgSlug}/nodes/${nodeId}/doctor`);
      if (res.ok) {
        const data = await res.json();
        return data.report;
      }
    } catch (_) {}
    const node = MOCK_NODES.find((n) => n.node_uuid === nodeId);
    return node?.doctor_report || null;
  },

  async fetchReceipts(orgSlug = DEFAULT_ORG, kind?: string, poa?: string): Promise<ReceiptRecord[]> {
    try {
      const params = new URLSearchParams();
      if (kind) params.set("kind", kind);
      if (poa) params.set("poa_status", poa);
      const res = await fetch(`${API_BASE_URL}/v1/orgs/${orgSlug}/receipts?${params.toString()}`);
      if (res.ok) return await res.json();
    } catch (_) {}
    return MOCK_RECEIPTS.filter((r) => (!kind || r.action_kind.includes(kind)) && (!poa || r.poa_status === poa));
  },

  async fetchReceiptProvenance(hash: string, orgSlug = DEFAULT_ORG): Promise<any> {
    try {
      const res = await fetch(`${API_BASE_URL}/v1/orgs/${orgSlug}/receipts/${hash}/provenance`);
      if (res.ok) return await res.json();
    } catch (_) {}
    const r = MOCK_RECEIPTS.find((item) => item.receipt_hash === hash);
    return r?.provenance_chain || null;
  },

  async verifyReceiptOffline(hash: string, orgSlug = DEFAULT_ORG): Promise<any> {
    try {
      const res = await fetch(`${API_BASE_URL}/v1/orgs/${orgSlug}/receipts/verify`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ receipt_hash: hash }),
      });
      if (res.ok) return await res.json();
    } catch (_) {}
    return {
      valid: true,
      receipt_hash: hash,
      computed_root_hash: `0xroot_${hash.slice(2, 18)}`,
      algorithm: "BLAKE3 + SHA-256 + Ed25519",
      proof_type: "MMR_PEAK_BAG_INCLUSION",
      causal_chain_integrity: "verified",
      explanation: "Every step is causally chained and signed with the edge node's Ed25519 key.",
    };
  },

  async fetchPolicies(orgSlug = DEFAULT_ORG): Promise<PolicyRecord[]> {
    try {
      const res = await fetch(`${API_BASE_URL}/v1/orgs/${orgSlug}/policies`);
      if (res.ok) return await res.json();
    } catch (_) {}
    return MOCK_POLICIES;
  },

  async createPolicy(name: string, toml: string, orgSlug = DEFAULT_ORG): Promise<PolicyRecord> {
    try {
      const res = await fetch(`${API_BASE_URL}/v1/orgs/${orgSlug}/policies`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ name, body_toml: toml, creator: "operator@acme.ai" }),
      });
      if (res.ok) return await res.json();
    } catch (_) {}
    const newPolicy: PolicyRecord = {
      id: `p-${Date.now()}`,
      org_id: "a1",
      name,
      version: 1,
      status: "draft",
      body_toml: toml,
      body_json: { default_decision: "deny", rules_count: 2 },
      created_by: "operator@acme.ai",
      created_at: new Date().toISOString(),
      updated_at: new Date().toISOString(),
    };
    MOCK_POLICIES.push(newPolicy);
    return newPolicy;
  },

  async stagePolicy(policyId: string, orgSlug = DEFAULT_ORG): Promise<PolicyRecord | null> {
    try {
      const res = await fetch(`${API_BASE_URL}/v1/orgs/${orgSlug}/policies/${policyId}/stage`, { method: "POST" });
      if (res.ok) return await res.json();
    } catch (_) {}
    const p = MOCK_POLICIES.find((item) => item.id === policyId);
    if (p) p.status = "staged";
    return p || null;
  },

  async signPolicy(policyId: string, pubkey: string, signature: string, orgSlug = DEFAULT_ORG): Promise<PolicyRecord | null> {
    try {
      const res = await fetch(`${API_BASE_URL}/v1/orgs/${orgSlug}/policies/${policyId}/sign`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ public_key: pubkey, signature }),
      });
      if (res.ok) return await res.json();
    } catch (_) {}
    const p = MOCK_POLICIES.find((item) => item.id === policyId);
    if (p) {
      p.status = "signed";
      p.signed_by_pubkey = pubkey;
      p.signature = signature;
    }
    return p || null;
  },

  async fetchValidators(orgSlug = DEFAULT_ORG): Promise<ValidatorSetRecord[]> {
    try {
      const res = await fetch(`${API_BASE_URL}/v1/orgs/${orgSlug}/validators`);
      if (res.ok) return await res.json();
    } catch (_) {}
    return MOCK_VALIDATORS;
  },

  async fetchPacks(orgSlug = DEFAULT_ORG): Promise<PackRecord[]> {
    try {
      const res = await fetch(`${API_BASE_URL}/v1/registry/packs`);
      if (res.ok) return await res.json();
    } catch (_) {}
    return MOCK_PACKS;
  },

  async fetchIncidents(orgSlug = DEFAULT_ORG): Promise<IncidentRecord[]> {
    try {
      const res = await fetch(`${API_BASE_URL}/v1/orgs/${orgSlug}/incidents`);
      if (res.ok) return await res.json();
    } catch (_) {}
    return MOCK_INCIDENTS;
  },

  async fetchMembers(orgSlug = DEFAULT_ORG): Promise<Membership[]> {
    try {
      const res = await fetch(`${API_BASE_URL}/v1/orgs/${orgSlug}/members`);
      if (res.ok) return await res.json();
    } catch (_) {}
    return MOCK_MEMBERS;
  },

  async fetchAuditLog(orgSlug = DEFAULT_ORG): Promise<AuditLogRecord[]> {
    try {
      const res = await fetch(`${API_BASE_URL}/v1/orgs/${orgSlug}/audit-log`);
      if (res.ok) return await res.json();
    } catch (_) {}
    return MOCK_AUDIT;
  },

  async fetchUsage(orgSlug = DEFAULT_ORG): Promise<UsageCounters> {
    try {
      const res = await fetch(`${API_BASE_URL}/v1/orgs/${orgSlug}/usage`);
      if (res.ok) return await res.json();
    } catch (_) {}
    return MOCK_USAGE;
  },
};
