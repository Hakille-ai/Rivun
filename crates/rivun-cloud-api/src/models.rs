//! Data models for Rivun Cloud Multi-Tenant API.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use rivun_telemetry::{FleetDoctorReport, FleetDoctorStatus};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UserRole {
    Owner,
    Admin,
    Operator,
    Auditor,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Organization {
    pub id: Uuid,
    pub name: String,
    pub slug: String,
    pub plan: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: Uuid,
    pub email: String,
    pub name: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Membership {
    pub user_id: Uuid,
    pub org_id: Uuid,
    pub role: UserRole,
    pub user_email: String,
    pub user_name: String,
    pub joined_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiToken {
    pub id: Uuid,
    pub org_id: Uuid,
    pub name: String,
    pub token_hash: String,
    pub scopes: Vec<String>,
    pub created_by: Uuid,
    pub created_at: DateTime<Utc>,
    pub revoked_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeRecord {
    pub id: Uuid,
    pub org_id: Uuid,
    pub public_key: Option<String>,
    pub node_uuid: Uuid,
    pub label: String,
    pub tags: Vec<String>,
    pub status: String, // "online", "degraded", "offline"
    pub last_seen_at: DateTime<Utc>,
    pub bridge_version: String,
    pub doctor_status: FleetDoctorStatus,
    pub doctor_report: Option<FleetDoctorReport>,
    pub metrics: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReceiptRecord {
    pub id: Uuid,
    pub org_id: Uuid,
    pub node_id: Uuid,
    pub node_label: String,
    pub receipt_hash: String,
    pub action_kind: String,
    pub poa_status: String, // "verified", "single_signer", "none"
    pub provenance_root_hash: Option<String>,
    pub provenance_chain: Option<serde_json::Value>,
    pub occurred_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PolicyStatus {
    Draft,
    Staged,
    Signed,
    Active,
    Archived,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyRecord {
    pub id: Uuid,
    pub org_id: Uuid,
    pub name: String,
    pub version: u32,
    pub status: PolicyStatus,
    pub body_toml: String,
    pub body_json: serde_json::Value,
    pub signed_by_pubkey: Option<String>,
    pub signature: Option<String>,
    pub created_by: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidatorSetRecord {
    pub id: Uuid,
    pub org_id: Uuid,
    pub epoch: u64,
    pub threshold: u16,
    pub members: Vec<ValidatorMember>,
    pub active_from: DateTime<Utc>,
    pub status: String, // "active", "proposed", "retired"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidatorMember {
    pub node_id: Uuid,
    pub public_key: String,
    pub label: String,
    pub status: String,
    pub uptime_pct: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttestationRecord {
    pub id: Uuid,
    pub org_id: Uuid,
    pub node_id: Uuid,
    pub request_hash: String,
    pub threshold_met: bool,
    pub attestations_count: u16,
    pub threshold: u16,
    pub status: String,
    pub occurred_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackRecord {
    pub id: Uuid,
    pub org_id: Option<Uuid>, // None = global catalog pack
    pub name: String,
    pub version: String,
    pub category: String,
    pub description: String,
    pub author: String,
    pub manifest_hash: String,
    pub signature: Option<String>,
    pub visibility: String, // "public", "private", "preview"
    pub published_by: String,
    pub published_at: DateTime<Utc>,
    pub downloads: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IncidentRecord {
    pub id: Uuid,
    pub org_id: Uuid,
    pub node_id: Uuid,
    pub node_label: String,
    pub severity: String, // "critical", "warning", "info"
    pub snapshot: serde_json::Value,
    pub resolved: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditLogRecord {
    pub id: Uuid,
    pub org_id: Uuid,
    pub actor_email: String,
    pub actor_role: UserRole,
    pub action: String,
    pub target: String,
    pub details: serde_json::Value,
    pub ip_address: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageCounters {
    pub org_id: Uuid,
    pub period: String,
    pub active_nodes: usize,
    pub receipts_ingested: u64,
    pub packs_published: usize,
    pub policies_deployed: usize,
    pub last_updated: DateTime<Utc>,
}
