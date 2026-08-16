//! Adversarial Challenger Test Suite for Milestone 4
//! Verified targets:
//! 1. 6-stage ProvenanceChainDigest causal integrity & per-stage link tampering detection
//! 2. SSE event stream broadcasting, multi-line formatting, high-fanout concurrency, HTTP streaming
//! 3. WebSocket RFC 6455 duplex exchange, ping/pong, frame size boundary enforcement (code 1009)
//! 4. Full E2E AI Agent workflow (Session -> Negotiate -> Intent -> Policy -> Journal Receipt -> Provenance -> Verify -> MCP)

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use std::sync::Mutex;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use uuid::Uuid;

use zap_agent::{
    AGENT_PROTOCOL_SCHEMA_VERSION, AgentId, AgentIntent, CapabilityNegotiationRequest,
    DelegationRequest, IntentKind,
};
use zap_core::now_micros;
use zap_crypto::Keypair;
use zap_gateway::{
    GatewayConfig, HttpAgentGateway, ProvenanceChainBuilder, ProvenanceChainDigest,
    ProvenanceStage, SseBroker, SseEvent, WebSocketHandler, WsFrame, compute_ws_accept,
};
use zap_ledger::ReceiptJournalStore;
use zap_memory::MemoryJournalStore;
use zap_policy::PolicySet;

// ============================================================================
// 1. 6-STAGE PROVENANCE CHAIN & LINK TAMPERING DETECTION
// ============================================================================

#[tokio::test]
async fn test_empirical_6_stage_provenance_causal_chain_integrity() {
    let keypair = Keypair::generate();
    let session_id = Uuid::new_v4();
    let intent_id = Uuid::new_v4();

    let mut intent = AgentIntent::new(
        session_id,
        AgentId::new("agent.empirical").unwrap(),
        IntentKind::Act,
        "Execute empirical multi-stage transaction",
    );
    intent.intent_id = intent_id;

    let chain = ProvenanceChainBuilder::new(session_id, intent_id)
        .with_intent(&intent)
        .expect("Intent stage must build")
        .with_negotiation(
            &serde_json::json!({
                "protocol": "zap.agent.negotiation.v1",
                "capabilities": ["driver.execute:compute", "memory.journal:read"]
            }),
            BTreeMap::from([("agent_role".to_string(), serde_json::json!("orchestrator"))]),
        )
        .expect("Negotiation stage must build")
        .with_policy(
            "sha256:d5a4c9b8e217036578efb9231456acde0987123456789abcdef0123456789abc",
            "ALLOW",
            BTreeMap::from([(
                "rule_matched".to_string(),
                serde_json::json!("policy.default.allow"),
            )]),
        )
        .expect("Policy stage must build")
        .with_driver(
            "driver.matrix_compute.v2",
            "input_sha256_hash_value_1234567890abcdef",
            "output_sha256_hash_value_fedcba0987654321",
            BTreeMap::from([("wasm_memory_pages".to_string(), serde_json::json!(16))]),
        )
        .expect("Driver stage must build")
        .with_poa(
            &[
                "poa_attestation_sig_validator_node_1".to_string(),
                "poa_attestation_sig_validator_node_2".to_string(),
                "poa_attestation_sig_validator_node_3".to_string(),
            ],
            BTreeMap::from([("quorum_threshold".to_string(), serde_json::json!("3/3"))]),
        )
        .expect("PoA stage must build")
        .with_receipt(
            "rcpt-tx-998877665544332211",
            1_700_500_000_000,
            BTreeMap::from([("journal_segment_id".to_string(), serde_json::json!(42))]),
        )
        .expect("Receipt stage must build")
        .build_and_sign(&keypair)
        .expect("Chain must sign cleanly");

    // Assert structural invariants
    assert_eq!(chain.steps.len(), 6);
    assert_eq!(chain.schema_version, 1);
    assert_eq!(chain.session_id, session_id);
    assert_eq!(chain.intent_id, intent_id);
    assert_eq!(chain.node_id, keypair.node_id());

    // Verify all individual stages via verify_step
    let stages = [
        ProvenanceStage::Intent,
        ProvenanceStage::Negotiation,
        ProvenanceStage::Policy,
        ProvenanceStage::Driver,
        ProvenanceStage::Poa,
        ProvenanceStage::Receipt,
    ];
    for stage in stages {
        assert!(
            chain.verify_step(stage).is_ok(),
            "Step {:?} must individually verify",
            stage
        );
    }

    // Verify complete chain report
    let report = chain.verify(&keypair.verifying_key()).unwrap();
    assert!(report.valid, "Complete chain must report valid");
    assert_eq!(report.verified_steps, 6);
    assert!(report.failed_stage.is_none());
    assert!(report.failure_reason.is_none());
    assert_eq!(report.root_hash, chain.root_hash);
    assert_eq!(report.node_id, keypair.node_id());
}

