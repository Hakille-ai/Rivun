//! Deterministic route planning for Rivun envelopes.

use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;
use rivun_capability::CapabilityId;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum RivunRouterError {
    #[error("route `{route}` target must set exactly one destination")]
    InvalidTarget { route: String },
    #[error("route `{0}` subject pattern must not be empty")]
    EmptySubjectPattern(String),
    #[error("route `{0}` requires a peer grant but does not target exactly one peer")]
    InvalidPeerGrantRequirement(String),
}

pub type Result<T> = std::result::Result<T, RivunRouterError>;

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct RouteTable {
    #[serde(default)]
    pub routes: Vec<RouteRule>,
}

impl RouteTable {
    pub fn new(routes: Vec<RouteRule>) -> Result<Self> {
        let table = Self { routes };
        table.validate()?;
        Ok(table)
    }

    pub fn validate(&self) -> Result<()> {
        for (index, route) in self.routes.iter().enumerate() {
            let name = route_name(index, route);
            route.target.validate(&name)?;
            if route.requires_peer_grant.is_some() && route.target.peer.is_none() {
                return Err(RivunRouterError::InvalidPeerGrantRequirement(name));
            }
            if let Some(subject) = route.matches.subject.as_deref()
                && subject.trim().is_empty()
            {
                return Err(RivunRouterError::EmptySubjectPattern(name));
            }
        }
        Ok(())
    }

    pub fn decide(&self, message: &RouteMessage) -> RouteDecision {
        for (index, route) in self.routes.iter().enumerate() {
            if route.matches.matches(message) {
                return RouteDecision {
                    target: route.target.clone(),
                    matched_rule_index: Some(index),
                    matched_rule_name: route.name.clone(),
                    reason: route
                        .description
                        .clone()
                        .unwrap_or_else(|| format!("matched route {}", route_name(index, route))),
                };
            }
        }

        RouteDecision {
            target: RouteTarget::default_for(message),
            matched_rule_index: None,
            matched_rule_name: None,
            reason: "default route".to_string(),
        }
    }

