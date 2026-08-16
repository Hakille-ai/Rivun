use std::collections::BTreeMap;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use uuid::Uuid;

use zap_agent::{
    AGENT_PROTOCOL_SCHEMA_VERSION, AgentId, AgentIntent, CapabilityNegotiationRequest,
    DelegationRequest, IntentKind,
};
use zap_crypto::Keypair;
use zap_gateway::{
    AgentGatewayServer, GatewayConfig, HttpAgentGateway, McpEngine, ProvenanceChainBuilder,
    ProvenanceStage, SseBroker, WebSocketHandler, WsFrame, compute_ws_accept,
    mcp_tools::ToolExecutionContext,
};
use zap_policy::PolicySet;

#[tokio::test]
async fn test_mcp_initialize() {
    let keypair = Keypair::generate();
    let ctx = ToolExecutionContext {
        node: None,
        node_keypair: Some(Arc::new(keypair)),
        policy_set: Arc::new(PolicySet::default()),
        journal: None,
        memory: None,
    };
    let engine = McpEngine::new(ctx);

    let init_req = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": { "name": "test_client", "version": "1.0.0" }
        }
    });

    let resp = engine.handle_jsonrpc_value(init_req).await;
    assert_eq!(resp.id, Some(serde_json::json!(1)));
    let result = resp.result.expect("initialize should succeed");
    assert_eq!(result["protocolVersion"], "2024-11-05");
    assert_eq!(result["serverInfo"]["name"], "zap-gateway");
}

#[tokio::test]
async fn test_mcp_tools_list_and_call() {
    let keypair = Keypair::generate();
    let ctx = ToolExecutionContext {
        node: None,
        node_keypair: Some(Arc::new(keypair)),
        policy_set: Arc::new(PolicySet::default()),
        journal: None,
        memory: None,
    };
    let engine = McpEngine::new(ctx);

    // 1. List tools
    let list_req = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "tools/list"
    });
    let list_resp = engine.handle_jsonrpc_value(list_req).await;
    let list_result = list_resp.result.expect("tools/list must succeed");
    let tools = list_result["tools"].as_array().expect("tools array");
    let tool_names: Vec<&str> = tools.iter().filter_map(|t| t["name"].as_str()).collect();
    assert!(tool_names.contains(&"zap_send"));
    assert!(tool_names.contains(&"zap_query"));
    assert!(tool_names.contains(&"zap_agent_intent"));
    assert!(tool_names.contains(&"zap_receipts_verify"));

    // 2. Call zap_send
    let call_req = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 3,
        "method": "tools/call",
        "params": {
            "name": "zap_send",
            "arguments": {
                "target": Uuid::new_v4().to_string(),
                "action": "ping",
                "payload": "hello zap"
            }
        }
    });
    let call_resp = engine.handle_jsonrpc_value(call_req).await;
    let call_result = call_resp.result.expect("tools/call must succeed");
    assert_eq!(call_result["isError"], false);
    let content = &call_result["content"][0]["text"];
    assert!(content.as_str().unwrap().contains("success"));
}

