//! Multi-Transport Native HTTP REST, SSE Stream, and WebSocket Router.

use bytes::Bytes;
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    net::SocketAddr,
    sync::{Arc, Mutex},
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::broadcast,
};
use tracing::{debug, info, warn};
use uuid::Uuid;
use zap_agent::{
    AGENT_PROTOCOL_SCHEMA_VERSION, AgentId, AgentIntent, AgentSession,
    CapabilityNegotiationRequest, CapabilityNegotiationResponse, DelegationDecision,
    DelegationRequest, DelegationResponse, Validate,
};
use zap_core::{ZapFlags, ZapFrame, now_micros};
use zap_crypto::{Keypair, PublicKey};
use zap_ledger::{
    ReceiptJournalStore, ReceiptReplicationRequest, ReceiptReplicationResponse, SignedActionReceipt,
};
use zap_memory::MemoryJournalStore;
use zap_node::ZapNode;
use zap_policy::{PolicyDecision, PolicyInput, PolicySet};
use zap_telemetry::PrometheusExporter;

use crate::config::GatewayConfig;
use crate::error::{Result, ZapGatewayError};
use crate::mcp::McpEngine;
use crate::mcp::tools::ToolExecutionContext;
use crate::provenance::{ProvenanceChainBuilder, ProvenanceChainDigest};
use crate::transports::sse::{SseBroker, SseEvent};
use crate::transports::ws::{
    WS_CLOSE_MESSAGE_TOO_BIG, WS_CLOSE_NORMAL, WS_OPCODE_BINARY, WS_OPCODE_CLOSE, WS_OPCODE_PING,
    WS_OPCODE_TEXT, WebSocketHandler, WsFrame, compute_ws_accept,
};

pub struct HttpAgentGateway {
    config: GatewayConfig,
    node: Option<Arc<ZapNode>>,
    keypair: Option<Arc<Keypair>>,
    policy_set: Arc<PolicySet>,
    journal: Option<Arc<Mutex<ReceiptJournalStore>>>,
    memory: Option<Arc<Mutex<MemoryJournalStore>>>,
    sse_broker: SseBroker,
    mcp_engine: McpEngine,
}

impl HttpAgentGateway {
    pub fn new(
        config: GatewayConfig,
        node: Option<Arc<ZapNode>>,
        keypair: Option<Arc<Keypair>>,
        policy_set: Arc<PolicySet>,
        journal: Option<Arc<Mutex<ReceiptJournalStore>>>,
        memory: Option<Arc<Mutex<MemoryJournalStore>>>,
        sse_broker: SseBroker,
    ) -> Self {
        let ctx = ToolExecutionContext {
            node: node.clone(),
            node_keypair: keypair.clone(),
            policy_set: policy_set.clone(),
            journal: journal.clone(),
            memory: memory.clone(),
        };
        let mcp_engine = McpEngine::new(ctx);

        Self {
            config,
            node,
            keypair,
            policy_set,
            journal,
            memory,
            sse_broker,
            mcp_engine,
        }
    }

    pub fn sse_broker(&self) -> &SseBroker {
        &self.sse_broker
    }

    pub fn mcp_engine(&self) -> &McpEngine {
        &self.mcp_engine
    }

    pub fn memory(&self) -> Option<&Arc<Mutex<MemoryJournalStore>>> {
        self.memory.as_ref()
    }

    pub fn journal(&self) -> Option<&Arc<Mutex<ReceiptJournalStore>>> {
        self.journal.as_ref()
    }

    pub async fn run_server(self: Arc<Self>, listener: TcpListener) -> Result<()> {
        info!("HTTP Agent Gateway listening on {}", self.config.http_bind);

        loop {
            let (stream, remote_addr) = match listener.accept().await {
                Ok(conn) => conn,
                Err(err) => {
                    warn!("Accept error in Gateway TCP listener: {err}");
                    continue;
                }
            };

            let gateway = self.clone();
            tokio::spawn(async move {
                if let Err(e) = gateway.handle_connection(stream, remote_addr).await {
                    debug!("Gateway connection {} ended: {}", remote_addr, e);
                }
            });
        }
    }

