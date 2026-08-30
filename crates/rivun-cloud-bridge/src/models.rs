//! Data transfer models for Rivun Cloud Bridge.

use serde::{Deserialize, Serialize};
use uuid::Uuid;
use rivun_telemetry::FleetDoctorReport;

pub const POLICY_BUNDLE_SIGNATURE_DOMAIN: &[u8] = b"Rivun-POLICY-BUNDLE-v1";
pub const BRIDGE_VERSION: &str = "0.1.0";

/// Configuration for the Rivun Cloud Bridge edge daemon.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BridgeConfig {
    /// Target organization slug or ID on Rivun Cloud.
    pub org_slug: String,
    /// Edge node unique identifier.
    pub node_id: Uuid,
    /// Optional human-readable node label.
    #[serde(default)]
    pub label: Option<String>,
    /// Node tags (e.g. ["region:eu-west", "role:validator"]).
    #[serde(default)]
    pub tags: Vec<String>,
    /// Base URL of Rivun Cloud API (e.g. "https://api.rivun.cloud" or "http://localhost:8080").
    pub cloud_url: String,
    /// Scoped API token issued by Rivun Cloud (`ingest:write`, `policies:read`).
    pub api_token: String,
    /// Trusted operator public keys allowed to deploy signed policies (Ed25519 public keys in base64 or hex).
    #[serde(default)]
    pub authorized_operators: Vec<String>,
    /// Telemetry & heartbeat push interval in seconds (default 10s).
    #[serde(default = "default_heartbeat_secs")]
    pub heartbeat_interval_secs: u64,
    /// Policy bundle polling interval in seconds (default 15s).
    #[serde(default = "default_policy_pull_secs")]
    pub policy_pull_interval_secs: u64,
    /// Local path where active policy file should be maintained.
    #[serde(default = "default_policy_path")]
    pub local_policy_path: String,
}

fn default_heartbeat_secs() -> u64 {
    10
}

fn default_policy_pull_secs() -> u64 {
    15
}

fn default_policy_path() -> String {
    "policy.toml".to_string()
}

/// Telemetry payload sent from edge bridge to Rivun Cloud API.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TelemetryIngestPayload {
    pub node_id: Uuid,
    pub public_key: Option<String>,
    pub label: Option<String>,
    pub tags: Vec<String>,
    pub bridge_version: String,
    pub timestamp_micros: u64,
    pub doctor_report: FleetDoctorReport,
    pub metrics: serde_json::Value,
}

/// Compact receipt metadata (hash + metadata only — zero private/sensitive payloads).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReceiptIngestItem {
    pub receipt_hash: String,
    pub node_id: Uuid,
    pub action_kind: String,
    pub poa_status: String,
    pub provenance_root_hash: Option<String>,
    pub occurred_at_micros: u64,
}

/// Batch ingestion of receipts metadata.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReceiptIngestBatch {
    pub node_id: Uuid,
    pub items: Vec<ReceiptIngestItem>,
    pub sent_at_micros: u64,
}

/// Signed Policy bundle waiting for deployment on edge nodes.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PolicyBundle {
    pub id: Uuid,
    pub org_id: String,
    pub name: String,
    pub version: u32,
    pub body_toml: String,
    pub signed_by_pubkey: String,
    pub signature: String,
    pub created_at_micros: u64,
}

/// Ingestion payload for redacted incident snapshots.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IncidentIngestPayload {
    pub node_id: Uuid,
    pub severity: String,
    pub snapshot: serde_json::Value,
    pub captured_at_micros: u64,
}

/// Generic ingestion response from Rivun Cloud API.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IngestResponse {
    pub status: String,
    pub accepted_count: usize,
    pub message: Option<String>,
}