#[tokio::test]
async fn test_empirical_adversarial_tamper_matrix_all_6_stages() {
    let keypair = Keypair::generate();
    let session_id = Uuid::new_v4();
    let intent_id = Uuid::new_v4();

    let mut intent = AgentIntent::new(
        session_id,
        AgentId::new("agent.adversary").unwrap(),
        IntentKind::Act,
        "Tamper challenge intent",
    );
    intent.intent_id = intent_id;

    let base_chain = ProvenanceChainBuilder::new(session_id, intent_id)
        .with_intent(&intent)
        .unwrap()
        .with_negotiation(&serde_json::json!({"negotiate": "cap.v1"}), BTreeMap::new())
        .unwrap()
        .with_policy("policy_hash_1", "ALLOW", BTreeMap::new())
        .unwrap()
        .with_driver("driver_v1", "in_hash", "out_hash", BTreeMap::new())
        .unwrap()
        .with_poa(&["sig_a".to_string()], BTreeMap::new())
        .unwrap()
        .with_receipt("rcpt_1", 1_700_000_000, BTreeMap::new())
        .unwrap()
        .build_and_sign(&keypair)
        .unwrap();

    let stages = [
        (0, ProvenanceStage::Intent),
        (1, ProvenanceStage::Negotiation),
        (2, ProvenanceStage::Policy),
        (3, ProvenanceStage::Driver),
        (4, ProvenanceStage::Poa),
        (5, ProvenanceStage::Receipt),
    ];

    // 1. Test tampering input_data_hash for EACH stage (0..5)
    for (idx, expected_stage) in stages {
        let mut tampered = base_chain.clone();
        tampered.steps[idx].input_data_hash = format!("corrupted_input_hash_{idx}");

        let report = tampered.verify(&keypair.verifying_key()).unwrap();
        assert!(
            !report.valid,
            "Tampering input_data_hash at stage {:?} must invalidate chain",
            expected_stage
        );
        assert_eq!(
            report.failed_stage,
            Some(expected_stage),
            "Failed stage must match {:?} exactly",
            expected_stage
        );
        assert!(
            report.failure_reason.is_some(),
            "Failure reason must be populated"
        );
    }

    // 2. Test tampering previous_hash for intermediate stages (1..5)
    for (idx, expected_stage) in &stages[1..] {
        let mut tampered = base_chain.clone();
        tampered.steps[*idx].previous_hash = Some("broken_causal_link_hash".to_string());

        let report = tampered.verify(&keypair.verifying_key()).unwrap();
        assert!(
            !report.valid,
            "Breaking previous_hash link at stage {:?} must invalidate chain",
            expected_stage
        );
        assert_eq!(
            report.failed_stage,
            Some(*expected_stage),
            "Failed stage must match {:?} on broken link",
            expected_stage
        );
    }

    // 3. Test tampering root_hash directly
    let mut tampered_root = base_chain.clone();
    tampered_root.root_hash =
        "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".to_string();
    let report_root = tampered_root.verify(&keypair.verifying_key()).unwrap();
    assert!(!report_root.valid);
    assert!(
        report_root
            .failure_reason
            .unwrap()
            .contains("Merkle root mismatch")
    );

    // 4. Test signer key mismatch
    let other_keypair = Keypair::generate();
    let report_signer = base_chain.verify(&other_keypair.verifying_key()).unwrap();
    assert!(!report_signer.valid);
    assert!(
        report_signer
            .failure_reason
            .unwrap()
            .contains("Signer node ID mismatch")
    );

    // 5. Test signature byte flipping
    let mut tampered_sig = base_chain.clone();
    let mut sig_bytes = hex::decode(&tampered_sig.signature).unwrap();
    sig_bytes[10] ^= 0xAA;
    tampered_sig.signature = hex::encode(sig_bytes);
    let report_sig = tampered_sig.verify(&keypair.verifying_key()).unwrap();
    assert!(!report_sig.valid);
    assert!(
        report_sig
            .failure_reason
            .unwrap()
            .contains("Ed25519 signature")
    );
}

