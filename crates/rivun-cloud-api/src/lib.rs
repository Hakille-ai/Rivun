//! Rivun Cloud API Library
//!
//! Multi-tenant zero-trust SaaS API for Rivun/ZAP.

pub mod db;
pub mod events;
pub mod models;
pub mod routes;

use axum::{
    routing::{get, post},
    Router,
};
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;

pub use db::CloudDatabase;
pub use events::EventBroker;
pub use routes::ingest::AppState;

pub fn build_app(state: AppState) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    Router::new()
        // Health & System
        .route("/healthz", get(|| async { "ok" }))
        .route("/v1/status", get(|| async {
            axum::Json(serde_json::json!({
                "service": "rivun-cloud-api",
                "version": "0.1.0",
                "status": "operational",
                "zero_trust_invariants": "enforced"
            }))
        }))
        // Ingest Endpoints (Bridge -> Cloud)
        .route("/v1/orgs/{org}/ingest/telemetry", post(routes::ingest::handle_ingest_telemetry))
        .route("/v1/orgs/{org}/ingest/receipts", post(routes::ingest::handle_ingest_receipts))
        .route("/v1/orgs/{org}/policies/pending", get(routes::ingest::handle_get_pending_policies))
        .route("/v1/orgs/{org}/policies/{id}/ack", post(routes::ingest::handle_policy_ack))
        // Fleet & Doctor
        .route("/v1/orgs/{org}/nodes", get(routes::fleet::handle_list_nodes))
        .route("/v1/orgs/{org}/nodes/{id}", get(routes::fleet::handle_get_node))
        .route("/v1/orgs/{org}/nodes/{id}/doctor", get(routes::fleet::handle_get_node_doctor))
        .route("/v1/orgs/{org}/nodes/{id}/metrics", get(routes::fleet::handle_get_node_metrics))
        .route("/v1/orgs/{org}/topology", get(routes::fleet::handle_get_topology))
        // Receipts & Provenance Ledger
        .route("/v1/orgs/{org}/receipts", get(routes::receipts::handle_list_receipts))
        .route("/v1/orgs/{org}/receipts/{hash}/provenance", get(routes::receipts::handle_get_receipt_provenance))
        .route("/v1/orgs/{org}/receipts/verify", post(routes::receipts::handle_verify_receipt))
        // Policies & Signature Staging
        .route("/v1/orgs/{org}/policies", get(routes::policies::handle_list_policies).post(routes::policies::handle_create_policy))
        .route("/v1/orgs/{org}/policies/{id}/stage", post(routes::policies::handle_stage_policy))
        .route("/v1/orgs/{org}/policies/{id}/sign", post(routes::policies::handle_sign_policy))
        .route("/v1/orgs/{org}/policies/{id}/diff", get(routes::policies::handle_get_policy_diff))
        // Validators & Consensus
        .route("/v1/orgs/{org}/validators", get(routes::validators::handle_list_validators))
        .route("/v1/orgs/{org}/validators/rotate", post(routes::validators::handle_rotate_validators))
        .route("/v1/orgs/{org}/validators/attestations", get(routes::validators::handle_list_attestations))
        // Domain Packs Registry & Marketplace
        .route("/v1/registry/packs", get(routes::marketplace::handle_list_packs).post(routes::marketplace::handle_publish_pack))
        .route("/v1/orgs/{org}/packs", get(routes::marketplace::handle_list_org_packs))
        .route("/v1/orgs/{org}/packs/install", post(routes::marketplace::handle_install_pack))
        // Incidents & Forensics
        .route("/v1/orgs/{org}/incidents", get(routes::incidents::handle_list_incidents).post(routes::incidents::handle_create_incident))
        // Organizations, Members, Audit & Usage
        .route("/v1/orgs", get(routes::orgs::handle_list_orgs))
        .route("/v1/orgs/{org}", get(routes::orgs::handle_get_org))
        .route("/v1/orgs/{org}/members", get(routes::orgs::handle_list_members).post(routes::orgs::handle_add_member))
        .route("/v1/orgs/{org}/audit-log", get(routes::orgs::handle_list_audit_log))
        .route("/v1/orgs/{org}/usage", get(routes::orgs::handle_get_usage))
        // Real-Time SSE Stream
        .route("/v1/orgs/{org}/events", get(routes::orgs::handle_org_events_sse))
        .layer(cors)
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}
