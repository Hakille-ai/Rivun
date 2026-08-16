use std::collections::BTreeMap;
use std::fs;
use std::process::Command;
use std::sync::Arc;
use tempfile::tempdir;
use tokio::net::TcpListener;
use uuid::Uuid;

use zap_agent::{AgentId, AgentIntent, IntentKind};
use zap_crypto::Keypair;
use zap_gateway::{AgentGatewayServer, GatewayConfig, ProvenanceChainBuilder};
use zap_ledger::ReceiptJournalStore;
use zap_policy::PolicySet;

#[test]
fn test_cli_provenance_verify_with_keyfile() {
    let dir = tempdir().unwrap();
    let keypair = Keypair::generate();
    let key_file = dir.path().join("node.key");
    fs::write(&key_file, keypair.to_key_file_toml().unwrap()).unwrap();

    let session_id = Uuid::new_v4();
    let intent_id = Uuid::new_v4();
    let intent = AgentIntent::new(
        session_id,
        AgentId::new("cli_tester").unwrap(),
        IntentKind::Act,
        "CLI verification action",
    );

    let chain = ProvenanceChainBuilder::new(session_id, intent_id)
        .with_intent(&intent)
        .unwrap()
        .with_negotiation(
            &serde_json::json!({"negotiated_capability": "driver.execute:test"}),
            BTreeMap::new(),
        )
        .unwrap()
        .with_policy("policy_hash_allow", "ALLOW", BTreeMap::new())
        .unwrap()
        .with_driver("driver.test.v1", "in_hash", "out_hash", BTreeMap::new())
        .unwrap()
        .with_poa(&["sig_poa_1".to_string()], BTreeMap::new())
        .unwrap()
        .with_receipt("rcpt-001", 1_700_000_000, BTreeMap::new())
        .unwrap()
        .build_and_sign(&keypair)
        .unwrap();

    let chain_file = dir.path().join("chain.json");
    fs::write(&chain_file, serde_json::to_string_pretty(&chain).unwrap()).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_zap"))
        .args([
            "provenance",
            "verify",
            "--chain",
            chain_file.to_str().unwrap(),
            "--key",
            key_file.to_str().unwrap(),
            "--json",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["valid"], true);
    assert_eq!(json["verified_steps"], 6);
}

#[test]
fn test_cli_provenance_verify_with_public_key_hex() {
    let dir = tempdir().unwrap();
    let keypair = Keypair::generate();
    let pub_key_hex = hex::encode(keypair.verifying_key().to_bytes());

    let session_id = Uuid::new_v4();
    let intent_id = Uuid::new_v4();
    let intent = AgentIntent::new(
        session_id,
        AgentId::new("cli_tester").unwrap(),
        IntentKind::Act,
        "Public key hex action",
    );

    let chain = ProvenanceChainBuilder::new(session_id, intent_id)
        .with_intent(&intent)
        .unwrap()
        .with_policy("policy_hash", "ALLOW", BTreeMap::new())
        .unwrap()
        .with_receipt("rcpt-pk", 1_700_000_000, BTreeMap::new())
        .unwrap()
        .build_and_sign(&keypair)
        .unwrap();

    let chain_file = dir.path().join("chain_pk.json");
    fs::write(&chain_file, serde_json::to_string_pretty(&chain).unwrap()).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_zap"))
        .args([
            "provenance",
            "verify",
            "--chain",
            chain_file.to_str().unwrap(),
            "--public-key",
            &pub_key_hex,
            "--json",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["valid"], true);
    assert_eq!(json["verified_steps"], 3);
}

#[test]
fn test_cli_provenance_verify_tampered_fails() {
    let dir = tempdir().unwrap();
    let keypair = Keypair::generate();
    let pub_key_hex = hex::encode(keypair.verifying_key().to_bytes());

    let session_id = Uuid::new_v4();
    let intent_id = Uuid::new_v4();
    let intent = AgentIntent::new(
        session_id,
        AgentId::new("cli_tester").unwrap(),
        IntentKind::Act,
        "Tamper action",
    );

    let mut chain = ProvenanceChainBuilder::new(session_id, intent_id)
        .with_intent(&intent)
        .unwrap()
        .with_policy("policy_hash", "ALLOW", BTreeMap::new())
        .unwrap()
        .with_receipt("rcpt-tampered", 1_700_000_000, BTreeMap::new())
        .unwrap()
        .build_and_sign(&keypair)
        .unwrap();

    // Tamper with policy hash
    chain.steps[1].input_data_hash = "corrupted_hash".to_string();

    let chain_file = dir.path().join("tampered_chain.json");
    fs::write(&chain_file, serde_json::to_string_pretty(&chain).unwrap()).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_zap"))
        .args([
            "provenance",
            "verify",
            "--chain",
            chain_file.to_str().unwrap(),
            "--public-key",
            &pub_key_hex,
            "--json",
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
}

#[test]
fn test_cli_receipts_verify_with_provenance_flag() {
    let dir = tempdir().unwrap();
    let key = Keypair::generate();
    let journal_dir = dir.path().join("receipts");
    let sender = Keypair::generate();
    let journal = ReceiptJournalStore::open_with_keypair(&journal_dir, key.clone());
    let frame = zap_core::ZapFrame::new(
        sender.node_id(),
        key.node_id(),
        zap_core::ZapFlags::empty(),
        bytes::Bytes::from_static(b"payload"),
    )
    .unwrap();
    let signed = zap_crypto::sign_frame(&sender, &frame).unwrap();
    let receipt = zap_ledger::SignedActionReceipt::new(
        &key,
        &signed,
        "echo",
        Some(b"ok"),
        frame.header.timestamp_micros + 100,
        None,
    )
    .unwrap();
    journal.append(&receipt, false).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_zap"))
        .args([
            "receipts",
            "verify",
            "--dir",
            journal_dir.to_str().unwrap(),
            "--provenance",
            "--json",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["verified"], true);
    assert_eq!(json["provenance"], true);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_cli_gateway_status_query() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let keypair = Keypair::generate();
    let config = GatewayConfig::new(addr);
    let policy_set = Arc::new(PolicySet::default());

    let server = AgentGatewayServer::new(
        config,
        None,
        Some(Arc::new(keypair)),
        Some(policy_set),
        None,
        None,
    );

    tokio::spawn(async move {
        let _ = server.run_on_listener(listener).await;
    });

    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    // Run `zap gateway status`
    let output = Command::new(env!("CARGO_BIN_EXE_zap"))
        .args([
            "gateway",
            "status",
            "--addr",
            &format!("http://{addr}"),
            "--json",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["status"], "ok");
}
