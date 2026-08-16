use flate2::read::GzDecoder;
use std::fs;
use std::io::Read;
use tempfile::tempdir;
use uuid::Uuid;
use zap_crypto::Keypair;
use zap_ledger::{ReceiptSegmentManifest, SignedReceiptSegmentManifest};
use zap_store::DomainPackRegistry;
use zap_telemetry::{
    ActionCounter, FleetDoctor, FleetDoctorStatus, FleetNodeHealth, FleetNodeState, FleetTopology,
    IncidentCapturer, PeerCounter, PeerTrustGauge, PrometheusExporter, ReasonCounter,
    SecretRedactor, TransportCounter, ZapNodeMetricsSnapshot,
};

#[test]
fn test_challenger_corrupted_wal_detection() {
    let dir = tempdir().unwrap();
    let mem_dir = dir.path().join("memory");
    fs::create_dir_all(&mem_dir).unwrap();

    // 1. Truncated WAL (< 8 bytes)
    let wal_truncated = mem_dir.join("truncated.wal");
    fs::write(&wal_truncated, b"SHORT").unwrap();

    let node_id = Uuid::new_v4();
    let report = FleetDoctor::evaluate(node_id, None, None, Some(&mem_dir), None);
    assert_eq!(
        report.overall_status,
        FleetDoctorStatus::Failed,
        "Truncated WAL must fail doctor evaluation"
    );
    let check = report
        .checks
        .iter()
        .find(|c| c.category == "replay_guard")
        .expect("replay_guard check exists");
    assert_eq!(check.status, FleetDoctorStatus::Failed);
    assert!(
        check
            .detail
            .as_ref()
            .unwrap()
            .contains("invalid magic header")
    );

    // 2. Corrupted Magic in WAL (8 bytes, wrong content)
    fs::remove_file(&wal_truncated).unwrap();
    let wal_corrupted = mem_dir.join("corrupted.wal");
    fs::write(&wal_corrupted, b"DEADBEEF").unwrap();

    let report2 = FleetDoctor::evaluate(node_id, None, None, Some(&mem_dir), None);
    assert_eq!(
        report2.overall_status,
        FleetDoctorStatus::Failed,
        "Corrupted magic WAL must fail doctor evaluation"
    );
    let check2 = report2
        .checks
        .iter()
        .find(|c| c.category == "replay_guard")
        .unwrap();
    assert_eq!(check2.status, FleetDoctorStatus::Failed);
    assert!(
        check2
            .detail
            .as_ref()
            .unwrap()
            .contains("invalid magic header")
    );

    // 3. Valid Magic WAL (b"ZAPFRM01")
    fs::remove_file(&wal_corrupted).unwrap();
    let wal_valid = mem_dir.join("valid.wal");
    fs::write(&wal_valid, b"ZAPFRM01valid_frame_payload_here").unwrap();

    let report3 = FleetDoctor::evaluate(node_id, None, None, Some(&mem_dir), None);
    let check3 = report3
        .checks
        .iter()
        .find(|c| c.category == "replay_guard")
        .unwrap();
    assert_eq!(check3.status, FleetDoctorStatus::Passed);
    assert!(
        check3
            .detail
            .as_ref()
            .unwrap()
            .contains("Verified 1 WAL file(s) with valid ZAPFRM01")
    );
}

