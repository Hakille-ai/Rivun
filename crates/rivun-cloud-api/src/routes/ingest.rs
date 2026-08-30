use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde_json::json;
use uuid::Uuid;
use rivun_telemetry::FleetDoctorReport;

use crate::db::CloudDatabase;
use crate::events::EventBroker;
use crate::models::ReceiptRecord;

#[derive(Clone)]
pub struct AppState {
    pub db: CloudDatabase,
    pub events: EventBroker,
}

#[derive(serde::Deserialize)]
pub struct TelemetryPayload {
    pub node_id: Uuid,
    pub label: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    pub bridge_version: String,
    pub timestamp_micros: u64,
    pub doctor_report: FleetDoctorReport,
    pub metrics: serde_json::Value,
}

pub async fn handle_ingest_telemetry(
    State(state): State<AppState>,
    Path(org_slug): Path<String>,
    Json(payload): Json<TelemetryPayload>,
) -> impl IntoResponse {
    let org = match state.db.get_org_by_slug_or_id(&org_slug).await {
        Some(o) => o,
        None => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({ "error": "Organization not found" })),
            )
                .into_response();
        }
    };

    state
        .db
        .upsert_node_telemetry(
            org.id,
            payload.node_id,
            payload.label.clone(),
            payload.tags.clone(),
            payload.bridge_version.clone(),
            payload.doctor_report.clone(),
            payload.metrics.clone(),
        )
        .await;

    state.events.publish(
        org.id,
        "doctor_updated",
        json!({
            "node_id": payload.node_id,
            "label": payload.label,
            "overall_status": payload.doctor_report.overall_status,
            "timestamp_micros": payload.timestamp_micros,
        }),
    );

    (
        StatusCode::OK,
        Json(json!({
            "status": "accepted",
            "accepted_count": 1,
            "message": "Telemetry received and indexed"
        })),
    )
        .into_response()
}

#[derive(serde::Deserialize)]
pub struct ReceiptIngestItemReq {
    pub receipt_hash: String,
    pub action_kind: String,
    pub poa_status: String,
    pub provenance_root_hash: Option<String>,
    pub occurred_at_micros: u64,
}

#[derive(serde::Deserialize)]
pub struct ReceiptBatchReq {
    pub node_id: Uuid,
    pub items: Vec<ReceiptIngestItemReq>,
    pub sent_at_micros: u64,
}

pub async fn handle_ingest_receipts(
    State(state): State<AppState>,
    Path(org_slug): Path<String>,
    Json(batch): Json<ReceiptBatchReq>,
) -> impl IntoResponse {
    let org = match state.db.get_org_by_slug_or_id(&org_slug).await {
        Some(o) => o,
        None => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({ "error": "Organization not found" })),
            )
                .into_response();
        }
    };

    let node = state.db.get_node(org.id, batch.node_id).await;
    let node_label = node.map(|n| n.label).unwrap_or_else(|| format!("node-{}", &batch.node_id.to_string()[..8]));

    let records: Vec<ReceiptRecord> = batch
        .items
        .into_iter()
        .map(|item| {
            let occurred_at = chrono::DateTime::from_timestamp_micros(item.occurred_at_micros as i64)
                .unwrap_or_else(chrono::Utc::now);
            ReceiptRecord {
                id: Uuid::new_v4(),
                org_id: org.id,
                node_id: batch.node_id,
                node_label: node_label.clone(),
                receipt_hash: item.receipt_hash,
                action_kind: item.action_kind,
                poa_status: item.poa_status,
                provenance_root_hash: item.provenance_root_hash,
                provenance_chain: None,
                occurred_at,
            }
        })
        .collect();

    let count = records.len();
    state.db.ingest_receipts_batch(org.id, batch.node_id, records).await;

    state.events.publish(
        org.id,
        "receipts_ingested",
        json!({
            "node_id": batch.node_id,
            "node_label": node_label,
            "count": count,
        }),
    );

    (
        StatusCode::OK,
        Json(json!({
            "status": "accepted",
            "accepted_count": count,
            "message": format!("{count} receipt metadata records ingested")
        })),
    )
        .into_response()
}

pub async fn handle_get_pending_policies(
    State(state): State<AppState>,
    Path(org_slug): Path<String>,
) -> impl IntoResponse {
    let org = match state.db.get_org_by_slug_or_id(&org_slug).await {
        Some(o) => o,
        None => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({ "error": "Organization not found" })),
            )
                .into_response();
        }
    };

    let policies = state.db.get_pending_signed_policies(org.id).await;
    let bundles: Vec<serde_json::Value> = policies
        .into_iter()
        .map(|p| {
            json!({
                "id": p.id,
                "org_id": org.slug.clone(),
                "name": p.name,
                "version": p.version,
                "body_toml": p.body_toml,
                "signed_by_pubkey": p.signed_by_pubkey.unwrap_or_default(),
                "signature": p.signature.unwrap_or_default(),
                "created_at_micros": p.created_at.timestamp_micros(),
            })
        })
        .collect();

    (StatusCode::OK, Json(bundles)).into_response()
}

#[derive(serde::Deserialize)]
pub struct PolicyAckReq {
    pub node_id: Uuid,
    pub applied_version: u32,
    pub acknowledged_at_micros: u64,
}

pub async fn handle_policy_ack(
    State(state): State<AppState>,
    Path((org_slug, policy_id)): Path<(String, Uuid)>,
    Json(ack): Json<PolicyAckReq>,
) -> impl IntoResponse {
    let org = match state.db.get_org_by_slug_or_id(&org_slug).await {
        Some(o) => o,
        None => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({ "error": "Organization not found" })),
            )
                .into_response();
        }
    };

    state.events.publish(
        org.id,
        "policy_applied",
        json!({
            "policy_id": policy_id,
            "node_id": ack.node_id,
            "applied_version": ack.applied_version,
        }),
    );

    (
        StatusCode::OK,
        Json(json!({ "status": "ok", "message": "Policy deployment acknowledged" })),
    )
        .into_response()
}
