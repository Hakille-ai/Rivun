//! Local intent compiler for ZAP.
//!
//! This crate is intentionally deterministic. It is the auditable Phase 2
//! foundation from the PDF: natural-language or agent-originated intent becomes
//! typed action steps before any ZAP frame is emitted.

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use thiserror::Error;

pub const COMPILER_ID: &str = "zap-intent-rule-v1";

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ZapIntentError {
    #[error("intent is empty")]
    EmptyIntent,
    #[error("unsupported intent: {0}")]
    UnsupportedIntent(String),
    #[error("JSON intent must contain a string `action` field")]
    MissingJsonAction,
    #[error("JSON intent must contain a string `subject` field when `action` is omitted")]
    MissingJsonSubject,
    #[error("intent policy denied step `{subject}`: {reason}")]
    PolicyDenied { subject: String, reason: String },
    #[error("failed to serialize intent payload: {0}")]
    Json(String),
}

pub type Result<T> = std::result::Result<T, ZapIntentError>;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IntentPlan {
    pub original: String,
    pub compiler: String,
    pub steps: Vec<IntentStep>,
    pub notes: Vec<String>,
}

impl IntentPlan {
    pub fn new(original: impl Into<String>, steps: Vec<IntentStep>) -> Self {
        Self {
            original: original.into(),
            compiler: COMPILER_ID.to_string(),
            steps,
            notes: Vec::new(),
        }
    }

