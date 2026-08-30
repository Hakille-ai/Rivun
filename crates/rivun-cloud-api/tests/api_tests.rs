use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use tower::ServiceExt;
use uuid::Uuid;
use rivun_cloud_api::{build_app, AppState, CloudDatabase, EventBroker};
use rivun_telemetry::FleetDoctor;

#[tokio::test]
async fn test_full_cloud_api_lifecycle() {
    let db = CloudDatabase::new();
    db.seed_demo_data().await;
    let events = EventBroker::new(64);
    let app = build_app(AppState { db, events });

    // 1. Check healthz
    let req = Request::builder()
        .uri("/healthz")
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // 2. List nodes for org "acme"
    let req = Request::builder()
        .uri("/v1/orgs/acme/nodes")
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let nodes: Vec<Value> = serde_json::from_slice(&bytes).unwrap();
    assert!(nodes.len() >= 5);

    // 3. Ingest Telemetry from a new node
    let new_node_id = Uuid::new_v4();
    let doctor_report = FleetDoctor::evaluate(new_node_id, None, None, None, None);
    let telemetry_payload = json!({
        "node_id": new_node_id,
        "label": "cloud-test-node-01",
        "tags": ["env:test"],
        "bridge_version": "0.1.0",
        "timestamp_micros": 100000,
        "doctor_report": doctor_report,
        "metrics": { "actions_count": 10 }
    });

    let req = Request::builder()
        .method("POST")
        .uri("/v1/orgs/acme/ingest/telemetry")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_vec(&telemetry_payload).unwrap()))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // 4. Ingest Receipts batch
    let batch_payload = json!({
        "node_id": new_node_id,
        "items": [
            {
                "receipt_hash": "0xdeadbeef1122334455667788",
                "action_kind": "action.test:run",
                "poa_status": "verified",
                "provenance_root_hash": "0xroot9999",
                "occurred_at_micros": 200000
            }
        ],
        "sent_at_micros": 200500
    });

    let req = Request::builder()
        .method("POST")
        .uri("/v1/orgs/acme/ingest/receipts")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_vec(&batch_payload).unwrap()))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // 5. Query Receipts
    let req = Request::builder()
        .uri("/v1/orgs/acme/receipts?kind=action.test")
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let receipts: Vec<Value> = serde_json::from_slice(&bytes).unwrap();
    assert!(!receipts.is_empty());

    // 6. Create Policy Draft -> Stage -> Sign
    let policy_toml = r#"default_decision = "deny"
[[rules]]
name = "allow_all_telemetry"
kind = "telemetry"
decision = "allow"
"#;
    let create_policy_payload = json!({
        "name": "staging-test-policy",
        "body_toml": policy_toml,
        "creator": "alice@acme.ai"
    });

    let req = Request::builder()
        .method("POST")
        .uri("/v1/orgs/acme/policies")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_vec(&create_policy_payload).unwrap()))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let created_policy: Value = serde_json::from_slice(&bytes).unwrap();
    let policy_id = created_policy["id"].as_str().unwrap();

    // Stage policy
    let req = Request::builder()
        .method("POST")
        .uri(format!("/v1/orgs/acme/policies/{}/stage", policy_id))
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // Sign policy
    let sign_payload = json!({
        "public_key": "ed25519_operator_pk_base64",
        "signature": "ed25519_operator_sig_base64"
    });
    let req = Request::builder()
        .method("POST")
        .uri(format!("/v1/orgs/acme/policies/{}/sign", policy_id))
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_vec(&sign_payload).unwrap()))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // 7. Check pending policies for edge bridge
    let req = Request::builder()
        .uri("/v1/orgs/acme/policies/pending")
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let pending: Vec<Value> = serde_json::from_slice(&bytes).unwrap();
    assert!(pending.iter().any(|p| p["id"] == policy_id));

    // 8. Marketplace packs list
    let req = Request::builder()
        .uri("/v1/registry/packs")
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let packs: Vec<Value> = serde_json::from_slice(&bytes).unwrap();
    assert!(packs.len() >= 7);
}