    async fn handle_connection(
        &self,
        mut stream: TcpStream,
        _remote_addr: SocketAddr,
    ) -> Result<()> {
        let mut buffer = Vec::new();
        let mut temp_buf = [0u8; 8192];
        let mut header_end_offset = None;

        // Read until we find the end of HTTP headers (\r\n\r\n or \n\n) or reach max_frame_size
        while buffer.len() < self.config.max_frame_size {
            let n = stream.read(&mut temp_buf).await?;
            if n == 0 {
                break;
            }
            buffer.extend_from_slice(&temp_buf[..n]);

            if let Some(pos) = buffer.windows(4).position(|w| w == b"\r\n\r\n") {
                header_end_offset = Some(pos + 4);
                break;
            } else if let Some(pos) = buffer.windows(2).position(|w| w == b"\n\n") {
                header_end_offset = Some(pos + 2);
                break;
            }
        }

        if buffer.is_empty() {
            return Ok(());
        }

        let header_end = match header_end_offset {
            Some(offset) => offset,
            None => buffer.len(),
        };

        let request_str = String::from_utf8_lossy(&buffer[..header_end]).into_owned();
        let mut lines = request_str.lines();
        let request_line = match lines.next() {
            Some(l) => l,
            None => return Ok(()),
        };

        let parts: Vec<&str> = request_line.split_whitespace().collect();
        if parts.len() < 2 {
            return self
                .send_raw_response(
                    &mut stream,
                    400,
                    "Bad Request",
                    "text/plain",
                    b"Malformed request line",
                )
                .await;
        }

        let method = parts[0];
        let path_and_query = parts[1];
        let path = path_and_query.split('?').next().unwrap_or(path_and_query);

        // Parse headers
        let mut headers = BTreeMap::new();
        for line in lines {
            if line.is_empty() {
                break;
            }
            if let Some((k, v)) = line.split_once(':') {
                headers.insert(k.trim().to_ascii_lowercase(), v.trim().to_string());
            }
        }

        // Check authentication if configured
        if let Some(expected_token) = &self.config.auth_token {
            let auth_header = headers
                .get("authorization")
                .map(|s| s.as_str())
                .unwrap_or("");
            let token = auth_header.strip_prefix("Bearer ").unwrap_or(auth_header);
            if token != expected_token {
                if let Some(node) = &self.node {
                    node.record_agent_gateway_request("http", "401");
                }
                return self
                    .send_json_response(
                        &mut stream,
                        401,
                        "Unauthorized",
                        &json!({ "error": "Unauthorized", "code": "UNAUTHORIZED" }),
                    )
                    .await;
            }
        }

        // Check for WebSocket Upgrade
        let upgrade_header = headers.get("upgrade").map(|s| s.to_ascii_lowercase());
        let connection_header = headers.get("connection").map(|s| s.to_ascii_lowercase());
        let is_ws_upgrade = upgrade_header.as_deref() == Some("websocket")
            && connection_header
                .as_deref()
                .map(|c| c.contains("upgrade"))
                .unwrap_or(false);

        if is_ws_upgrade && path == "/v1/agent/ws" {
            let ws_key = match headers.get("sec-websocket-key") {
                Some(k) => k.clone(),
                None => {
                    return self
                        .send_raw_response(
                            &mut stream,
                            400,
                            "Bad Request",
                            "text/plain",
                            b"Missing Sec-WebSocket-Key",
                        )
                        .await;
                }
            };

            return self.handle_websocket_upgrade(stream, &ws_key).await;
        }

        // Handle SSE Stream
        if method == "GET" && (path == "/v1/agent/stream" || path == "/v1/agent/events") {
            return self.handle_sse_stream(stream).await;
        }

        // Parse Content-Length and read complete request body up to config.max_frame_size
        let content_length: usize = headers
            .get("content-length")
            .and_then(|val| val.parse::<usize>().ok())
            .unwrap_or(0);

        if content_length > self.config.max_frame_size {
            return self
                .send_json_response(
                    &mut stream,
                    413,
                    "Payload Too Large",
                    &json!({ "error": "Payload Too Large", "code": "PAYLOAD_TOO_LARGE" }),
                )
                .await;
        }

        let mut current_body_len = buffer.len().saturating_sub(header_end);
        while current_body_len < content_length {
            let to_read = (content_length - current_body_len).min(temp_buf.len());
            let n = match tokio::time::timeout(
                std::time::Duration::from_millis(50),
                stream.read(&mut temp_buf[..to_read]),
            )
            .await
            {
                Ok(Ok(n)) if n > 0 => n,
                _ => break,
            };
            buffer.extend_from_slice(&temp_buf[..n]);
            current_body_len += n;
        }

        let body_bytes = if buffer.len() >= header_end {
            let available = &buffer[header_end..];
            if content_length > 0 && available.len() >= content_length {
                &available[..content_length]
            } else {
                available
            }
        } else {
            &[]
        };

        // REST routing
        match (method, path) {
            ("GET", "/v1/health") | ("GET", "/healthz") | ("GET", "/health") => {
                let node_id = self
                    .keypair
                    .as_ref()
                    .map(|k| k.node_id())
                    .unwrap_or_else(Uuid::new_v4);
                self.send_json_response(
                    &mut stream,
                    200,
                    "OK",
                    &json!({ "status": "ok", "node_id": node_id, "timestamp_micros": now_micros().unwrap_or(0) }),
                ).await
            }

            ("GET", "/metrics") => {
                if let Some(node) = &self.node {
                    let snapshot = node.metrics_snapshot();
                    let prometheus_text = PrometheusExporter::export(&snapshot);
                    self.send_raw_response(
                        &mut stream,
                        200,
                        "OK",
                        "text/plain; version=0.0.4",
                        prometheus_text.as_bytes(),
                    )
                    .await
                } else {
                    self.send_raw_response(
                        &mut stream,
                        200,
                        "OK",
                        "text/plain",
                        b"# No active node metrics\n",
                    )
                    .await
                }
            }

            ("POST", "/v1/agent/intents") => self.handle_post_intent(&mut stream, body_bytes).await,

            ("GET", "/v1/agent/sessions") => {
                let snap = self.node.as_ref().map(|n| n.metrics_snapshot());
                let resp = json!({
                    "active_sessions": snap.as_ref().map(|s| s.agent_sessions_active).unwrap_or(0),
                    "status": "ok",
                    "timestamp_micros": now_micros().unwrap_or(0),
                });
                self.send_json_response(&mut stream, 200, "OK", &resp).await
            }

            ("POST", "/v1/agent/sessions") => {
                self.handle_post_session(&mut stream, body_bytes).await
            }

            ("GET", path) if path.starts_with("/v1/agent/sessions/") => {
                let session_id_str = path.trim_start_matches("/v1/agent/sessions/");
                if let Ok(session_id) = Uuid::parse_str(session_id_str) {
                    let resp = json!({
                        "schema_version": 1,
                        "session_id": session_id,
                        "status": "running",
                        "updated_at_micros": now_micros().unwrap_or(0),
                    });
                    self.send_json_response(&mut stream, 200, "OK", &resp).await
                } else {
                    self.send_json_response(
                        &mut stream,
                        400,
                        "Bad Request",
                        &json!({ "error": "Invalid session_id UUID format", "code": "INVALID_UUID" }),
                    ).await
                }
            }

            ("GET", "/v1/agent/receipts") => {
                self.handle_get_receipts(&mut stream, path_and_query).await
            }

            ("POST", "/v1/agent/delegate") => {
                self.handle_post_delegate(&mut stream, body_bytes).await
            }

            ("POST", "/v1/agent/negotiate") => {
                self.handle_post_negotiate(&mut stream, body_bytes).await
            }

            ("POST", "/v1/agent/provenance/verify") => {
                self.handle_post_provenance_verify(&mut stream, body_bytes)
                    .await
            }

            ("POST", "/v1/agent/mcp") => {
                let body_str = String::from_utf8_lossy(body_bytes);
                let resp_str = self.mcp_engine.handle_jsonrpc_str(&body_str).await;
                self.send_raw_response(
                    &mut stream,
                    200,
                    "OK",
                    "application/json",
                    resp_str.as_bytes(),
                )
                .await
            }

            _ => {
                if let Some(node) = &self.node {
                    node.record_agent_gateway_request("http", "404");
                }
                self.send_json_response(
                    &mut stream,
                    404,
                    "Not Found",
                    &json!({ "error": format!("Route not found: {method} {path}"), "code": "NOT_FOUND" }),
                ).await
            }
        }
    }

