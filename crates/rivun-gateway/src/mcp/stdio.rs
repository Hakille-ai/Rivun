//! MCP stdio transport loop.

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tracing::{error, info};

use crate::mcp::McpEngine;

pub struct McpStdioTransport {
    engine: McpEngine,
}

impl McpStdioTransport {
    pub fn new(engine: McpEngine) -> Self {
        Self { engine }
    }

    pub async fn run(self) -> Result<(), std::io::Error> {
        let stdin = tokio::io::stdin();
        let mut stdout = tokio::io::stdout();
        let mut reader = BufReader::new(stdin);
        let mut line = String::new();

        info!("MCP stdio transport started. Listening on stdin...");

        loop {
            line.clear();
            let bytes_read = reader.read_line(&mut line).await?;
            if bytes_read == 0 {
                break; // EOF
            }

            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }

            let response_str = self.engine.handle_jsonrpc_str(trimmed).await;
            if let Err(e) = stdout.write_all(response_str.as_bytes()).await {
                error!("Failed to write to MCP stdout: {}", e);
                break;
            }
            if let Err(e) = stdout.write_all(b"\n").await {
                error!("Failed to write newline to MCP stdout: {}", e);
                break;
            }
            if let Err(e) = stdout.flush().await {
                error!("Failed to flush MCP stdout: {}", e);
                break;
            }
        }

        info!("MCP stdio transport terminated.");
        Ok(())
    }
}
