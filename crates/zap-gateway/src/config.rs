use serde::{Deserialize, Serialize};
use std::{net::SocketAddr, path::PathBuf};

pub const DEFAULT_GATEWAY_HTTP_ADDR: &str = "127.0.0.1:8080";
pub const DEFAULT_MAX_FRAME_SIZE: usize = 4 * 1024 * 1024; // 4MB

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatewayConfig {
    /// HTTP REST, SSE and WebSocket bind address.
    #[serde(default = "default_http_bind")]
    pub http_bind: SocketAddr,

    /// Optional shared secret / bearer token for gateway authentication.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auth_token: Option<String>,

    /// Maximum allowed WebSocket and HTTP frame/payload size in bytes (defaults to 4MB).
    #[serde(default = "default_max_frame_size")]
    pub max_frame_size: usize,

    /// Allowed CORS origins (defaults to `["*"]`).
    #[serde(default = "default_cors_origins")]
    pub cors_allowed_origins: Vec<String>,

    /// Maximum requests per second per client IP (None for unlimited).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rate_limit_per_second: Option<u32>,

    /// Enable Model Context Protocol (MCP) server over standard I/O streams.
    #[serde(default)]
    pub enable_mcp_stdio: bool,

    /// Storage directory for binary receipt journal records.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub journal_dir: Option<PathBuf>,

    /// Storage directory for memory journal records.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub memory_dir: Option<PathBuf>,
}

fn default_http_bind() -> SocketAddr {
    DEFAULT_GATEWAY_HTTP_ADDR
        .parse()
        .expect("static default http address is valid")
}

fn default_max_frame_size() -> usize {
    DEFAULT_MAX_FRAME_SIZE
}

fn default_cors_origins() -> Vec<String> {
    vec!["*".to_string()]
}

impl Default for GatewayConfig {
    fn default() -> Self {
        Self {
            http_bind: default_http_bind(),
            auth_token: None,
            max_frame_size: DEFAULT_MAX_FRAME_SIZE,
            cors_allowed_origins: default_cors_origins(),
            rate_limit_per_second: None,
            enable_mcp_stdio: false,
            journal_dir: None,
            memory_dir: None,
        }
    }
}

impl GatewayConfig {
    pub fn new(http_bind: SocketAddr) -> Self {
        Self {
            http_bind,
            ..Default::default()
        }
    }

    pub fn with_auth_token(mut self, token: impl Into<String>) -> Self {
        self.auth_token = Some(token.into());
        self
    }

    pub fn with_max_frame_size(mut self, max_bytes: usize) -> Self {
        self.max_frame_size = max_bytes;
        self
    }
}