#[tokio::test]
async fn test_mcp_resources_and_prompts() {
    let keypair = Keypair::generate();
    let ctx = ToolExecutionContext {
        node: None,
        node_keypair: Some(Arc::new(keypair)),
        policy_set: Arc::new(PolicySet::default()),
        journal: None,
        memory: None,
    };
    let engine = McpEngine::new(ctx);

    // Resources list
    let res_list_req = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 4,
        "method": "resources/list"
    });
    let res_list_resp = engine.handle_jsonrpc_value(res_list_req).await;
    let res_list = res_list_resp.result.expect("resources/list must succeed");
    let resources = res_list["resources"].as_array().unwrap();
    let uris: Vec<&str> = resources.iter().filter_map(|r| r["uri"].as_str()).collect();
    assert!(uris.contains(&"zap://ledger/receipts"));
    assert!(uris.contains(&"zap://node/status"));
    assert!(uris.contains(&"zap://fleet/topology"));

    // Resource read
    let res_read_req = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 5,
        "method": "resources/read",
        "params": { "uri": "zap://node/status" }
    });
    let res_read_resp = engine.handle_jsonrpc_value(res_read_req).await;
    let res_read = res_read_resp.result.expect("resources/read must succeed");
    assert!(!res_read["contents"].as_array().unwrap().is_empty());

    // Prompts list
    let prompt_list_req = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 6,
        "method": "prompts/list"
    });
    let prompt_list_resp = engine.handle_jsonrpc_value(prompt_list_req).await;
    let prompt_list = prompt_list_resp.result.expect("prompts/list must succeed");
    let prompts = prompt_list["prompts"].as_array().unwrap();
    let pnames: Vec<&str> = prompts.iter().filter_map(|p| p["name"].as_str()).collect();
    assert!(pnames.contains(&"goal_decomposition"));
    assert!(pnames.contains(&"capability_negotiation"));
    assert!(pnames.contains(&"safe_execution_verification"));

    // Prompt get
    let prompt_get_req = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 7,
        "method": "prompts/get",
        "params": {
            "name": "goal_decomposition",
            "arguments": { "objective": "Deploy domain pack" }
        }
    });
    let prompt_get_resp = engine.handle_jsonrpc_value(prompt_get_req).await;
    let prompt_get = prompt_get_resp.result.expect("prompts/get must succeed");
    let msgs = prompt_get["messages"].as_array().unwrap();
    assert!(
        msgs[0]["content"]["text"]
            .as_str()
            .unwrap()
            .contains("Deploy domain pack")
    );
}

#[tokio::test]
async fn test_mcp_error_handling() {
    let keypair = Keypair::generate();
    let ctx = ToolExecutionContext {
        node: None,
        node_keypair: Some(Arc::new(keypair)),
        policy_set: Arc::new(PolicySet::default()),
        journal: None,
        memory: None,
    };
    let engine = McpEngine::new(ctx);

    // Method not found (-32601)
    let unknown_req = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 8,
        "method": "unknown/method"
    });
    let resp = engine.handle_jsonrpc_value(unknown_req).await;
    assert!(resp.error.is_some());
    assert_eq!(resp.error.unwrap().code, -32601);

    // Invalid JSON parse (-32700)
    let bad_json_resp = engine.handle_jsonrpc_str("{ invalid_json: ").await;
    let parsed: serde_json::Value = serde_json::from_str(&bad_json_resp).unwrap();
    assert_eq!(parsed["error"]["code"], -32700);
}

#[tokio::test]
async fn test_provenance_chain_6_stages_and_tamper_detection() {
    let keypair = Keypair::generate();
    let session_id = Uuid::new_v4();
    let intent_id = Uuid::new_v4();

    let mut intent = AgentIntent::new(
        session_id,
        AgentId::new("planner.m4").unwrap(),
        IntentKind::Act,
        "Execute distributed robotics action",
    );
    intent.intent_id = intent_id;

    let chain = ProvenanceChainBuilder::new(session_id, intent_id)
        .with_intent(&intent)
        .unwrap()
        .with_negotiation(
            &serde_json::json!({"negotiated_capability": "driver.execute:motor"}),
            BTreeMap::new(),
        )
        .unwrap()
        .with_policy("policy_hash_allow", "ALLOW", BTreeMap::new())
        .unwrap()
        .with_driver(
            "driver.motor.v1",
            "in_hash_1",
            "out_hash_1",
            BTreeMap::new(),
        )
        .unwrap()
        .with_poa(&["sig1".to_string(), "sig2".to_string()], BTreeMap::new())
        .unwrap()
        .with_receipt("rcpt-12345", 1_700_000_000, BTreeMap::new())
        .unwrap()
        .build_and_sign(&keypair)
        .unwrap();

    // Verify all 6 stages
    assert_eq!(chain.steps.len(), 6);
    let report = chain.verify(&keypair.verifying_key()).unwrap();
    assert!(report.valid);
    assert_eq!(report.verified_steps, 6);

    for stage in [
        ProvenanceStage::Intent,
        ProvenanceStage::Negotiation,
        ProvenanceStage::Policy,
        ProvenanceStage::Driver,
        ProvenanceStage::Poa,
        ProvenanceStage::Receipt,
    ] {
        assert!(chain.verify_step(stage).is_ok());
    }

    // Tamper detection: change driver step output
    let mut tampered = chain.clone();
    tampered.steps[3].input_data_hash = "corrupted_driver_hash".to_string();
    let tampered_report = tampered.verify(&keypair.verifying_key()).unwrap();
    assert!(!tampered_report.valid);
    assert_eq!(tampered_report.failed_stage, Some(ProvenanceStage::Driver));
    assert!(tampered.verify_step(ProvenanceStage::Driver).is_err());
}

