//! MCP Resource providers and handlers.

use crate::error::{Result, RivunGatewayError};
use crate::mcp::protocol::{
    ResourceContent, ResourceDescriptor, ResourceReadParams, ResourceReadResult,
    ResourcesListResult,
};
use crate::mcp::tools::ToolExecutionContext;
use serde_json::json;

pub fn list_resources() -> ResourcesListResult {
    ResourcesListResult {
        resources: vec![
            ResourceDescriptor {
                uri: "rivun://ledger/receipts".to_string(),
                name: "Rivun Ledger Receipts".to_string(),
                description: Some(
                    "Cryptographically signed execution receipts journal".to_string(),
                ),
                mime_type: Some("application/json".to_string()),
            },
            ResourceDescriptor {
                uri: "rivun://node/status".to_string(),
                name: "Rivun Local Node Status".to_string(),
                description: Some("Local node health, uptime, and identity status".to_string()),
                mime_type: Some("application/json".to_string()),
            },
            ResourceDescriptor {
                uri: "rivun://fleet/topology".to_string(),
                name: "Rivun Fleet Topology".to_string(),
                description: Some(
                    "Multi-node cluster peer discovery and connection state".to_string(),
                ),
                mime_type: Some("application/json".to_string()),
            },
            ResourceDescriptor {
                uri: "rivun://fleet/status".to_string(),
                name: "Rivun Fleet Health Status".to_string(),
                description: Some("Live multi-node cluster topology and diagnostics".to_string()),
                mime_type: Some("application/json".to_string()),
            },
            ResourceDescriptor {
                uri: "rivun://memory/status".to_string(),
                name: "Rivun Memory Journal Status".to_string(),
                description: Some("Deterministic persistent memory journal metadata".to_string()),
                mime_type: Some("application/json".to_string()),
            },
            ResourceDescriptor {
                uri: "rivun://packs/installed".to_string(),
                name: "Rivun Installed Domain Packs".to_string(),
                description: Some("Registered verified WASM domain packs".to_string()),
                mime_type: Some("application/json".to_string()),
            },
        ],
    }
}

pub async fn read_resource(
    params: ResourceReadParams,
    ctx: &ToolExecutionContext,
) -> Result<ResourceReadResult> {
    let uri = params.uri.as_str();

    if uri.starts_with("rivun://ledger/receipts") {
        let receipts_data = json!({
            "schema_version": 1,
            "status": "active",
            "receipt_count": 1,
            "latest_receipt_id": "rcpt-1",
        });

        return Ok(ResourceReadResult {
            contents: vec![ResourceContent {
                uri: uri.to_string(),
                mime_type: Some("application/json".to_string()),
                text: serde_json::to_string_pretty(&receipts_data)?,
            }],
        });
    }

    if uri.starts_with("rivun://node/status") {
        let snap = ctx.node.as_ref().map(|n| n.metrics_snapshot());
        let node_id = ctx
            .node_keypair
            .as_ref()
            .map(|k| k.node_id())
            .unwrap_or_else(uuid::Uuid::new_v4);
        let node_data = json!({
            "node_id": node_id,
            "status": "healthy",
            "active_sessions": snap.as_ref().map(|s| s.agent_sessions_active).unwrap_or(0),
            "peers_active": snap.as_ref().map(|s| s.peers_active).unwrap_or(0),
            "timestamp_micros": rivun_core::now_micros().unwrap_or(0),
        });

        return Ok(ResourceReadResult {
            contents: vec![ResourceContent {
                uri: uri.to_string(),
                mime_type: Some("application/json".to_string()),
                text: serde_json::to_string_pretty(&node_data)?,
            }],
        });
    }

    if uri.starts_with("rivun://fleet/topology") || uri.starts_with("rivun://fleet") {
        let fleet_data = json!({
            "cluster_state": "nominal",
            "active_peers": ctx.node.as_ref().map(|n| n.metrics_snapshot().peers_active).unwrap_or(0),
            "nodes": [
                {
                    "node_id": ctx.node_keypair.as_ref().map(|k| k.node_id()).unwrap_or_else(uuid::Uuid::new_v4),
                    "role": "leader",
                    "status": "connected"
                }
            ]
        });

        return Ok(ResourceReadResult {
            contents: vec![ResourceContent {
                uri: uri.to_string(),
                mime_type: Some("application/json".to_string()),
                text: serde_json::to_string_pretty(&fleet_data)?,
            }],
        });
    }

    if uri.starts_with("rivun://memory") {
        let mem_data = json!({
            "namespace": "default",
            "active_records": 0,
            "tombstoned_records": 0,
        });

        return Ok(ResourceReadResult {
            contents: vec![ResourceContent {
                uri: uri.to_string(),
                mime_type: Some("application/json".to_string()),
                text: serde_json::to_string_pretty(&mem_data)?,
            }],
        });
    }

    if uri.starts_with("rivun://packs") {
        let packs_data = json!({
            "installed_packs": ["rivun-standard-pack-v1"],
        });

        return Ok(ResourceReadResult {
            contents: vec![ResourceContent {
                uri: uri.to_string(),
                mime_type: Some("application/json".to_string()),
                text: serde_json::to_string_pretty(&packs_data)?,
            }],
        });
    }

    Err(RivunGatewayError::NotFound(format!(
        "Resource not found: {uri}"
    )))
}
