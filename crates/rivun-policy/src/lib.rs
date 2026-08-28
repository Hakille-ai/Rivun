//! Deterministic policy evaluation for typed Rivun messages.
//!
//! This crate is deliberately model-agnostic. It evaluates facts about a typed
//! message and returns the gate that must be satisfied before execution or
//! forwarding can continue.

use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;
use uuid::Uuid;
use rivun_capability::CapabilityId;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum RivunPolicyError {
    #[error("policy rule `{0}` subject pattern must not be empty")]
    EmptySubjectPattern(String),
    #[error("policy rule `{0}` kind must not be empty")]
    EmptyKind(String),
    #[error("policy rule `{0}` content_type must not be empty")]
    EmptyContentType(String),
    #[error("policy rule `{0}` uses require_grant without required_capability")]
    MissingRequiredCapability(String),
    #[error("policy default_decision must be allow or deny")]
    InvalidDefaultDecision,
    #[error("failed to parse TOML policy: {0}")]
    Toml(String),
}

pub type Result<T> = std::result::Result<T, RivunPolicyError>;

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct PolicySet {
    #[serde(default)]
    pub default_decision: PolicyDecision,
    #[serde(default)]
    pub rules: Vec<PolicyRule>,
}

impl PolicySet {
    pub fn new(rules: Vec<PolicyRule>) -> Result<Self> {
        Self::new_with_default(PolicyDecision::Allow, rules)
    }

    pub fn new_with_default(
        default_decision: PolicyDecision,
        rules: Vec<PolicyRule>,
    ) -> Result<Self> {
        let set = Self {
            default_decision,
            rules,
        };
        set.validate()?;
        Ok(set)
    }

    pub fn from_toml_str(input: &str) -> Result<Self> {
        let set: Self =
            toml::from_str(input).map_err(|error| RivunPolicyError::Toml(error.to_string()))?;
        set.validate()?;
        Ok(set)
    }

    pub fn validate(&self) -> Result<()> {
        if !matches!(
            self.default_decision,
            PolicyDecision::Allow | PolicyDecision::Deny
        ) {
            return Err(RivunPolicyError::InvalidDefaultDecision);
        }
        for (index, rule) in self.rules.iter().enumerate() {
            rule.validate(&format!("#{index}"))?;
        }
        Ok(())
    }

