//! High-level agent protocol contracts for Rivun.
//!
//! The agent protocol is a JSON contract layer intended to travel inside `ZENV`
//! envelopes. It stays above the wire protocol and below any particular model,
//! planner, CLI, or node integration.

use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
    str::FromStr,
};
use thiserror::Error;
use uuid::Uuid;
use rivun_capability::CapabilityId;

pub const AGENT_PROTOCOL_SCHEMA_VERSION: u8 = 1;
pub const AGENT_CONTENT_TYPE: &str = "application/rivun-agent+json";

pub const AGENT_INTENT_SUBJECT: &str = "rivun.agent.intent";
pub const AGENT_SESSION_SUBJECT: &str = "rivun.agent.session";
pub const AGENT_DELEGATION_REQUEST_SUBJECT: &str = "rivun.agent.delegation.request";
pub const AGENT_DELEGATION_RESPONSE_SUBJECT: &str = "rivun.agent.delegation.response";
pub const AGENT_CAPABILITY_NEGOTIATION_REQUEST_SUBJECT: &str =
    "rivun.agent.capability_negotiation.request";
pub const AGENT_CAPABILITY_NEGOTIATION_RESPONSE_SUBJECT: &str =
    "rivun.agent.capability_negotiation.response";
pub const AGENT_STATUS_SUBJECT: &str = "rivun.agent.status";
pub const AGENT_RESULT_SUBJECT: &str = "rivun.agent.result";
pub const AGENT_ERROR_SUBJECT: &str = "rivun.agent.error";

const MAX_AGENT_ID_BYTES: usize = 128;
const MAX_SHORT_TEXT_BYTES: usize = 512;
const MAX_LONG_TEXT_BYTES: usize = 16 * 1024;
const MAX_ERROR_DEPTH: usize = 8;

pub type Result<T> = std::result::Result<T, RivunAgentError>;

