use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde_json::json;
use uuid::Uuid;

use crate::models::{AuditLogRecord, PackRecord, UserRole};
use crate::routes::ingest::AppState;

pub async fn handle_list_packs(
    State(state): State<AppState>,
) -> impl IntoResponse {
    let packs = state.db.list_packs(None).await;
    (StatusCode::OK, Json(packs)).into_response()
}

pub async fn handle_list_org_packs(
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

    let packs = state.db.list_packs(Some(org.id)).await;
    (StatusCode::OK, Json(packs)).into_response()
}

#[derive(serde::Deserialize)]
pub struct PublishPackReq {
    pub name: String,
    pub version: String,
    pub category: String,
    pub description: String,
    pub author: String,
    pub manifest_hash: String,
    pub signature: String,
    pub visibility: String,
}

pub async fn handle_publish_pack(
    State(_state): State<AppState>,
    Json(payload): Json<PublishPackReq>,
) -> impl IntoResponse {
    let pack_id = Uuid::new_v4();
    let record = PackRecord {
        id: pack_id,
        org_id: None,
        name: payload.name.clone(),
        version: payload.version,
        category: payload.category,
        description: payload.description,
        author: payload.author,
        manifest_hash: payload.manifest_hash,
        signature: Some(payload.signature),
        visibility: payload.visibility,
        published_by: "publisher".to_string(),
        published_at: chrono::Utc::now(),
        downloads: 0,
    };

    (StatusCode::CREATED, Json(record)).into_response()
}

#[derive(serde::Deserialize)]
pub struct InstallPackReq {
    pub pack_id: Uuid,
    pub target_nodes: Vec<Uuid>,
}

pub async fn handle_install_pack(
    State(state): State<AppState>,
    Path(org_slug): Path<String>,
    Json(payload): Json<InstallPackReq>,
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

    state.db.record_audit_log(AuditLogRecord {
        id: Uuid::new_v4(),
        org_id: org.id,
        actor_email: "operator@rivun.cloud".to_string(),
        actor_role: UserRole::Operator,
        action: "pack.install".to_string(),
        target: format!("Pack-{}", payload.pack_id),
        details: json!({ "target_nodes": payload.target_nodes }),
        ip_address: None,
        created_at: chrono::Utc::now(),
    }).await;

    state.events.publish(
        org.id,
        "pack_installed",
        json!({
            "pack_id": payload.pack_id,
            "target_nodes_count": payload.target_nodes.len(),
            "status": "dispatched",
        }),
    );

    (
        StatusCode::ACCEPTED,
        Json(json!({
            "status": "dispatched",
            "pack_id": payload.pack_id,
            "target_nodes": payload.target_nodes,
            "message": "Pack bundle installation staged for fleet sync"
        })),
    )
        .into_response()
}