    pub fn evaluate(&self, input: &PolicyInput<'_>) -> PolicyEvaluation {
        for (index, rule) in self.rules.iter().enumerate() {
            if rule.matches(input) {
                return PolicyEvaluation::from_rule(index, rule, input);
            }
        }
        let allowed = self.default_decision == PolicyDecision::Allow;
        PolicyEvaluation {
            decision: self.default_decision,
            allowed,
            matched_rule_index: None,
            matched_rule_name: None,
            reason: if allowed {
                "default allow".to_string()
            } else {
                "default deny".to_string()
            },
            required_poa: false,
            required_capability: None,
            human_approval_required: false,
            simulation_required: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PolicyRule {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subject: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_node: Option<Uuid>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_node: Option<Uuid>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_type: Option<String>,
    #[serde(default)]
    pub decision: PolicyDecision,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub required_capability: Option<CapabilityId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

impl PolicyRule {
    pub fn validate(&self, fallback_name: &str) -> Result<()> {
        let name = self.name.as_deref().unwrap_or(fallback_name).to_string();
        if self
            .kind
            .as_deref()
            .is_some_and(|kind| kind.trim().is_empty())
        {
            return Err(RivunPolicyError::EmptyKind(name));
        }
        if self
            .subject
            .as_deref()
            .is_some_and(|subject| subject.trim().is_empty())
        {
            return Err(RivunPolicyError::EmptySubjectPattern(name));
        }
        if self
            .content_type
            .as_deref()
            .is_some_and(|content_type| content_type.trim().is_empty())
        {
            return Err(RivunPolicyError::EmptyContentType(name));
        }
        if self.decision == PolicyDecision::RequireGrant && self.required_capability.is_none() {
            return Err(RivunPolicyError::MissingRequiredCapability(name));
        }
        Ok(())
    }

    pub fn matches(&self, input: &PolicyInput<'_>) -> bool {
        optional_field_matches(self.kind.as_deref(), input.kind)
            && optional_subject_matches(self.subject.as_deref(), input.subject)
            && self
                .source_node
                .map(|source| Some(source) == input.source_node)
                .unwrap_or(true)
            && self
                .target_node
                .map(|target| Some(target) == input.target_node)
                .unwrap_or(true)
            && optional_content_type_matches(self.content_type.as_deref(), input.content_type)
    }
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PolicyDecision {
    #[default]
    Allow,
    Deny,
    RequirePoa,
    RequireGrant,
    HumanApproval,
    SimulateFirst,
}

#[derive(Debug, Clone, Copy)]
pub struct PolicyInput<'a> {
    pub kind: &'a str,
    pub subject: &'a str,
    pub source_node: Option<Uuid>,
    pub target_node: Option<Uuid>,
    pub content_type: Option<&'a str>,
    pub consensus_protected: bool,
    pub granted_capabilities: &'a BTreeSet<CapabilityId>,
    pub human_approved: bool,
    pub simulation_passed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PolicyEvaluation {
    pub decision: PolicyDecision,
    pub allowed: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub matched_rule_index: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub matched_rule_name: Option<String>,
    pub reason: String,
    pub required_poa: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub required_capability: Option<CapabilityId>,
    pub human_approval_required: bool,
    pub simulation_required: bool,
}

impl PolicyEvaluation {
    fn from_rule(index: usize, rule: &PolicyRule, input: &PolicyInput<'_>) -> Self {
        let required_capability = rule.required_capability.clone();
        let allowed = match rule.decision {
            PolicyDecision::Allow => true,
            PolicyDecision::Deny => false,
            PolicyDecision::RequirePoa => input.consensus_protected,
            PolicyDecision::RequireGrant => required_capability
                .as_ref()
                .is_some_and(|capability| input.granted_capabilities.contains(capability)),
            PolicyDecision::HumanApproval => input.human_approved,
            PolicyDecision::SimulateFirst => input.simulation_passed,
        };
        Self {
            decision: rule.decision,
            allowed,
            matched_rule_index: Some(index),
            matched_rule_name: rule.name.clone(),
            reason: rule
                .reason
                .clone()
                .unwrap_or_else(|| format!("matched policy rule #{}", index)),
            required_poa: rule.decision == PolicyDecision::RequirePoa,
            required_capability,
            human_approval_required: rule.decision == PolicyDecision::HumanApproval,
            simulation_required: rule.decision == PolicyDecision::SimulateFirst,
        }
    }
}

fn optional_field_matches(pattern: Option<&str>, value: &str) -> bool {
    match pattern {
        Some("*") => true,
        Some(pattern) => pattern.eq_ignore_ascii_case(value),
        None => true,
    }
}

fn optional_content_type_matches(pattern: Option<&str>, value: Option<&str>) -> bool {
    match (pattern, value) {
        (Some("*"), _) => true,
        (Some(pattern), Some(value)) => pattern.eq_ignore_ascii_case(value),
        (Some(_), None) => false,
        (None, _) => true,
    }
}

fn optional_subject_matches(pattern: Option<&str>, value: &str) -> bool {
    let Some(pattern) = pattern else {
        return true;
    };
    if pattern == "*" {
        return true;
    }
    if let Some(prefix) = pattern.strip_suffix('*') {
        return value.starts_with(prefix);
    }
    pattern == value
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(consensus_protected: bool, grants: &BTreeSet<CapabilityId>) -> PolicyInput<'_> {
        PolicyInput {
            kind: "action",
            subject: "safety.emergency_stop",
            source_node: None,
            target_node: None,
            content_type: Some("application/json"),
            consensus_protected,
            granted_capabilities: grants,
            human_approved: false,
            simulation_passed: false,
        }
    }

    #[test]
    fn unmatched_message_uses_default_allow_for_compatibility() {
        let policy = PolicySet::default();
        let grants = BTreeSet::new();
        let evaluation = policy.evaluate(&input(false, &grants));

        assert_eq!(evaluation.decision, PolicyDecision::Allow);
        assert!(evaluation.allowed);
        assert_eq!(evaluation.reason, "default allow");
    }

    #[test]
    fn unmatched_message_can_default_deny() {
        let policy = PolicySet::new_with_default(PolicyDecision::Deny, Vec::new()).unwrap();
        let grants = BTreeSet::new();
        let evaluation = policy.evaluate(&input(false, &grants));

        assert_eq!(evaluation.decision, PolicyDecision::Deny);
        assert!(!evaluation.allowed);
        assert_eq!(evaluation.reason, "default deny");
    }

    #[test]
    fn policy_rejects_non_terminal_default_decision() {
        let error =
            PolicySet::new_with_default(PolicyDecision::RequirePoa, Vec::new()).unwrap_err();

        assert_eq!(error, RivunPolicyError::InvalidDefaultDecision);
    }

    #[test]
    fn require_poa_fails_closed_until_consensus_is_present() {
        let policy = PolicySet::new(vec![PolicyRule {
            name: Some("safety".to_string()),
            kind: Some("action".to_string()),
            subject: Some("safety.*".to_string()),
            source_node: None,
            target_node: None,
            content_type: None,
            decision: PolicyDecision::RequirePoa,
            required_capability: None,
            reason: None,
        }])
        .unwrap();
        let grants = BTreeSet::new();

        assert!(!policy.evaluate(&input(false, &grants)).allowed);
        assert!(policy.evaluate(&input(true, &grants)).allowed);
    }

    #[test]
    fn require_grant_checks_explicit_capability() {
        let capability = CapabilityId::new("driver.execute:echo").unwrap();
        let policy = PolicySet::new(vec![PolicyRule {
            name: None,
            kind: Some("action".to_string()),
            subject: Some("echo".to_string()),
            source_node: None,
            target_node: None,
            content_type: None,
            decision: PolicyDecision::RequireGrant,
            required_capability: Some(capability.clone()),
            reason: None,
        }])
        .unwrap();
        let input = PolicyInput {
            kind: "action",
            subject: "echo",
            source_node: None,
            target_node: None,
            content_type: None,
            consensus_protected: false,
            granted_capabilities: &BTreeSet::new(),
            human_approved: false,
            simulation_passed: false,
        };
        assert!(!policy.evaluate(&input).allowed);

        let mut grants = BTreeSet::new();
        grants.insert(capability);
        let input = PolicyInput {
            granted_capabilities: &grants,
            ..input
        };
        assert!(policy.evaluate(&input).allowed);
    }
}
