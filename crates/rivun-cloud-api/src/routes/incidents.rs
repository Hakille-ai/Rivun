use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde_json::json;
use uuid::Uuid;

use crate::models::IncidentRecord;
use crate::routes::ingest::AppState;

pub async fn handle_list_incidents(
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

    let incidents = state.db.list_incidents(org.id).await;
    (StatusCode::OK, Json(incidents)).into_response()
}

#[derive(serde::Deserialize)]
pub struct CreateIncidentReq {
    pub node_id: Uuid,
    pub severity: String,
    pub snapshot: serde_json::Value,
}

pub async fn handle_create_incident(
    State(state): State<AppState>,
    Path(org_slug): Path<String>,
    Json(payload): Json<CreateIncidentReq>,
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

    let node = state.db.get_node(org.id, payload.node_id).await;
    let node_label = node.map(|n| n.label).unwrap_or_else(|| format!("node-{}", &payload.node_id.to_string()[..8]));

    let incident_id = Uuid::new_v4();
    let record = IncidentRecord {
        id: incident_id,
        org_id: org.id,
        node_id: payload.node_id,
        node_label: node_label.clone(),
        severity: payload.severity.clone(),
        snapshot: payload.snapshot.clone(),
        resolved: false,
        created_at: chrono::Utc::now(),
    };

    state.events.publish(
        org.id,
        "incident_fired",
        json!({
            "incident_id": incident_id,
            "node_id": payload.node_id,
            "node_label": node_label,
            "severity": payload.severity,
        }),
    );

    (StatusCode::CREATED, Json(record)).into_response()
}
