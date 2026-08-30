export type FleetDoctorStatus = "passed" | "warning" | "failed";

export interface FleetDoctorCheck {
  category: string;
  name: string;
  status: FleetDoctorStatus;
  summary: string;
  detail?: string;
}

export interface FleetDoctorReport {
  timestamp_micros: number;
  node_id: string;
  overall_status: FleetDoctorStatus;
  checks: FleetDoctorCheck[];
  summary: string;
}

export interface NodeRecord {
  id: string;
  org_id: string;
  public_key?: string;
  node_uuid: string;
  label: string;
  tags: string[];
  status: "online" | "degraded" | "offline";
  last_seen_at: string;
  bridge_version: string;
  doctor_status: FleetDoctorStatus;
  doctor_report?: FleetDoctorReport;
  metrics: {
    actions_total?: number;
    cpu_usage_pct?: number;
    memory_mb?: number;
    peer_count?: number;
    poa_success_rate?: number;
    [key: string]: any;
  };
}

export interface ProvenanceStep {
  stage: "intent" | "negotiation" | "policy" | "consensus" | "driver" | "poa" | "receipt";
  step_hash: string;
  input_data_hash: string;
  previous_hash?: string;
  timestamp_micros: number;
  metadata?: Record<string, any>;
}

export interface ProvenanceChain {
  schema_version: number;
  chain_id: string;
  root_hash: string;
  verified?: boolean;
  steps: ProvenanceStep[];
}

export interface ReceiptRecord {
  id: string;
  org_id: string;
  node_id: string;
  node_label: string;
  receipt_hash: string;
  action_kind: string;
  poa_status: "verified" | "single_signer" | "none";
  provenance_root_hash?: string;
  provenance_chain?: ProvenanceChain;
  occurred_at: string;
}

export type PolicyStatus = "draft" | "staged" | "signed" | "active" | "archived";

export interface PolicyRecord {
  id: string;
  org_id: string;
  name: string;
  version: number;
  status: PolicyStatus;
  body_toml: string;
  body_json: {
    default_decision: string;
    rules_count: number;
    [key: string]: any;
  };
  signed_by_pubkey?: string;
  signature?: string;
  created_by: string;
  created_at: string;
  updated_at: string;
}

export interface ValidatorMember {
  node_id: string;
  public_key: string;
  label: string;
  status: string;
  uptime_pct: number;
}

export interface ValidatorSetRecord {
  id: string;
  org_id: string;
  epoch: number;
  threshold: number;
  members: ValidatorMember[];
  active_from: string;
  status: string;
}

export interface PackRecord {
  id: string;
  org_id?: string;
  name: string;
  version: string;
  category: string;
  description: string;
  author: string;
  manifest_hash: string;
  signature?: string;
  visibility: "public" | "private" | "preview";
  published_by: string;
  published_at: string;
  downloads: number;
}

export interface IncidentRecord {
  id: string;
  org_id: string;
  node_id: string;
  node_label: string;
  severity: "critical" | "warning" | "info";
  snapshot: Record<string, any>;
  resolved: boolean;
  created_at: string;
}

export type UserRole = "owner" | "admin" | "operator" | "auditor";

export interface Membership {
  user_id: string;
  org_id: string;
  role: UserRole;
  user_email: string;
  user_name: string;
  joined_at: string;
}

export interface AuditLogRecord {
  id: string;
  org_id: string;
  actor_email: string;
  actor_role: UserRole;
  action: string;
  target: string;
  details: Record<string, any>;
  ip_address?: string;
  created_at: string;
}

export interface UsageCounters {
  org_id: string;
  period: string;
  active_nodes: number;
  receipts_ingested: number;
  packs_published: number;
  policies_deployed: number;
  last_updated: string;
}