// ============================================================================
// 2. SSE EVENT STREAM BROADCASTING
// ============================================================================

#[test]
fn test_empirical_sse_event_wire_formatting_and_multiline() {
    let single_line = SseEvent::new("status_update", r#"{"running":true}"#).with_id("evt-101");
    let wire = single_line.to_sse_wire_format();
    assert_eq!(
        wire,
        "id: evt-101\nevent: status_update\ndata: {\"running\":true}\n\n"
    );

    let mut multiline = SseEvent::new("log_chunk", "line 1\nline 2\nline 3");
    multiline.retry_ms = Some(5000);
    let ml_wire = multiline.to_sse_wire_format();
    assert_eq!(
        ml_wire,
        "retry: 5000\nevent: log_chunk\ndata: line 1\ndata: line 2\ndata: line 3\n\n"
    );
}

#[tokio::test]
async fn test_empirical_sse_broker_high_fanout_concurrency() {
    let broker = SseBroker::new(256);
    let subscriber_count = 20;
    let mut receivers = Vec::new();

    for _ in 0..subscriber_count {
        receivers.push(broker.subscribe());
    }
    assert_eq!(broker.subscriber_count(), subscriber_count);

    // Broadcast 20 events
    for i in 0..20 {
        broker.send(SseEvent::new(
            "agent_telemetry",
            format!(r#"{{"sequence":{i}}}"#),
        ));
    }

    // Verify all 20 subscribers receive all 20 events in order
    for mut rx in receivers {
        for expected_seq in 0..20 {
            let event = rx.recv().await.expect("Subscriber should receive event");
            assert_eq!(event.event_type, "agent_telemetry");
            assert_eq!(event.data, format!(r#"{{"sequence":{expected_seq}}}"#));
        }
    }
}

#[tokio::test]
async fn test_empirical_sse_http_streaming_over_wire() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let keypair = Keypair::generate();
    let config = GatewayConfig::new(addr);
    let sse_broker = SseBroker::default();

    let gateway = Arc::new(HttpAgentGateway::new(
        config,
        None,
        Some(Arc::new(keypair)),
        Arc::new(PolicySet::default()),
        None,
        None,
        sse_broker.clone(),
    ));

    tokio::spawn(async move {
        let _ = gateway.run_server(listener).await;
    });

    let mut stream = TcpStream::connect(addr).await.unwrap();
    let req = format!(
        "GET /v1/agent/stream HTTP/1.1\r\nHost: {}\r\nAccept: text/event-stream\r\n\r\n",
        addr
    );
    stream.write_all(req.as_bytes()).await.unwrap();

    let mut buf = vec![0u8; 2048];
    let n = stream.read(&mut buf).await.unwrap();
    let initial_resp = String::from_utf8_lossy(&buf[..n]);

    assert!(initial_resp.starts_with("HTTP/1.1 200 OK"));
    assert!(initial_resp.contains("Content-Type: text/event-stream"));
    assert!(initial_resp.contains("event: connected"));
    assert!(initial_resp.contains(r#"{"status":"ready"}"#));

    // Send a status event and read from wire
    sse_broker.send(SseEvent::new("agent_status", r#"{"state":"processing"}"#));

    let n = stream.read(&mut buf).await.unwrap();
    let event_wire = String::from_utf8_lossy(&buf[..n]);
    assert!(event_wire.contains("event: agent_status"));
    assert!(event_wire.contains(r#"{"state":"processing"}"#));
}

// ============================================================================
// 3. WEBSOCKET DUPLEX EXCHANGE & FRAMING BOUNDARIES
// ============================================================================

#[tokio::test]
async fn test_empirical_ws_duplex_handshake_and_frames() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let keypair = Keypair::generate();
    let config = GatewayConfig::new(addr);

    let gateway = Arc::new(HttpAgentGateway::new(
        config,
        None,
        Some(Arc::new(keypair)),
        Arc::new(PolicySet::default()),
        None,
        None,
        SseBroker::default(),
    ));

    tokio::spawn(async move {
        let _ = gateway.run_server(listener).await;
    });

    let mut stream = TcpStream::connect(addr).await.unwrap();
    let test_ws_key = "x3JJHMbDL1EzLkh9GBhXDw==";
    let expected_accept = compute_ws_accept(test_ws_key);

    let req = format!(
        "GET /v1/agent/ws HTTP/1.1\r\n\
Host: {}\r\n\
Upgrade: websocket\r\n\
Connection: Upgrade\r\n\
Sec-WebSocket-Key: {}\r\n\
Sec-WebSocket-Version: 13\r\n\r\n",
        addr, test_ws_key
    );
    stream.write_all(req.as_bytes()).await.unwrap();

    let mut resp_buf = vec![0u8; 1024];
    let n = stream.read(&mut resp_buf).await.unwrap();
    let handshake_resp = String::from_utf8_lossy(&resp_buf[..n]);

    assert!(handshake_resp.starts_with("HTTP/1.1 101 Switching Protocols"));
    assert!(handshake_resp.contains(&format!("Sec-WebSocket-Accept: {}", expected_accept)));

    let ws_handler = WebSocketHandler::default_max();

    // 1. Send Text Frame -> Receive Ack Frame
    let text_msg = r#"{"agent":"tester","payload":"execute_action"}"#;
    let text_frame = WsFrame::text(text_msg);
    ws_handler
        .write_frame(&mut stream, &text_frame)
        .await
        .unwrap();

    let ack_frame = ws_handler.read_frame(&mut stream).await.unwrap();
    assert_eq!(
        ack_frame.opcode,
        zap_gateway::transports::ws::WS_OPCODE_TEXT
    );
    let ack_json: serde_json::Value = serde_json::from_slice(&ack_frame.payload).unwrap();
    assert_eq!(ack_json["status"], "acknowledged");
    assert_eq!(ack_json["bytes_received"], text_msg.len());

    // 2. Send Ping -> Receive Pong
    let ping_data = b"ping_test_payload_123".to_vec();
    let ping_frame = WsFrame::ping(ping_data.clone());
    ws_handler
        .write_frame(&mut stream, &ping_frame)
        .await
        .unwrap();

    let pong_frame = ws_handler.read_frame(&mut stream).await.unwrap();
    assert_eq!(
        pong_frame.opcode,
        zap_gateway::transports::ws::WS_OPCODE_PONG
    );
    assert_eq!(pong_frame.payload, ping_data);

    // 3. Send Close -> Receive Close
    let close_frame = WsFrame::close(1000, "client_closing");
    ws_handler
        .write_frame(&mut stream, &close_frame)
        .await
        .unwrap();

    let server_close = ws_handler.read_frame(&mut stream).await.unwrap();
    assert_eq!(
        server_close.opcode,
        zap_gateway::transports::ws::WS_OPCODE_CLOSE
    );
}

#[tokio::test]
async fn test_empirical_ws_frame_size_overflow_rejection() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let keypair = Keypair::generate();
    // Configure tiny 512-byte max frame limit
    let config = GatewayConfig::new(addr).with_max_frame_size(512);

    let gateway = Arc::new(HttpAgentGateway::new(
        config,
        None,
        Some(Arc::new(keypair)),
        Arc::new(PolicySet::default()),
        None,
        None,
        SseBroker::default(),
    ));

    tokio::spawn(async move {
        let _ = gateway.run_server(listener).await;
    });

    let mut stream = TcpStream::connect(addr).await.unwrap();
    let ws_key = "dGhlIHNhbXBsZSBub25jZQ==";
    let req = format!(
        "GET /v1/agent/ws HTTP/1.1\r\nHost: {}\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: {}\r\nSec-WebSocket-Version: 13\r\n\r\n",
        addr, ws_key
    );
    stream.write_all(req.as_bytes()).await.unwrap();
    let mut resp_buf = vec![0u8; 1024];
    let _ = stream.read(&mut resp_buf).await.unwrap();

    // Send a 1024-byte payload using a client handler that allows larger writes
    let client_ws_handler = WebSocketHandler::new(4096);
    let oversized_payload = "A".repeat(1024);
    let oversized_frame = WsFrame::text(oversized_payload);
    client_ws_handler
        .write_frame(&mut stream, &oversized_frame)
        .await
        .unwrap();

    // Server must reject with WS_CLOSE_MESSAGE_TOO_BIG (1009)
    let close_reply = client_ws_handler.read_frame(&mut stream).await.unwrap();
    assert_eq!(
        close_reply.opcode,
        zap_gateway::transports::ws::WS_OPCODE_CLOSE
    );
    let close_code = u16::from_be_bytes([close_reply.payload[0], close_reply.payload[1]]);
    assert_eq!(close_code, 1009); // WS_CLOSE_MESSAGE_TOO_BIG
}

// ============================================================================
// 4. FULL E2E AI AGENT WORKFLOW
// ============================================================================

#[tokio::test]
async fn test_empirical_full_e2e_ai_agent_workflow() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let keypair = Arc::new(Keypair::generate());
    let config = GatewayConfig::new(addr);
    let policy_set = Arc::new(PolicySet::default());
    let sse_broker = SseBroker::default();

    let temp_dir = tempfile::tempdir().unwrap();
    let memory_store = Arc::new(Mutex::new(MemoryJournalStore::open(
        temp_dir.path().join("memory"),
    )));
    let journal_store = Arc::new(Mutex::new(ReceiptJournalStore::open_with_keypair(
        temp_dir.path().join("receipts"),
        (*keypair).clone(),
    )));

    let gateway = Arc::new(HttpAgentGateway::new(
        config,
        None,
        Some(keypair.clone()),
        policy_set.clone(),
        Some(journal_store.clone()),
        Some(memory_store.clone()),
        sse_broker.clone(),
    ));

    let gw_clone = gateway.clone();
    tokio::spawn(async move {
        let _ = gw_clone.run_server(listener).await;
    });

    let mut sse_rx = sse_broker.subscribe();

    // Step 1: Initialize Session via POST /v1/agent/sessions
    let session_id = Uuid::new_v4();
    let session_body = serde_json::json!({
        "session_id": session_id.to_string(),
        "owner_agent": "orchestrator_prime"
    });
    let s_json = serde_json::to_string(&session_body).unwrap();
    let mut stream = TcpStream::connect(addr).await.unwrap();
    let req = format!(
        "POST /v1/agent/sessions HTTP/1.1\r\nHost: {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        addr,
        s_json.len(),
        s_json
    );
    stream.write_all(req.as_bytes()).await.unwrap();
    let mut buf = vec![0u8; 2048];
    let n = stream.read(&mut buf).await.unwrap();
    let s_resp = String::from_utf8_lossy(&buf[..n]);
    assert!(s_resp.starts_with("HTTP/1.1 201 Created"));

    // Step 2: Negotiate Capabilities via POST /v1/agent/negotiate
    let neg_req = CapabilityNegotiationRequest {
        schema_version: AGENT_PROTOCOL_SCHEMA_VERSION,
        negotiation_id: Uuid::new_v4(),
        session_id,
        requester_agent: AgentId::new("orchestrator_prime").unwrap(),
        required_capabilities: BTreeSet::from([zap_capability::CapabilityId::new(
            "driver.execute:matrix_mul",
        )
        .unwrap()]),
        optional_capabilities: BTreeSet::new(),
        desired_intents: BTreeSet::from([IntentKind::Act]),
        metadata: BTreeMap::new(),
    };
    let n_json = serde_json::to_string(&neg_req).unwrap();
    let mut stream = TcpStream::connect(addr).await.unwrap();
    let req = format!(
        "POST /v1/agent/negotiate HTTP/1.1\r\nHost: {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        addr,
        n_json.len(),
        n_json
    );
    stream.write_all(req.as_bytes()).await.unwrap();
    let n = stream.read(&mut buf).await.unwrap();
    let n_resp = String::from_utf8_lossy(&buf[..n]);
    assert!(n_resp.starts_with("HTTP/1.1 200 OK"));
    assert!(n_resp.contains("accepted"));

    // Step 3: Submit Agent Intent via POST /v1/agent/intents
    let mut intent = AgentIntent::new(
        session_id,
        AgentId::new("orchestrator_prime").unwrap(),
        IntentKind::Act,
        "Execute distributed physics simulation step",
    );
    intent.input = serde_json::json!({"particles": 10000, "dt": 0.01});
    let intent_json = serde_json::to_string(&intent).unwrap();

    let mut stream = TcpStream::connect(addr).await.unwrap();
    let req = format!(
        "POST /v1/agent/intents HTTP/1.1\r\nHost: {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        addr,
        intent_json.len(),
        intent_json
    );
    stream.write_all(req.as_bytes()).await.unwrap();
    let n = stream.read(&mut buf).await.unwrap();
    let resp_str = String::from_utf8_lossy(&buf[..n]);
    assert!(resp_str.starts_with("HTTP/1.1 202 Accepted"));

    // Parse response body containing the provenance chain
    let body_start = resp_str.find("\r\n\r\n").map(|p| p + 4).unwrap_or(0);
    let resp_json: serde_json::Value = serde_json::from_str(&resp_str[body_start..]).unwrap();
    assert_eq!(resp_json["status"], "accepted");
    assert!(resp_json["provenance"].is_object());

    let chain: ProvenanceChainDigest =
        serde_json::from_value(resp_json["provenance"].clone()).unwrap();

    // Step 4: Verify SSE event broadcast
    let sse_event = sse_rx.recv().await.unwrap();
    assert_eq!(sse_event.event_type, "agent_status");
    let sse_payload: serde_json::Value = serde_json::from_str(&sse_event.data).unwrap();
    assert_eq!(sse_payload["status"], "accepted");
    assert_eq!(sse_payload["session_id"], session_id.to_string());

    // Step 5: Verify Provenance via POST /v1/agent/provenance/verify
    let mut stream = TcpStream::connect(addr).await.unwrap();
    let chain_json = serde_json::to_string(&chain).unwrap();
    let req = format!(
        "POST /v1/agent/provenance/verify HTTP/1.1\r\nHost: {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        addr,
        chain_json.len(),
        chain_json
    );
    stream.write_all(req.as_bytes()).await.unwrap();
    let n = stream.read(&mut buf).await.unwrap();
    let v_resp_str = String::from_utf8_lossy(&buf[..n]);
    let v_body_start = v_resp_str.find("\r\n\r\n").map(|p| p + 4).unwrap_or(0);
    let v_report: serde_json::Value = serde_json::from_str(&v_resp_str[v_body_start..]).unwrap();
    assert_eq!(v_report["valid"], true);
    assert_eq!(v_report["node_id"], keypair.node_id().to_string());

    // Step 6: Query Receipts via GET /v1/agent/receipts
    let mut stream = TcpStream::connect(addr).await.unwrap();
    let req = format!(
        "GET /v1/agent/receipts HTTP/1.1\r\nHost: {}\r\nConnection: close\r\n\r\n",
        addr
    );
    stream.write_all(req.as_bytes()).await.unwrap();
    let n = stream.read(&mut buf).await.unwrap();
    let r_resp_str = String::from_utf8_lossy(&buf[..n]);
    assert!(r_resp_str.starts_with("HTTP/1.1 200 OK"));
    assert!(r_resp_str.contains("receipts"));

    // Step 7: Subtask Delegation via POST /v1/agent/delegate
    let del_req = DelegationRequest {
        schema_version: AGENT_PROTOCOL_SCHEMA_VERSION,
        delegation_id: Uuid::new_v4(),
        session_id,
        parent_intent_id: intent.intent_id,
        from_agent: AgentId::new("orchestrator_prime").unwrap(),
        to_agent: Some(AgentId::new("worker_sub_1").unwrap()),
        objective: "Compute physics slice [0..5000]".to_string(),
        required_capabilities: BTreeSet::new(),
        constraints: Vec::new(),
        context: Vec::new(),
        deadline_unix_micros: Some(now_micros().unwrap_or(0) + 60_000_000),
        metadata: BTreeMap::new(),
    };
    let d_json = serde_json::to_string(&del_req).unwrap();
    let mut stream = TcpStream::connect(addr).await.unwrap();
    let req = format!(
        "POST /v1/agent/delegate HTTP/1.1\r\nHost: {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        addr,
        d_json.len(),
        d_json
    );
    stream.write_all(req.as_bytes()).await.unwrap();
    let n = stream.read(&mut buf).await.unwrap();
    let d_resp_str = String::from_utf8_lossy(&buf[..n]);
    assert!(d_resp_str.starts_with("HTTP/1.1 200 OK"));
    assert!(d_resp_str.contains("worker_sub_1"));

    // Step 8: MCP Protocol Tool Call via POST /v1/agent/mcp
    let mcp_call = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 999,
        "method": "tools/call",
        "params": {
            "name": "zap_agent_intent",
            "arguments": {
                "session_id": session_id.to_string(),
                "source_agent": "orchestrator_prime",
                "kind": "act",
                "objective": "MCP-dispatched subtask intent"
            }
        }
    });
    let m_json = serde_json::to_string(&mcp_call).unwrap();
    let mut stream = TcpStream::connect(addr).await.unwrap();
    let req = format!(
        "POST /v1/agent/mcp HTTP/1.1\r\nHost: {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        addr,
        m_json.len(),
        m_json
    );
    stream.write_all(req.as_bytes()).await.unwrap();
    let mut buf = vec![0u8; 8192];
    let n = stream.read(&mut buf).await.unwrap();
    let m_resp_str = String::from_utf8_lossy(&buf[..n]);
    assert!(m_resp_str.starts_with("HTTP/1.1 200 OK"));
    assert!(m_resp_str.contains("accepted"));
    assert!(m_resp_str.contains("provenance"));
}

#[tokio::test]
async fn test_empirical_out_of_order_and_missing_link_rejection() {
    let keypair = Keypair::generate();
    let session_id = Uuid::new_v4();
    let intent_id = Uuid::new_v4();

    let mut intent = AgentIntent::new(
        session_id,
        AgentId::new("agent.order_test").unwrap(),
        IntentKind::Act,
        "Order test objective",
    );
    intent.intent_id = intent_id;

    let valid_chain = ProvenanceChainBuilder::new(session_id, intent_id)
        .with_intent(&intent)
        .unwrap()
        .with_negotiation(&serde_json::json!({"cap": 1}), BTreeMap::new())
        .unwrap()
        .with_policy("policy_hash", "ALLOW", BTreeMap::new())
        .unwrap()
        .with_driver("driver_id", "in", "out", BTreeMap::new())
        .unwrap()
        .with_poa(&["sig1".to_string()], BTreeMap::new())
        .unwrap()
        .with_receipt("rcpt-1", 1_700_000_000, BTreeMap::new())
        .unwrap()
        .build_and_sign(&keypair)
        .unwrap();

    // 1. Swap Step 1 (Negotiation) and Step 2 (Policy) -> Causal break detected
    let mut swapped_chain = valid_chain.clone();
    swapped_chain.steps.swap(1, 2);
    let report_swapped = swapped_chain.verify(&keypair.verifying_key()).unwrap();
    assert!(!report_swapped.valid);
    assert_eq!(report_swapped.failed_stage, Some(ProvenanceStage::Policy));

    // 2. Set previous_hash = None on an intermediate step (e.g. Driver step)
    let mut missing_link_chain = valid_chain.clone();
    missing_link_chain.steps[3].previous_hash = None;
    let report_missing = missing_link_chain.verify(&keypair.verifying_key()).unwrap();
    assert!(!report_missing.valid);
    assert_eq!(report_missing.failed_stage, Some(ProvenanceStage::Driver));
    assert!(
        report_missing
            .failure_reason
            .unwrap()
            .contains("missing previous_hash link")
    );

    // 3. Set previous_hash on Intent (Step 0) -> rejected
    let mut bad_intent_chain = valid_chain.clone();
    bad_intent_chain.steps[0].previous_hash = Some("illegal_prev_hash_on_root".to_string());
    let report_bad_intent = bad_intent_chain.verify(&keypair.verifying_key()).unwrap();
    assert!(!report_bad_intent.valid);
    assert_eq!(
        report_bad_intent.failed_stage,
        Some(ProvenanceStage::Intent)
    );
    assert!(
        report_bad_intent
            .failure_reason
            .unwrap()
            .contains("First step must not have previous_hash")
    );

    // 4. Empty chain steps -> rejected
    let mut empty_chain = valid_chain.clone();
    empty_chain.steps.clear();
    let report_empty = empty_chain.verify(&keypair.verifying_key()).unwrap();
    assert!(!report_empty.valid);
    assert!(
        report_empty
            .failure_reason
            .unwrap()
            .contains("contains no steps")
    );
}

#[tokio::test]
async fn test_empirical_http_cors_and_bearer_auth_and_routing() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let keypair = Keypair::generate();
    let config = GatewayConfig::new(addr).with_auth_token("challenge_bearer_token_xyz");

    let gateway = Arc::new(HttpAgentGateway::new(
        config,
        None,
        Some(Arc::new(keypair)),
        Arc::new(PolicySet::default()),
        None,
        None,
        SseBroker::default(),
    ));

    tokio::spawn(async move {
        let _ = gateway.run_server(listener).await;
    });

    // 1. Missing / Invalid Auth Header -> 401 Unauthorized
    let mut stream = TcpStream::connect(addr).await.unwrap();
    let req = format!(
        "GET /v1/agent/sessions HTTP/1.1\r\nHost: {}\r\nConnection: close\r\n\r\n",
        addr
    );
    stream.write_all(req.as_bytes()).await.unwrap();
    let mut buf = vec![0u8; 1024];
    let n = stream.read(&mut buf).await.unwrap();
    let resp = String::from_utf8_lossy(&buf[..n]);
    assert!(resp.starts_with("HTTP/1.1 401 Unauthorized"));

    // 2. Valid Bearer Token -> 200 OK + CORS headers verified
    let mut stream = TcpStream::connect(addr).await.unwrap();
    let req = format!(
        "GET /v1/agent/sessions HTTP/1.1\r\nHost: {}\r\nAuthorization: Bearer challenge_bearer_token_xyz\r\nConnection: close\r\n\r\n",
        addr
    );
    stream.write_all(req.as_bytes()).await.unwrap();
    let n = stream.read(&mut buf).await.unwrap();
    let resp = String::from_utf8_lossy(&buf[..n]);
    assert!(resp.starts_with("HTTP/1.1 200 OK"));
    assert!(resp.contains("Access-Control-Allow-Origin: *"));
    assert!(resp.contains("Access-Control-Allow-Methods: GET, POST, OPTIONS"));
    assert!(resp.contains("Access-Control-Allow-Headers: Content-Type, Authorization"));

    // 3. Unknown route -> 404 Not Found
    let mut stream = TcpStream::connect(addr).await.unwrap();
    let req = format!(
        "GET /v1/non_existent_route HTTP/1.1\r\nHost: {}\r\nAuthorization: Bearer challenge_bearer_token_xyz\r\nConnection: close\r\n\r\n",
        addr
    );
    stream.write_all(req.as_bytes()).await.unwrap();
    let n = stream.read(&mut buf).await.unwrap();
    let resp = String::from_utf8_lossy(&buf[..n]);
    assert!(resp.starts_with("HTTP/1.1 404 Not Found"));
}