    async fn handle_get_receipts(
        &self,
        stream: &mut TcpStream,
        _path_and_query: &str,
    ) -> Result<()> {
        let receipts_resp = if let Some(journal_lock) = &self.journal {
            let journal = journal_lock
                .lock()
                .map_err(|e| ZapGatewayError::Internal(e.to_string()))?;
            let req = ReceiptReplicationRequest {
                schema_version: 1,
                after_processed_at_micros: None,
                until_processed_at_micros: None,
                limit: Some(50),
                kind: None,
                subject: None,
                source_node: None,
                target_node: None,
            };
            let receipts = journal.query(&req).unwrap_or_default();
            ReceiptReplicationResponse::new(
                self.keypair
                    .as_ref()
                    .map(|k| k.node_id())
                    .unwrap_or_else(Uuid::new_v4),
                receipts,
                false,
            )
        } else {
            ReceiptReplicationResponse::new(
                self.keypair
                    .as_ref()
                    .map(|k| k.node_id())
                    .unwrap_or_else(Uuid::new_v4),
                vec![],
                false,
            )
        };

        self.send_json_response(stream, 200, "OK", &json!(receipts_resp))
            .await
    }

    async fn handle_post_intent(&self, stream: &mut TcpStream, body: &[u8]) -> Result<()> {
        let intent: AgentIntent = match serde_json::from_slice(body) {
            Ok(i) => i,
            Err(e) => {
                if let Some(node) = &self.node {
                    node.record_agent_gateway_request("http", "400");
                }
                return self.send_json_response(
                    stream,
                    400,
                    "Bad Request",
                    &json!({ "error": format!("Malformed AgentIntent JSON: {e}"), "code": "INVALID_INTENT" }),
                ).await;
            }
        };

        if let Err(e) = intent.validate() {
            if let Some(node) = &self.node {
                node.record_agent_gateway_request("http", "400");
            }
            return self.send_json_response(
                stream,
                400,
                "Bad Request",
                &json!({ "error": format!("AgentIntent validation failed: {e}"), "code": "VALIDATION_FAILED" }),
            ).await;
        }

        // Policy evaluation
        let empty_caps = BTreeSet::new();
        let source_agent_str = intent.source_agent.to_string();
        let policy_input = PolicyInput {
            kind: "act",
            subject: &source_agent_str,
            source_node: None,
            target_node: None,
            content_type: Some("application/json"),
            consensus_protected: false,
            granted_capabilities: &empty_caps,
            human_approved: false,
            simulation_passed: false,
        };
        let eval = self.policy_set.evaluate(&policy_input);
        if !eval.allowed || matches!(eval.decision, PolicyDecision::Deny) {
            if let Some(node) = &self.node {
                node.record_agent_gateway_request("http", "403");
            }
            return self.send_json_response(
                stream,
                403,
                "Forbidden",
                &json!({ "error": format!("Policy denied intent: {:?}", eval.decision), "code": "POLICY_DENIED" }),
            ).await;
        }

        // Journal and provenance creation
        let (seq, receipt_id) =
            if let (Some(journal_lock), Some(key)) = (&self.journal, &self.keypair) {
                let now = now_micros().unwrap_or(0);
                let frame = ZapFrame::with_timestamp(
                    key.node_id(),
                    key.node_id(),
                    ZapFlags::SIGNED,
                    now,
                    Bytes::copy_from_slice(intent.objective.as_bytes()),
                )
                .map_err(|e| ZapGatewayError::Internal(e.to_string()))?;
                let signed_receipt = SignedActionReceipt::new(
                    key,
                    &frame,
                    format!("intent:{}", intent.intent_id),
                    None,
                    now,
                    None,
                )
                .map_err(|e| ZapGatewayError::Internal(e.to_string()))?;
                let journal = journal_lock
                    .lock()
                    .map_err(|e| ZapGatewayError::Internal(e.to_string()))?;
                journal
                    .append(&signed_receipt, false)
                    .map_err(|e| ZapGatewayError::Internal(e.to_string()))?;
                (1, format!("rcpt-{}", intent.intent_id))
            } else {
                (1, format!("rcpt-{}", intent.intent_id))
            };

        let provenance = if let Some(key) = &self.keypair {
            let builder = ProvenanceChainBuilder::new(intent.session_id, intent.intent_id)
                .with_intent(&intent)?
                .with_policy("policy_default_sha256", "ALLOW", BTreeMap::new())?
                .with_receipt(&receipt_id, now_micros().unwrap_or(0), BTreeMap::new())?;
            Some(builder.build_and_sign(key)?)
        } else {
            None
        };

        if let Some(node) = &self.node {
            node.record_agent_gateway_request("http", "202");
        }

        // Broadcast status over SSE
        self.sse_broker.send(SseEvent::new(
            "agent_status",
            json!({
                "session_id": intent.session_id,
                "intent_id": intent.intent_id,
                "status": "accepted",
                "sequence": seq,
            })
            .to_string(),
        ));

        self.send_json_response(
            stream,
            202,
            "Accepted",
            &json!({
                "status": "accepted",
                "intent_id": intent.intent_id,
                "session_id": intent.session_id,
                "sequence": seq,
                "receipt_id": receipt_id,
                "provenance": provenance,
            }),
        )
        .await
    }

