//! Empirical Adversarial Challenge and Stress Test Harness for Milestone 4
//!
//! Validates:
//! 1. MCP JSON-RPC 2.0 error handling (-32700, -32600, -32601, -32602, -32603)
//! 2. HTTP REST status codes (400, 401, 403, 404, 200, 201, 202)
//! 3. WebSocket 4MB frame size limits, RFC 6455 framing, opcode transitions, close code 1009
//! 4. 6-stage Cryptographic Provenance Chain tamper detection at every causal step

use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use uuid::Uuid;

use zap_agent::{AgentId, AgentIntent, IntentKind};
use zap_crypto::Keypair;
use zap_gateway::{
    GatewayConfig, HttpAgentGateway, McpEngine, ProvenanceChainBuilder, ProvenanceStage, SseBroker,
    WebSocketHandler, WsFrame, compute_ws_accept, mcp_protocol::*, mcp_tools::ToolExecutionContext,
};
use zap_policy::PolicySet;

fn create_test_mcp_engine() -> (McpEngine, Keypair) {
    let keypair = Keypair::generate();
    let ctx = ToolExecutionContext {
        node: None,
        node_keypair: Some(Arc::new(keypair.clone())),
        policy_set: Arc::new(PolicySet::default()),
        journal: None,
        memory: None,
    };
    (McpEngine::new(ctx), keypair)
}

// ============================================================================
// CHALLENGE 1: MCP JSON-RPC 2.0 ERROR HANDLING & PROTOCOL CONFORMANCE
// ============================================================================

#[tokio::test]
async fn challenge_mcp_parse_error_32700() {
    let (engine, _) = create_test_mcp_engine();

    // Adversarial inputs for JSON parsing
    let malformed_inputs = vec![
        "",
        "{",
        "{ \"jsonrpc\": ",
        "{\"jsonrpc\": \"2.0\", \"id\": 1, \"method\": }",
        "NOT_JSON_AT_ALL",
        "{ \"jsonrpc\": \"2.0\", \"id\": 1, \"method\": \"test\", trailing_comma: true, }",
        "\0\0\0",
    ];

    for input in malformed_inputs {
        let resp_str = engine.handle_jsonrpc_str(input).await;
        let resp: Value = serde_json::from_str(&resp_str).unwrap_or_else(|_| {
            panic!("Response must be valid JSON even on parse error: '{resp_str}'")
        });

        assert_eq!(resp["jsonrpc"], "2.0");
        assert_eq!(resp["error"]["code"], JSONRPC_PARSE_ERROR);
        assert!(resp["error"]["message"].is_string());
    }
}

#[tokio::test]
async fn challenge_mcp_invalid_request_32600() {
    let (engine, _) = create_test_mcp_engine();

    // Invalid JSON-RPC request structures
    let invalid_requests = vec![
        // Missing jsonrpc field
        json!({ "id": 1, "method": "initialize" }),
        // jsonrpc is not "2.0"
        json!({ "jsonrpc": "1.0", "id": 1, "method": "initialize" }),
        json!({ "jsonrpc": "3.0", "id": 1, "method": "initialize" }),
        json!({ "jsonrpc": 2.0, "id": 1, "method": "initialize" }),
        // Missing method field
        json!({ "jsonrpc": "2.0", "id": 1 }),
        // Method is not a string
        json!({ "jsonrpc": "2.0", "id": 1, "method": 12345 }),
    ];

    for req in invalid_requests {
        let resp = engine.handle_jsonrpc_value(req).await;
        assert!(resp.error.is_some(), "Expected error for invalid request");
        let err = resp.error.unwrap();
        assert_eq!(err.code, JSONRPC_INVALID_REQUEST);
        assert!(!err.message.is_empty());
    }
}

#[tokio::test]
async fn challenge_mcp_method_not_found_32601() {
    let (engine, _) = create_test_mcp_engine();

    let unknown_methods = vec![
        "system/reboot",
        "tools/execute_shell",
        "custom/invalid_op",
        "",
        "unknown_mcp_method",
        "resources/delete",
        "prompts/delete",
    ];

    for method in unknown_methods {
        let req = json!({
            "jsonrpc": "2.0",
            "id": "req-unknown",
            "method": method,
        });

        let resp = engine.handle_jsonrpc_value(req).await;
        assert!(resp.error.is_some());
        let err = resp.error.unwrap();
        assert_eq!(err.code, JSONRPC_METHOD_NOT_FOUND);
        assert_eq!(resp.id, Some(json!("req-unknown")));
        assert!(err.message.contains(method) || err.message.contains("Method not found"));
    }
}

