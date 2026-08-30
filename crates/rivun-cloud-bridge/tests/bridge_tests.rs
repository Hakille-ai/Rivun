use base64::{Engine as _, engine::general_purpose::STANDARD_NO_PAD};
use tempfile::NamedTempFile;
use uuid::Uuid;
use rivun_cloud_bridge::{
    POLICY_BUNDLE_SIGNATURE_DOMAIN, PolicyBundle, PolicyVerifier, ReceiptIngestItem,
    TelemetryIngestPayload,
};
use rivun_crypto::Keypair;
use rivun_policy::PolicySet;
use rivun_telemetry::FleetDoctor;

#[test]
fn test_policy_bundle_verification_and_atomic_apply() {
    let operator_keypair = Keypair::generate();
    let operator_pubkey_b64 = STANDARD_NO_PAD.encode(operator_keypair.verifying_key().to_bytes());

    let policy_toml = r#"
default_decision = "deny"

[[rules]]
name = "allow_telemetry"
kind = "telemetry"
decision = "allow"

[[rules]]
name = "require_poa_safety"
subject = "safety.*"
decision = "require_poa"
"#;

    let bundle = PolicyBundle {
        id: Uuid::new_v4(),
        org_id: "acme-corp".to_string(),
        name: "production-security".to_string(),
        version: 1,
        body_toml: policy_toml.to_string(),
        signed_by_pubkey: operator_pubkey_b64.clone(),
        signature: "".to_string(),
        created_at_micros: 1000,
    };

    let signing_msg = PolicyVerifier::compute_signing_message(&bundle);
    let signature_bytes =
        operator_keypair.sign_domain_message(POLICY_BUNDLE_SIGNATURE_DOMAIN, &signing_msg);
    let signature_b64 = STANDARD_NO_PAD.encode(signature_bytes);

    let signed_bundle = PolicyBundle {
        signature: signature_b64,
        ..bundle
    };

    // 1. Verify bundle
    let authorized = vec![operator_pubkey_b64.clone()];
    let verified_policy = PolicyVerifier::verify_bundle(&signed_bundle, &authorized).unwrap();
    assert_eq!(verified_policy.rules.len(), 2);

    // 2. Test Atomic Apply to temp file
    let tmp = NamedTempFile::new().unwrap();
    let applied =
        PolicyVerifier::apply_bundle_to_path(&signed_bundle, tmp.path(), &authorized).unwrap();
    assert_eq!(applied.rules.len(), 2);

    let read_back = std::fs::read_to_string(tmp.path()).unwrap();
    let parsed_back = PolicySet::from_toml_str(&read_back).unwrap();
    assert_eq!(parsed_back.rules.len(), 2);

    // 3. Test tampering rejection
    let mut tampered_bundle = signed_bundle.clone();
    tampered_bundle.body_toml = r#"default_decision = "allow""#.to_string();
    assert!(PolicyVerifier::verify_bundle(&tampered_bundle, &authorized).is_err());

    // 4. Test unauthorized operator rejection
    let unauthorized = vec!["some_other_pubkey".to_string()];
    assert!(PolicyVerifier::verify_bundle(&signed_bundle, &unauthorized).is_err());
}

#[test]
fn test_telemetry_and_receipt_models() {
    let node_id = Uuid::new_v4();
    let report = FleetDoctor::evaluate(node_id, None, None, None, None);

    let telemetry = TelemetryIngestPayload {
        node_id,
        public_key: None,
        label: Some("edge-node-alpha".to_string()),
        tags: vec!["datacenter:fra1".to_string()],
        bridge_version: "0.1.0".to_string(),
        timestamp_micros: 123456789,
        doctor_report: report,
        metrics: serde_json::json!({ "actions_total": 42 }),
    };

    let serialized = serde_json::to_string(&telemetry).unwrap();
    assert!(serialized.contains("edge-node-alpha"));

    let receipt_item = ReceiptIngestItem {
        receipt_hash: "abcd1234efgh5678".to_string(),
        node_id,
        action_kind: "order.settle".to_string(),
        poa_status: "verified".to_string(),
        provenance_root_hash: Some("root_hash_9999".to_string()),
        occurred_at_micros: 987654321,
    };

    let item_json = serde_json::to_string(&receipt_item).unwrap();
    assert!(item_json.contains("order.settle"));
}