#[derive(Debug, Error)]
pub enum RivunAgentError {
    #[error("{entity} schema version {version} is unsupported")]
    UnsupportedSchemaVersion { entity: &'static str, version: u8 },
    #[error("{entity} field `{field}` must not be empty")]
    EmptyField {
        entity: &'static str,
        field: &'static str,
    },
    #[error("{entity} field `{field}` exceeds maximum length of {max} bytes")]
    FieldTooLong {
        entity: &'static str,
        field: &'static str,
        max: usize,
    },
    #[error("{entity} field `{field}` has invalid identifier `{value}`")]
    InvalidIdentifier {
        entity: &'static str,
        field: &'static str,
        value: String,
    },
    #[error("{entity} field `{field}` must not be the nil UUID")]
    NilUuid {
        entity: &'static str,
        field: &'static str,
    },
    #[error("{entity} updated_at_micros must be greater than or equal to created_at_micros")]
    InvalidTimestampOrder { entity: &'static str },
    #[error("{entity} must request or offer at least one capability")]
    EmptyCapabilityNegotiation { entity: &'static str },
    #[error("accepted delegation response must include assigned_agent")]
    AcceptedDelegationMissingAssignee,
    #[error("rejected delegation response must include reason")]
    RejectedDelegationMissingReason,
    #[error("agent result status must be completed, failed, or cancelled")]
    ResultStatusNotTerminal,
    #[error("failed agent result must include error")]
    FailedResultMissingError,
    #[error("agent error cause nesting exceeds {max} levels")]
    ErrorCauseTooDeep { max: usize },
    #[error(
        "Provenance step verification failed for stage {stage:?}: expected {expected}, got {actual}"
    )]
    StepVerificationFailed {
        stage: ProvenanceStage,
        expected: String,
        actual: String,
    },
    #[error("Provenance missing required step: {0:?}")]
    MissingStep(ProvenanceStage),
    #[error("Provenance root signature verification failed")]
    InvalidProvenanceSignature,
    #[error("Provenance chain is empty or invalid: {0}")]
    InvalidProvenanceChain(String),
    #[error("Crypto error: {0}")]
    Crypto(#[from] rivun_crypto::RivunCryptoError),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

pub mod provenance;
pub use provenance::*;

pub mod swarm;
pub use swarm::*;

pub trait Validate {
    fn validate(&self) -> Result<()>;
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(transparent)]
pub struct AgentId(String);

impl AgentId {
    pub fn new(input: impl Into<String>) -> Result<Self> {
        let input = input.into();
        validate_identifier("agent_id", "value", &input)?;
        Ok(Self(input))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for AgentId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl FromStr for AgentId {
    type Err = RivunAgentError;

    fn from_str(input: &str) -> Result<Self> {
        Self::new(input)
    }
}

impl<'de> Deserialize<'de> for AgentId {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let input = String::deserialize(deserializer)?;
        Self::new(input).map_err(serde::de::Error::custom)
    }
}

#[derive(
    Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash,
)]
#[serde(rename_all = "snake_case")]
pub enum IntentKind {
    #[default]
    Act,
    Answer,
    Observe,
    Plan,
    Query,
    Transform,
    Delegate,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Priority {
    Low,
    #[default]
    Normal,
    High,
    Critical,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AgentStatus {
    #[default]
    Queued,
    Negotiating,
    Running,
    Waiting,
    Blocked,
    Completed,
    Failed,
    Cancelled,
}

impl AgentStatus {
    pub const fn is_terminal(self) -> bool {
        matches!(self, Self::Completed | Self::Failed | Self::Cancelled)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct IntentConstraint {
    pub name: String,
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    pub value: Value,
}

impl Validate for IntentConstraint {
    fn validate(&self) -> Result<()> {
        validate_non_empty("intent_constraint", "name", &self.name)?;
        validate_max_len(
            "intent_constraint",
            "name",
            &self.name,
            MAX_SHORT_TEXT_BYTES,
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ContextReference {
    pub kind: String,
    pub uri: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub digest: Option<String>,
    #[serde(default)]
    pub metadata: BTreeMap<String, Value>,
}

impl ContextReference {
    pub fn new(kind: impl Into<String>, uri: impl Into<String>) -> Self {
        Self {
            kind: kind.into(),
            uri: uri.into(),
            digest: None,
            metadata: BTreeMap::new(),
        }
    }
}

impl Validate for ContextReference {
    fn validate(&self) -> Result<()> {
        validate_non_empty("context_reference", "kind", &self.kind)?;
        validate_non_empty("context_reference", "uri", &self.uri)?;
        validate_max_len(
            "context_reference",
            "kind",
            &self.kind,
            MAX_SHORT_TEXT_BYTES,
        )?;
        validate_max_len("context_reference", "uri", &self.uri, MAX_LONG_TEXT_BYTES)?;
        if let Some(digest) = &self.digest {
            validate_non_empty("context_reference", "digest", digest)?;
            validate_max_len("context_reference", "digest", digest, MAX_SHORT_TEXT_BYTES)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AgentIntent {
    pub schema_version: u8,
    pub intent_id: Uuid,
    pub session_id: Uuid,
    pub source_agent: AgentId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_agent: Option<AgentId>,
    #[serde(default)]
    pub kind: IntentKind,
    pub objective: String,
    #[serde(default)]
    pub input: Value,
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub required_capabilities: BTreeSet<CapabilityId>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub constraints: Vec<IntentConstraint>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub context: Vec<ContextReference>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deadline_unix_micros: Option<u64>,
    #[serde(default)]
    pub priority: Priority,
    #[serde(default)]
    pub metadata: BTreeMap<String, Value>,
}

impl AgentIntent {
    pub fn new(
        session_id: Uuid,
        source_agent: AgentId,
        kind: IntentKind,
        objective: impl Into<String>,
    ) -> Self {
        Self {
            schema_version: AGENT_PROTOCOL_SCHEMA_VERSION,
            intent_id: Uuid::new_v4(),
            session_id,
            source_agent,
            target_agent: None,
            kind,
            objective: objective.into(),
            input: Value::Null,
            required_capabilities: BTreeSet::new(),
            constraints: Vec::new(),
            context: Vec::new(),
            deadline_unix_micros: None,
            priority: Priority::Normal,
            metadata: BTreeMap::new(),
        }
    }
}

impl Validate for AgentIntent {
    fn validate(&self) -> Result<()> {
        validate_schema("agent_intent", self.schema_version)?;
        validate_uuid("agent_intent", "intent_id", self.intent_id)?;
        validate_uuid("agent_intent", "session_id", self.session_id)?;
        validate_non_empty("agent_intent", "objective", &self.objective)?;
        validate_max_len(
            "agent_intent",
            "objective",
            &self.objective,
            MAX_LONG_TEXT_BYTES,
        )?;
        for constraint in &self.constraints {
            constraint.validate()?;
        }
        for reference in &self.context {
            reference.validate()?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AgentSession {
    pub schema_version: u8,
    pub session_id: Uuid,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub root_intent_id: Option<Uuid>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_session_id: Option<Uuid>,
    pub owner_agent: AgentId,
    #[serde(default)]
    pub status: AgentStatus,
    pub created_at_micros: u64,
    pub updated_at_micros: u64,
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub accepted_capabilities: BTreeSet<CapabilityId>,
    #[serde(default)]
    pub metadata: BTreeMap<String, Value>,
}

impl AgentSession {
    pub fn new(session_id: Uuid, owner_agent: AgentId, now_micros: u64) -> Self {
        Self {
            schema_version: AGENT_PROTOCOL_SCHEMA_VERSION,
            session_id,
            root_intent_id: None,
            parent_session_id: None,
            owner_agent,
            status: AgentStatus::Queued,
            created_at_micros: now_micros,
            updated_at_micros: now_micros,
            accepted_capabilities: BTreeSet::new(),
            metadata: BTreeMap::new(),
        }
    }
}

impl Validate for AgentSession {
    fn validate(&self) -> Result<()> {
        validate_schema("agent_session", self.schema_version)?;
        validate_uuid("agent_session", "session_id", self.session_id)?;
        if let Some(root_intent_id) = self.root_intent_id {
            validate_uuid("agent_session", "root_intent_id", root_intent_id)?;
        }
        if let Some(parent_session_id) = self.parent_session_id {
            validate_uuid("agent_session", "parent_session_id", parent_session_id)?;
        }
        validate_timestamp_order(
            "agent_session",
            self.created_at_micros,
            self.updated_at_micros,
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DelegationRequest {
    pub schema_version: u8,
    pub delegation_id: Uuid,
    pub session_id: Uuid,
    pub parent_intent_id: Uuid,
    pub from_agent: AgentId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to_agent: Option<AgentId>,
    pub objective: String,
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub required_capabilities: BTreeSet<CapabilityId>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub constraints: Vec<IntentConstraint>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub context: Vec<ContextReference>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deadline_unix_micros: Option<u64>,
    #[serde(default)]
    pub metadata: BTreeMap<String, Value>,
}

impl Validate for DelegationRequest {
    fn validate(&self) -> Result<()> {
        validate_schema("delegation_request", self.schema_version)?;
        validate_uuid("delegation_request", "delegation_id", self.delegation_id)?;
        validate_uuid("delegation_request", "session_id", self.session_id)?;
        validate_uuid(
            "delegation_request",
            "parent_intent_id",
            self.parent_intent_id,
        )?;
        validate_non_empty("delegation_request", "objective", &self.objective)?;
        validate_max_len(
            "delegation_request",
            "objective",
            &self.objective,
            MAX_LONG_TEXT_BYTES,
        )?;
        for constraint in &self.constraints {
            constraint.validate()?;
        }
        for reference in &self.context {
            reference.validate()?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DelegationDecision {
    #[default]
    Accepted,
    Rejected,
    CounterOffer,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DelegationResponse {
    pub schema_version: u8,
    pub delegation_id: Uuid,
    pub session_id: Uuid,
    pub respondent_agent: AgentId,
    #[serde(default)]
    pub decision: DelegationDecision,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assigned_agent: Option<AgentId>,
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub accepted_capabilities: BTreeSet<CapabilityId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub estimated_completion_unix_micros: Option<u64>,
    #[serde(default)]
    pub metadata: BTreeMap<String, Value>,
}

impl Validate for DelegationResponse {
    fn validate(&self) -> Result<()> {
        validate_schema("delegation_response", self.schema_version)?;
        validate_uuid("delegation_response", "delegation_id", self.delegation_id)?;
        validate_uuid("delegation_response", "session_id", self.session_id)?;
        match self.decision {
            DelegationDecision::Accepted if self.assigned_agent.is_none() => {
                Err(RivunAgentError::AcceptedDelegationMissingAssignee)
            }
            DelegationDecision::Rejected if self.reason_is_empty() => {
                Err(RivunAgentError::RejectedDelegationMissingReason)
            }
            _ => {
                if let Some(reason) = &self.reason {
                    validate_max_len(
                        "delegation_response",
                        "reason",
                        reason,
                        MAX_SHORT_TEXT_BYTES,
                    )?;
                }
                Ok(())
            }
        }
    }
}

impl DelegationResponse {
    fn reason_is_empty(&self) -> bool {
        self.reason
            .as_deref()
            .is_none_or(|reason| reason.trim().is_empty())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CapabilityNegotiationRequest {
    pub schema_version: u8,
    pub negotiation_id: Uuid,
    pub session_id: Uuid,
    pub requester_agent: AgentId,
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub required_capabilities: BTreeSet<CapabilityId>,
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub optional_capabilities: BTreeSet<CapabilityId>,
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub desired_intents: BTreeSet<IntentKind>,
    #[serde(default)]
    pub metadata: BTreeMap<String, Value>,
}

impl Validate for CapabilityNegotiationRequest {
    fn validate(&self) -> Result<()> {
        validate_schema("capability_negotiation_request", self.schema_version)?;
        validate_uuid(
            "capability_negotiation_request",
            "negotiation_id",
            self.negotiation_id,
        )?;
        validate_uuid(
            "capability_negotiation_request",
            "session_id",
            self.session_id,
        )?;
        if self.required_capabilities.is_empty()
            && self.optional_capabilities.is_empty()
            && self.desired_intents.is_empty()
        {
            return Err(RivunAgentError::EmptyCapabilityNegotiation {
                entity: "capability_negotiation_request",
            });
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityNegotiationDecision {
    Accepted,
    #[default]
    Partial,
    Rejected,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CapabilityNegotiationResponse {
    pub schema_version: u8,
    pub negotiation_id: Uuid,
    pub session_id: Uuid,
    pub responder_agent: AgentId,
    #[serde(default)]
    pub decision: CapabilityNegotiationDecision,
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub accepted_capabilities: BTreeSet<CapabilityId>,
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub unsupported_capabilities: BTreeSet<CapabilityId>,
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub supported_intents: BTreeSet<IntentKind>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires_at_unix_micros: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(default)]
    pub metadata: BTreeMap<String, Value>,
}

impl Validate for CapabilityNegotiationResponse {
    fn validate(&self) -> Result<()> {
        validate_schema("capability_negotiation_response", self.schema_version)?;
        validate_uuid(
            "capability_negotiation_response",
            "negotiation_id",
            self.negotiation_id,
        )?;
        validate_uuid(
            "capability_negotiation_response",
            "session_id",
            self.session_id,
        )?;
        if self.accepted_capabilities.is_empty()
            && self.unsupported_capabilities.is_empty()
            && self.supported_intents.is_empty()
        {
            return Err(RivunAgentError::EmptyCapabilityNegotiation {
                entity: "capability_negotiation_response",
            });
        }
        if let Some(reason) = &self.reason {
            validate_max_len(
                "capability_negotiation_response",
                "reason",
                reason,
                MAX_SHORT_TEXT_BYTES,
            )?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AgentStatusUpdate {
    pub schema_version: u8,
    pub session_id: Uuid,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub intent_id: Option<Uuid>,
    pub agent_id: AgentId,
    #[serde(default)]
    pub status: AgentStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub progress_per_mille: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    pub updated_at_micros: u64,
    #[serde(default)]
    pub metadata: BTreeMap<String, Value>,
}

impl Validate for AgentStatusUpdate {
    fn validate(&self) -> Result<()> {
        validate_schema("agent_status_update", self.schema_version)?;
        validate_uuid("agent_status_update", "session_id", self.session_id)?;
        if let Some(intent_id) = self.intent_id {
            validate_uuid("agent_status_update", "intent_id", intent_id)?;
        }
        if let Some(progress) = self.progress_per_mille
            && progress > 1000
        {
            return Err(RivunAgentError::InvalidIdentifier {
                entity: "agent_status_update",
                field: "progress_per_mille",
                value: progress.to_string(),
            });
        }
        if let Some(message) = &self.message {
            validate_max_len(
                "agent_status_update",
                "message",
                message,
                MAX_SHORT_TEXT_BYTES,
            )?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AgentArtifact {
    pub name: String,
    pub uri: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub media_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub digest: Option<String>,
    #[serde(default)]
    pub metadata: BTreeMap<String, Value>,
}

impl Validate for AgentArtifact {
    fn validate(&self) -> Result<()> {
        validate_non_empty("agent_artifact", "name", &self.name)?;
        validate_non_empty("agent_artifact", "uri", &self.uri)?;
        validate_max_len("agent_artifact", "name", &self.name, MAX_SHORT_TEXT_BYTES)?;
        validate_max_len("agent_artifact", "uri", &self.uri, MAX_LONG_TEXT_BYTES)?;
        if let Some(media_type) = &self.media_type {
            validate_non_empty("agent_artifact", "media_type", media_type)?;
            validate_max_len(
                "agent_artifact",
                "media_type",
                media_type,
                MAX_SHORT_TEXT_BYTES,
            )?;
        }
        if let Some(digest) = &self.digest {
            validate_non_empty("agent_artifact", "digest", digest)?;
            validate_max_len("agent_artifact", "digest", digest, MAX_SHORT_TEXT_BYTES)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AgentErrorCategory {
    #[default]
    InvalidRequest,
    CapabilityUnavailable,
    PolicyDenied,
    Timeout,
    Runtime,
    Internal,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AgentErrorInfo {
    pub code: String,
    pub message: String,
    #[serde(default)]
    pub category: AgentErrorCategory,
    #[serde(default)]
    pub retryable: bool,
    #[serde(default)]
    pub details: BTreeMap<String, Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cause: Option<Box<AgentErrorInfo>>,
}

impl AgentErrorInfo {
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            category: AgentErrorCategory::InvalidRequest,
            retryable: false,
            details: BTreeMap::new(),
            cause: None,
        }
    }

    fn validate_with_depth(&self, depth: usize) -> Result<()> {
        if depth > MAX_ERROR_DEPTH {
            return Err(RivunAgentError::ErrorCauseTooDeep {
                max: MAX_ERROR_DEPTH,
            });
        }
        validate_identifier("agent_error", "code", &self.code)?;
        validate_non_empty("agent_error", "message", &self.message)?;
        validate_max_len("agent_error", "message", &self.message, MAX_LONG_TEXT_BYTES)?;
        if let Some(cause) = &self.cause {
            cause.validate_with_depth(depth + 1)?;
        }
        Ok(())
    }
}

impl Validate for AgentErrorInfo {
    fn validate(&self) -> Result<()> {
        self.validate_with_depth(0)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AgentResult {
    pub schema_version: u8,
    pub result_id: Uuid,
    pub session_id: Uuid,
    pub intent_id: Uuid,
    pub agent_id: AgentId,
    pub status: AgentStatus,
    #[serde(default)]
    pub outputs: BTreeMap<String, Value>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub artifacts: Vec<AgentArtifact>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<AgentErrorInfo>,
    pub completed_at_micros: u64,
    #[serde(default)]
    pub metadata: BTreeMap<String, Value>,
}

impl Validate for AgentResult {
    fn validate(&self) -> Result<()> {
        validate_schema("agent_result", self.schema_version)?;
        validate_uuid("agent_result", "result_id", self.result_id)?;
        validate_uuid("agent_result", "session_id", self.session_id)?;
        validate_uuid("agent_result", "intent_id", self.intent_id)?;
        if !self.status.is_terminal() {
            return Err(RivunAgentError::ResultStatusNotTerminal);
        }
        if self.status == AgentStatus::Failed && self.error.is_none() {
            return Err(RivunAgentError::FailedResultMissingError);
        }
        if let Some(error) = &self.error {
            error.validate()?;
        }
        for artifact in &self.artifacts {
            artifact.validate()?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AgentErrorReport {
    pub schema_version: u8,
    pub error_id: Uuid,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<Uuid>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub intent_id: Option<Uuid>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_id: Option<AgentId>,
    pub error: AgentErrorInfo,
    pub observed_at_micros: u64,
    #[serde(default)]
    pub metadata: BTreeMap<String, Value>,
}

impl Validate for AgentErrorReport {
    fn validate(&self) -> Result<()> {
        validate_schema("agent_error_report", self.schema_version)?;
        validate_uuid("agent_error_report", "error_id", self.error_id)?;
        if let Some(session_id) = self.session_id {
            validate_uuid("agent_error_report", "session_id", session_id)?;
        }
        if let Some(intent_id) = self.intent_id {
            validate_uuid("agent_error_report", "intent_id", intent_id)?;
        }
        self.error.validate()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", content = "payload", rename_all = "snake_case")]
pub enum AgentMessage {
    Intent(AgentIntent),
    Session(AgentSession),
    DelegationRequest(DelegationRequest),
    DelegationResponse(DelegationResponse),
    CapabilityNegotiationRequest(CapabilityNegotiationRequest),
    CapabilityNegotiationResponse(CapabilityNegotiationResponse),
    Status(AgentStatusUpdate),
    Result(AgentResult),
    Error(AgentErrorReport),
}

impl AgentMessage {
    pub fn subject(&self) -> &'static str {
        match self {
            Self::Intent(_) => AGENT_INTENT_SUBJECT,
            Self::Session(_) => AGENT_SESSION_SUBJECT,
            Self::DelegationRequest(_) => AGENT_DELEGATION_REQUEST_SUBJECT,
            Self::DelegationResponse(_) => AGENT_DELEGATION_RESPONSE_SUBJECT,
            Self::CapabilityNegotiationRequest(_) => AGENT_CAPABILITY_NEGOTIATION_REQUEST_SUBJECT,
            Self::CapabilityNegotiationResponse(_) => AGENT_CAPABILITY_NEGOTIATION_RESPONSE_SUBJECT,
            Self::Status(_) => AGENT_STATUS_SUBJECT,
            Self::Result(_) => AGENT_RESULT_SUBJECT,
            Self::Error(_) => AGENT_ERROR_SUBJECT,
        }
    }

    pub fn to_json_vec(&self) -> Result<Vec<u8>> {
        self.validate()?;
        Ok(serde_json::to_vec(self)?)
    }

    pub fn to_json_string(&self) -> Result<String> {
        self.validate()?;
        Ok(serde_json::to_string(self)?)
    }

    pub fn from_json_slice(input: &[u8]) -> Result<Self> {
        let message: Self = serde_json::from_slice(input)?;
        message.validate()?;
        Ok(message)
    }

    pub fn from_json_str(input: &str) -> Result<Self> {
        let message: Self = serde_json::from_str(input)?;
        message.validate()?;
        Ok(message)
    }
}

impl Validate for AgentMessage {
    fn validate(&self) -> Result<()> {
        match self {
            Self::Intent(value) => value.validate(),
            Self::Session(value) => value.validate(),
            Self::DelegationRequest(value) => value.validate(),
            Self::DelegationResponse(value) => value.validate(),
            Self::CapabilityNegotiationRequest(value) => value.validate(),
            Self::CapabilityNegotiationResponse(value) => value.validate(),
            Self::Status(value) => value.validate(),
            Self::Result(value) => value.validate(),
            Self::Error(value) => value.validate(),
        }
    }
}

pub fn agent_message_subjects() -> &'static [&'static str] {
    &[
        AGENT_INTENT_SUBJECT,
        AGENT_SESSION_SUBJECT,
        AGENT_DELEGATION_REQUEST_SUBJECT,
        AGENT_DELEGATION_RESPONSE_SUBJECT,
        AGENT_CAPABILITY_NEGOTIATION_REQUEST_SUBJECT,
        AGENT_CAPABILITY_NEGOTIATION_RESPONSE_SUBJECT,
        AGENT_STATUS_SUBJECT,
        AGENT_RESULT_SUBJECT,
        AGENT_ERROR_SUBJECT,
    ]
}

pub fn agent_message_json_schema() -> Value {
    serde_json::json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$id": "https://rivun.local/schemas/agent-message-v1.json",
        "title": "Rivun AgentMessage v1",
        "type": "object",
        "additionalProperties": false,
        "required": ["type", "payload"],
        "properties": {
            "type": {
                "type": "string",
                "enum": [
                    "intent",
                    "session",
                    "delegation_request",
                    "delegation_response",
                    "capability_negotiation_request",
                    "capability_negotiation_response",
                    "status",
                    "result",
                    "error"
                ]
            },
            "payload": {
                "type": "object",
                "required": ["schema_version"],
                "properties": {
                    "schema_version": { "const": AGENT_PROTOCOL_SCHEMA_VERSION }
                }
            }
        },
        "x-rivun": {
            "content_type": AGENT_CONTENT_TYPE,
            "subjects": agent_message_subjects(),
            "schema_version": AGENT_PROTOCOL_SCHEMA_VERSION
        }
    })
}

fn validate_schema(entity: &'static str, version: u8) -> Result<()> {
    if version != AGENT_PROTOCOL_SCHEMA_VERSION {
        return Err(RivunAgentError::UnsupportedSchemaVersion { entity, version });
    }
    Ok(())
}

fn validate_uuid(entity: &'static str, field: &'static str, value: Uuid) -> Result<()> {
    if value.is_nil() {
        return Err(RivunAgentError::NilUuid { entity, field });
    }
    Ok(())
}

fn validate_timestamp_order(entity: &'static str, created: u64, updated: u64) -> Result<()> {
    if updated < created {
        return Err(RivunAgentError::InvalidTimestampOrder { entity });
    }
    Ok(())
}

fn validate_identifier(entity: &'static str, field: &'static str, value: &str) -> Result<()> {
    validate_non_empty(entity, field, value)?;
    validate_max_len(entity, field, value, MAX_AGENT_ID_BYTES)?;
    if !value
        .bytes()
        .all(|byte| matches!(byte, b'a'..=b'z' | b'0'..=b'9' | b'.' | b':' | b'_' | b'-'))
    {
        return Err(RivunAgentError::InvalidIdentifier {
            entity,
            field,
            value: value.to_string(),
        });
    }
    Ok(())
}

fn validate_non_empty(entity: &'static str, field: &'static str, value: &str) -> Result<()> {
    if value.trim().is_empty() {
        return Err(RivunAgentError::EmptyField { entity, field });
    }
    Ok(())
}

fn validate_max_len(
    entity: &'static str,
    field: &'static str,
    value: &str,
    max: usize,
) -> Result<()> {
    if value.len() > max {
        return Err(RivunAgentError::FieldTooLong { entity, field, max });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn agent(id: &str) -> AgentId {
        AgentId::new(id).unwrap()
    }

    fn capability(id: &str) -> CapabilityId {
        CapabilityId::new(id).unwrap()
    }

    fn session_id() -> Uuid {
        Uuid::parse_str("11111111-1111-4111-8111-111111111111").unwrap()
    }

    fn intent_id() -> Uuid {
        Uuid::parse_str("22222222-2222-4222-8222-222222222222").unwrap()
    }

    #[test]
    fn serializes_intent_json_stably() {
        let mut intent = AgentIntent::new(
            session_id(),
            agent("planner.main"),
            IntentKind::Act,
            "open valve",
        );
        intent.intent_id = intent_id();
        intent.target_agent = Some(agent("executor.safety"));
        intent.input = json!({"valve":"v-7"});
        intent
            .required_capabilities
            .insert(capability("driver.execute:valve.open"));
        intent.priority = Priority::High;

        let json = AgentMessage::Intent(intent).to_json_string().unwrap();

        assert_eq!(
            json,
            r#"{"type":"intent","payload":{"schema_version":1,"intent_id":"22222222-2222-4222-8222-222222222222","session_id":"11111111-1111-4111-8111-111111111111","source_agent":"planner.main","target_agent":"executor.safety","kind":"act","objective":"open valve","input":{"valve":"v-7"},"required_capabilities":["driver.execute:valve.open"],"priority":"high","metadata":{}}}"#
        );
    }

    #[test]
    fn deserializing_agent_id_rejects_unstable_identifier() {
        let error = serde_json::from_str::<AgentIntent>(
            r#"{
                "schema_version":1,
                "intent_id":"22222222-2222-4222-8222-222222222222",
                "session_id":"11111111-1111-4111-8111-111111111111",
                "source_agent":"Planner Main",
                "kind":"act",
                "objective":"open valve",
                "input":null,
                "priority":"normal",
                "metadata":{}
            }"#,
        )
        .unwrap_err();

        assert!(error.to_string().contains("invalid identifier"));
    }

    #[test]
    fn validates_session_timestamps() {
        let session = AgentSession {
            schema_version: 1,
            session_id: session_id(),
            root_intent_id: Some(intent_id()),
            parent_session_id: None,
            owner_agent: agent("planner.main"),
            status: AgentStatus::Running,
            created_at_micros: 20,
            updated_at_micros: 10,
            accepted_capabilities: BTreeSet::new(),
            metadata: BTreeMap::new(),
        };

        assert!(matches!(
            session.validate(),
            Err(RivunAgentError::InvalidTimestampOrder {
                entity: "agent_session"
            })
        ));
    }

    #[test]
    fn accepted_delegation_requires_assignee() {
        let response = DelegationResponse {
            schema_version: 1,
            delegation_id: Uuid::parse_str("33333333-3333-4333-8333-333333333333").unwrap(),
            session_id: session_id(),
            respondent_agent: agent("executor.safety"),
            decision: DelegationDecision::Accepted,
            assigned_agent: None,
            accepted_capabilities: BTreeSet::new(),
            reason: None,
            estimated_completion_unix_micros: None,
            metadata: BTreeMap::new(),
        };

        assert!(matches!(
            response.validate(),
            Err(RivunAgentError::AcceptedDelegationMissingAssignee)
        ));
    }

    #[test]
    fn capability_negotiation_must_not_be_empty() {
        let request = CapabilityNegotiationRequest {
            schema_version: 1,
            negotiation_id: Uuid::parse_str("44444444-4444-4444-8444-444444444444").unwrap(),
            session_id: session_id(),
            requester_agent: agent("planner.main"),
            required_capabilities: BTreeSet::new(),
            optional_capabilities: BTreeSet::new(),
            desired_intents: BTreeSet::new(),
            metadata: BTreeMap::new(),
        };

        assert!(matches!(
            request.validate(),
            Err(RivunAgentError::EmptyCapabilityNegotiation { .. })
        ));
    }

    #[test]
    fn result_requires_terminal_status_and_error_for_failure() {
        let mut result = AgentResult {
            schema_version: 1,
            result_id: Uuid::parse_str("55555555-5555-4555-8555-555555555555").unwrap(),
            session_id: session_id(),
            intent_id: intent_id(),
            agent_id: agent("executor.safety"),
            status: AgentStatus::Running,
            outputs: BTreeMap::new(),
            artifacts: Vec::new(),
            error: None,
            completed_at_micros: 42,
            metadata: BTreeMap::new(),
        };

        assert!(matches!(
            result.validate(),
            Err(RivunAgentError::ResultStatusNotTerminal)
        ));

        result.status = AgentStatus::Failed;
        assert!(matches!(
            result.validate(),
            Err(RivunAgentError::FailedResultMissingError)
        ));

        result.error = Some(AgentErrorInfo::new("runtime.timeout", "driver timed out"));
        result.validate().unwrap();
    }

    #[test]
    fn agent_message_roundtrips_with_subject() {
        let mut accepted = BTreeSet::new();
        accepted.insert(capability("driver.execute:valve.open"));
        let response = CapabilityNegotiationResponse {
            schema_version: 1,
            negotiation_id: Uuid::parse_str("44444444-4444-4444-8444-444444444444").unwrap(),
            session_id: session_id(),
            responder_agent: agent("executor.safety"),
            decision: CapabilityNegotiationDecision::Accepted,
            accepted_capabilities: accepted,
            unsupported_capabilities: BTreeSet::new(),
            supported_intents: BTreeSet::from([IntentKind::Act]),
            expires_at_unix_micros: Some(100),
            reason: None,
            metadata: BTreeMap::new(),
        };
        let message = AgentMessage::CapabilityNegotiationResponse(response);
        let encoded = message.to_json_vec().unwrap();
        let decoded = AgentMessage::from_json_slice(&encoded).unwrap();

        assert_eq!(
            decoded.subject(),
            AGENT_CAPABILITY_NEGOTIATION_RESPONSE_SUBJECT
        );
        assert_eq!(decoded, message);
    }

    #[test]
    fn exports_agent_message_json_schema_metadata() {
        let schema = agent_message_json_schema();

        assert_eq!(schema["x-rivun"]["content_type"], AGENT_CONTENT_TYPE);
        assert_eq!(
            schema["x-rivun"]["schema_version"],
            AGENT_PROTOCOL_SCHEMA_VERSION
        );
        assert!(
            schema["x-rivun"]["subjects"]
                .as_array()
                .unwrap()
                .iter()
                .any(|subject| subject == AGENT_INTENT_SUBJECT)
        );
    }

    #[test]
    fn validates_nested_error_depth() {
        let mut error = AgentErrorInfo::new("internal.root", "root");
        for index in 0..=MAX_ERROR_DEPTH {
            let mut parent = AgentErrorInfo::new(format!("internal.{index}"), "wrapped");
            parent.cause = Some(Box::new(error));
            error = parent;
        }

        assert!(matches!(
            error.validate(),
            Err(RivunAgentError::ErrorCauseTooDeep { .. })
        ));
    }
}