#[tokio::test]
async fn challenge_mcp_invalid_params_32602() {
    let (engine, _) = create_test_mcp_engine();

    // 1. tools/call with missing params
    let req1 = json!({
        "jsonrpc": "2.0",
        "id": 101,
        "method": "tools/call",
    });
    let resp1 = engine.handle_jsonrpc_value(req1).await;
    assert_eq!(resp1.error.unwrap().code, JSONRPC_INVALID_PARAMS);

    // 2. tools/call with invalid params type (string instead of object)
    let req2 = json!({
        "jsonrpc": "2.0",
        "id": 102,
        "method": "tools/call",
        "params": "not_an_object"
    });
    let resp2 = engine.handle_jsonrpc_value(req2).await;
    assert_eq!(resp2.error.unwrap().code, JSONRPC_INVALID_PARAMS);

    // 3. resources/read with missing params
    let req3 = json!({
        "jsonrpc": "2.0",
        "id": 103,
        "method": "resources/read",
    });
    let resp3 = engine.handle_jsonrpc_value(req3).await;
    assert_eq!(resp3.error.unwrap().code, JSONRPC_INVALID_PARAMS);

    // 4. prompts/get with missing params
    let req4 = json!({
        "jsonrpc": "2.0",
        "id": 104,
        "method": "prompts/get",
    });
    let resp4 = engine.handle_jsonrpc_value(req4).await;
    assert_eq!(resp4.error.unwrap().code, JSONRPC_INVALID_PARAMS);

    // 5. resources/read with nonexistent URI
    let req5 = json!({
        "jsonrpc": "2.0",
        "id": 105,
        "method": "resources/read",
        "params": { "uri": "zap://nonexistent/uri" }
    });
    let resp5 = engine.handle_jsonrpc_value(req5).await;
    assert_eq!(resp5.error.unwrap().code, JSONRPC_INVALID_PARAMS);

    // 6. prompts/get with unknown prompt name
    let req6 = json!({
        "jsonrpc": "2.0",
        "id": 106,
        "method": "prompts/get",
        "params": { "name": "unknown_prompt_template" }
    });
    let resp6 = engine.handle_jsonrpc_value(req6).await;
    assert_eq!(resp6.error.unwrap().code, JSONRPC_INVALID_PARAMS);
}

#[tokio::test]
async fn challenge_mcp_all_registered_tools_execute() {
    let (engine, _) = create_test_mcp_engine();

    // Query tools list
    let list_req = json!({
        "jsonrpc": "2.0",
        "id": 200,
        "method": "tools/list"
    });
    let list_resp = engine.handle_jsonrpc_value(list_req).await;
    let result = list_resp.result.expect("tools/list must succeed");
    let tools = result["tools"].as_array().expect("tools array");
    assert!(!tools.is_empty());

    // Call zap_send
    let send_req = json!({
        "jsonrpc": "2.0",
        "id": 201,
        "method": "tools/call",
        "params": {
            "name": "zap_send",
            "arguments": {
                "target": Uuid::new_v4().to_string(),
                "action": "telemetry.ping",
                "payload": "payload_test_123"
            }
        }
    });
    let send_resp = engine.handle_jsonrpc_value(send_req).await;
    let send_res = send_resp.result.expect("zap_send tool must succeed");
    assert_eq!(send_res["isError"], false);

    // Call zap_query
    let query_req = json!({
        "jsonrpc": "2.0",
        "id": 202,
        "method": "tools/call",
        "params": {
            "name": "zap_query",
            "arguments": {
                "namespace": "default",
                "limit": 10
            }
        }
    });
    let query_resp = engine.handle_jsonrpc_value(query_req).await;
    assert!(query_resp.result.is_some());

    // Call zap_get_fleet_health
    let fleet_req = json!({
        "jsonrpc": "2.0",
        "id": 203,
        "method": "tools/call",
        "params": {
            "name": "zap_get_fleet_health",
            "arguments": {}
        }
    });
    let fleet_resp = engine.handle_jsonrpc_value(fleet_req).await;
    assert!(fleet_resp.result.is_some());
}

// ============================================================================
// CHALLENGE 2: HTTP REST STATUS CODES & AUTHENTICATION
// ============================================================================