    async fn handle_post_session(&self, stream: &mut TcpStream, body: &[u8]) -> Result<()> {
        let session_val: Value = match serde_json::from_slice(body) {
            Ok(v) => v,
            Err(e) => {
                if let Some(node) = &self.node {
                    node.record_agent_gateway_request("http", "400");
                }
                return self
                    .send_json_response(
                        stream,
                        400,
                        "Bad Request",
                        &json!({ "error": format!("Invalid JSON: {e}"), "code": "INVALID_JSON" }),
                    )
                    .await;
            }
        };

        let owner_agent = session_val
            .get("owner_agent")
            .and_then(|a| a.as_str())
            .unwrap_or("agent_default");
        let agent_id = match AgentId::new(owner_agent) {
            Ok(id) => id,
            Err(e) => {
                return self.send_json_response(
                    stream,
                    400,
                    "Bad Request",
                    &json!({ "error": format!("Invalid owner_agent ID: {e}"), "code": "INVALID_AGENT_ID" }),
                ).await;
            }
        };

        let session_id = session_val
            .get("session_id")
            .and_then(|s| s.as_str())
            .and_then(|s| Uuid::parse_str(s).ok())
            .unwrap_or_else(Uuid::new_v4);

        let now = now_micros().unwrap_or(0);
        let session = AgentSession::new(session_id, agent_id, now);

        if let Some(node) = &self.node {
            node.inc_agent_session();
            node.record_agent_gateway_request("http", "201");
        }

        self.send_json_response(stream, 201, "Created", &json!(session))
            .await
    }

