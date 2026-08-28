use std::collections::BTreeSet;

use serde_json::Value;
use rivun_agent::{AGENT_CONTENT_TYPE, AGENT_INTENT_SUBJECT, AgentMessage};

fn fixture(name: &str) -> Value {
    let path = format!("{}/../../fixtures/{name}", env!("CARGO_MANIFEST_DIR"));
    let contents = std::fs::read_to_string(&path).expect("fixture should be readable");
    serde_json::from_str(&contents).expect("fixture should be valid JSON")
}

#[test]
fn agent_intent_fixture_matches_agent_protocol_contract() {
    let root = fixture("agent-intent-message-v1.json");

    assert_eq!(root["fixture_schema_version"], 1);
    assert_eq!(root["subject"], AGENT_INTENT_SUBJECT);
    assert_eq!(root["content_type"], AGENT_CONTENT_TYPE);

    let body = serde_json::to_vec(&root["body_json"]).expect("body_json should serialize");
    let message =
        AgentMessage::from_json_slice(&body).expect("body_json should be valid agent protocol");

    match message {
        AgentMessage::Intent(intent) => {
            assert_eq!(intent.schema_version, 1);
            assert_eq!(intent.source_agent.as_str(), "planner.main");
            assert_eq!(
                intent
                    .target_agent
                    .as_ref()
                    .expect("fixture should target an executor")
                    .as_str(),
                "executor.safety"
            );
            assert_eq!(intent.objective, "open valve");
            assert!(
                intent
                    .required_capabilities
                    .iter()
                    .any(|capability| capability.as_str() == "driver.execute:valve.open")
            );
        }
        other => panic!("expected intent fixture, got {other:?}"),
    }
}

#[test]
fn all_agent_message_fixtures_match_their_declared_subjects() {
    for name in [
        "agent-intent-message-v1.json",
        "agent-session-message-v1.json",
        "agent-delegation-request-message-v1.json",
        "agent-delegation-response-message-v1.json",
        "agent-capability-negotiation-request-message-v1.json",
        "agent-capability-negotiation-response-message-v1.json",
    ] {
        let root = fixture(name);
        assert_eq!(root["fixture_schema_version"], 1, "{name}");
        assert_eq!(root["content_type"], AGENT_CONTENT_TYPE, "{name}");
        let body = serde_json::to_vec(&root["body_json"]).expect("body_json should serialize");
        let message =
            AgentMessage::from_json_slice(&body).expect("body_json should be valid agent protocol");
        assert_eq!(
            root["subject"].as_str().expect("subject should be string"),
            message.subject(),
            "{name}"
        );
    }
}

#[test]
fn control_subjects_fixture_lists_unique_v1_control_subjects() {
    let root = fixture("control-subjects-v1.json");

    assert_eq!(root["fixture_schema_version"], 1);
    assert_eq!(root["envelope"]["magic"], "ZENV");
    assert_eq!(root["envelope"]["version"], 1);
    assert_eq!(root["envelope"]["kind_name"], "control");
    assert_eq!(root["envelope"]["kind_value"], 8);

    let subjects = root["subjects"]
        .as_array()
        .expect("subjects should be an array");
    assert!(!subjects.is_empty());

    let mut seen = BTreeSet::new();
    for entry in subjects {
        let subject = entry["subject"]
            .as_str()
            .expect("subject entry should have a subject");
        let content_type = entry["content_type"]
            .as_str()
            .expect("subject entry should have a content_type");
        let purpose = entry["purpose"]
            .as_str()
            .expect("subject entry should have a purpose");

        assert!(subject.starts_with("rivun."));
        assert!(content_type.starts_with("application/rivun-"));
        assert!(content_type.ends_with("+json"));
        assert!(!purpose.trim().is_empty());
        assert!(
            seen.insert(subject.to_string()),
            "duplicate subject {subject}"
        );
    }

    assert!(seen.contains("rivun.registry.bundle.manifest.request"));
    assert!(seen.contains("rivun.registry.bundle.manifest.response"));
    assert!(seen.contains("rivun.receipts.request"));
    assert!(seen.contains("rivun.receipts.response"));
}