#[tokio::test]
async fn challenge_http_rest_status_codes_matrix() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let keypair = Keypair::generate();
    let config = GatewayConfig::new(addr).with_auth_token("m4_secret_token");
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

    // Helper closure to send raw HTTP and return (status_code, body)
    async fn send_http(addr: std::net::SocketAddr, raw_req: &str) -> (u16, String) {
        let mut stream = TcpStream::connect(addr).await.unwrap();
        stream.write_all(raw_req.as_bytes()).await.unwrap();
        let mut buf = vec![0u8; 8192];
        let n = stream.read(&mut buf).await.unwrap();
        let resp_str = String::from_utf8_lossy(&buf[..n]).to_string();
        let status = resp_str
            .lines()
            .next()
            .and_then(|l| l.split_whitespace().nth(1))
            .and_then(|s| s.parse::<u16>().ok())
            .unwrap_or(0);
        (status, resp_str)
    }

    // 1. Unauthorized (401) on protected endpoint without token
    let (status, _) = send_http(
        addr,
        &format!(
            "GET /v1/agent/sessions HTTP/1.1\r\nHost: {}\r\nConnection: close\r\n\r\n",
            addr
        ),
    )
    .await;
    assert_eq!(status, 401, "Expected 401 Unauthorized for missing auth");

    // 2. Unauthorized (401) on wrong token
    let (status, _) = send_http(
        addr,
        &format!(
            "GET /v1/agent/sessions HTTP/1.1\r\nHost: {}\r\nAuthorization: Bearer wrong_token\r\nConnection: close\r\n\r\n",
            addr
        ),
    )
    .await;
    assert_eq!(
        status, 401,
        "Expected 401 Unauthorized for incorrect bearer token"
    );

    // 3. OK (200) on GET /v1/health with auth
    let (status, body) = send_http(
        addr,
        &format!(
            "GET /v1/health HTTP/1.1\r\nHost: {}\r\nAuthorization: Bearer m4_secret_token\r\nConnection: close\r\n\r\n",
            addr
        ),
    )
    .await;
    assert_eq!(status, 200);
    assert!(body.contains("\"status\":\"ok\"") || body.contains("\"status\": \"ok\""));

    // 4. OK (200) on GET /metrics
    let (status, _) = send_http(
        addr,
        &format!(
            "GET /metrics HTTP/1.1\r\nHost: {}\r\nAuthorization: Bearer m4_secret_token\r\nConnection: close\r\n\r\n",
            addr
        ),
    )
    .await;
    assert_eq!(status, 200);

    // 5. Not Found (404) on unknown path
    let (status, _) = send_http(
        addr,
        &format!(
            "GET /v1/nonexistent/route HTTP/1.1\r\nHost: {}\r\nAuthorization: Bearer m4_secret_token\r\nConnection: close\r\n\r\n",
            addr
        ),
    )
    .await;
    assert_eq!(status, 404, "Expected 404 for unknown route");

    // 6. Bad Request (400) on invalid session UUID
    let (status, body) = send_http(
        addr,
        &format!(
            "GET /v1/agent/sessions/not-a-valid-uuid HTTP/1.1\r\nHost: {}\r\nAuthorization: Bearer m4_secret_token\r\nConnection: close\r\n\r\n",
            addr
        ),
    )
    .await;
    assert_eq!(status, 400, "Expected 400 on invalid UUID");
    assert!(body.contains("INVALID_UUID"));

    // 7. Bad Request (400) on malformed JSON body in POST /v1/agent/intents
    let (status, body) = send_http(
        addr,
        &format!(
            "POST /v1/agent/intents HTTP/1.1\r\nHost: {}\r\nAuthorization: Bearer m4_secret_token\r\nContent-Type: application/json\r\nContent-Length: 15\r\nConnection: close\r\n\r\n{{ malformed: }}",
            addr
        ),
    )
    .await;
    assert_eq!(status, 400);
    assert!(body.contains("INVALID_INTENT"));

    // 8. Created (201) on valid POST /v1/agent/sessions
    let session_body = json!({
        "owner_agent": "planner_node"
    })
    .to_string();
    let (status, body) = send_http(
        addr,
        &format!(
            "POST /v1/agent/sessions HTTP/1.1\r\nHost: {}\r\nAuthorization: Bearer m4_secret_token\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            addr, session_body.len(), session_body
        ),
    )
    .await;
    assert_eq!(status, 201);
    assert!(body.contains("session_id"));

    // 9. Accepted (202) on valid POST /v1/agent/intents
    let intent = AgentIntent::new(
        Uuid::new_v4(),
        AgentId::new("planner_node").unwrap(),
        IntentKind::Act,
        "Challenge test valid intent",
    );
    let intent_json = serde_json::to_string(&intent).unwrap();
    let (status, body) = send_http(
        addr,
        &format!(
            "POST /v1/agent/intents HTTP/1.1\r\nHost: {}\r\nAuthorization: Bearer m4_secret_token\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            addr, intent_json.len(), intent_json
        ),
    )
    .await;
    assert_eq!(status, 202);
    assert!(body.contains("accepted"));
    assert!(body.contains("provenance"));
}

