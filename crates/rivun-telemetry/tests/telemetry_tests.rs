use tempfile::tempdir;
use uuid::Uuid;
use rivun_telemetry::{
    ActionCounter, FleetDoctor, FleetDoctorStatus, FleetNodeHealth, FleetNodeState, FleetTopology,
    IncidentCapturer, PeerCounter, PeerTrustGauge, PrometheusExporter, ReasonCounter,
    SecretRedactor, TransportCounter, RivunNodeMetricsSnapshot,
};

#[test]
fn test_metrics_parity_all_16_metrics() {
    let node_id = Uuid::new_v4();
    let peer_id = Uuid::new_v4();

    let snapshot = RivunNodeMetricsSnapshot {
        node_id,
        frames_sent_total: vec![PeerCounter {
            peer: peer_id,
            value: 42,
        }],
        frames_received_total: vec![PeerCounter {
            peer: peer_id,
            value: 100,
        }],
        frames_rejected_total: vec![ReasonCounter {
            reason: "invalid_signature".to_string(),
            value: 3,
        }],
        driver_execution_errors_total: vec![ActionCounter {
            action: "exec_wasm".to_string(),
            value: 1,
        }],
        peer_trust_status: vec![PeerTrustGauge {
            peer: peer_id,
            status: "trusted".to_string(),
            value: 1,
        }],
        registry_signature_valid: Some(1),
        capability_cache_age_seconds: Some(120),
        receipt_log_verify_failures_total: 0,
        poa_attestation_failures_total: 0,
        replay_rejections_total: 5,
        replay_drops_total: 5,
        journal_segment_rotations_total: 2,
        segment_manifest_errors_total: 0,
        pack_verification_failures_total: 1,
        store_verifications_total: 10,
        agent_gateway_requests_total: vec![TransportCounter {
            transport: "mcp_stdio".to_string(),
            status: "200".to_string(),
            value: 15,
        }],
        agent_sessions_active: 3,
        provenance_verification_failures_total: 0,
        peers_active: 2,
    };

    let text = PrometheusExporter::export(&snapshot);

    assert!(text.contains("rivun_frames_sent_total"));
    assert!(text.contains("rivun_frames_received_total"));
    assert!(text.contains("rivun_frames_rejected_total"));
    assert!(text.contains("rivun_driver_execution_errors_total"));
    assert!(text.contains("rivun_peer_trust_status"));
    assert!(text.contains("rivun_registry_signature_valid"));
    assert!(text.contains("rivun_capability_cache_age_seconds"));
    assert!(text.contains("rivun_receipt_log_verify_failures_total"));
    assert!(text.contains("rivun_poa_attestation_failures_total"));
    assert!(text.contains("rivun_replay_rejections_total"));
    assert!(text.contains("rivun_replay_drops_total"));
    assert!(text.contains("rivun_journal_segment_rotations_total"));
    assert!(text.contains("rivun_segment_manifest_errors_total"));
    assert!(text.contains("rivun_pack_verification_failures_total"));
    assert!(text.contains("rivun_store_verifications_total"));
    assert!(text.contains("rivun_agent_gateway_requests_total"));
    assert!(text.contains("rivun_agent_sessions_active"));
    assert!(text.contains("rivun_provenance_verification_failures_total"));
    assert!(text.contains("rivun_peers_active"));
}

