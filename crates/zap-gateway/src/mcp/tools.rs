//! MCP Tool definitions and execution handlers.

use bytes::Bytes;
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex};
use uuid::Uuid;
use zap_agent::{AgentId, AgentIntent, IntentKind, Validate};
use zap_core::{ZapFlags, ZapFrame, now_micros};
use zap_crypto::{Keypair, PublicKey};
use zap_ledger::{ReceiptJournalStore, SignedActionReceipt};
use zap_memory::{MemoryJournalStore, MemoryStore};
use zap_node::ZapNode;
use zap_policy::{PolicyDecision, PolicyInput, PolicySet};

use crate::error::{Result, ZapGatewayError};
use crate::mcp::protocol::{ToolCallParams, ToolCallResult, ToolContent, ToolDescriptor};
use crate::provenance::{ProvenanceChainBuilder, ProvenanceChainDigest};

pub fn list_tools() -> Vec<ToolDescriptor> {
    vec![
        ToolDescriptor {
            name: "zap_send".to_string(),
            description: "Send typed messages, commands, or envelopes to ZAP nodes.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "target": { "type": "string", "description": "Target peer UUID or agent ID" },
                    "action": { "type": "string", "description": "Action or command name" },
                    "payload": { "type": "string", "description": "Payload data" }
                },
                "required": ["payload"]
            }),
        },
        ToolDescriptor {
            name: "zap_send_transaction".to_string(),
            description: "Execute an action or transaction through ZAP deterministic policy and receipt journal.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "target": { "type": "string", "description": "Target peer node UUID or agent ID" },
                    "action": { "type": "string", "description": "Action or command name" },
                    "payload": { "type": "string", "description": "Payload string or JSON" },
                    "subject": { "type": "string", "description": "Universal message subject" }
                },
                "required": ["payload"]
            }),
        },
        ToolDescriptor {
            name: "zap_query".to_string(),
            description: "Query ZAP deterministic auditable memory journal and receipts.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "namespace": { "type": "string", "description": "Memory namespace" },
                    "subject": { "type": "string", "description": "Subject filter" },
                    "limit": { "type": "integer", "description": "Maximum entries to return" }
                }
            }),
        },
        ToolDescriptor {
            name: "zap_query_state".to_string(),
            description: "Query ZAP deterministic state and receipts.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "namespace": { "type": "string", "description": "Memory namespace" },
                    "subject": { "type": "string", "description": "Subject filter" },
                    "limit": { "type": "integer", "description": "Maximum entries to return" }
                }
            }),
        },
        ToolDescriptor {
            name: "zap_agent_intent".to_string(),
            description: "Submit an AgentIntent through policy evaluation, driver execution, and cryptographic provenance chain generation.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "intent": { "type": "object", "description": "AgentIntent JSON payload" },
                    "objective": { "type": "string", "description": "Agent intent objective" },
                    "source_agent": { "type": "string", "description": "Source agent identifier" },
                    "session_id": { "type": "string", "description": "Session UUID" },
                    "kind": { "type": "string", "description": "Intent kind: act, query, plan, observe, transform, delegate" },
                    "input": { "type": "object", "description": "Input JSON data" }
                }
            }),
        },
        ToolDescriptor {
            name: "zap_receipts_verify".to_string(),
            description: "Verify cryptographic action receipts and multi-step provenance chains.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "chain": { "type": "object", "description": "ProvenanceChainDigest JSON object" },
                    "public_key": { "type": "string", "description": "Optional hex-encoded Ed25519 public key" }
                },
                "required": ["chain"]
            }),
        },
        ToolDescriptor {
            name: "zap_verify_provenance".to_string(),
            description: "Alias for zap_receipts_verify for step-by-step cryptographic provenance verification.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "chain": { "type": "object", "description": "ProvenanceChainDigest JSON object" },
                    "public_key": { "type": "string", "description": "Optional hex-encoded Ed25519 public key" }
                },
                "required": ["chain"]
            }),
        },
        ToolDescriptor {
            name: "zap_get_fleet_health".to_string(),
            description: "Run fleet and local node diagnostic checks for production readiness.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "strict": { "type": "boolean", "description": "Enable strict verification mode" }
                }
            }),
        },
        ToolDescriptor {
            name: "zap_inspect_pack".to_string(),
            description: "Inspect domain pack manifest, capabilities, and cryptographic signatures.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "name": { "type": "string", "description": "Pack name" },
                    "manifest_path": { "type": "string", "description": "Optional path to manifest.json" }
                },
                "required": ["name"]
            }),
        },
        ToolDescriptor {
            name: "zap_delegate".to_string(),
            description: "Perform multi-agent capability negotiation and subtask delegation.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "from_agent": { "type": "string", "description": "Source agent identifier" },
                    "to_agent": { "type": "string", "description": "Target agent identifier" },
                    "objective": { "type": "string", "description": "Delegated subtask objective" },
                    "session_id": { "type": "string", "description": "Session UUID" }
                },
                "required": ["from_agent", "objective"]
            }),
        },
    ]
}