#[tokio::test]
async fn test_http_rest_intent_submission_and_receipts() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let keypair = Keypair::generate();
    let config = GatewayConfig::new(addr);
    let policy_set = Arc::new(PolicySet::default());
    let sse_broker = SseBroker::default();

    let gateway = Arc::new(HttpAgentGateway::new(
        config,
        None,
        Some(Arc::new(keypair)),
        policy_set,
        None,
        None,
        sse_broker,
    ));

    tokio::spawn(async move {
        let _ = gateway.run_server(listener).await;
    });

    // Send HTTP POST /v1/agent/intents
    let mut stream = TcpStream::connect(addr).await.unwrap();
    let session_id = Uuid::new_v4();
    let intent = AgentIntent::new(
        session_id,
        AgentId::new("agent.rest").unwrap(),
        IntentKind::Act,
        "REST test objective",
    );
    let intent_json = serde_json::to_string(&intent).unwrap();

    let req = format!(
        "POST /v1/agent/intents HTTP/1.1\r\n\
Host: {}\r\n\
Content-Type: application/json\r\n\
Content-Length: {}\r\n\
Connection: close\r\n\r\n{}",
        addr,
        intent_json.len(),
        intent_json
    );

    stream.write_all(req.as_bytes()).await.unwrap();
    let mut response_buf = vec![0u8; 4096];
    let n = stream.read(&mut response_buf).await.unwrap();
    let response_str = String::from_utf8_lossy(&response_buf[..n]);

    assert!(response_str.starts_with("HTTP/1.1 202 Accepted"));
    assert!(response_str.contains("REST test objective") || response_str.contains("accepted"));
    assert!(response_str.contains("intent_id"));
}