#[test]
fn test_challenger_journal_manifest_and_segment_failures() {
    let dir = tempdir().unwrap();
    let receipts_dir = dir.path().join("receipts");
    fs::create_dir_all(&receipts_dir).unwrap();

    let node_id = Uuid::new_v4();

    // 1. Corrupted segment magic in .zjseg
    let seg_file = receipts_dir.join("0000000000000001.zjseg");
    fs::write(&seg_file, b"BADMAGIC010101").unwrap();

    let report = FleetDoctor::evaluate(node_id, None, Some(&receipts_dir), None, None);
    assert_eq!(
        report.overall_status,
        FleetDoctorStatus::Failed,
        "Bad segment magic must fail doctor evaluation"
    );
    let check = report
        .checks
        .iter()
        .find(|c| c.category == "journal")
        .expect("journal check exists");
    assert_eq!(check.status, FleetDoctorStatus::Failed);
    assert!(check.detail.as_ref().unwrap().contains("invalid magic"));

    // Fix segment magic
    fs::write(&seg_file, b"ZJSEG001valid_segment_data").unwrap();

    // 2. Corrupted manifest signature
    let manifest_file = receipts_dir.join("0000000000000001.zjmanifest.json.sig");
    let keypair = Keypair::generate();
    let dummy_manifest = ReceiptSegmentManifest {
        schema_version: 1,
        node_id: keypair.node_id(),
        segment_id: Uuid::new_v4(),
        segment_sequence: 1,
        receipts_count: 1,
        segment_bytes: 100,
        segment_hash: "blake3:0000000000000000000000000000000000000000000000000000000000000000"
            .to_string(),
        first_receipt_hash:
            "blake3:1111111111111111111111111111111111111111111111111111111111111111".to_string(),
        last_receipt_hash:
            "blake3:2222222222222222222222222222222222222222222222222222222222222222".to_string(),
        first_processed_at_micros: 1000,
        last_processed_at_micros: 2000,
        previous_segment_hash: None,
    };
    let signed_manifest = SignedReceiptSegmentManifest::sign(&keypair, dummy_manifest).unwrap();
    let mut manifest_json: serde_json::Value =
        serde_json::from_str(&signed_manifest.to_json_string().unwrap()).unwrap();

    // Tamper with receipts_count in signed manifest JSON
    manifest_json["manifest"]["receipts_count"] = serde_json::json!(9999);
    fs::write(
        &manifest_file,
        serde_json::to_string_pretty(&manifest_json).unwrap(),
    )
    .unwrap();

    let report2 = FleetDoctor::evaluate(node_id, None, Some(&receipts_dir), None, None);
    assert_eq!(
        report2.overall_status,
        FleetDoctorStatus::Failed,
        "Tampered manifest must fail signature verification"
    );
    let check2 = report2
        .checks
        .iter()
        .find(|c| c.category == "journal")
        .unwrap();
    assert_eq!(check2.status, FleetDoctorStatus::Failed);
    assert!(
        check2
            .detail
            .as_ref()
            .unwrap()
            .contains("signature invalid")
    );

    // 3. Valid signed manifest
    fs::write(&manifest_file, signed_manifest.to_json_string().unwrap()).unwrap();
    let report3 = FleetDoctor::evaluate(node_id, None, Some(&receipts_dir), None, None);
    let check3 = report3
        .checks
        .iter()
        .find(|c| c.category == "journal")
        .unwrap();
    assert_eq!(check3.status, FleetDoctorStatus::Passed);
    assert!(
        check3
            .detail
            .as_ref()
            .unwrap()
            .contains("Receipt journal verified: 1 segment(s), 1 signed manifest(s)")
    );
}

#[test]
fn test_challenger_invalid_pack_registry_signatures() {
    let dir = tempdir().unwrap();
    let config_path = dir.path().join("zap.toml");
    fs::write(&config_path, "[node]\n").unwrap();

    let registry_path = dir.path().join("registry.json");
    let keypair = Keypair::generate();

    let mut registry = DomainPackRegistry::empty(Some("test-generator".to_string()));

    // 1. Sign registry properly
    registry.sign(&keypair).unwrap();
    assert!(registry.verify_signature().is_ok());

    // Write valid registry
    fs::write(
        &registry_path,
        serde_json::to_string_pretty(&registry).unwrap(),
    )
    .unwrap();

    let node_id = Uuid::new_v4();
    let report = FleetDoctor::evaluate(node_id, Some(&config_path), None, None, None);
    let check = report
        .checks
        .iter()
        .find(|c| c.category == "pack_registry")
        .unwrap();
    assert_eq!(check.status, FleetDoctorStatus::Passed);
    assert!(
        check
            .detail
            .as_ref()
            .unwrap()
            .contains("verified with valid signature")
    );

    // 2. Tamper with registry content (change generated_by after signing)
    let mut tampered_json: serde_json::Value =
        serde_json::from_str(&registry.to_json_string().unwrap()).unwrap();
    tampered_json["generated_by"] = serde_json::json!("malicious_hacker");
    fs::write(
        &registry_path,
        serde_json::to_string_pretty(&tampered_json).unwrap(),
    )
    .unwrap();

    let report_tampered = FleetDoctor::evaluate(node_id, Some(&config_path), None, None, None);
    assert_eq!(
        report_tampered.overall_status,
        FleetDoctorStatus::Failed,
        "Tampered pack registry must fail doctor check"
    );
    let check_tampered = report_tampered
        .checks
        .iter()
        .find(|c| c.category == "pack_registry")
        .unwrap();
    assert_eq!(check_tampered.status, FleetDoctorStatus::Failed);
    assert!(
        check_tampered
            .detail
            .as_ref()
            .unwrap()
            .contains("Pack registry signature invalid")
    );

    // 3. Unsigned registry
    registry.signature = None;
    fs::write(
        &registry_path,
        serde_json::to_string_pretty(&registry).unwrap(),
    )
    .unwrap();
    let report_unsigned = FleetDoctor::evaluate(node_id, Some(&config_path), None, None, None);
    let check_unsigned = report_unsigned
        .checks
        .iter()
        .find(|c| c.category == "pack_registry")
        .unwrap();
    assert_eq!(check_unsigned.status, FleetDoctorStatus::Warning);
    assert!(
        check_unsigned
            .detail
            .as_ref()
            .unwrap()
            .contains("present but unsigned")
    );
}

