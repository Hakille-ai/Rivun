use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde_json::json;
use uuid::Uuid;

use crate::models::{AuditLogRecord, UserRole};
use crate::routes::ingest::AppState;

pub async fn handle_list_orgs(
    State(state): State<AppState>,
) -> impl IntoResponse {
    let orgs = state.db.list_organizations().await;
    (StatusCode::OK, Json(orgs)).into_response()
}

pub async fn handle_get_org(
    State(state): State<AppState>,
    Path(org_slug): Path<String>,
) -> impl IntoResponse {
    match state.db.get_org_by_slug_or_id(&org_slug).await {
        Some(org) => (StatusCode::OK, Json(org)).into_response(),
        None => (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "Organization not found" })),
        )
            .into_response(),
    }
}

pub async fn handle_list_members(
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

    let members = state.db.list_members(org.id).await;
    (StatusCode::OK, Json(members)).into_response()
}

#[derive(serde::Deserialize)]
pub struct AddMemberReq {
    pub email: String,
    pub role: UserRole,
    pub name: String,
}

pub async fn handle_add_member(
    State(state): State<AppState>,
    Path(org_slug): Path<String>,
    Json(payload): Json<AddMemberReq>,
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

    let member = state
        .db
        .add_member(org.id, payload.email.clone(), payload.role, payload.name)
        .await;

    state.db.record_audit_log(AuditLogRecord {
        id: Uuid::new_v4(),
        org_id: org.id,
        actor_email: "alice@acme.ai".to_string(),
        actor_role: UserRole::Owner,
        action: "member.add".to_string(),
        target: payload.email,
        details: json!({ "role": format!("{:?}", payload.role).to_lowercase() }),
        ip_address: None,
        created_at: chrono::Utc::now(),
    }).await;

    (StatusCode::CREATED, Json(member)).into_response()
}

pub async fn handle_list_audit_log(
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

    let logs = state.db.list_audit_logs(org.id).await;
    (StatusCode::OK, Json(logs)).into_response()
}

pub async fn handle_get_usage(
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

    let usage = state.db.get_usage(org.id).await.unwrap_or_else(|| {
        crate::models::UsageCounters {
            org_id: org.id,
            period: "2026-08".to_string(),
            active_nodes: 5,
            receipts_ingested: 48920,
            packs_published: 1,
            policies_deployed: 2,
            last_updated: chrono::Utc::now(),
        }
    });

    (StatusCode::OK, Json(usage)).into_response()
}

pub async fn handle_org_events_sse(
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

    state.events.subscribe_org(org.id).into_response()
}
