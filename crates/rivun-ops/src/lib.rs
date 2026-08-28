//! Operational contracts for running Rivun in production.
//!
//! This crate intentionally avoids daemon-specific dependencies. It gives
//! release tooling, config generators, and operator automation a shared set of
//! validated data structures for observability, governance, audit, and release
//! manifests.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;
use uuid::Uuid;

pub const OBSERVABILITY_SCHEMA_VERSION: u8 = 1;
pub const GOVERNANCE_SCHEMA_VERSION: u8 = 1;
pub const RELEASE_SCHEMA_VERSION: u8 = 1;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum RivunOpsError {
    #[error("{field} must not be empty")]
    Empty { field: &'static str },
    #[error("{field} must be greater than zero")]
    Zero { field: &'static str },
    #[error("{field} must be between {min} and {max}")]
    OutOfRange {
        field: &'static str,
        min: u64,
        max: u64,
    },
    #[error("duplicate {kind} `{id}`")]
    Duplicate { kind: &'static str, id: String },
    #[error("unknown {kind} `{id}`")]
    Unknown { kind: &'static str, id: String },
    #[error("group `{group}` requires {required} approvals but has {available} eligible members")]
    ApprovalThresholdTooHigh {
        group: String,
        required: u16,
        available: usize,
    },
    #[error("policy `{policy}` requires unknown group `{group}`")]
    PolicyReferencesUnknownGroup { policy: String, group: String },
    #[error("audit entry {sequence} hash mismatch")]
    AuditHashMismatch { sequence: u64 },
    #[error("audit entry {sequence} previous hash mismatch")]
    AuditPreviousHashMismatch { sequence: u64 },
    #[error("audit sequence expected {expected}, got {actual}")]
    AuditSequenceGap { expected: u64, actual: u64 },
    #[error("release version `{0}` is not a supported semantic version")]
    InvalidVersion(String),
    #[error("git sha `{0}` must be 40 lowercase hexadecimal characters")]
    InvalidGitSha(String),
    #[error("artifact `{name}` {field} checksum must be 64 lowercase hexadecimal characters")]
    InvalidChecksum { name: String, field: &'static str },
    #[error("signature references unknown artifact `{0}`")]
    UnknownSignatureArtifact(String),
    #[error("failed to decode TOML: {0}")]
    TomlDecode(String),
}

pub type Result<T> = std::result::Result<T, RivunOpsError>;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ObservabilityConfig {
    pub schema_version: u8,
    pub service: ServiceIdentity,
    pub metrics: MetricsConfig,
    pub tracing: TracingConfig,
    pub logs: LogConfig,
    pub health: HealthPolicy,
}

impl ObservabilityConfig {
    pub fn validate(&self) -> Result<()> {
        if self.schema_version != OBSERVABILITY_SCHEMA_VERSION {
            return Err(RivunOpsError::OutOfRange {
                field: "observability.schema_version",
                min: OBSERVABILITY_SCHEMA_VERSION as u64,
                max: OBSERVABILITY_SCHEMA_VERSION as u64,
            });
        }
        self.service.validate()?;
        self.metrics.validate()?;
        self.tracing.validate()?;
        self.logs.validate()?;
        self.health.validate()
    }

    pub fn from_toml_str(input: &str) -> Result<Self> {
        let config: Self =
            toml::from_str(input).map_err(|error| RivunOpsError::TomlDecode(error.to_string()))?;
        config.validate()?;
        Ok(config)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ServiceIdentity {
    pub service_name: String,
    pub environment: String,
    pub cluster: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub region: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub labels: BTreeMap<String, String>,
}

impl ServiceIdentity {
    pub fn validate(&self) -> Result<()> {
        require_non_empty("service.service_name", &self.service_name)?;
        require_non_empty("service.environment", &self.environment)?;
        require_non_empty("service.cluster", &self.cluster)?;
        if let Some(region) = &self.region {
            require_non_empty("service.region", region)?;
        }
        validate_labels("service.labels", &self.labels)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MetricsConfig {
    pub enabled: bool,
    pub bind: String,
    pub path: String,
    pub scrape_interval_seconds: u64,
    #[serde(default)]
    pub include_runtime_metrics: bool,
    #[serde(default)]
    pub include_driver_metrics: bool,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub labels: BTreeMap<String, String>,
}

impl MetricsConfig {
    pub fn validate(&self) -> Result<()> {
        require_non_empty("metrics.bind", &self.bind)?;
        require_non_empty("metrics.path", &self.path)?;
        if !self.path.starts_with('/') {
            return Err(RivunOpsError::Empty {
                field: "metrics.path must start with /",
            });
        }
        require_nonzero(
            "metrics.scrape_interval_seconds",
            self.scrape_interval_seconds,
        )?;
        validate_labels("metrics.labels", &self.labels)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TracingConfig {
    pub enabled: bool,
    pub otlp_endpoint: String,
    pub protocol: OtlpProtocol,
    pub sample_ratio: f64,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub resource_attributes: BTreeMap<String, String>,
}

impl TracingConfig {
    pub fn validate(&self) -> Result<()> {
        if self.enabled {
            require_non_empty("tracing.otlp_endpoint", &self.otlp_endpoint)?;
        }
        if !(0.0..=1.0).contains(&self.sample_ratio) {
            return Err(RivunOpsError::OutOfRange {
                field: "tracing.sample_ratio",
                min: 0,
                max: 1,
            });
        }
        validate_labels("tracing.resource_attributes", &self.resource_attributes)
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OtlpProtocol {
    Grpc,
    HttpProtobuf,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LogConfig {
    pub format: LogFormat,
    pub level: String,
    #[serde(default)]
    pub redact_payloads: bool,
    #[serde(default)]
    pub include_span_ids: bool,
}

impl LogConfig {
    pub fn validate(&self) -> Result<()> {
        require_non_empty("logs.level", &self.level)
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LogFormat {
    Compact,
    Json,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HealthPolicy {
    pub stale_after_seconds: u64,
    #[serde(default)]
    pub checks: Vec<HealthCheckSpec>,
}

impl HealthPolicy {
    pub fn validate(&self) -> Result<()> {
        require_nonzero("health.stale_after_seconds", self.stale_after_seconds)?;
        let mut names = BTreeSet::new();
        for check in &self.checks {
            check.validate()?;
            if !names.insert(check.name.clone()) {
                return Err(RivunOpsError::Duplicate {
                    kind: "health check",
                    id: check.name.clone(),
                });
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HealthCheckSpec {
    pub name: String,
    pub severity: HealthSeverity,
    pub target: HealthTarget,
    pub timeout_ms: u64,
    pub interval_seconds: u64,
    pub consecutive_failures: u16,
}

impl HealthCheckSpec {
    pub fn validate(&self) -> Result<()> {
        require_non_empty("health.checks.name", &self.name)?;
        require_nonzero("health.checks.timeout_ms", self.timeout_ms)?;
        require_nonzero("health.checks.interval_seconds", self.interval_seconds)?;
        if self.consecutive_failures == 0 {
            return Err(RivunOpsError::Zero {
                field: "health.checks.consecutive_failures",
            });
        }
        self.target.validate()
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum HealthSeverity {
    Advisory,
    Warning,
    Critical,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum HealthTarget {
    Tcp {
        addr: String,
    },
    Http {
        url: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        expected_status: Option<u16>,
    },
    FileExists {
        path: String,
    },
    Command {
        program: String,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        args: Vec<String>,
    },
}

impl HealthTarget {
    fn validate(&self) -> Result<()> {
        match self {
            Self::Tcp { addr } => require_non_empty("health.target.tcp.addr", addr),
            Self::Http {
                url,
                expected_status,
            } => {
                require_non_empty("health.target.http.url", url)?;
                if !url.starts_with("http://") && !url.starts_with("https://") {
                    return Err(RivunOpsError::Empty {
                        field: "health.target.http.url must start with http:// or https://",
                    });
                }
                if let Some(status) = expected_status
                    && !(100..=599).contains(status)
                {
                    return Err(RivunOpsError::OutOfRange {
                        field: "health.target.http.expected_status",
                        min: 100,
                        max: 599,
                    });
                }
                Ok(())
            }
            Self::FileExists { path } => require_non_empty("health.target.file.path", path),
            Self::Command { program, args } => {
                require_non_empty("health.target.command.program", program)?;
                for arg in args {
                    require_non_empty("health.target.command.args", arg)?;
                }
                Ok(())
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HealthReport {
    pub generated_at_micros: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub node_id: Option<Uuid>,
    #[serde(default)]
    pub checks: Vec<HealthCheckResult>,
}

impl HealthReport {
    pub fn overall_status(&self) -> HealthStatus {
        self.checks
            .iter()
            .map(|check| check.status)
            .max()
            .unwrap_or(HealthStatus::Unknown)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HealthCheckResult {
    pub name: String,
    pub status: HealthStatus,
    pub observed_at_micros: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latency_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum HealthStatus {
    Healthy,
    Degraded,
    Critical,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GovernanceConfig {
    pub schema_version: u8,
    #[serde(default)]
    pub operators: Vec<Operator>,
    #[serde(default)]
    pub groups: Vec<GovernanceGroup>,
    #[serde(default)]
    pub policies: Vec<ApprovalPolicy>,
    pub audit: AuditConfig,
}

impl GovernanceConfig {
    pub fn validate(&self) -> Result<()> {
        if self.schema_version != GOVERNANCE_SCHEMA_VERSION {
            return Err(RivunOpsError::OutOfRange {
                field: "governance.schema_version",
                min: GOVERNANCE_SCHEMA_VERSION as u64,
                max: GOVERNANCE_SCHEMA_VERSION as u64,
            });
        }
        self.audit.validate()?;

        let mut operator_ids = BTreeSet::new();
        for operator in &self.operators {
            operator.validate()?;
            if !operator_ids.insert(operator.id.clone()) {
                return Err(RivunOpsError::Duplicate {
                    kind: "operator",
                    id: operator.id.clone(),
                });
            }
        }

        let mut group_ids = BTreeSet::new();
        for group in &self.groups {
            group.validate()?;
            if !group_ids.insert(group.id.clone()) {
                return Err(RivunOpsError::Duplicate {
                    kind: "governance group",
                    id: group.id.clone(),
                });
            }
            for member in &group.members {
                if !operator_ids.contains(member) {
                    return Err(RivunOpsError::Unknown {
                        kind: "operator",
                        id: member.clone(),
                    });
                }
            }
            let eligible = self.eligible_group_members(&group.id).len();
            if group.min_approvals as usize > eligible {
                return Err(RivunOpsError::ApprovalThresholdTooHigh {
                    group: group.id.clone(),
                    required: group.min_approvals,
                    available: eligible,
                });
            }
        }

        let mut policy_ids = BTreeSet::new();
        for policy in &self.policies {
            policy.validate()?;
            if !policy_ids.insert(policy.id.clone()) {
                return Err(RivunOpsError::Duplicate {
                    kind: "approval policy",
                    id: policy.id.clone(),
                });
            }
            for quorum in &policy.required_groups {
                if !group_ids.contains(&quorum.group_id) {
                    return Err(RivunOpsError::PolicyReferencesUnknownGroup {
                        policy: policy.id.clone(),
                        group: quorum.group_id.clone(),
                    });
                }
            }
        }

        Ok(())
    }

    pub fn from_toml_str(input: &str) -> Result<Self> {
        let config: Self =
            toml::from_str(input).map_err(|error| RivunOpsError::TomlDecode(error.to_string()))?;
        config.validate()?;
        Ok(config)
    }

    pub fn policy_for_action(&self, action: &str) -> Option<&ApprovalPolicy> {
        self.policies
            .iter()
            .find(|policy| subject_matches(&policy.action_pattern, action))
    }

    pub fn evaluate_approvals(
        &self,
        policy_id: &str,
        approvals: &[Approval],
    ) -> Result<ApprovalEvaluation> {
        self.validate()?;
        let policy = self
            .policies
            .iter()
            .find(|policy| policy.id == policy_id)
            .ok_or_else(|| RivunOpsError::Unknown {
                kind: "approval policy",
                id: policy_id.to_string(),
            })?;

        let mut missing = Vec::new();
        let mut rejected_by = Vec::new();
        for quorum in &policy.required_groups {
            let group = self.group(&quorum.group_id)?;
            let mut approving_operators = BTreeSet::new();
            for approval in approvals.iter().filter(|approval| {
                approval.policy_id == policy.id && approval.group_id == quorum.group_id
            }) {
                let operator = self.operator(&approval.operator_id)?;
                if !operator.disabled && group.accepts(operator, &approval.role) {
                    match approval.decision {
                        ApprovalDecision::Approve => {
                            approving_operators.insert(operator.id.clone());
                        }
                        ApprovalDecision::Reject => rejected_by.push(operator.id.clone()),
                    }
                }
            }
            let actual = approving_operators.len() as u16;
            if actual < quorum.approvals_required {
                missing.push(MissingApproval {
                    group_id: quorum.group_id.clone(),
                    required: quorum.approvals_required,
                    actual,
                });
            }
        }

        Ok(ApprovalEvaluation {
            policy_id: policy.id.clone(),
            allowed: missing.is_empty() && rejected_by.is_empty(),
            missing,
            rejected_by,
        })
    }

    fn group(&self, id: &str) -> Result<&GovernanceGroup> {
        self.groups
            .iter()
            .find(|group| group.id == id)
            .ok_or_else(|| RivunOpsError::Unknown {
                kind: "governance group",
                id: id.to_string(),
            })
    }

    fn operator(&self, id: &str) -> Result<&Operator> {
        self.operators
            .iter()
            .find(|operator| operator.id == id)
            .ok_or_else(|| RivunOpsError::Unknown {
                kind: "operator",
                id: id.to_string(),
            })
    }

    fn eligible_group_members(&self, group_id: &str) -> Vec<&Operator> {
        let Some(group) = self.groups.iter().find(|group| group.id == group_id) else {
            return Vec::new();
        };
        self.operators
            .iter()
            .filter(|operator| !operator.disabled && group.accepts_any_role(operator))
            .collect()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Operator {
    pub id: String,
    pub display_name: String,
    #[serde(default)]
    pub roles: BTreeSet<Role>,
    #[serde(default)]
    pub groups: BTreeSet<String>,
    #[serde(default)]
    pub disabled: bool,
}

impl Operator {
    fn validate(&self) -> Result<()> {
        require_non_empty("operator.id", &self.id)?;
        require_non_empty("operator.display_name", &self.display_name)?;
        if self.roles.is_empty() {
            return Err(RivunOpsError::Empty {
                field: "operator.roles",
            });
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    Operator,
    Sre,
    SecurityOfficer,
    ReleaseManager,
    RegistryMaintainer,
    Auditor,
    IncidentCommander,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GovernanceGroup {
    pub id: String,
    pub description: String,
    pub min_approvals: u16,
    #[serde(default)]
    pub accepted_roles: BTreeSet<Role>,
    #[serde(default)]
    pub members: BTreeSet<String>,
}

impl GovernanceGroup {
    fn validate(&self) -> Result<()> {
        require_non_empty("group.id", &self.id)?;
        require_non_empty("group.description", &self.description)?;
        if self.min_approvals == 0 {
            return Err(RivunOpsError::Zero {
                field: "group.min_approvals",
            });
        }
        if self.accepted_roles.is_empty() {
            return Err(RivunOpsError::Empty {
                field: "group.accepted_roles",
            });
        }
        if self.members.is_empty() {
            return Err(RivunOpsError::Empty {
                field: "group.members",
            });
        }
        Ok(())
    }

    fn accepts(&self, operator: &Operator, approval_role: &Role) -> bool {
        self.members.contains(&operator.id)
            && operator.groups.contains(&self.id)
            && operator.roles.contains(approval_role)
            && self.accepted_roles.contains(approval_role)
    }

    fn accepts_any_role(&self, operator: &Operator) -> bool {
        self.members.contains(&operator.id)
            && operator.groups.contains(&self.id)
            && operator
                .roles
                .iter()
                .any(|role| self.accepted_roles.contains(role))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ApprovalPolicy {
    pub id: String,
    pub action_pattern: String,
    #[serde(default)]
    pub required_groups: Vec<GroupQuorum>,
    #[serde(default)]
    pub require_audit_entry: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub break_glass_role: Option<Role>,
}

impl ApprovalPolicy {
    fn validate(&self) -> Result<()> {
        require_non_empty("policy.id", &self.id)?;
        require_non_empty("policy.action_pattern", &self.action_pattern)?;
        if self.required_groups.is_empty() {
            return Err(RivunOpsError::Empty {
                field: "policy.required_groups",
            });
        }
        for quorum in &self.required_groups {
            quorum.validate()?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GroupQuorum {
    pub group_id: String,
    pub approvals_required: u16,
}

impl GroupQuorum {
    fn validate(&self) -> Result<()> {
        require_non_empty("policy.required_groups.group_id", &self.group_id)?;
        if self.approvals_required == 0 {
            return Err(RivunOpsError::Zero {
                field: "policy.required_groups.approvals_required",
            });
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Approval {
    pub policy_id: String,
    pub request_id: String,
    pub operator_id: String,
    pub group_id: String,
    pub role: Role,
    pub decision: ApprovalDecision,
    pub approved_at_micros: u64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalDecision {
    Approve,
    Reject,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ApprovalEvaluation {
    pub policy_id: String,
    pub allowed: bool,
    #[serde(default)]
    pub missing: Vec<MissingApproval>,
    #[serde(default)]
    pub rejected_by: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MissingApproval {
    pub group_id: String,
    pub required: u16,
    pub actual: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AuditConfig {
    pub log_path: String,
    #[serde(default)]
    pub hash_chained: bool,
    #[serde(default)]
    pub retain_days: u16,
}

impl AuditConfig {
    fn validate(&self) -> Result<()> {
        require_non_empty("audit.log_path", &self.log_path)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AuditEntry {
    pub sequence: u64,
    pub timestamp_micros: u64,
    pub actor: String,
    pub action: String,
    pub subject: String,
    pub outcome: AuditOutcome,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub details: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub previous_hash: Option<String>,
    pub entry_hash: String,
}

impl AuditEntry {
    pub fn new(
        sequence: u64,
        previous_hash: Option<String>,
        draft: AuditEntryDraft,
    ) -> Result<Self> {
        let mut entry = Self {
            sequence,
            timestamp_micros: draft.timestamp_micros,
            actor: draft.actor,
            action: draft.action,
            subject: draft.subject,
            outcome: draft.outcome,
            details: draft.details,
            previous_hash,
            entry_hash: String::new(),
        };
        require_non_empty("audit.actor", &entry.actor)?;
        require_non_empty("audit.action", &entry.action)?;
        require_non_empty("audit.subject", &entry.subject)?;
        entry.entry_hash = entry.compute_hash();
        Ok(entry)
    }

    pub fn compute_hash(&self) -> String {
        let input = AuditHashInput {
            sequence: self.sequence,
            timestamp_micros: self.timestamp_micros,
            actor: &self.actor,
            action: &self.action,
            subject: &self.subject,
            outcome: self.outcome,
            details: &self.details,
            previous_hash: self.previous_hash.as_deref(),
        };
        let encoded = serde_json::to_vec(&input).expect("audit hash input is serializable");
        hex::encode(blake3::hash(&encoded).as_bytes())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AuditEntryDraft {
    pub timestamp_micros: u64,
    pub actor: String,
    pub action: String,
    pub subject: String,
    pub outcome: AuditOutcome,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub details: BTreeMap<String, String>,
}

impl AuditEntryDraft {
    pub fn new(
        timestamp_micros: u64,
        actor: impl Into<String>,
        action: impl Into<String>,
        subject: impl Into<String>,
        outcome: AuditOutcome,
    ) -> Self {
        Self {
            timestamp_micros,
            actor: actor.into(),
            action: action.into(),
            subject: subject.into(),
            outcome,
            details: BTreeMap::new(),
        }
    }

    pub fn with_detail(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.details.insert(key.into(), value.into());
        self
    }
}

#[derive(Debug, Serialize)]
struct AuditHashInput<'a> {
    sequence: u64,
    timestamp_micros: u64,
    actor: &'a str,
    action: &'a str,
    subject: &'a str,
    outcome: AuditOutcome,
    details: &'a BTreeMap<String, String>,
    previous_hash: Option<&'a str>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AuditOutcome {
    Approved,
    Rejected,
    Executed,
    Failed,
    Observed,
}

pub fn verify_audit_chain(entries: &[AuditEntry]) -> Result<()> {
    let mut previous_hash = None;
    for (index, entry) in entries.iter().enumerate() {
        let expected_sequence = index as u64;
        if entry.sequence != expected_sequence {
            return Err(RivunOpsError::AuditSequenceGap {
                expected: expected_sequence,
                actual: entry.sequence,
            });
        }
        if entry.previous_hash != previous_hash {
            return Err(RivunOpsError::AuditPreviousHashMismatch {
                sequence: entry.sequence,
            });
        }
        let computed = entry.compute_hash();
        if computed != entry.entry_hash {
            return Err(RivunOpsError::AuditHashMismatch {
                sequence: entry.sequence,
            });
        }
        previous_hash = Some(entry.entry_hash.clone());
    }
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReleaseManifest {
    pub schema_version: u8,
    pub version: String,
    pub channel: ReleaseChannel,
    pub git_sha: String,
    pub created_at_micros: u64,
    #[serde(default)]
    pub artifacts: Vec<ReleaseArtifact>,
    #[serde(default)]
    pub signatures: Vec<ArtifactSignature>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sbom_path: Option<String>,
}

impl ReleaseManifest {
    pub fn validate(&self) -> Result<()> {
        if self.schema_version != RELEASE_SCHEMA_VERSION {
            return Err(RivunOpsError::OutOfRange {
                field: "release.schema_version",
                min: RELEASE_SCHEMA_VERSION as u64,
                max: RELEASE_SCHEMA_VERSION as u64,
            });
        }
        validate_semver(&self.version)?;
        validate_git_sha(&self.git_sha)?;
        require_nonzero("release.created_at_micros", self.created_at_micros)?;
        if self.artifacts.is_empty() {
            return Err(RivunOpsError::Empty {
                field: "release.artifacts",
            });
        }

        let mut artifact_names = BTreeSet::new();
        for artifact in &self.artifacts {
            artifact.validate()?;
            if !artifact_names.insert(artifact.name.clone()) {
                return Err(RivunOpsError::Duplicate {
                    kind: "release artifact",
                    id: artifact.name.clone(),
                });
            }
        }

        for signature in &self.signatures {
            signature.validate()?;
            if !artifact_names.contains(&signature.artifact_name) {
                return Err(RivunOpsError::UnknownSignatureArtifact(
                    signature.artifact_name.clone(),
                ));
            }
        }

        if let Some(sbom_path) = &self.sbom_path {
            require_non_empty("release.sbom_path", sbom_path)?;
        }

        Ok(())
    }

    pub fn from_json_str(input: &str) -> std::result::Result<Self, serde_json::Error> {
        serde_json::from_str(input)
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ReleaseChannel {
    Nightly,
    Preview,
    Stable,
    Security,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactKind {
    CliBinary,
    ContainerImage,
    SourceArchive,
    RegistryBundle,
    Checksums,
    Sbom,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReleaseArtifact {
    pub name: String,
    pub kind: ArtifactKind,
    pub target: String,
    pub path: String,
    pub size_bytes: u64,
    pub sha256: String,
    pub blake3: String,
}

impl ReleaseArtifact {
    fn validate(&self) -> Result<()> {
        require_non_empty("artifact.name", &self.name)?;
        require_non_empty("artifact.target", &self.target)?;
        require_non_empty("artifact.path", &self.path)?;
        require_nonzero("artifact.size_bytes", self.size_bytes)?;
        validate_checksum(&self.name, "sha256", &self.sha256)?;
        validate_checksum(&self.name, "blake3", &self.blake3)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ArtifactSignature {
    pub artifact_name: String,
    pub signer: String,
    pub public_key: String,
    pub signature: String,
}

impl ArtifactSignature {
    fn validate(&self) -> Result<()> {
        require_non_empty("signature.artifact_name", &self.artifact_name)?;
        require_non_empty("signature.signer", &self.signer)?;
        require_non_empty("signature.public_key", &self.public_key)?;
        require_non_empty("signature.signature", &self.signature)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReleasePackagePlan {
    pub version: String,
    pub targets: Vec<String>,
    #[serde(default)]
    pub include_sbom: bool,
    #[serde(default)]
    pub sign_artifacts: bool,
    #[serde(default)]
    pub include_registry_bundle: bool,
}

impl ReleasePackagePlan {
    pub fn expected_artifact_names(&self) -> Result<Vec<String>> {
        validate_semver(&self.version)?;
        let mut names = Vec::new();
        for target in &self.targets {
            require_non_empty("release.targets", target)?;
            names.push(format!("rivun-{}-{target}", self.version));
        }
        if self.include_registry_bundle {
            names.push(format!("rivunstore-bundle-{}", self.version));
        }
        if self.include_sbom {
            names.push(format!("rivun-sbom-{}.spdx.json", self.version));
        }
        Ok(names)
    }
}

fn require_non_empty(field: &'static str, value: &str) -> Result<()> {
    if value.trim().is_empty() {
        Err(RivunOpsError::Empty { field })
    } else {
        Ok(())
    }
}

fn require_nonzero(field: &'static str, value: u64) -> Result<()> {
    if value == 0 {
        Err(RivunOpsError::Zero { field })
    } else {
        Ok(())
    }
}

fn validate_labels(field: &'static str, labels: &BTreeMap<String, String>) -> Result<()> {
    for (key, value) in labels {
        require_non_empty(field, key)?;
        require_non_empty(field, value)?;
    }
    Ok(())
}

fn subject_matches(pattern: &str, value: &str) -> bool {
    if pattern == "*" {
        return true;
    }
    if let Some(prefix) = pattern.strip_suffix('*') {
        return value.starts_with(prefix);
    }
    pattern == value
}

fn validate_semver(version: &str) -> Result<()> {
    let (core, prerelease) = version
        .split_once('-')
        .map_or((version, None), |(core, prerelease)| {
            (core, Some(prerelease))
        });
    let parts = core.split('.').collect::<Vec<_>>();
    let core_valid = parts.len() == 3
        && parts.iter().all(|part| {
            !part.is_empty()
                && part.chars().all(|ch| ch.is_ascii_digit())
                && (part == &"0" || !part.starts_with('0'))
        });
    let prerelease_valid = prerelease.is_none_or(|prerelease| {
        !prerelease.is_empty()
            && prerelease.split('.').all(|part| {
                !part.is_empty()
                    && part
                        .chars()
                        .all(|ch| ch.is_ascii_alphanumeric() || ch == '-')
            })
    });
    let valid = core_valid && prerelease_valid;
    if valid {
        Ok(())
    } else {
        Err(RivunOpsError::InvalidVersion(version.to_string()))
    }
}

fn validate_git_sha(sha: &str) -> Result<()> {
    if sha.len() == 40 && sha.chars().all(is_lower_hex) {
        Ok(())
    } else {
        Err(RivunOpsError::InvalidGitSha(sha.to_string()))
    }
}

fn validate_checksum(name: &str, field: &'static str, checksum: &str) -> Result<()> {
    if checksum.len() == 64 && checksum.chars().all(is_lower_hex) {
        Ok(())
    } else {
        Err(RivunOpsError::InvalidChecksum {
            name: name.to_string(),
            field,
        })
    }
}

fn is_lower_hex(ch: char) -> bool {
    ch.is_ascii_digit() || ('a'..='f').contains(&ch)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn governance_config() -> GovernanceConfig {
        GovernanceConfig {
            schema_version: GOVERNANCE_SCHEMA_VERSION,
            operators: vec![
                Operator {
                    id: "alice".to_string(),
                    display_name: "Alice".to_string(),
                    roles: BTreeSet::from([Role::ReleaseManager, Role::Operator]),
                    groups: BTreeSet::from(["release".to_string()]),
                    disabled: false,
                },
                Operator {
                    id: "bob".to_string(),
                    display_name: "Bob".to_string(),
                    roles: BTreeSet::from([Role::SecurityOfficer, Role::Operator]),
                    groups: BTreeSet::from(["security".to_string()]),
                    disabled: false,
                },
            ],
            groups: vec![
                GovernanceGroup {
                    id: "release".to_string(),
                    description: "Release approvers".to_string(),
                    min_approvals: 1,
                    accepted_roles: BTreeSet::from([Role::ReleaseManager]),
                    members: BTreeSet::from(["alice".to_string()]),
                },
                GovernanceGroup {
                    id: "security".to_string(),
                    description: "Security approvers".to_string(),
                    min_approvals: 1,
                    accepted_roles: BTreeSet::from([Role::SecurityOfficer]),
                    members: BTreeSet::from(["bob".to_string()]),
                },
            ],
            policies: vec![ApprovalPolicy {
                id: "stable-release".to_string(),
                action_pattern: "release.stable".to_string(),
                required_groups: vec![
                    GroupQuorum {
                        group_id: "release".to_string(),
                        approvals_required: 1,
                    },
                    GroupQuorum {
                        group_id: "security".to_string(),
                        approvals_required: 1,
                    },
                ],
                require_audit_entry: true,
                break_glass_role: Some(Role::IncidentCommander),
            }],
            audit: AuditConfig {
                log_path: ".rivun/audit.jsonl".to_string(),
                hash_chained: true,
                retain_days: 365,
            },
        }
    }

    #[test]
    fn observability_validation_rejects_duplicate_health_checks() {
        let config = ObservabilityConfig {
            schema_version: OBSERVABILITY_SCHEMA_VERSION,
            service: ServiceIdentity {
                service_name: "rivun-node".to_string(),
                environment: "prod".to_string(),
                cluster: "primary".to_string(),
                region: None,
                labels: BTreeMap::new(),
            },
            metrics: MetricsConfig {
                enabled: true,
                bind: "0.0.0.0:9109".to_string(),
                path: "/metrics".to_string(),
                scrape_interval_seconds: 15,
                include_runtime_metrics: true,
                include_driver_metrics: true,
                labels: BTreeMap::new(),
            },
            tracing: TracingConfig {
                enabled: true,
                otlp_endpoint: "http://otel-collector:4317".to_string(),
                protocol: OtlpProtocol::Grpc,
                sample_ratio: 0.25,
                resource_attributes: BTreeMap::new(),
            },
            logs: LogConfig {
                format: LogFormat::Json,
                level: "info".to_string(),
                redact_payloads: true,
                include_span_ids: true,
            },
            health: HealthPolicy {
                stale_after_seconds: 60,
                checks: vec![
                    HealthCheckSpec {
                        name: "udp-bind".to_string(),
                        severity: HealthSeverity::Critical,
                        target: HealthTarget::Tcp {
                            addr: "127.0.0.1:7000".to_string(),
                        },
                        timeout_ms: 500,
                        interval_seconds: 10,
                        consecutive_failures: 3,
                    },
                    HealthCheckSpec {
                        name: "udp-bind".to_string(),
                        severity: HealthSeverity::Critical,
                        target: HealthTarget::Tcp {
                            addr: "127.0.0.1:7001".to_string(),
                        },
                        timeout_ms: 500,
                        interval_seconds: 10,
                        consecutive_failures: 3,
                    },
                ],
            },
        };

        assert_eq!(
            config.validate(),
            Err(RivunOpsError::Duplicate {
                kind: "health check",
                id: "udp-bind".to_string()
            })
        );
    }

    #[test]
    fn governance_approval_requires_each_group() {
        let config = governance_config();
        config.validate().unwrap();

        let partial = config
            .evaluate_approvals(
                "stable-release",
                &[Approval {
                    policy_id: "stable-release".to_string(),
                    request_id: "rel-1".to_string(),
                    operator_id: "alice".to_string(),
                    group_id: "release".to_string(),
                    role: Role::ReleaseManager,
                    decision: ApprovalDecision::Approve,
                    approved_at_micros: 10,
                }],
            )
            .unwrap();

        assert!(!partial.allowed);
        assert_eq!(partial.missing[0].group_id, "security");

        let complete = config
            .evaluate_approvals(
                "stable-release",
                &[
                    Approval {
                        policy_id: "stable-release".to_string(),
                        request_id: "rel-1".to_string(),
                        operator_id: "alice".to_string(),
                        group_id: "release".to_string(),
                        role: Role::ReleaseManager,
                        decision: ApprovalDecision::Approve,
                        approved_at_micros: 10,
                    },
                    Approval {
                        policy_id: "stable-release".to_string(),
                        request_id: "rel-1".to_string(),
                        operator_id: "bob".to_string(),
                        group_id: "security".to_string(),
                        role: Role::SecurityOfficer,
                        decision: ApprovalDecision::Approve,
                        approved_at_micros: 11,
                    },
                ],
            )
            .unwrap();

        assert!(complete.allowed);
    }

    #[test]
    fn audit_chain_detects_mutation() {
        let first = AuditEntry::new(
            0,
            None,
            AuditEntryDraft::new(
                100,
                "alice",
                "release.approve",
                "v0.1.0",
                AuditOutcome::Approved,
            ),
        )
        .unwrap();
        let second = AuditEntry::new(
            1,
            Some(first.entry_hash.clone()),
            AuditEntryDraft::new(
                101,
                "ci",
                "release.package",
                "v0.1.0",
                AuditOutcome::Executed,
            ),
        )
        .unwrap();
        let mut chain = vec![first, second];

        verify_audit_chain(&chain).unwrap();
        chain[1].subject = "v0.1.1".to_string();

        assert_eq!(
            verify_audit_chain(&chain),
            Err(RivunOpsError::AuditHashMismatch { sequence: 1 })
        );
    }

    #[test]
    fn release_manifest_validates_artifacts_and_signatures() {
        let checksum = "a".repeat(64);
        let manifest = ReleaseManifest {
            schema_version: RELEASE_SCHEMA_VERSION,
            version: "0.1.0".to_string(),
            channel: ReleaseChannel::Stable,
            git_sha: "0123456789abcdef0123456789abcdef01234567".to_string(),
            created_at_micros: 42,
            artifacts: vec![ReleaseArtifact {
                name: "rivun-0.1.0-x86_64-unknown-linux-gnu".to_string(),
                kind: ArtifactKind::CliBinary,
                target: "x86_64-unknown-linux-gnu".to_string(),
                path: "dist/rivun-0.1.0-linux.tar.gz".to_string(),
                size_bytes: 100,
                sha256: checksum.clone(),
                blake3: checksum,
            }],
            signatures: vec![ArtifactSignature {
                artifact_name: "rivun-0.1.0-x86_64-unknown-linux-gnu".to_string(),
                signer: "release-bot".to_string(),
                public_key: "key".to_string(),
                signature: "sig".to_string(),
            }],
            sbom_path: Some("dist/sbom.spdx.json".to_string()),
        };

        manifest.validate().unwrap();
    }

    #[test]
    fn release_plan_names_expected_artifacts() {
        let plan = ReleasePackagePlan {
            version: "0.2.0-rc.1".to_string(),
            targets: vec!["x86_64-unknown-linux-gnu".to_string()],
            include_sbom: true,
            sign_artifacts: true,
            include_registry_bundle: true,
        };

        assert_eq!(
            plan.expected_artifact_names().unwrap(),
            vec![
                "rivun-0.2.0-rc.1-x86_64-unknown-linux-gnu",
                "rivunstore-bundle-0.2.0-rc.1",
                "rivun-sbom-0.2.0-rc.1.spdx.json"
            ]
        );
    }
}