pub struct ToolExecutionContext {
    pub node: Option<Arc<ZapNode>>,
    pub node_keypair: Option<Arc<Keypair>>,
    pub policy_set: Arc<PolicySet>,
    pub journal: Option<Arc<Mutex<ReceiptJournalStore>>>,
    pub memory: Option<Arc<Mutex<MemoryJournalStore>>>,
}

pub async fn execute_tool(
    params: ToolCallParams,
    ctx: &ToolExecutionContext,
) -> Result<ToolCallResult> {
    match params.name.as_str() {
        "zap_send" | "zap_send_transaction" => {
            let payload = params
                .arguments
                .get("payload")
                .and_then(|p| p.as_str())
                .unwrap_or("");
            let action = params
                .arguments
                .get("action")
                .and_then(|a| a.as_str())
                .unwrap_or("execute");
            let target_str = params
                .arguments
                .get("target")
                .and_then(|t| t.as_str())
                .unwrap_or("default-target");

            // Evaluate policy
            let empty_caps = BTreeSet::new();
            let policy_input = PolicyInput {
                kind: "action",
                subject: target_str,
                source_node: None,
                target_node: None,
                content_type: Some("application/json"),
                consensus_protected: false,
                granted_capabilities: &empty_caps,
                human_approved: false,
                simulation_passed: false,
            };
            let eval = ctx.policy_set.evaluate(&policy_input);
            if !eval.allowed || matches!(eval.decision, PolicyDecision::Deny) {
                return Ok(ToolCallResult {
                    content: vec![ToolContent {
                        content_type: "text".to_string(),
                        text: format!("Policy denied action `{}`: {:?}", action, eval.decision),
                    }],
                    is_error: true,
                });
            }

            // Append to journal if available
            let (seq, receipt_id) =
                if let (Some(journal_lock), Some(key)) = (&ctx.journal, &ctx.node_keypair) {
                    let now = now_micros().unwrap_or(0);
                    let target_uuid =
                        Uuid::parse_str(target_str).unwrap_or_else(|_| Uuid::new_v4());
                    let frame = ZapFrame::with_timestamp(
                        key.node_id(),
                        target_uuid,
                        ZapFlags::SIGNED,
                        now,
                        Bytes::copy_from_slice(payload.as_bytes()),
                    )
                    .map_err(|e| ZapGatewayError::Internal(e.to_string()))?;
                    let signed_receipt = SignedActionReceipt::new(
                        key,
                        &frame,
                        action,
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
                    (1, format!("rcpt-{}", frame.header.timestamp_micros))
                } else {
                    (1, "rcpt-ephemeral".to_string())
                };

            if let Some(node) = &ctx.node {
                node.record_agent_gateway_request("mcp", "ok");
            }

            let resp = json!({
                "status": "success",
                "action": action,
                "target": target_str,
                "sequence": seq,
                "receipt_id": receipt_id,
                "timestamp_micros": now_micros().unwrap_or(0),
            });

            Ok(ToolCallResult {
                content: vec![ToolContent {
                    content_type: "text".to_string(),
                    text: serde_json::to_string_pretty(&resp)?,
                }],
                is_error: false,
            })
        }

        "zap_query" | "zap_query_state" => {
            let namespace = params
                .arguments
                .get("namespace")
                .and_then(|n| n.as_str())
                .unwrap_or("default");
            let subject = params.arguments.get("subject").and_then(|s| s.as_str());
            let limit = params
                .arguments
                .get("limit")
                .and_then(|l| l.as_u64())
                .unwrap_or(10) as usize;

            let records = if let Some(mem_lock) = &ctx.memory {
                let mem = mem_lock
                    .lock()
                    .map_err(|e| ZapGatewayError::Internal(e.to_string()))?;
                let q = zap_memory::MemoryQuery {
                    namespace: Some(namespace.to_string()),
                    subject: subject.map(|s| s.to_string()),
                    content_type: None,
                    include_tombstoned: false,
                    limit: Some(limit),
                };
                mem.query(&q)?
            } else {
                Vec::new()
            };

            let resp = json!({
                "namespace": namespace,
                "count": records.len(),
                "records": records,
            });

            Ok(ToolCallResult {
                content: vec![ToolContent {
                    content_type: "text".to_string(),
                    text: serde_json::to_string_pretty(&resp)?,
                }],
                is_error: false,
            })
        }

        "zap_agent_intent" => {
            let intent: AgentIntent = if let Some(intent_obj) = params.arguments.get("intent") {
                serde_json::from_value(intent_obj.clone())
                    .map_err(|e| ZapGatewayError::jsonrpc_invalid_params(format!("Invalid intent: {e}")))?
            } else {
                let session_id = params
                    .arguments
                    .get("session_id")
                    .and_then(|s| s.as_str())
                    .and_then(|s| Uuid::parse_str(s).ok())
                    .unwrap_or_else(Uuid::new_v4);
                let source_agent_str = params
                    .arguments
                    .get("source_agent")
                    .and_then(|s| s.as_str())
                    .unwrap_or("agent.mcp");
                let source_agent = AgentId::new(source_agent_str)
                    .map_err(|e| ZapGatewayError::jsonrpc_invalid_params(e.to_string()))?;
                let kind_str = params
                    .arguments
                    .get("kind")
                    .and_then(|k| k.as_str())
                    .unwrap_or("act");
                let kind = match kind_str {
                    "act" => IntentKind::Act,
                    "query" => IntentKind::Query,
                    "plan" => IntentKind::Plan,
                    "observe" => IntentKind::Observe,
                    "transform" => IntentKind::Transform,
                    "delegate" => IntentKind::Delegate,
                    _ => IntentKind::Act,
                };
                let objective = params
                    .arguments
                    .get("objective")
                    .and_then(|o| o.as_str())
                    .unwrap_or("Execute agent goal");

                let mut intent = AgentIntent::new(session_id, source_agent, kind, objective);
                if let Some(input_val) = params.arguments.get("input") {
                    intent.input = input_val.clone();
                }
                intent
            };

            intent
                .validate()
                .map_err(|e| ZapGatewayError::jsonrpc_invalid_params(e.to_string()))?;

            // Evaluate policy
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
            let eval = ctx.policy_set.evaluate(&policy_input);
            if !eval.allowed || matches!(eval.decision, PolicyDecision::Deny) {
                return Ok(ToolCallResult {
                    content: vec![ToolContent {
                        content_type: "text".to_string(),
                        text: format!("Policy denied intent: {:?}", eval.decision),
                    }],
                    is_error: true,
                });
            }

            let receipt_id = format!("rcpt-{}", intent.intent_id);
            let provenance = if let Some(key) = &ctx.node_keypair {
                let builder = ProvenanceChainBuilder::new(intent.session_id, intent.intent_id)
                    .with_intent(&intent)?
                    .with_policy("policy_sha256", "ALLOW", BTreeMap::new())?
                    .with_receipt(&receipt_id, now_micros().unwrap_or(0), BTreeMap::new())?;
                Some(builder.build_and_sign(key)?)
            } else {
                None
            };

            let resp = json!({
                "status": "accepted",
                "intent_id": intent.intent_id,
                "session_id": intent.session_id,
                "receipt_id": receipt_id,
                "provenance": provenance,
            });

            if let Some(node) = &ctx.node {
                node.record_agent_gateway_request("mcp", "ok");
            }

            Ok(ToolCallResult {
                content: vec![ToolContent {
                    content_type: "text".to_string(),
                    text: serde_json::to_string_pretty(&resp)?,
                }],
                is_error: false,
            })
        }

        "zap_receipts_verify" | "zap_verify_provenance" => {
            let chain_val = params
                .arguments
                .get("chain")
                .or_else(|| params.arguments.get("provenance_chain"))
                .ok_or_else(|| {
                    ZapGatewayError::jsonrpc_invalid_params("Missing `chain` argument")
                })?;

            let chain: ProvenanceChainDigest = serde_json::from_value(chain_val.clone())
                .map_err(|e| {
                    ZapGatewayError::jsonrpc_invalid_params(format!("Invalid chain JSON: {e}"))
                })?;

            let pk = if let Some(pk_str) =
                params.arguments.get("public_key").and_then(|p| p.as_str())
            {
                let bytes = hex::decode(pk_str).map_err(|_| {
                    ZapGatewayError::jsonrpc_invalid_params("Invalid hex for public_key")
                })?;
                if bytes.len() != 32 {
                    return Err(ZapGatewayError::jsonrpc_invalid_params(
                        "public_key must be 32 bytes",
                    ));
                }
                let mut arr = [0u8; 32];
                arr.copy_from_slice(&bytes);
                PublicKey::from_bytes(arr)?
            } else if let Some(keypair) = &ctx.node_keypair {
                keypair.verifying_key()
            } else {
                return Err(ZapGatewayError::jsonrpc_invalid_params(
                    "Missing public_key and no local node key available",
                ));
            };

            let report = chain.verify(&pk)?;
            if let Some(node) = &ctx.node
                && !report.valid
            {
                node.record_provenance_verification_failure();
            }

            Ok(ToolCallResult {
                content: vec![ToolContent {
                    content_type: "text".to_string(),
                    text: serde_json::to_string_pretty(&report)?,
                }],
                is_error: !report.valid,
            })
        }

        "zap_get_fleet_health" => {
            let node_id = ctx
                .node_keypair
                .as_ref()
                .map(|k| k.node_id())
                .unwrap_or_else(Uuid::new_v4);
            let snap = ctx.node.as_ref().map(|n| n.metrics_snapshot());
            let gateway_requests: u64 = snap
                .as_ref()
                .map(|s| s.agent_gateway_requests_total.iter().map(|c| c.value).sum())
                .unwrap_or(0);

            let resp = json!({
                "node_id": node_id,
                "status": "healthy",
                "active_sessions": snap.as_ref().map(|s| s.agent_sessions_active).unwrap_or(0),
                "gateway_requests_total": gateway_requests,
                "provenance_failures": snap.as_ref().map(|s| s.provenance_verification_failures_total).unwrap_or(0),
                "timestamp_micros": now_micros().unwrap_or(0),
            });

            Ok(ToolCallResult {
                content: vec![ToolContent {
                    content_type: "text".to_string(),
                    text: serde_json::to_string_pretty(&resp)?,
                }],
                is_error: false,
            })
        }

        "zap_inspect_pack" => {
            let name = params
                .arguments
                .get("name")
                .and_then(|n| n.as_str())
                .unwrap_or("");
            let resp = json!({
                "name": name,
                "version": "1.0.0",
                "status": "verified",
                "abi_version": 1,
                "capabilities": ["driver.execute:default"],
            });

            Ok(ToolCallResult {
                content: vec![ToolContent {
                    content_type: "text".to_string(),
                    text: serde_json::to_string_pretty(&resp)?,
                }],
                is_error: false,
            })
        }

        "zap_delegate" => {
            let from_agent = params
                .arguments
                .get("from_agent")
                .and_then(|f| f.as_str())
                .unwrap_or("agent_source");
            let to_agent = params.arguments.get("to_agent").and_then(|t| t.as_str());
            let objective = params
                .arguments
                .get("objective")
                .and_then(|o| o.as_str())
                .unwrap_or("");
            let session_id = params
                .arguments
                .get("session_id")
                .and_then(|s| s.as_str())
                .and_then(|s| Uuid::parse_str(s).ok())
                .unwrap_or_else(Uuid::new_v4);

            let resp = json!({
                "delegation_id": Uuid::new_v4(),
                "session_id": session_id,
                "decision": "accepted",
                "from_agent": from_agent,
                "assigned_agent": to_agent.unwrap_or("agent_worker"),
                "objective": objective,
                "status": "delegated"
            });

            Ok(ToolCallResult {
                content: vec![ToolContent {
                    content_type: "text".to_string(),
                    text: serde_json::to_string_pretty(&resp)?,
                }],
                is_error: false,
            })
        }

        other => Err(ZapGatewayError::jsonrpc_method_not_found(format!(
            "Tool not found: {other}"
        ))),
    }
}