#[test]
fn test_fleet_doctor_evaluation_6_criteria() {
    let node_id = Uuid::new_v4();
    let mut topology = FleetTopology::new(node_id, "test_cluster");

    let peer_id = Uuid::new_v4();
    topology.register_node(FleetNodeState {
        node_id: peer_id,
        addr: None,
        trust_status: "trusted".to_string(),
        health_status: FleetNodeHealth::Healthy,
        capabilities: vec!["core".to_string()],
        rtt_ms: Some(10),
        last_seen_micros: 1000,
    });

    let report = FleetDoctor::evaluate(node_id, None, None, None, Some(&topology));

    assert_eq!(report.node_id, node_id);
    assert_eq!(report.checks.len(), 7);

    let categories: Vec<String> = report.checks.iter().map(|c| c.category.clone()).collect();
    assert!(categories.contains(&"network".to_string()));
    assert!(categories.contains(&"storage".to_string()));
    assert!(categories.contains(&"replay_guard".to_string()));
    assert!(categories.contains(&"journal".to_string()));
    assert!(categories.contains(&"pack_registry".to_string()));
    assert!(categories.contains(&"certificate_validity".to_string()));
    assert!(categories.contains(&"peer_trust".to_string()));
}

#[test]
fn test_incident_capturer_redaction_and_tar_archive() {
    let dir = tempdir().unwrap();
    let config_path = dir.path().join("rivun.toml");

    let raw_config = r#"
[node]
node_id = "12345678-1234-1234-1234-1234567890ab"

[security]
private_key = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
auth_token = "secret_bearer_token_xyz"
"#;
    std::fs::write(&config_path, raw_config).unwrap();

    let node_id = Uuid::new_v4();
    let metrics = "rivun_replay_rejections_total 1\n";

    let snapshot = IncidentCapturer::capture(node_id, metrics, Some(&config_path));

    assert_eq!(snapshot.node_id, node_id);
    let cfg_redacted = snapshot
        .config_summary
        .get("config_content_redacted")
        .unwrap();
    assert!(
        !cfg_redacted.contains("0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef")
    );
    assert!(cfg_redacted.contains("[REDACTED]"));

    let tar_bytes = IncidentCapturer::build_tar_archive(&snapshot).unwrap();
    assert!(tar_bytes.len() > 1024);
}

#[test]
fn test_secret_redactor() {
    let input = "private_key = \"supersecretkey123\"\nnode_id = \"node1\"\n";
    let redacted = SecretRedactor::redact_text(input);
    assert!(!redacted.contains("supersecretkey123"));
    assert!(redacted.contains("private_key = \"[REDACTED]\""));
}

#[test]
fn test_fleet_doctor_evaluation_corrupted_wal_and_manifests() {
    let dir = tempdir().unwrap();
    let mem_dir = dir.path().join("memory");
    let receipts_dir = dir.path().join("receipts");
    std::fs::create_dir_all(&mem_dir).unwrap();
    std::fs::create_dir_all(&receipts_dir).unwrap();

    // Corrupted WAL file
    let wal_file = mem_dir.join("frames.wal");
    std::fs::write(&wal_file, b"BADMAGICCORRUPT").unwrap();

    // Corrupted manifest file
    let manifest_file = receipts_dir.join("0001.zjmanifest.json.sig");
    std::fs::write(&manifest_file, r#"{"manifest":{"node_id":"00000000-0000-0000-0000-000000000000","segment_id":"00000000-0000-0000-0000-000000000000","segment_sequence":1,"schema_version":1,"receipts_count":1,"first_processed_at_micros":0,"last_processed_at_micros":0,"segment_hash":"blake3:0000000000000000000000000000000000000000000000000000000000000000","previous_segment_hash":null},"signer_node_id":"00000000-0000-0000-0000-000000000000","signer_public_key":"AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA","signature":"AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"}"#).unwrap();

    let node_id = Uuid::new_v4();
    let report = FleetDoctor::evaluate(node_id, None, Some(&receipts_dir), Some(&mem_dir), None);

    assert_eq!(report.overall_status, FleetDoctorStatus::Failed);
    let replay_check = report
        .checks
        .iter()
        .find(|c| c.category == "replay_guard")
        .unwrap();
    assert_eq!(replay_check.status, FleetDoctorStatus::Failed);

    let journal_check = report
        .checks
        .iter()
        .find(|c| c.category == "journal")
        .unwrap();
    assert_eq!(journal_check.status, FleetDoctorStatus::Failed);
}