#[test]
fn test_challenger_quorum_failure_threshold_and_degradation() {
    let local_id = Uuid::new_v4();
    let mut topology = FleetTopology::new(local_id, "test_cluster");

    // Case 1: Multi-node topology where active peers < quorum threshold (Warning)
    // 4 nodes total -> Quorum threshold = (4 * 2 / 3) + 1 = 2 + 1 = 3.
    for _ in 0..3 {
        let peer_id = Uuid::new_v4();
        topology.register_node(FleetNodeState {
            node_id: peer_id,
            addr: None,
            trust_status: "trusted".to_string(),
            health_status: FleetNodeHealth::Unreachable, // Unreachable peers
            capabilities: vec![],
            rtt_ms: None,
            last_seen_micros: 0,
        });
    }

    assert_eq!(topology.nodes.len(), 4);
    assert_eq!(topology.active_peer_count(), 0);

    let report = FleetDoctor::evaluate(local_id, None, None, None, Some(&topology));
    let cert_check = report
        .checks
        .iter()
        .find(|c| c.category == "certificate_validity")
        .unwrap();
    // 1 active node (local) < quorum threshold (3) -> Warning
    assert_eq!(cert_check.status, FleetDoctorStatus::Warning);
    assert!(
        cert_check
            .detail
            .as_ref()
            .unwrap()
            .contains("below quorum threshold (3/4)")
    );

    // Case 2: Mark peers active so active nodes >= threshold -> Passed
    for node in topology.nodes.values_mut() {
        if node.node_id != local_id {
            node.health_status = FleetNodeHealth::Healthy;
        }
    }
    assert_eq!(topology.active_peer_count(), 3);

    let report_passed = FleetDoctor::evaluate(local_id, None, None, None, Some(&topology));
    let cert_check_passed = report_passed
        .checks
        .iter()
        .find(|c| c.category == "certificate_validity")
        .unwrap();
    assert_eq!(cert_check_passed.status, FleetDoctorStatus::Passed);
    assert!(
        cert_check_passed
            .detail
            .as_ref()
            .unwrap()
            .contains("validator quorum threshold met")
    );
}

