use serde::{Deserialize, Serialize};
use thiserror::Error;
use zap_agent::ZapAgentError;
use zap_core::ZapError;
use zap_crypto::ZapCryptoError;
use zap_ledger::ZapLedgerError;
use zap_memory::ZapMemoryError;

use crate::provenance::ProvenanceStage;

pub type Result<T> = std::result::Result<T, ZapGatewayError>;

#[derive(Debug, Error)]
pub enum ZapGatewayError {
    #[error("Core error: {0}")]
    Core(#[from] ZapError),

    #[error("Ledger error: {0}")]
    Ledger(#[from] ZapLedgerError),

    #[error("Memory error: {0}")]
    Memory(#[from] ZapMemoryError),

    #[error("Agent protocol error: {0}")]
    Agent(#[from] ZapAgentError),

    #[error("Cryptographic error: {0}")]
    Crypto(#[from] ZapCryptoError),

    #[error("Policy denied: {0}")]
    PolicyDenied(String),

    #[error("Unauthorized: {0}")]
    Unauthorized(String),

    #[error("Too many requests (rate limit exceeded)")]
    RateLimited,

    #[error("Frame size {size} bytes exceeds maximum allowed limit of {max} bytes")]
    FrameSizeExceeded { size: usize, max: usize },

    #[error("MCP JSON-RPC error [{code}]: {message}")]
    JsonRpc {
        code: i64,
        message: String,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    #[error("Provenance step verification failed for stage {stage:?}: expected {expected}, got {actual}")]
    StepVerificationFailed {
        stage: ProvenanceStage,
        expected: String,
        actual: String,
    },

    #[error("Provenance missing required step: {0:?}")]
    MissingStep(ProvenanceStage),

    #[error("Provenance root signature verification failed")]
    InvalidProvenanceSignature,

    #[error("Provenance chain is empty or invalid")]
    InvalidProvenanceChain(String),

    #[error("Resource not found: {0}")]
    NotFound(String),

    #[error("Method not allowed: {0}")]
    MethodNotAllowed(String),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON serialization error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("HTTP error: {0}")]
    Http(String),

    #[error("Internal gateway error: {0}")]
    Internal(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GatewayErrorResponse {
    pub error: String,
    pub code: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
}

impl ZapGatewayError {
    pub fn jsonrpc_parse_error(msg: impl Into<String>) -> Self {
        Self::JsonRpc {
            code: -32700,
            message: msg.into(),
            source: None,
        }
    }

    pub fn jsonrpc_invalid_request(msg: impl Into<String>) -> Self {
        Self::JsonRpc {
            code: -32600,
            message: msg.into(),
            source: None,
        }
    }

    pub fn jsonrpc_method_not_found(msg: impl Into<String>) -> Self {
        Self::JsonRpc {
            code: -32601,
            message: msg.into(),
            source: None,
        }
    }

    pub fn jsonrpc_invalid_params(msg: impl Into<String>) -> Self {
        Self::JsonRpc {
            code: -32602,
            message: msg.into(),
            source: None,
        }
    }

    pub fn jsonrpc_internal_error(msg: impl Into<String>) -> Self {
        Self::JsonRpc {
            code: -32603,
            message: msg.into(),
            source: None,
        }
    }
}
