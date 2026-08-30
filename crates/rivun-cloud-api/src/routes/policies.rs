use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde_json::json;
use uuid::Uuid;

use crate::models::{AuditLogRecord, PolicyStatus, UserRole};
use crate::routes::ingest::AppState;

pub async fn handle_list_policies(
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

    let policies = state.db.list_policies(org.id).await;
    (StatusCode::OK, Json(policies)).into_response()
}

#[derive(serde::Deserialize)]
pub struct CreatePolicyReq {
    pub name: String,
    pub body_toml: String,
    pub creator: Option<String>,
}

pub async fn handle_create_policy(
    State(state): State<AppState>,
    Path(org_slug): Path<String>,
    Json(payload): Json<CreatePolicyReq>,
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

    let creator = payload.creator.unwrap_or_else(|| "operator@rivun.cloud".to_string());
    match state
        .db
        .create_policy(org.id, payload.name.clone(), payload.body_toml, creator.clone())
        .await
    {
        Ok(policy) => {
            state.db.record_audit_log(AuditLogRecord {
                id: Uuid::new_v4(),
                org_id: org.id,
                actor_email: creator,
                actor_role: UserRole::Operator,
                action: "policy.create".to_string(),
                target: payload.name,
                details: json!({ "policy_id": policy.id, "status": "draft" }),
                ip_address: None,
                created_at: chrono::Utc::now(),
            }).await;

            (StatusCode::CREATED, Json(policy)).into_response()
        }
        Err(err) => (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": err })),
        )
            .into_response(),
    }
}

pub async fn handle_stage_policy(
    State(state): State<AppState>,
    Path((org_slug, policy_id)): Path<(String, Uuid)>,
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

    match state.db.stage_policy(org.id, policy_id).await {
        Ok(policy) => {
            state.events.publish(
                org.id,
                "policy_staged",
                json!({
                    "policy_id": policy.id,
                    "name": policy.name,
                    "version": policy.version,
                    "message": "Policy staged — awaiting local operator Ed25519 signature via Rivun Control"
                }),
            );

            state.db.record_audit_log(AuditLogRecord {
                id: Uuid::new_v4(),
                org_id: org.id,
                actor_email: "operator@rivun.cloud".to_string(),
                actor_role: UserRole::Operator,
                action: "policy.stage".to_string(),
                target: policy.name.clone(),
                details: json!({ "policy_id": policy.id, "status": "staged" }),
                ip_address: None,
                created_at: chrono::Utc::now(),
            }).await;

            (StatusCode::OK, Json(policy)).into_response()
        }
        Err(err) => (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": err })),
        )
            .into_response(),
    }
}

#[derive(serde::Deserialize)]
pub struct SignPolicyReq {
    pub public_key: String,
    pub signature: String,
}

pub async fn handle_sign_policy(
    State(state): State<AppState>,
    Path((org_slug, policy_id)): Path<(String, Uuid)>,
    Json(payload): Json<SignPolicyReq>,
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

    match state
        .db
        .submit_policy_signature(org.id, policy_id, payload.public_key.clone(), payload.signature.clone())
        .await
    {
        Ok(policy) => {
            state.events.publish(
                org.id,
                "policy_signed",
                json!({
                    "policy_id": policy.id,
                    "name": policy.name,
                    "signed_by_pubkey": payload.public_key,
                }),
            );

            state.db.record_audit_log(AuditLogRecord {
                id: Uuid::new_v4(),
                org_id: org.id,
                actor_email: "operator@rivun.cloud".to_string(),
                actor_role: UserRole::Operator,
                action: "policy.sign".to_string(),
                target: policy.name.clone(),
                details: json!({
                    "policy_id": policy.id,
                    "signed_by": payload.public_key,
                    "status": "signed"
                }),
                ip_address: None,
                created_at: chrono::Utc::now(),
            }).await;

            (StatusCode::OK, Json(policy)).into_response()
        }
        Err(err) => (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": err })),
        )
            .into_response(),
    }
}

pub async fn handle_get_policy_diff(
    State(state): State<AppState>,
    Path((org_slug, policy_id)): Path<(String, Uuid)>,
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

    let target_policy = match state.db.get_policy(org.id, policy_id).await {
        Some(p) => p,
        None => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({ "error": "Policy not found" })),
            )
                .into_response();
        }
    };

    let active_policy = state
        .db
        .list_policies(org.id)
        .await
        .into_iter()
        .find(|p| p.status == PolicyStatus::Active);

    let active_toml = active_policy
        .as_ref()
        .map(|p| p.body_toml.clone())
        .unwrap_or_default();

    (
        StatusCode::OK,
        Json(json!({
            "policy_id": target_policy.id,
            "target_version": target_policy.version,
            "target_status": target_policy.status,
            "target_toml": target_policy.body_toml,
            "active_version": active_policy.map(|p| p.version),
            "active_toml": active_toml,
        })),
    )
        .into_response()
}