    pub fn apply_policy(&mut self, policy: &IntentPolicy) -> Result<IntentPolicyReport> {
        policy.apply(self)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IntentStep {
    pub kind: String,
    pub subject: String,
    pub action: String,
    pub content_type: String,
    pub payload: String,
    pub payload_format: PayloadFormat,
    pub requires_consensus: bool,
    pub rationale: String,
}

impl IntentStep {
    pub fn text(
        action: impl Into<String>,
        payload: impl Into<String>,
        requires_consensus: bool,
        rationale: impl Into<String>,
    ) -> Self {
        let action = action.into();
        Self {
            kind: "action".to_string(),
            subject: action.clone(),
            action,
            content_type: PayloadFormat::Text.content_type().to_string(),
            payload: payload.into(),
            payload_format: PayloadFormat::Text,
            requires_consensus,
            rationale: rationale.into(),
        }
    }

    pub fn json(
        action: impl Into<String>,
        payload: Value,
        requires_consensus: bool,
        rationale: impl Into<String>,
    ) -> Result<Self> {
        let action = action.into();
        Ok(Self {
            kind: "action".to_string(),
            subject: action.clone(),
            action,
            content_type: PayloadFormat::Json.content_type().to_string(),
            payload: serde_json::to_string(&payload)
                .map_err(|err| ZapIntentError::Json(err.to_string()))?,
            payload_format: PayloadFormat::Json,
            requires_consensus,
            rationale: rationale.into(),
        })
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PayloadFormat {
    Text,
    Json,
}

impl PayloadFormat {
    pub const fn content_type(self) -> &'static str {
        match self {
            Self::Text => "text/plain",
            Self::Json => "application/json",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IntentRule {
    pub id: String,
    pub description: String,
    pub subjects: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IntentExplanation {
    pub compiler: String,
    pub normalized: String,
    pub rules: Vec<IntentRule>,
    pub plan: IntentPlan,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum IntentPolicyDecision {
    #[default]
    Allow,
    Deny,
    RequirePoa,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IntentPolicyRule {
    #[serde(default)]
    pub kind: Option<String>,
    #[serde(default)]
    pub subject: Option<String>,
    #[serde(default)]
    pub action: Option<String>,
    pub decision: IntentPolicyDecision,
    #[serde(default)]
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IntentPolicy {
    #[serde(default)]
    pub default_decision: IntentPolicyDecision,
    #[serde(default)]
    pub rules: Vec<IntentPolicyRule>,
}

impl Default for IntentPolicy {
    fn default() -> Self {
        Self {
            default_decision: IntentPolicyDecision::Allow,
            rules: Vec::new(),
        }
    }
}

impl IntentPolicy {
    pub fn apply(&self, plan: &mut IntentPlan) -> Result<IntentPolicyReport> {
        let mut decisions = Vec::with_capacity(plan.steps.len());
        for step in &mut plan.steps {
            let matched = self.rules.iter().find(|rule| rule.matches(step));
            let decision = matched
                .map(|rule| rule.decision)
                .unwrap_or(self.default_decision);
            let reason = matched
                .and_then(|rule| rule.reason.clone())
                .unwrap_or_else(|| "default policy decision".to_string());

            match decision {
                IntentPolicyDecision::Allow => {}
                IntentPolicyDecision::RequirePoa => {
                    if !step.requires_consensus {
                        step.requires_consensus = true;
                        step.rationale = format!("{}; policy requires PoA", step.rationale);
                    }
                }
                IntentPolicyDecision::Deny => {
                    return Err(ZapIntentError::PolicyDenied {
                        subject: step.subject.clone(),
                        reason,
                    });
                }
            }

            decisions.push(IntentStepPolicyDecision {
                kind: step.kind.clone(),
                subject: step.subject.clone(),
                action: step.action.clone(),
                decision,
                reason,
                requires_consensus: step.requires_consensus,
            });
        }

        if decisions
            .iter()
            .any(|decision| decision.decision == IntentPolicyDecision::RequirePoa)
        {
            plan.notes
                .push("intent policy required Proof-of-Action for one or more steps".to_string());
        }

        Ok(IntentPolicyReport { decisions })
    }
}

impl IntentPolicyRule {
    fn matches(&self, step: &IntentStep) -> bool {
        field_matches(self.kind.as_deref(), &step.kind)
            && field_matches(self.subject.as_deref(), &step.subject)
            && field_matches(self.action.as_deref(), &step.action)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IntentPolicyReport {
    pub decisions: Vec<IntentStepPolicyDecision>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IntentStepPolicyDecision {
    pub kind: String,
    pub subject: String,
    pub action: String,
    pub decision: IntentPolicyDecision,
    pub reason: String,
    pub requires_consensus: bool,
}

pub fn compile_intent(input: &str) -> Result<IntentPlan> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(ZapIntentError::EmptyIntent);
    }

    if let Some(plan) = compile_json_intent(trimmed)? {
        return Ok(plan);
    }

    let normalized = normalize(trimmed);
    let mut steps = Vec::new();
    if let Some(step) = compile_echo(trimmed, &normalized) {
        steps.push(step);
    }
    if let Some(step) = compile_thermostat(&normalized)? {
        steps.push(step);
    }
    if let Some(step) = compile_safety_critical(&normalized)? {
        steps.push(step);
    }

    if steps.is_empty() {
        return Err(ZapIntentError::UnsupportedIntent(trimmed.to_string()));
    }

    let mut plan = IntentPlan::new(trimmed, steps);
    if plan.steps.len() > 1 {
        plan.notes
            .push("intent expanded into multiple ZAP action steps".to_string());
    }
    Ok(plan)
}

pub fn explain_intent(input: &str) -> Result<IntentExplanation> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(ZapIntentError::EmptyIntent);
    }
    let plan = compile_intent(trimmed)?;
    Ok(IntentExplanation {
        compiler: COMPILER_ID.to_string(),
        normalized: normalize(trimmed),
        rules: vec![
            IntentRule {
                id: "echo".to_string(),
                description: "echo, say, or dire prefix becomes an echo action".to_string(),
                subjects: vec!["echo".to_string()],
            },
            IntentRule {
                id: "thermostat".to_string(),
                description: "temperature wording plus first number becomes thermostat.setpoint"
                    .to_string(),
                subjects: vec!["thermostat.setpoint".to_string()],
            },
            IntentRule {
                id: "safety_stop".to_string(),
                description: "emergency stop wording becomes a consensus safety action".to_string(),
                subjects: vec!["safety.emergency_stop".to_string()],
            },
            IntentRule {
                id: "structured_json".to_string(),
                description: "structured JSON intent maps directly to one typed step".to_string(),
                subjects: plan.steps.iter().map(|step| step.subject.clone()).collect(),
            },
        ],
        plan,
    })
}

fn compile_json_intent(input: &str) -> Result<Option<IntentPlan>> {
    if !input.starts_with('{') {
        return Ok(None);
    }
    let value = match serde_json::from_str::<Value>(input) {
        Ok(value) => value,
        Err(_) => return Ok(None),
    };
    let action = value.get("action").and_then(Value::as_str);
    let subject = value
        .get("subject")
        .and_then(Value::as_str)
        .or(action)
        .ok_or(ZapIntentError::MissingJsonSubject)?;
    let kind = value
        .get("kind")
        .and_then(Value::as_str)
        .unwrap_or("action");
    let requires_consensus = value
        .get("requires_consensus")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let payload = match value.get("payload") {
        Some(Value::String(payload)) => payload.clone(),
        Some(payload) => {
            serde_json::to_string(payload).map_err(|err| ZapIntentError::Json(err.to_string()))?
        }
        None => String::new(),
    };
    let payload_format = match value.get("payload") {
        Some(Value::String(_)) | None => PayloadFormat::Text,
        Some(_) => PayloadFormat::Json,
    };
    let content_type = value
        .get("content_type")
        .and_then(Value::as_str)
        .unwrap_or(payload_format.content_type());
    Ok(Some(IntentPlan::new(
        input,
        vec![IntentStep {
            kind: kind.to_string(),
            subject: subject.to_string(),
            action: action.unwrap_or(subject).to_string(),
            content_type: content_type.to_string(),
            payload,
            payload_format,
            requires_consensus,
            rationale: "structured JSON intent".to_string(),
        }],
    )))
}

fn compile_echo(original: &str, normalized: &str) -> Option<IntentStep> {
    for prefix in ["echo ", "say ", "dire "] {
        if normalized.starts_with(prefix) {
            return Some(IntentStep::text(
                "echo",
                original[prefix.len()..].trim(),
                false,
                "explicit echo intent",
            ));
        }
    }
    None
}

fn compile_thermostat(normalized: &str) -> Result<Option<IntentStep>> {
    if !(normalized.contains("thermostat") || normalized.contains("temp")) {
        return Ok(None);
    }
    let Some(temperature_c) = first_number(normalized) else {
        return Ok(None);
    };
    Ok(Some(IntentStep::json(
        "thermostat.setpoint",
        json!({ "temperature_c": temperature_c }),
        false,
        "temperature setpoint intent",
    )?))
}

fn compile_safety_critical(normalized: &str) -> Result<Option<IntentStep>> {
    let emergency_stop = normalized.contains("emergency stop")
        || normalized.contains("critical stop")
        || normalized.contains("safety stop")
        || normalized.contains("arret urgence")
        || normalized.contains("arret d urgence")
        || (normalized.contains("urgence")
            && (normalized.contains("stop") || normalized.contains("arret")));
    if !emergency_stop {
        return Ok(None);
    }

    Ok(Some(IntentStep::json(
        "safety.emergency_stop",
        json!({
            "reason": "operator_request"
        }),
        true,
        "critical safety intent requires Proof-of-Action validation",
    )?))
}

fn normalize(input: &str) -> String {
    input
        .trim()
        .chars()
        .flat_map(char::to_lowercase)
        .map(fold_latin_diacritic)
        .map(|ch| match ch {
            ',' => '.',
            'a'..='z' | '0'..='9' | '.' | '-' | '+' => ch,
            _ if ch.is_whitespace() => ' ',
            _ => ' ',
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn fold_latin_diacritic(ch: char) -> char {
    match ch {
        '\u{00E0}' | '\u{00E1}' | '\u{00E2}' | '\u{00E3}' | '\u{00E4}' | '\u{00E5}' => 'a',
        '\u{00E7}' => 'c',
        '\u{00E8}' | '\u{00E9}' | '\u{00EA}' | '\u{00EB}' => 'e',
        '\u{00EC}' | '\u{00ED}' | '\u{00EE}' | '\u{00EF}' => 'i',
        '\u{00F1}' => 'n',
        '\u{00F2}' | '\u{00F3}' | '\u{00F4}' | '\u{00F5}' | '\u{00F6}' => 'o',
        '\u{00F9}' | '\u{00FA}' | '\u{00FB}' | '\u{00FC}' => 'u',
        '\u{00FD}' | '\u{00FF}' => 'y',
        other => other,
    }
}

fn first_number(input: &str) -> Option<f64> {
    let mut start = None;
    let mut end = 0;
    for (index, ch) in input.char_indices() {
        let is_number_char = ch.is_ascii_digit() || ch == '.' || ch == '-' || ch == '+';
        if is_number_char {
            if start.is_none() {
                start = Some(index);
            }
            end = index + ch.len_utf8();
        } else if let Some(number_start) = start {
            if let Ok(value) = input[number_start..end].parse::<f64>() {
                return Some(value);
            }
            start = None;
        }
    }
    start.and_then(|start| input[start..end].parse::<f64>().ok())
}

fn field_matches(pattern: Option<&str>, value: &str) -> bool {
    match pattern {
        None => true,
        Some("*") => true,
        Some(pattern) => pattern == value,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compiles_echo_intent() {
        let plan = compile_intent("echo hello zap").unwrap();

        assert_eq!(plan.steps.len(), 1);
        assert_eq!(plan.steps[0].kind, "action");
        assert_eq!(plan.steps[0].subject, "echo");
        assert_eq!(plan.steps[0].action, "echo");
        assert_eq!(plan.steps[0].content_type, "text/plain");
        assert_eq!(plan.steps[0].payload, "hello zap");
        assert!(!plan.steps[0].requires_consensus);
    }

    #[test]
    fn compiles_thermostat_intent() {
        let plan = compile_intent("Ajuster la temperature de la piece a 20 degres").unwrap();
        let payload: Value = serde_json::from_str(&plan.steps[0].payload).unwrap();

        assert_eq!(plan.steps[0].action, "thermostat.setpoint");
        assert_eq!(plan.steps[0].subject, "thermostat.setpoint");
        assert_eq!(payload["temperature_c"], 20.0);
        assert_eq!(plan.steps[0].payload_format, PayloadFormat::Json);
        assert_eq!(plan.steps[0].content_type, "application/json");
    }

    #[test]
    fn compiles_safety_stop_as_consensus_step() {
        let plan = compile_intent("declencher arret urgence robot").unwrap();
        let payload: Value = serde_json::from_str(&plan.steps[0].payload).unwrap();

        assert_eq!(plan.steps[0].action, "safety.emergency_stop");
        assert_eq!(payload["reason"], "operator_request");
        assert!(plan.steps[0].requires_consensus);
    }

    #[test]
    fn compiles_accented_french_safety_stop() {
        let plan = compile_intent("D\u{00E9}clencher arr\u{00EA}t d'urgence robot").unwrap();
        let payload: Value = serde_json::from_str(&plan.steps[0].payload).unwrap();

        assert_eq!(plan.steps[0].action, "safety.emergency_stop");
        assert_eq!(payload["reason"], "operator_request");
        assert!(plan.steps[0].requires_consensus);
    }

    #[test]
    fn explains_intent_with_rule_metadata() {
        let explanation = explain_intent("Ajuster la temperature a 19").unwrap();

        assert_eq!(explanation.compiler, COMPILER_ID);
        assert_eq!(explanation.normalized, "ajuster la temperature a 19");
        assert_eq!(explanation.plan.steps[0].subject, "thermostat.setpoint");
        assert!(explanation.rules.iter().any(|rule| rule.id == "thermostat"));
    }

    #[test]
    fn policy_can_require_poa_for_matching_subject() {
        let mut plan = compile_intent("Ajuster la temperature a 20").unwrap();
        let report = plan
            .apply_policy(&IntentPolicy {
                default_decision: IntentPolicyDecision::Allow,
                rules: vec![IntentPolicyRule {
                    kind: Some("action".to_string()),
                    subject: Some("thermostat.setpoint".to_string()),
                    action: None,
                    decision: IntentPolicyDecision::RequirePoa,
                    reason: Some("thermostat changes require operator approval".to_string()),
                }],
            })
            .unwrap();

        assert!(plan.steps[0].requires_consensus);
        assert_eq!(
            report.decisions[0].decision,
            IntentPolicyDecision::RequirePoa
        );
        assert_eq!(
            report.decisions[0].reason,
            "thermostat changes require operator approval"
        );
    }

    #[test]
    fn policy_can_deny_matching_subject() {
        let mut plan = compile_intent("echo hello").unwrap();
        let error = plan
            .apply_policy(&IntentPolicy {
                default_decision: IntentPolicyDecision::Allow,
                rules: vec![IntentPolicyRule {
                    kind: None,
                    subject: Some("echo".to_string()),
                    action: None,
                    decision: IntentPolicyDecision::Deny,
                    reason: Some("echo disabled".to_string()),
                }],
            })
            .unwrap_err();

        assert_eq!(
            error,
            ZapIntentError::PolicyDenied {
                subject: "echo".to_string(),
                reason: "echo disabled".to_string()
            }
        );
    }

    #[test]
    fn compiles_pdf_style_multi_step_intent() {
        let plan = compile_intent(
            "Ajuster la temperature de la piece a 20 et declencher arret urgence robot",
        )
        .unwrap();

        assert_eq!(plan.steps.len(), 2);
        assert_eq!(plan.steps[0].action, "thermostat.setpoint");
        assert_eq!(plan.steps[1].action, "safety.emergency_stop");
        assert!(plan.steps[1].requires_consensus);
        assert_eq!(plan.notes.len(), 1);
    }

    #[test]
    fn compiles_structured_json_intent() {
        let plan = compile_intent(
            r#"{"action":"robot.stop","payload":{"reason":"safety"},"requires_consensus":true}"#,
        )
        .unwrap();

        assert_eq!(plan.steps[0].action, "robot.stop");
        assert_eq!(plan.steps[0].kind, "action");
        assert_eq!(plan.steps[0].subject, "robot.stop");
        assert_eq!(plan.steps[0].payload, r#"{"reason":"safety"}"#);
        assert_eq!(plan.steps[0].payload_format, PayloadFormat::Json);
        assert!(plan.steps[0].requires_consensus);
    }

    #[test]
    fn compiles_structured_json_universal_event() {
        let plan = compile_intent(
            r#"{"kind":"event","subject":"sensor.temperature","payload":{"c":21.5},"content_type":"application/json"}"#,
        )
        .unwrap();

        assert_eq!(plan.steps[0].kind, "event");
        assert_eq!(plan.steps[0].subject, "sensor.temperature");
        assert_eq!(plan.steps[0].action, "sensor.temperature");
        assert_eq!(plan.steps[0].payload, r#"{"c":21.5}"#);
        assert_eq!(plan.steps[0].content_type, "application/json");
        assert!(!plan.steps[0].requires_consensus);
    }

    #[test]
    fn rejects_empty_and_unknown_intents() {
        assert!(matches!(
            compile_intent(" "),
            Err(ZapIntentError::EmptyIntent)
        ));
        assert!(matches!(
            compile_intent("make the impossible thing"),
            Err(ZapIntentError::UnsupportedIntent(_))
        ));
    }
}
