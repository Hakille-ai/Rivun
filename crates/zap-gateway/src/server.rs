//! Agent Gateway Server orchestrating MCP, HTTP REST, SSE, and WebSocket transports.

use std::sync::{Arc, Mutex};
use tokio::net::TcpListener;
use tracing::info;
use zap_crypto::Keypair;
use zap_ledger::ReceiptJournalStore;
use zap_memory::MemoryJournalStore;
use zap_node::ZapNode;
use zap_policy::PolicySet;

use crate::config::GatewayConfig;
use crate::error::Result;
use crate::mcp::McpEngine;
use crate::mcp::stdio::McpStdioTransport;
use crate::transports::http::HttpAgentGateway;
use crate::transports::sse::SseBroker;

pub struct AgentGatewayServer {
    config: GatewayConfig,
    http_gateway: Arc<HttpAgentGateway>,
}

impl AgentGatewayServer {
    pub fn new(
        config: GatewayConfig,
        node: Option<Arc<ZapNode>>,
        keypair: Option<Arc<Keypair>>,
        policy_set: Option<Arc<PolicySet>>,
        journal: Option<Arc<Mutex<ReceiptJournalStore>>>,
        memory: Option<Arc<Mutex<MemoryJournalStore>>>,
    ) -> Self {
        let policy = policy_set.unwrap_or_else(|| Arc::new(PolicySet::default()));
        let sse_broker = SseBroker::default();

        let http_gateway = Arc::new(HttpAgentGateway::new(
            config.clone(),
            node,
            keypair,
            policy,
            journal,
            memory,
            sse_broker,
        ));

        Self {
            config,
            http_gateway,
        }
    }

    pub fn sse_broker(&self) -> &SseBroker {
        self.http_gateway.sse_broker()
    }

    pub fn mcp_engine(&self) -> &McpEngine {
        self.http_gateway.mcp_engine()
    }

    pub async fn run(&self) -> Result<()> {
        if self.config.enable_mcp_stdio {
            let mcp_engine = self.http_gateway.mcp_engine().clone();
            tokio::spawn(async move {
                let stdio_transport = McpStdioTransport::new(mcp_engine);
                if let Err(e) = stdio_transport.run().await {
                    tracing::error!("MCP stdio transport error: {e}");
                }
            });
        }

        let listener = TcpListener::bind(self.config.http_bind).await?;
        info!(
            "ZAP Agent Gateway successfully bound to {}",
            listener.local_addr()?
        );

        self.http_gateway.clone().run_server(listener).await
    }

    pub async fn run_on_listener(&self, listener: TcpListener) -> Result<()> {
        self.http_gateway.clone().run_server(listener).await
    }
}