#[test]
fn registry_bundle_manifest_request_fixture_matches_control_envelope_shape() {
    let root = fixture("zenv-control-registry-bundle-manifest-request.json");
    let envelope = &root["envelope"];

    assert_eq!(root["fixture_schema_version"], 1);
    assert_eq!(envelope["magic"], "ZENV");
    assert_eq!(envelope["version"], 1);
    assert_eq!(envelope["kind_name"], "control");
    assert_eq!(envelope["kind_value"], 8);
    assert_eq!(envelope["reserved"], 0);
    assert_eq!(
        envelope["correlation_id"],
        "00000000-0000-0000-0000-000000000000"
    );
    assert_eq!(
        envelope["causation_id"],
        "00000000-0000-0000-0000-000000000000"
    );
    assert_eq!(envelope["subject"], "rivun.registry.bundle.manifest.request");
    assert_eq!(
        envelope["content_type"],
        "application/rivun-registry-bundle-manifest+json"
    );
    assert_eq!(envelope["metadata_base64"], "");
    assert_eq!(envelope["body_json"]["schema_version"], 1);
    assert_eq!(envelope["body_json"]["require_publication"], true);
    assert_eq!(envelope["body_json"]["require_drivers"], true);
}

#[test]
fn unsigned_control_frame_fixture_documents_absent_security_trailers() {
    let root = fixture("protocol/zenv-unsigned-control-frame-v1.json");
    let envelope = &root["envelope"];
    let security = &root["security"];

    assert_eq!(root["fixture_schema_version"], 1);
    assert_eq!(envelope["magic"], "ZENV");
    assert_eq!(envelope["version"], 1);
    assert_eq!(envelope["kind_name"], "control");
    assert_eq!(envelope["kind_value"], 8);
    assert_eq!(envelope["reserved"], 0);
    assert_eq!(envelope["subject"], "rivun.registry.index.request");
    assert_eq!(
        envelope["content_type"],
        "application/rivun-registry-index+json"
    );
    assert_eq!(envelope["metadata_base64"], "");
    assert_eq!(envelope["body_json"]["schema_version"], 1);
    assert_eq!(envelope["body_json"]["require_signature"], false);

    assert_eq!(security["signed"], false);
    assert_eq!(security["encrypted"], false);
    assert_eq!(security["signature_hint_hex"], "0000000000000000");
    assert!(security["auth_trailer"].is_null());
    assert!(security["poa_trailer"].is_null());
}

#[test]
fn receipt_sample_fixture_has_stable_response_shape() {
    let root = fixture("protocol/receipt-sample-v1.json");
    let body = &root["body_json"];
    let receipts = body["receipts"]
        .as_array()
        .expect("receipts should be an array");

    assert_eq!(root["fixture_schema_version"], 1);
    assert_eq!(root["subject"], "rivun.receipts.response");
    assert_eq!(root["content_type"], "application/rivun-receipts+json");
    assert_eq!(body["schema_version"], 1);
    assert_eq!(body["truncated"], false);
    assert_eq!(receipts.len(), 1);

    let receipt = &receipts[0];
    assert_eq!(receipt["schema_version"], 1);
    assert_eq!(receipt["frame_id"], "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb");
    assert_eq!(receipt["subject"], "rivun.registry.index.request");
    assert_eq!(
        receipt["content_type"],
        "application/rivun-registry-index+json"
    );
    assert_eq!(receipt["policy_decision"], "allow");
    assert_eq!(receipt["outcome"], "accepted");
    assert!(
        receipt["body_hash"]
            .as_str()
            .expect("body_hash should be a string")
            .starts_with("blake3:")
    );
    assert!(
        receipt["finished_at_unix_micros"]
            .as_i64()
            .expect("finished timestamp should be numeric")
            >= receipt["started_at_unix_micros"]
                .as_i64()
                .expect("started timestamp should be numeric")
    );
}