    async fn handle_post_delegate(&self, stream: &mut TcpStream, body: &[u8]) -> Result<()> {
        let req: DelegationRequest = match serde_json::from_slice(body) {
            Ok(r) => r,
            Err(e) => {
                return self.send_json_response(
                    stream,
                    400,
                    "Bad Request",
                    &json!({ "error": format!("Invalid DelegationRequest: {e}"), "code": "INVALID_REQUEST" }),
                ).await;
            }
        };

        if let Err(e) = req.validate() {
            return self.send_json_response(
                stream,
                400,
                "Bad Request",
                &json!({ "error": format!("Validation failed: {e}"), "code": "VALIDATION_FAILED" }),
            ).await;
        }

        let resp = DelegationResponse {
            schema_version: AGENT_PROTOCOL_SCHEMA_VERSION,
            delegation_id: req.delegation_id,
            session_id: req.session_id,
            respondent_agent: req
                .to_agent
                .clone()
                .unwrap_or_else(|| AgentId::new("agent.delegate").unwrap()),
            decision: DelegationDecision::Accepted,
            assigned_agent: req
                .to_agent
                .or_else(|| Some(AgentId::new("agent.worker").unwrap())),
            accepted_capabilities: req.required_capabilities,
            reason: None,
            estimated_completion_unix_micros: Some(now_micros().unwrap_or(0) + 10_000_000),
            metadata: BTreeMap::new(),
        };

        if let Some(node) = &self.node {
            node.record_agent_gateway_request("http", "200");
        }

        self.send_json_response(stream, 200, "OK", &json!(resp))
            .await
    }

