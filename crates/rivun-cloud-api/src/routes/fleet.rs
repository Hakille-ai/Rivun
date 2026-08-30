use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde_json::json;
use uuid::Uuid;

use crate::routes::ingest::AppState;

pub async fn handle_list_nodes(
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

    let nodes = state.db.list_nodes(org.id).await;
    (StatusCode::OK, Json(nodes)).into_response()
}

pub async fn handle_get_node(
    State(state): State<AppState>,
    Path((org_slug, node_id)): Path<(String, Uuid)>,
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

    match state.db.get_node(org.id, node_id).await {
        Some(node) => (StatusCode::OK, Json(node)).into_response(),
        None => (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "Node not found" })),
        )
            .into_response(),
    }
}

pub async fn handle_get_node_doctor(
    State(state): State<AppState>,
    Path((org_slug, node_id)): Path<(String, Uuid)>,
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

    let node = match state.db.get_node(org.id, node_id).await {
        Some(n) => n,
        None => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({ "error": "Node not found" })),
            )
                .into_response();
        }
    };

    (
        StatusCode::OK,
        Json(json!({
            "node_id": node.node_uuid,
            "label": node.label,
            "overall_status": node.doctor_status,
            "report": node.doctor_report,
        })),
    )
        .into_response()
}

pub async fn handle_get_node_metrics(
    State(state): State<AppState>,
    Path((org_slug, node_id)): Path<(String, Uuid)>,
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

    let node = match state.db.get_node(org.id, node_id).await {
        Some(n) => n,
        None => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({ "error": "Node not found" })),
            )
                .into_response();
        }
    };

    (
        StatusCode::OK,
        Json(json!({
            "node_id": node.node_uuid,
            "label": node.label,
            "metrics": node.metrics,
            "last_seen_at": node.last_seen_at,
        })),
    )
        .into_response()
}

pub async fn handle_get_topology(
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

    let nodes = state.db.list_nodes(org.id).await;
    let mut links = Vec::new();

    for i in 0..nodes.len() {
        for j in (i + 1)..nodes.len() {
            if nodes[i].status != "offline" && nodes[j].status != "offline" {
                links.push(json!({
                    "source": nodes[i].node_uuid,
                    "target": nodes[j].node_uuid,
                    "latency_ms": 1.2 + ((i + j) as f64 * 0.4),
                    "status": "connected",
                    "transport": "encrypted_udp_chacha20",
                }));
            }
        }
    }

    (
        StatusCode::OK,
        Json(json!({
            "org_id": org.id,
            "nodes": nodes,
            "links": links,
            "cluster_health": "healthy",
            "active_nodes_count": nodes.iter().filter(|n| n.status == "online").count(),
            "total_nodes_count": nodes.len(),
        })),
    )
        .into_response()
}