#[tokio::test]
async fn test_http_rest_sessions_and_negotiate_and_delegate() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let keypair = Keypair::generate();
    let config = GatewayConfig::new(addr);
    let policy_set = Arc::new(PolicySet::default());
    let sse_broker = SseBroker::default();

    let gateway = Arc::new(HttpAgentGateway::new(
        config,
        None,
        Some(Arc::new(keypair)),
        policy_set,
        None,
        None,
        sse_broker,
    ));

    tokio::spawn(async move {
        let _ = gateway.run_server(listener).await;
    });

    // 1. POST /v1/agent/sessions
    let mut stream = TcpStream::connect(addr).await.unwrap();
    let session_id = Uuid::new_v4();
    let session_payload = serde_json::json!({
        "session_id": session_id.to_string(),
        "owner_agent": "planner_alpha"
    });
    let payload_str = serde_json::to_string(&session_payload).unwrap();
    let req = format!(
        "POST /v1/agent/sessions HTTP/1.1\r\nHost: {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        addr,
        payload_str.len(),
        payload_str
    );
    stream.write_all(req.as_bytes()).await.unwrap();
    let mut buf = vec![0u8; 2048];
    let n = stream.read(&mut buf).await.unwrap();
    let resp = String::from_utf8_lossy(&buf[..n]);
    assert!(resp.starts_with("HTTP/1.1 201 Created"));

    // 2. GET /v1/agent/sessions/{session_id}
    let mut stream = TcpStream::connect(addr).await.unwrap();
    let req = format!(
        "GET /v1/agent/sessions/{} HTTP/1.1\r\nHost: {}\r\nConnection: close\r\n\r\n",
        session_id, addr
    );
    stream.write_all(req.as_bytes()).await.unwrap();
    let mut buf = vec![0u8; 2048];
    let n = stream.read(&mut buf).await.unwrap();
    let resp = String::from_utf8_lossy(&buf[..n]);
    assert!(resp.starts_with("HTTP/1.1 200 OK"));
    assert!(resp.contains(&session_id.to_string()));

    // 3. POST /v1/agent/delegate
    let mut stream = TcpStream::connect(addr).await.unwrap();
    let del_req = DelegationRequest {
        schema_version: AGENT_PROTOCOL_SCHEMA_VERSION,
        delegation_id: Uuid::new_v4(),
        session_id,
        parent_intent_id: Uuid::new_v4(),
        from_agent: AgentId::new("planner_alpha").unwrap(),
        to_agent: Some(AgentId::new("worker_beta").unwrap()),
        objective: "Execute subtask".to_string(),
        required_capabilities: std::collections::BTreeSet::new(),
        constraints: Vec::new(),
        context: Vec::new(),
        deadline_unix_micros: None,
        metadata: BTreeMap::new(),
    };
    let del_str = serde_json::to_string(&del_req).unwrap();
    let req = format!(
        "POST /v1/agent/delegate HTTP/1.1\r\nHost: {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        addr,
        del_str.len(),
        del_str
    );
    stream.write_all(req.as_bytes()).await.unwrap();
    let mut buf = vec![0u8; 2048];
    let n = stream.read(&mut buf).await.unwrap();
    let resp = String::from_utf8_lossy(&buf[..n]);
    assert!(resp.starts_with("HTTP/1.1 200 OK"));
    assert!(resp.contains("worker_beta") || resp.contains("delegation_id"));

    // 4. POST /v1/agent/negotiate
    let mut stream = TcpStream::connect(addr).await.unwrap();
    let neg_req = CapabilityNegotiationRequest {
        schema_version: AGENT_PROTOCOL_SCHEMA_VERSION,
        negotiation_id: Uuid::new_v4(),
        session_id,
        requester_agent: AgentId::new("planner_alpha").unwrap(),
        required_capabilities: std::collections::BTreeSet::from([
            zap_capability::CapabilityId::new("driver.execute:test").unwrap(),
        ]),
        optional_capabilities: std::collections::BTreeSet::new(),
        desired_intents: std::collections::BTreeSet::from([IntentKind::Act]),
        metadata: BTreeMap::new(),
    };
    let neg_str = serde_json::to_string(&neg_req).unwrap();
    let req = format!(
        "POST /v1/agent/negotiate HTTP/1.1\r\nHost: {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        addr,
        neg_str.len(),
        neg_str
    );
    stream.write_all(req.as_bytes()).await.unwrap();
    let mut buf = vec![0u8; 2048];
    let n = stream.read(&mut buf).await.unwrap();
    let resp = String::from_utf8_lossy(&buf[..n]);
    assert!(resp.starts_with("HTTP/1.1 200 OK"));
    assert!(resp.contains("accepted"));
}

#[tokio::test]
async fn test_websocket_bridge_framing_and_size_limits() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let keypair = Keypair::generate();
    let config = GatewayConfig::new(addr).with_max_frame_size(1024); // 1KB max frame size
    let policy_set = Arc::new(PolicySet::default());
    let sse_broker = SseBroker::default();

    let gateway = Arc::new(HttpAgentGateway::new(
        config,
        None,
        Some(Arc::new(keypair)),
        policy_set,
        None,
        None,
        sse_broker,
    ));

    tokio::spawn(async move {
        let _ = gateway.run_server(listener).await;
    });

    let mut stream = TcpStream::connect(addr).await.unwrap();
    let ws_key = "dGhlIHNhbXBsZSBub25jZQ==";
    let req = format!(
        "GET /v1/agent/ws HTTP/1.1\r\n\
Host: {}\r\n\
Upgrade: websocket\r\n\
Connection: Upgrade\r\n\
Sec-WebSocket-Key: {}\r\n\
Sec-WebSocket-Version: 13\r\n\r\n",
        addr, ws_key
    );

    stream.write_all(req.as_bytes()).await.unwrap();
    let mut resp_buf = vec![0u8; 1024];
    let n = stream.read(&mut resp_buf).await.unwrap();
    let resp_str = String::from_utf8_lossy(&resp_buf[..n]);
    assert!(resp_str.starts_with("HTTP/1.1 101 Switching Protocols"));
    assert!(resp_str.contains(&compute_ws_accept(ws_key)));

    // Send a valid text frame
    let ws_handler = WebSocketHandler::new(1024);
    let text_frame = WsFrame::text(r#"{"action":"status_query"}"#);
    ws_handler
        .write_frame(&mut stream, &text_frame)
        .await
        .unwrap();

    let reply_frame = ws_handler.read_frame(&mut stream).await.unwrap();
    let reply_text = String::from_utf8_lossy(&reply_frame.payload);
    assert!(reply_text.contains("acknowledged"));
}