    async fn handle_post_negotiate(&self, stream: &mut TcpStream, body: &[u8]) -> Result<()> {
        let req: CapabilityNegotiationRequest = match serde_json::from_slice(body) {
            Ok(r) => r,
            Err(e) => {
                return self.send_json_response(
                    stream,
                    400,
                    "Bad Request",
                    &json!({ "error": format!("Invalid CapabilityNegotiationRequest: {e}"), "code": "INVALID_REQUEST" }),
                ).await;
            }
        };

        if let Err(e) = req.validate() {
            return self.send_json_response(
                stream,
                400,
                "Bad Request",
                &json!({ "error": format!("Validation failed: {e}"), "code": "VALIDATION_FAILED" }),
            ).await;
        }

        let resp = CapabilityNegotiationResponse {
            schema_version: AGENT_PROTOCOL_SCHEMA_VERSION,
            negotiation_id: req.negotiation_id,
            session_id: req.session_id,
            responder_agent: AgentId::new("gateway.responder").unwrap(),
            decision: zap_agent::CapabilityNegotiationDecision::Accepted,
            accepted_capabilities: req.required_capabilities,
            unsupported_capabilities: std::collections::BTreeSet::new(),
            supported_intents: req.desired_intents,
            expires_at_unix_micros: Some(now_micros().unwrap_or(0) + 60_000_000),
            reason: None,
            metadata: BTreeMap::new(),
        };

        if let Some(node) = &self.node {
            node.record_agent_gateway_request("http", "200");
        }

        self.send_json_response(stream, 200, "OK", &json!(resp))
            .await
    }

    async fn handle_post_provenance_verify(
        &self,
        stream: &mut TcpStream,
        body: &[u8],
    ) -> Result<()> {
        let body_val: Value = match serde_json::from_slice(body) {
            Ok(v) => v,
            Err(e) => {
                return self
                    .send_json_response(
                        stream,
                        400,
                        "Bad Request",
                        &json!({ "error": format!("Invalid JSON: {e}"), "code": "INVALID_JSON" }),
                    )
                    .await;
            }
        };

        let chain: ProvenanceChainDigest = match serde_json::from_value(body_val.clone()) {
            Ok(c) => c,
            Err(e) => {
                return self.send_json_response(
                    stream,
                    400,
                    "Bad Request",
                    &json!({ "error": format!("Invalid ProvenanceChainDigest: {e}"), "code": "INVALID_CHAIN" }),
                ).await;
            }
        };

        let pk = if let Some(pk_str) = body_val.get("public_key").and_then(|p| p.as_str()) {
            match hex::decode(pk_str) {
                Ok(b) if b.len() == 32 => {
                    let mut arr = [0u8; 32];
                    arr.copy_from_slice(&b);
                    PublicKey::from_bytes(arr)?
                }
                _ => {
                    return self
                        .send_json_response(
                            stream,
                            400,
                            "Bad Request",
                            &json!({ "error": "Invalid public_key hex format", "code": "INVALID_KEY" }),
                        )
                        .await;
                }
            }
        } else if let Some(keypair) = &self.keypair {
            keypair.verifying_key()
        } else {
            return self.send_json_response(
                stream,
                400,
                "Bad Request",
                &json!({ "error": "No public_key provided and no local node identity", "code": "MISSING_KEY" }),
            ).await;
        };

        let report = chain.verify(&pk)?;
        if !report.valid
            && let Some(node) = &self.node
        {
            node.record_provenance_verification_failure();
        }

        if let Some(node) = &self.node {
            node.record_agent_gateway_request("http", "200");
        }

        self.send_json_response(stream, 200, "OK", &json!(report))
            .await
    }