#[test]
fn test_challenger_secret_redactor_complex_edge_cases() {
    // Test 1: Multiple nested PEM keys (different header labels)
    let multi_pem = r#"
-----BEGIN EC PRIVATE KEY-----
MHQCAQEEIB4f...private_ec_key_data...
-----END EC PRIVATE KEY-----

-----BEGIN RSA PRIVATE KEY-----
MIIEowIBAAKCAQEA0...private_rsa_key_data...
-----END RSA PRIVATE KEY-----

-----BEGIN OPENSSH PRIVATE KEY-----
b3BlbnNzaC1rZXktdjEAAAA...private_ssh_key_data...
-----END OPENSSH PRIVATE KEY-----
"#;
    let redacted_pem = SecretRedactor::redact_text(multi_pem);
    assert!(!redacted_pem.contains("private_ec_key_data"));
    assert!(!redacted_pem.contains("private_rsa_key_data"));
    assert!(!redacted_pem.contains("private_ssh_key_data"));
    assert!(redacted_pem.contains("-----BEGIN EC PRIVATE KEY-----"));
    assert!(redacted_pem.contains("-----END EC PRIVATE KEY-----"));
    assert!(redacted_pem.contains("-----BEGIN RSA PRIVATE KEY-----"));
    assert!(redacted_pem.contains("-----END RSA PRIVATE KEY-----"));

    // Test 2: Complex JSON with various whitespace, quotes, single quotes, arrays
    let complex_json = r#"{
        "service": "zap-node",
        "private_key":   "sk_test_1234567890abcdef",
        "api_key": 'token_xyz_987654',
        "bearer_token": "secret_bearer_value",
        "public_data": "visible_info",
        "secret": 12345678,
        "nested": {
            "auth_token": "bearer secret_inside_nested"
        }
    }"#;
    let redacted_json = SecretRedactor::redact_text(complex_json);
    assert!(!redacted_json.contains("sk_test_1234567890abcdef"));
    assert!(!redacted_json.contains("token_xyz_987654"));
    assert!(!redacted_json.contains("secret_bearer_value"));
    assert!(!redacted_json.contains("secret_inside_nested"));
    assert!(redacted_json.contains("visible_info"));

    // Test 3: Standalone 64-char Hexadecimal tokens
    let raw_hex_text = "Key fingerprint: e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855 is valid.";
    let redacted_hex = SecretRedactor::redact_text(raw_hex_text);
    assert!(
        !redacted_hex.contains("e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855")
    );
    assert!(redacted_hex.contains("[REDACTED_SECRET_KEY]"));
    assert!(redacted_hex.contains("Key fingerprint: [REDACTED_SECRET_KEY] is valid."));
}

#[test]
fn test_challenger_tar_gz_decompression_and_contents() {
    let node_id = Uuid::new_v4();
    let metrics = "zap_replay_drops_total 42\nzap_peers_active 3\n";
    let snapshot = IncidentCapturer::capture(node_id, metrics, None);

    let gz_bytes = IncidentCapturer::build_tar_gz_archive(&snapshot).unwrap();

    // Verify gzip magic
    assert_eq!(&gz_bytes[0..2], &[0x1f, 0x8b]);

    // Decompress with GzDecoder
    let mut decoder = GzDecoder::new(&gz_bytes[..]);
    let mut tar_bytes = Vec::new();
    decoder.read_to_end(&mut tar_bytes).unwrap();

    assert_eq!(
        tar_bytes.len() % 512,
        0,
        "Decompressed tar must be 512-byte aligned"
    );

    // Inspect tarball entries
    let mut offset = 0;
    let mut found_snapshot = false;
    let mut found_metrics = false;
    let mut found_diagnostics = false;
    let mut found_config = false;
    let mut found_health = false;

    while offset + 512 <= tar_bytes.len() {
        let block = &tar_bytes[offset..offset + 512];
        if block.iter().all(|&b| b == 0) {
            // Reached zero blocks (end of archive)
            break;
        }

        // Parse filename
        let name_bytes = &block[0..100];
        let name_len = name_bytes.iter().position(|&b| b == 0).unwrap_or(100);
        let name = std::str::from_utf8(&name_bytes[..name_len]).unwrap();

        // Parse size (octal)
        let size_str = std::str::from_utf8(&block[124..135]).unwrap().trim();
        let size = usize::from_str_radix(size_str, 8).unwrap();

        // Check magic
        let magic = std::str::from_utf8(&block[257..262]).unwrap();
        assert_eq!(magic, "ustar");

        offset += 512;
        let content = &tar_bytes[offset..offset + size];

        match name {
            "snapshot.json" => {
                found_snapshot = true;
                let parsed: serde_json::Value = serde_json::from_slice(content).unwrap();
                assert_eq!(parsed["node_id"].as_str().unwrap(), node_id.to_string());
            }
            "metrics.prom" => {
                found_metrics = true;
                let s = std::str::from_utf8(content).unwrap();
                assert!(s.contains("zap_replay_drops_total 42"));
            }
            "diagnostics.txt" => {
                found_diagnostics = true;
                let s = std::str::from_utf8(content).unwrap();
                assert!(s.contains(&format!("ZAP Incident Snapshot Node ID: {node_id}")));
            }
            "config.redacted.toml" => {
                found_config = true;
            }
            "health.json" => {
                found_health = true;
                let parsed: serde_json::Value = serde_json::from_slice(content).unwrap();
                assert_eq!(parsed["status"].as_str().unwrap(), "healthy");
            }
            _ => {}
        }

        let padding = if !size.is_multiple_of(512) {
            512 - (size % 512)
        } else {
            0
        };
        offset += size + padding;
    }

    assert!(found_snapshot, "snapshot.json must be in archive");
    assert!(found_metrics, "metrics.prom must be in archive");
    assert!(found_diagnostics, "diagnostics.txt must be in archive");
    assert!(found_config, "config.redacted.toml must be in archive");
    assert!(found_health, "health.json must be in archive");
}