#[tokio::test]
async fn test_agent_gateway_server_builder_and_auth() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let keypair = Keypair::generate();
    let config = GatewayConfig::new(addr).with_auth_token("secret_bearer_token");
    let server = AgentGatewayServer::new(
        config,
        None,
        Some(Arc::new(keypair)),
        Some(Arc::new(PolicySet::default())),
        None,
        None,
    );

    assert_eq!(server.sse_broker().subscriber_count(), 0);
    assert!(server.mcp_engine().context().node_keypair.is_some());

    tokio::spawn(async move {
        let _ = server.run_on_listener(listener).await;
    });

    // 1. Request without token -> 401 Unauthorized
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

    // 2. Request with valid token -> 200 OK
    let mut stream = TcpStream::connect(addr).await.unwrap();
    let req = format!(
        "GET /v1/agent/sessions HTTP/1.1\r\nHost: {}\r\nAuthorization: Bearer secret_bearer_token\r\nConnection: close\r\n\r\n",
        addr
    );
    stream.write_all(req.as_bytes()).await.unwrap();
    let mut buf = vec![0u8; 1024];
    let n = stream.read(&mut buf).await.unwrap();
    let resp = String::from_utf8_lossy(&buf[..n]);
    assert!(resp.starts_with("HTTP/1.1 200 OK"));
}

#[tokio::test]
async fn test_http_body_chunked_buffering_and_payload_too_large() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let keypair = Keypair::generate();
    let config = GatewayConfig::new(addr).with_max_frame_size(32 * 1024); // 32KB max limit
    let policy_set = Arc::new(PolicySet::default());
    let sse_broker = SseBroker::default();

    let gateway = Arc::new(HttpAgentGateway::new(
        config,
        None,
        Some(Arc::new(keypair)),
        policy_set,
        None,
        None,
        sse_broker,
    ));

    tokio::spawn(async move {
        let _ = gateway.run_server(listener).await;
    });

    // 1. Send large (>8KB) valid intent across multiple TCP chunks to verify buffering
    let mut large_intent = AgentIntent::new(
        Uuid::new_v4(),
        AgentId::new("large_payload_agent").unwrap(),
        IntentKind::Act,
        "A".repeat(12 * 1024), // 12KB payload
    );
    large_intent.metadata.insert(
        "padding".to_string(),
        serde_json::Value::String("B".repeat(4 * 1024)),
    );
    let body_str = serde_json::to_string(&large_intent).unwrap();
    let header_str = format!(
        "POST /v1/agent/intents HTTP/1.1\r\nHost: {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        addr,
        body_str.len()
    );

    let mut stream = TcpStream::connect(addr).await.unwrap();
    // Send headers first
    stream.write_all(header_str.as_bytes()).await.unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(10)).await;

    // Send body in 4KB chunks
    let body_bytes = body_str.as_bytes();
    for chunk in body_bytes.chunks(4096) {
        stream.write_all(chunk).await.unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    }

    let mut buf = vec![0u8; 4096];
    let n = stream.read(&mut buf).await.unwrap();
    let resp = String::from_utf8_lossy(&buf[..n]);
    assert!(
        resp.starts_with("HTTP/1.1 202 Accepted"),
        "Expected 202 Accepted for 16KB chunked body, got: {resp}"
    );

    // 2. Send Content-Length exceeding 32KB max_frame_size -> 413 Payload Too Large
    let mut stream = TcpStream::connect(addr).await.unwrap();
    let oversize_headers = format!(
        "POST /v1/agent/intents HTTP/1.1\r\nHost: {}\r\nContent-Type: application/json\r\nContent-Length: 65536\r\nConnection: close\r\n\r\n",
        addr
    );
    stream.write_all(oversize_headers.as_bytes()).await.unwrap();
    let mut buf = vec![0u8; 1024];
    let n = stream.read(&mut buf).await.unwrap();
    let resp = String::from_utf8_lossy(&buf[..n]);
    assert!(
        resp.starts_with("HTTP/1.1 413 Payload Too Large"),
        "Expected 413 for oversized body, got: {resp}"
    );
}