    pub fn explain(&self, message: &RouteMessage) -> RouteExplanation {
        RouteExplanation {
            message: message.clone(),
            decision: self.decide(message),
            route_count: self.routes.len(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RouteRule {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requires_peer_grant: Option<CapabilityId>,
    #[serde(default, rename = "match")]
    pub matches: RouteMatch,
    pub target: RouteTarget,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct RouteMatch {
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
}

impl RouteMatch {
    pub fn matches(&self, message: &RouteMessage) -> bool {
        field_matches(self.kind.as_deref(), &message.kind)
            && pattern_matches(self.subject.as_deref(), &message.subject)
            && self
                .source_node
                .map(|source| source == message.source_node)
                .unwrap_or(true)
            && self
                .target_node
                .map(|target| target == message.target_node)
                .unwrap_or(true)
            && optional_field_matches(
                self.content_type.as_deref(),
                message.content_type.as_deref(),
            )
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct RouteTarget {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub local_driver: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub peer: Option<Uuid>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capability: Option<CapabilityId>,
    #[serde(default)]
    pub broadcast: bool,
    #[serde(default)]
    pub drop: bool,
}

impl RouteTarget {
    pub fn local_driver(action: impl Into<String>) -> Self {
        Self {
            local_driver: Some(action.into()),
            ..Self::default()
        }
    }

    pub fn peer(peer: Uuid) -> Self {
        Self {
            peer: Some(peer),
            ..Self::default()
        }
    }

    pub fn capability(capability: CapabilityId) -> Self {
        Self {
            capability: Some(capability),
            ..Self::default()
        }
    }

    pub fn broadcast() -> Self {
        Self {
            broadcast: true,
            ..Self::default()
        }
    }

    pub fn drop() -> Self {
        Self {
            drop: true,
            ..Self::default()
        }
    }

    fn default_for(message: &RouteMessage) -> Self {
        if message.kind == "action" && !message.subject.is_empty() {
            Self::local_driver(message.subject.clone())
        } else {
            Self::drop()
        }
    }

    fn validate(&self, route: &str) -> Result<()> {
        let set = self.local_driver.is_some() as u8
            + self.peer.is_some() as u8
            + self.capability.is_some() as u8
            + self.broadcast as u8
            + self.drop as u8;
        if set == 1 {
            Ok(())
        } else {
            Err(RivunRouterError::InvalidTarget {
                route: route.to_string(),
            })
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RouteMessage {
    pub source_node: Uuid,
    pub target_node: Uuid,
    pub kind: String,
    pub subject: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RouteDecision {
    pub target: RouteTarget,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub matched_rule_index: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub matched_rule_name: Option<String>,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RouteExplanation {
    pub message: RouteMessage,
    pub decision: RouteDecision,
    pub route_count: usize,
}

fn route_name(index: usize, route: &RouteRule) -> String {
    route.name.clone().unwrap_or_else(|| format!("#{index}"))
}

fn field_matches(pattern: Option<&str>, value: &str) -> bool {
    match pattern {
        Some(pattern) => pattern.eq_ignore_ascii_case(value),
        None => true,
    }
}

fn optional_field_matches(pattern: Option<&str>, value: Option<&str>) -> bool {
    match (pattern, value) {
        (Some(pattern), Some(value)) => pattern.eq_ignore_ascii_case(value),
        (Some(_), None) => false,
        (None, _) => true,
    }
}

fn pattern_matches(pattern: Option<&str>, value: &str) -> bool {
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

    fn message(subject: &str) -> RouteMessage {
        RouteMessage {
            source_node: Uuid::nil(),
            target_node: Uuid::nil(),
            kind: "action".to_string(),
            subject: subject.to_string(),
            content_type: Some("application/json".to_string()),
        }
    }

    #[test]
    fn routes_by_subject_prefix() {
        let peer = Uuid::new_v4();
        let table = RouteTable::new(vec![RouteRule {
            name: Some("thermostat-peer".to_string()),
            description: None,
            requires_peer_grant: None,
            matches: RouteMatch {
                kind: Some("action".to_string()),
                subject: Some("thermostat.*".to_string()),
                ..RouteMatch::default()
            },
            target: RouteTarget::peer(peer),
        }])
        .unwrap();

        let decision = table.decide(&message("thermostat.setpoint"));
        assert_eq!(decision.target.peer, Some(peer));
        assert_eq!(
            decision.matched_rule_name.as_deref(),
            Some("thermostat-peer")
        );
    }

    #[test]
    fn falls_back_to_local_action_driver() {
        let table = RouteTable::default();
        let decision = table.decide(&message("echo"));
        assert_eq!(decision.target.local_driver.as_deref(), Some("echo"));
    }

    #[test]
    fn rejects_ambiguous_targets() {
        let error = RouteTable::new(vec![RouteRule {
            name: Some("bad".to_string()),
            description: None,
            requires_peer_grant: None,
            matches: RouteMatch::default(),
            target: RouteTarget {
                local_driver: Some("echo".to_string()),
                drop: true,
                ..RouteTarget::default()
            },
        }])
        .unwrap_err();
        assert_eq!(
            error,
            RivunRouterError::InvalidTarget {
                route: "bad".to_string()
            }
        );
    }

    #[test]
    fn rejects_peer_grant_requirement_without_peer_target() {
        let error = RouteTable::new(vec![RouteRule {
            name: Some("bad-grant".to_string()),
            description: None,
            requires_peer_grant: Some(CapabilityId::new("driver.execute:echo").unwrap()),
            matches: RouteMatch::default(),
            target: RouteTarget::local_driver("echo"),
        }])
        .unwrap_err();
        assert_eq!(
            error,
            RivunRouterError::InvalidPeerGrantRequirement("bad-grant".to_string())
        );
    }
}