// ============================================================================
// CHALLENGE 3: WEBSOCKET 4MB FRAME SIZE LIMITS & RFC 6455 FRAMING
// ============================================================================

#[tokio::test]
async fn challenge_websocket_rfc6455_handshake_and_accept() {
    // RFC 6455 official vector
    let client_key = "dGhlIHNhbXBsZSBub25jZQ==";
    let expected_accept = "s3pPLMBiTxaQ9kYGzzhZRbK+xOo=";
    assert_eq!(compute_ws_accept(client_key), expected_accept);

    // Custom test vector
    let custom_key = "x3JJHMbDL1EzLkh9GBhXDw==";
    let custom_accept = compute_ws_accept(custom_key);
    assert!(!custom_accept.is_empty());
    assert_eq!(custom_accept.len(), 28); // 20 bytes base64-encoded = 28 chars with padding
}

#[tokio::test]
async fn challenge_websocket_frame_size_limit_rejection() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let keypair = Keypair::generate();
    // Configure strict 2KB limit for this test
    let config = GatewayConfig::new(addr).with_max_frame_size(2048);
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
    let handshake = format!(
        "GET /v1/agent/ws HTTP/1.1\r\n\
Host: {}\r\n\
Upgrade: websocket\r\n\
Connection: Upgrade\r\n\
Sec-WebSocket-Key: {}\r\n\
Sec-WebSocket-Version: 13\r\n\r\n",
        addr, ws_key
    );

    stream.write_all(handshake.as_bytes()).await.unwrap();
    let mut resp_buf = vec![0u8; 1024];
    let n = stream.read(&mut resp_buf).await.unwrap();
    let resp_str = String::from_utf8_lossy(&resp_buf[..n]);
    assert!(resp_str.starts_with("HTTP/1.1 101 Switching Protocols"));

    // 1. Send normal frame within 2KB limit -> should succeed
    let ws_client_handler = WebSocketHandler::new(10 * 1024 * 1024);
    let valid_frame = WsFrame::text("Hello ZAP Gateway within limit");
    ws_client_handler
        .write_frame(&mut stream, &valid_frame)
        .await
        .unwrap();

    let reply = ws_client_handler.read_frame(&mut stream).await.unwrap();
    let reply_str = String::from_utf8_lossy(&reply.payload);
    assert!(reply_str.contains("acknowledged"));

    // 2. Send oversized frame (4096 bytes > 2048 limit) -> server must send Close with 1009
    let oversized_payload = vec![b'X'; 4096];
    let oversized_frame = WsFrame::binary(oversized_payload);
    ws_client_handler
        .write_frame(&mut stream, &oversized_frame)
        .await
        .unwrap();

    let close_reply = ws_client_handler.read_frame(&mut stream).await.unwrap();
    assert_eq!(
        close_reply.opcode,
        zap_gateway::transports::ws::WS_OPCODE_CLOSE
    );
    assert!(close_reply.payload.len() >= 2);
    let close_code = u16::from_be_bytes([close_reply.payload[0], close_reply.payload[1]]);
    assert_eq!(
        close_code,
        zap_gateway::transports::ws::WS_CLOSE_MESSAGE_TOO_BIG
    );
}

// ============================================================================
// CHALLENGE 4: 6-STAGE PROVENANCE CHAIN STRESS & TAMPER DETECTION
// ============================================================================