#[test]
fn test_challenger_prometheus_escaping_and_all_fields() {
    let node_id = Uuid::new_v4();
    let peer_1 = Uuid::new_v4();
    let peer_2 = Uuid::new_v4();

    let snapshot = ZapNodeMetricsSnapshot {
        node_id,
        frames_sent_total: vec![
            PeerCounter {
                peer: peer_1,
                value: 100,
            },
            PeerCounter {
                peer: peer_2,
                value: 200,
            },
        ],
        frames_received_total: vec![PeerCounter {
            peer: peer_1,
            value: 50,
        }],
        frames_rejected_total: vec![ReasonCounter {
            reason: "invalid_signature\nwith_quote\"and\\backslash".to_string(),
            value: 12,
        }],
        driver_execution_errors_total: vec![ActionCounter {
            action: "action_\"wasm\"_exec".to_string(),
            value: 4,
        }],
        peer_trust_status: vec![
            PeerTrustGauge {
                peer: peer_1,
                status: "trusted".to_string(),
                value: 1,
            },
            PeerTrustGauge {
                peer: peer_2,
                status: "quarantined".to_string(),
                value: 0,
            },
        ],
        registry_signature_valid: Some(1),
        capability_cache_age_seconds: Some(3600),
        receipt_log_verify_failures_total: 2,
        poa_attestation_failures_total: 1,
        replay_rejections_total: 10,
        replay_drops_total: 10,
        journal_segment_rotations_total: 5,
        segment_manifest_errors_total: 0,
        pack_verification_failures_total: 0,
        store_verifications_total: 25,
        agent_gateway_requests_total: vec![
            TransportCounter {
                transport: "mcp_stdio".to_string(),
                status: "200".to_string(),
                value: 50,
            },
            TransportCounter {
                transport: "http_rest".to_string(),
                status: "500".to_string(),
                value: 2,
            },
        ],
        agent_sessions_active: 7,
        provenance_verification_failures_total: 0,
        peers_active: 2,
    };

    let text = PrometheusExporter::export(&snapshot);

    // Verify Prometheus label escaping
    assert!(
        text.contains("reason=\"invalid_signature\\nwith_quote\\\"and\\\\backslash\""),
        "Prometheus text must escape newlines, quotes, and backslashes"
    );
    assert!(
        text.contains("action=\"action_\\\"wasm\\\"_exec\""),
        "Prometheus text must escape quotes in action names"
    );

    // Verify all 17 metric lines are present
    assert!(text.contains("zap_frames_sent_total{"));
    assert!(text.contains("zap_frames_received_total{"));
    assert!(text.contains("zap_frames_rejected_total{"));
    assert!(text.contains("zap_driver_execution_errors_total{"));
    assert!(text.contains("zap_peer_trust_status{"));
    assert!(text.contains("zap_registry_signature_valid{"));
    assert!(text.contains("zap_capability_cache_age_seconds{"));
    assert!(text.contains("zap_receipt_log_verify_failures_total{"));
    assert!(text.contains("zap_poa_attestation_failures_total{"));
    assert!(text.contains("zap_replay_rejections_total{"));
    assert!(text.contains("zap_replay_drops_total{"));
    assert!(text.contains("zap_journal_segment_rotations_total{"));
    assert!(text.contains("zap_segment_manifest_errors_total{"));
    assert!(text.contains("zap_pack_verification_failures_total{"));
    assert!(text.contains("zap_store_verifications_total{"));
    assert!(text.contains("zap_agent_gateway_requests_total{"));
    assert!(text.contains("zap_agent_sessions_active{"));
    assert!(text.contains("zap_provenance_verification_failures_total{"));
    assert!(text.contains("zap_peers_active{"));
}
