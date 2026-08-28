//! Rivun AI Agent Gateway & Model Context Protocol (MCP) Server.
//!
//! Provides:
//! - JSON-RPC 2.0 MCP server over stdio and HTTP
//! - Multi-transport Agent Gateway (HTTP REST, SSE Event Streaming, WebSocket Bridge)
//! - Cryptographic Provenance Chain Engine ($H_{\text{intent}} \to H_{\text{negotiation}} \to H_{\text{policy}} \to H_{\text{driver}} \to H_{\text{poa}} \to H_{\text{receipt}}$)

pub mod config;
pub mod error;
pub mod mcp;
pub mod provenance;
pub mod server;
pub mod transports;

pub use config::GatewayConfig;
pub use error::{GatewayErrorResponse, Result, RivunGatewayError};
pub use mcp::McpEngine;
pub use mcp::prompts as mcp_prompts;
pub use mcp::protocol as mcp_protocol;
pub use mcp::resources as mcp_resources;
pub use mcp::stdio::McpStdioTransport;
pub use mcp::tools as mcp_tools;
pub use provenance::{
    ProvenanceChainBuilder, ProvenanceChainDigest, ProvenanceStage, ProvenanceStep,
    ProvenanceVerificationReport, compute_root_hash,
};
pub use server::AgentGatewayServer;
pub use transports::{
    HttpAgentGateway, SseBroker, SseEvent, WebSocketHandler, WsFrame, compute_ws_accept,
};