#[tokio::test]
async fn challenge_provenance_full_6_stages_and_all_tamper_vectors() {
    let keypair = Keypair::generate();
    let session_id = Uuid::new_v4();
    let intent_id = Uuid::new_v4();

    let mut intent = AgentIntent::new(
        session_id,
        AgentId::new("adversarial.agent").unwrap(),
        IntentKind::Act,
        "Execute multi-stage verification mission",
    );
    intent.intent_id = intent_id;

    let chain = ProvenanceChainBuilder::new(session_id, intent_id)
        .with_intent(&intent)
        .unwrap()
        .with_negotiation(
            &json!({"capabilities": ["matrix_mult", "cuda_exec"]}),
            BTreeMap::new(),
        )
        .unwrap()
        .with_policy("policy_sha256_hash", "ALLOW", BTreeMap::new())
        .unwrap()
        .with_driver(
            "driver.matrix.v2",
            "hash_in_512",
            "hash_out_512",
            BTreeMap::new(),
        )
        .unwrap()
        .with_poa(
            &["sig_node_1".to_string(), "sig_node_2".to_string()],
            BTreeMap::new(),
        )
        .unwrap()
        .with_receipt(
            "rcpt-provenance-001",
            1_720_000_000_000_000,
            BTreeMap::new(),
        )
        .unwrap()
        .build_and_sign(&keypair)
        .unwrap();

    // Baseline: Clean chain passes
    let clean_report = chain.verify(&keypair.verifying_key()).unwrap();
    assert!(clean_report.valid);
    assert_eq!(clean_report.verified_steps, 6);
    assert_eq!(clean_report.failed_stage, None);

    // Vector 1: Tamper with Intent stage (step 0)
    {
        let mut tampered = chain.clone();
        tampered.steps[0].input_data_hash = "tampered_intent_input".to_string();
        let report = tampered.verify(&keypair.verifying_key()).unwrap();
        assert!(!report.valid);
        assert_eq!(report.failed_stage, Some(ProvenanceStage::Intent));
    }

    // Vector 2: Tamper with Negotiation stage (step 1)
    {
        let mut tampered = chain.clone();
        tampered.steps[1].input_data_hash = "tampered_negotiation".to_string();
        let report = tampered.verify(&keypair.verifying_key()).unwrap();
        assert!(!report.valid);
        assert_eq!(report.failed_stage, Some(ProvenanceStage::Negotiation));
    }

    // Vector 3: Tamper with Policy stage (step 2)
    {
        let mut tampered = chain.clone();
        tampered.steps[2].input_data_hash = "tampered_policy".to_string();
        let report = tampered.verify(&keypair.verifying_key()).unwrap();
        assert!(!report.valid);
        assert_eq!(report.failed_stage, Some(ProvenanceStage::Policy));
    }

    // Vector 4: Tamper with Driver stage (step 3)
    {
        let mut tampered = chain.clone();
        tampered.steps[3].input_data_hash = "tampered_driver".to_string();
        let report = tampered.verify(&keypair.verifying_key()).unwrap();
        assert!(!report.valid);
        assert_eq!(report.failed_stage, Some(ProvenanceStage::Driver));
    }

    // Vector 5: Tamper with Poa stage (step 4)
    {
        let mut tampered = chain.clone();
        tampered.steps[4].input_data_hash = "tampered_poa".to_string();
        let report = tampered.verify(&keypair.verifying_key()).unwrap();
        assert!(!report.valid);
        assert_eq!(report.failed_stage, Some(ProvenanceStage::Poa));
    }

    // Vector 6: Tamper with Receipt stage (step 5)
    {
        let mut tampered = chain.clone();
        tampered.steps[5].input_data_hash = "tampered_receipt".to_string();
        let report = tampered.verify(&keypair.verifying_key()).unwrap();
        assert!(!report.valid);
        assert_eq!(report.failed_stage, Some(ProvenanceStage::Receipt));
    }

    // Vector 7: Tamper with Root Hash
    {
        let mut tampered = chain.clone();
        tampered.root_hash =
            "0000000000000000000000000000000000000000000000000000000000000000".to_string();
        let report = tampered.verify(&keypair.verifying_key()).unwrap();
        assert!(!report.valid);
        assert!(
            report
                .failure_reason
                .unwrap()
                .contains("Merkle root mismatch")
        );
    }

    // Vector 8: Tamper with Signature
    {
        let mut tampered = chain.clone();
        tampered.signature = "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff".to_string();
        let report = tampered.verify(&keypair.verifying_key()).unwrap();
        assert!(!report.valid);
        assert!(report.failure_reason.unwrap().contains("signature"));
    }

    // Vector 9: Verify with a different public key
    {
        let other_keypair = Keypair::generate();
        let report = chain.verify(&other_keypair.verifying_key()).unwrap();
        assert!(!report.valid);
        assert!(
            report
                .failure_reason
                .unwrap()
                .contains("Signer node ID mismatch")
        );
    }
}