    async fn handle_sse_stream(&self, mut stream: TcpStream) -> Result<()> {
        let headers = "HTTP/1.1 200 OK\r\n\
Content-Type: text/event-stream\r\n\
Cache-Control: no-cache\r\n\
Connection: keep-alive\r\n\
Access-Control-Allow-Origin: *\r\n\r\n";

        stream.write_all(headers.as_bytes()).await?;
        stream.flush().await?;

        if let Some(node) = &self.node {
            node.record_agent_gateway_request("sse", "200");
        }

        let mut rx = self.sse_broker.subscribe();
        let initial_event = SseEvent::new("connected", r#"{"status":"ready"}"#);
        stream
            .write_all(initial_event.to_sse_wire_format().as_bytes())
            .await?;
        stream.flush().await?;

        loop {
            tokio::select! {
                event_res = rx.recv() => {
                    match event_res {
                        Ok(event) => {
                            if stream.write_all(event.to_sse_wire_format().as_bytes()).await.is_err() {
                                break; // Connection dropped
                            }
                            if stream.flush().await.is_err() {
                                break;
                            }
                        }
                        Err(broadcast::error::RecvError::Lagged(missed)) => {
                            warn!("SSE client lagged, missed {missed} events");
                        }
                        Err(broadcast::error::RecvError::Closed) => break,
                    }
                }
            }
        }

        Ok(())
    }

    async fn handle_websocket_upgrade(&self, mut stream: TcpStream, ws_key: &str) -> Result<()> {
        let accept_key = compute_ws_accept(ws_key);
        let handshake_resp = format!(
            "HTTP/1.1 101 Switching Protocols\r\n\
Upgrade: websocket\r\n\
Connection: Upgrade\r\n\
Sec-WebSocket-Accept: {}\r\n\r\n",
            accept_key
        );

        stream.write_all(handshake_resp.as_bytes()).await?;
        stream.flush().await?;

        if let Some(node) = &self.node {
            node.inc_agent_session();
            node.record_agent_gateway_request("ws", "101");
        }

        let ws_handler = WebSocketHandler::new(self.config.max_frame_size);
        let (mut reader, mut writer) = stream.into_split();

        loop {
            match ws_handler.read_frame(&mut reader).await {
                Ok(frame) => match frame.opcode {
                    WS_OPCODE_TEXT | WS_OPCODE_BINARY => {
                        let reply = json!({
                            "status": "acknowledged",
                            "bytes_received": frame.payload.len(),
                            "timestamp_micros": now_micros().unwrap_or(0),
                        });
                        let reply_frame = WsFrame::text(serde_json::to_string(&reply)?);
                        if let Err(e) = ws_handler.write_frame(&mut writer, &reply_frame).await {
                            warn!("WebSocket write error: {e}");
                            break;
                        }
                    }
                    WS_OPCODE_PING => {
                        let pong = WsFrame::pong(frame.payload);
                        let _ = ws_handler.write_frame(&mut writer, &pong).await;
                    }
                    WS_OPCODE_CLOSE => {
                        let close = WsFrame::close(WS_CLOSE_NORMAL, "goodbye");
                        let _ = ws_handler.write_frame(&mut writer, &close).await;
                        break;
                    }
                    _ => {}
                },
                Err(ZapGatewayError::FrameSizeExceeded { size, max }) => {
                    warn!("WebSocket frame size {size} exceeded max {max}");
                    let close_frame = WsFrame::close(WS_CLOSE_MESSAGE_TOO_BIG, "Message Too Big");
                    let _ = ws_handler.write_frame(&mut writer, &close_frame).await;
                    break;
                }
                Err(err) => {
                    debug!("WebSocket read finished: {err}");
                    break;
                }
            }
        }

        if let Some(node) = &self.node {
            node.dec_agent_session();
        }

        Ok(())
    }

    async fn send_json_response(
        &self,
        stream: &mut TcpStream,
        status_code: u16,
        status_text: &str,
        data: &Value,
    ) -> Result<()> {
        let json_bytes = serde_json::to_vec(data)?;
        self.send_raw_response(
            stream,
            status_code,
            status_text,
            "application/json",
            &json_bytes,
        )
        .await
    }

    async fn send_raw_response(
        &self,
        stream: &mut TcpStream,
        status_code: u16,
        status_text: &str,
        content_type: &str,
        body: &[u8],
    ) -> Result<()> {
        let headers = format!(
            "HTTP/1.1 {} {}\r\n\
Content-Type: {}\r\n\
Content-Length: {}\r\n\
Connection: close\r\n\
Access-Control-Allow-Origin: *\r\n\
Access-Control-Allow-Methods: GET, POST, OPTIONS\r\n\
Access-Control-Allow-Headers: Content-Type, Authorization\r\n\r\n",
            status_code,
            status_text,
            content_type,
            body.len()
        );

        stream.write_all(headers.as_bytes()).await?;
        if !body.is_empty() {
            stream.write_all(body).await?;
        }
        stream.flush().await?;
        Ok(())
    }
}
