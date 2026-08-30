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

pub async fn handle_list_validators(
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

    let validator_sets = state.db.list_validator_sets(org.id).await;
    (StatusCode::OK, Json(validator_sets)).into_response()
}

#[derive(serde::Deserialize)]
pub struct RotateValidatorsReq {
    pub new_threshold: u16,
    pub proposed_members: Vec<Uuid>,
    pub reason: String,
}

pub async fn handle_rotate_validators(
    State(state): State<AppState>,
    Path(org_slug): Path<String>,
    Json(payload): Json<RotateValidatorsReq>,
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

    let proposal_id = Uuid::new_v4();

    state.db.record_audit_log(AuditLogRecord {
        id: Uuid::new_v4(),
        org_id: org.id,
        actor_email: "operator@rivun.cloud".to_string(),
        actor_role: UserRole::Operator,
        action: "validator.propose_rotation".to_string(),
        target: format!("Epoch-Proposal-{proposal_id}"),
        details: json!({
            "new_threshold": payload.new_threshold,
            "proposed_count": payload.proposed_members.len(),
            "reason": payload.reason,
        }),
        ip_address: None,
        created_at: chrono::Utc::now(),
    }).await;

    state.events.publish(
        org.id,
        "validator_rotation_proposed",
        json!({
            "proposal_id": proposal_id,
            "new_threshold": payload.new_threshold,
            "message": "Validator rotation proposed — awaiting local operator signature via Rivun Control"
        }),
    );

    (
        StatusCode::ACCEPTED,
        Json(json!({
            "status": "proposed",
            "proposal_id": proposal_id,
            "required_action": "Sign rotation certificate locally in Rivun Control",
        })),
    )
        .into_response()
}

pub async fn handle_list_attestations(
    State(_state): State<AppState>,
    Path(_org_slug): Path<String>,
) -> impl IntoResponse {
    // Generate recent PoA attestation events stream
    let now = chrono::Utc::now();
    let events = (0..10).map(|i| {
        json!({
            "attestation_id": Uuid::new_v4(),
            "epoch": 1,
            "round": 1042 + i,
            "action_kind": if i % 2 == 0 { "action.smart_building:hvac_tune" } else { "safety.emergency_brake:actuate" },
            "threshold_required": 3,
            "attestations_collected": 3,
            "status": "quorum_reached",
            "timestamp": now - chrono::Duration::seconds((i * 45) as i64),
        })
    }).collect::<Vec<_>>();

    (StatusCode::OK, Json(events)).into_response()
}
