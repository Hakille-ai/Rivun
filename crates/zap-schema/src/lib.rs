//! Typed message contracts for ZAP envelopes.
//!
//! ZAP keeps model planning and machine intent outside the wire protocol. This
//! crate defines deterministic contracts that gateways, SDKs, operators, and
//! nodes can use to validate the `ZENV` messages those external systems emit.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{fmt, str::FromStr};
use thiserror::Error;
use zap_envelope::ZapMessageKind;

pub const MESSAGE_CONTRACT_SCHEMA_VERSION: u8 = 1;

#[derive(Debug, Error)]
pub enum ZapSchemaError {
    #[error("message contract schema version {0} is unsupported")]
    UnsupportedSchemaVersion(u8),
    #[error("message contract `{0}` kind must not be empty")]
    EmptyKind(String),
    #[error("message contract `{contract}` has invalid kind `{kind}`")]
    InvalidKind {
        contract: String,
        kind: String,
        #[source]
        source: zap_envelope::ZapEnvelopeError,
    },
    #[error("message contract `{0}` subject pattern must not be empty")]
    EmptySubject(String),
    #[error("message contract `{0}` content_type must not be empty when provided")]
    EmptyContentType(String),
    #[error("message contract `{0}` max_body_bytes must be greater than zero")]
    InvalidMaxBodyBytes(String),
    #[error("message body for `{contract}` exceeds max {max}: {actual}")]
    BodyTooLarge {
        contract: String,
        max: usize,
        actual: usize,
    },
    #[error("message metadata for `{contract}` exceeds max {max}: {actual}")]
    MetadataTooLarge {
        contract: String,
        max: usize,
        actual: usize,
    },
    #[error("message `{kind}` `{subject}` did not match any configured contract")]
    NoMatchingContract { kind: String, subject: String },
    #[error("message contract `{contract}` expected kind `{expected}`, got `{actual}`")]
    KindMismatch {
        contract: String,
        expected: String,
        actual: String,
    },
    #[error("message contract `{contract}` expected subject `{expected}`, got `{actual}`")]
    SubjectMismatch {
        contract: String,
        expected: String,
        actual: String,
    },
    #[error("message contract `{contract}` expected content_type `{expected}`, got `{actual}`")]
    ContentTypeMismatch {
        contract: String,
        expected: String,
        actual: String,
    },
    #[error("message contract `{contract}` expected an empty body")]
    BodyNotEmpty { contract: String },
    #[error("message contract `{contract}` expected UTF-8 body")]
    BodyNotUtf8 { contract: String },
    #[error("message contract `{contract}` expected JSON body")]
    BodyNotJson {
        contract: String,
        #[source]
        source: serde_json::Error,
    },
    #[error("message contract `{contract}` expected JSON object body")]
    BodyNotJsonObject { contract: String },
    #[error("message contract `{contract}` missing required body field `{field}`")]
    MissingBodyField { contract: String, field: String },
    #[error("message contract `{contract}` body field `{field}` is not allowed")]
    DisallowedBodyField { contract: String, field: String },
    #[error("message contract `{contract}` expected JSON metadata object")]
    MetadataNotJsonObject { contract: String },
    #[error("message contract `{contract}` metadata is not valid JSON")]
    MetadataNotJson {
        contract: String,
        #[source]
        source: serde_json::Error,
    },
    #[error("message contract `{contract}` missing required metadata field `{field}`")]
    MissingMetadataField { contract: String, field: String },
    #[error("failed to parse TOML message contract: {0}")]
    Toml(#[from] toml::de::Error),
    #[error("failed to parse JSON message contract: {0}")]
    Json(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, ZapSchemaError>;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MessageContract {
    pub schema_version: u8,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub kind: String,
    #[serde(default = "default_subject_pattern")]
    pub subject: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_body_bytes: Option<usize>,
    #[serde(default)]
    pub body: BodyContract,
    #[serde(default)]
    pub metadata: MetadataContract,
}

impl MessageContract {
    pub fn new(kind: impl Into<String>, subject: impl Into<String>) -> Self {
        Self {
            schema_version: MESSAGE_CONTRACT_SCHEMA_VERSION,
            name: None,
            kind: kind.into(),
            subject: subject.into(),
            content_type: None,
            max_body_bytes: None,
            body: BodyContract::default(),
            metadata: MetadataContract::default(),
        }
    }

    pub fn from_toml_str(input: &str) -> Result<Self> {
        let contract: Self = toml::from_str(input)?;
        contract.validate_static()?;
        Ok(contract)
    }

    pub fn from_json_str(input: &str) -> Result<Self> {
        let contract: Self = serde_json::from_str(input)?;
        contract.validate_static()?;
        Ok(contract)
    }

    pub fn contract_name(&self) -> String {
        self.name
            .clone()
            .unwrap_or_else(|| format!("{} {}", self.kind, self.subject))
    }

    pub fn validate_static(&self) -> Result<()> {
        let name = self.contract_name();
        if self.schema_version != MESSAGE_CONTRACT_SCHEMA_VERSION {
            return Err(ZapSchemaError::UnsupportedSchemaVersion(
                self.schema_version,
            ));
        }
        if self.kind.trim().is_empty() {
            return Err(ZapSchemaError::EmptyKind(name));
        }
        if self.kind != "*" {
            ZapMessageKind::from_str(&self.kind).map_err(|source| ZapSchemaError::InvalidKind {
                contract: name.clone(),
                kind: self.kind.clone(),
                source,
            })?;
        }
        if self.subject.trim().is_empty() {
            return Err(ZapSchemaError::EmptySubject(name));
        }
        if self
            .content_type
            .as_deref()
            .is_some_and(|content_type| content_type.trim().is_empty())
        {
            return Err(ZapSchemaError::EmptyContentType(name));
        }
        if matches!(self.max_body_bytes, Some(0)) {
            return Err(ZapSchemaError::InvalidMaxBodyBytes(name));
        }
        Ok(())
    }

    pub fn matches(&self, message: &MessageParts<'_>) -> bool {
        field_matches(&self.kind, message.kind)
            && subject_pattern_matches(&self.subject, message.subject)
            && self
                .content_type
                .as_deref()
                .map(|expected| {
                    message
                        .content_type
                        .is_some_and(|actual| expected.eq_ignore_ascii_case(actual))
                })
                .unwrap_or(true)
    }

    pub fn validate_message(&self, message: &MessageParts<'_>) -> Result<()> {
        self.validate_static()?;
        let name = self.contract_name();
        if !field_matches(&self.kind, message.kind) {
            return Err(ZapSchemaError::KindMismatch {
                contract: name,
                expected: self.kind.clone(),
                actual: message.kind.to_string(),
            });
        }
        let name = self.contract_name();
        if !subject_pattern_matches(&self.subject, message.subject) {
            return Err(ZapSchemaError::SubjectMismatch {
                contract: name,
                expected: self.subject.clone(),
                actual: message.subject.to_string(),
            });
        }
        if let Some(expected) = &self.content_type {
            let actual = message.content_type.unwrap_or("");
            if !expected.eq_ignore_ascii_case(actual) {
                return Err(ZapSchemaError::ContentTypeMismatch {
                    contract: self.contract_name(),
                    expected: expected.clone(),
                    actual: actual.to_string(),
                });
            }
        }
        if let Some(max) = self.max_body_bytes
            && message.body.len() > max
        {
            return Err(ZapSchemaError::BodyTooLarge {
                contract: self.contract_name(),
                max,
                actual: message.body.len(),
            });
        }
        self.body.validate(&self.contract_name(), message.body)?;
        self.metadata
            .validate(&self.contract_name(), message.metadata)?;
        Ok(())
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct MessageContractSet {
    #[serde(default)]
    pub require_match: bool,
    #[serde(default)]
    pub contracts: Vec<MessageContract>,
}

impl MessageContractSet {
    pub fn new(require_match: bool, contracts: Vec<MessageContract>) -> Result<Self> {
        let set = Self {
            require_match,
            contracts,
        };
        set.validate_static()?;
        Ok(set)
    }

    pub fn from_toml_str(input: &str) -> Result<Self> {
        let set: Self = toml::from_str(input)?;
        set.validate_static()?;
        Ok(set)
    }

    pub fn validate_static(&self) -> Result<()> {
        for contract in &self.contracts {
            contract.validate_static()?;
        }
        Ok(())
    }

    pub fn validate_message(&self, message: &MessageParts<'_>) -> Result<Option<String>> {
        if let Some(contract) = self
            .contracts
            .iter()
            .find(|contract| contract.matches(message))
        {
            contract.validate_message(message)?;
            return Ok(Some(contract.contract_name()));
        }
        if self.require_match {
            return Err(ZapSchemaError::NoMatchingContract {
                kind: message.kind.to_string(),
                subject: message.subject.to_string(),
            });
        }
        Ok(None)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BodyContract {
    #[serde(default)]
    pub format: BodyFormat,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub required_json_fields: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub allowed_json_fields: Vec<String>,
}

impl Default for BodyContract {
    fn default() -> Self {
        Self {
            format: BodyFormat::Any,
            required_json_fields: Vec::new(),
            allowed_json_fields: Vec::new(),
        }
    }
}

impl BodyContract {
    fn validate(&self, contract: &str, body: &[u8]) -> Result<()> {
        match self.format {
            BodyFormat::Any => {
                if self.required_json_fields.is_empty() && self.allowed_json_fields.is_empty() {
                    return Ok(());
                }
                self.validate_json_object(contract, body)
            }
            BodyFormat::Empty => {
                if body.is_empty() {
                    Ok(())
                } else {
                    Err(ZapSchemaError::BodyNotEmpty {
                        contract: contract.to_string(),
                    })
                }
            }
            BodyFormat::Utf8 => {
                std::str::from_utf8(body)
                    .map(|_| ())
                    .map_err(|_| ZapSchemaError::BodyNotUtf8 {
                        contract: contract.to_string(),
                    })
            }
            BodyFormat::JsonValue => {
                serde_json::from_slice::<Value>(body)
                    .map(|_| ())
                    .map_err(|source| ZapSchemaError::BodyNotJson {
                        contract: contract.to_string(),
                        source,
                    })
            }
            BodyFormat::JsonObject => self.validate_json_object(contract, body),
        }
    }

    fn validate_json_object(&self, contract: &str, body: &[u8]) -> Result<()> {
        let value = serde_json::from_slice::<Value>(body).map_err(|source| {
            ZapSchemaError::BodyNotJson {
                contract: contract.to_string(),
                source,
            }
        })?;
        let object = value
            .as_object()
            .ok_or_else(|| ZapSchemaError::BodyNotJsonObject {
                contract: contract.to_string(),
            })?;
        for field in &self.required_json_fields {
            if !object.contains_key(field) {
                return Err(ZapSchemaError::MissingBodyField {
                    contract: contract.to_string(),
                    field: field.clone(),
                });
            }
        }
        if !self.allowed_json_fields.is_empty() {
            for field in object.keys() {
                if !self
                    .allowed_json_fields
                    .iter()
                    .any(|allowed| allowed == field)
                {
                    return Err(ZapSchemaError::DisallowedBodyField {
                        contract: contract.to_string(),
                        field: field.clone(),
                    });
                }
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BodyFormat {
    #[default]
    Any,
    Empty,
    Utf8,
    JsonValue,
    JsonObject,
}

impl fmt::Display for BodyFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Any => "any",
            Self::Empty => "empty",
            Self::Utf8 => "utf8",
            Self::JsonValue => "json_value",
            Self::JsonObject => "json_object",
        })
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct MetadataContract {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_bytes: Option<usize>,
    #[serde(default)]
    pub json_object: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub required_json_fields: Vec<String>,
}

impl MetadataContract {
    fn validate(&self, contract: &str, metadata: &[u8]) -> Result<()> {
        if let Some(max) = self.max_bytes
            && metadata.len() > max
        {
            return Err(ZapSchemaError::MetadataTooLarge {
                contract: contract.to_string(),
                max,
                actual: metadata.len(),
            });
        }
        if !self.json_object && self.required_json_fields.is_empty() {
            return Ok(());
        }
        let value = serde_json::from_slice::<Value>(metadata).map_err(|source| {
            ZapSchemaError::MetadataNotJson {
                contract: contract.to_string(),
                source,
            }
        })?;
        let object = value
            .as_object()
            .ok_or_else(|| ZapSchemaError::MetadataNotJsonObject {
                contract: contract.to_string(),
            })?;
        for field in &self.required_json_fields {
            if !object.contains_key(field) {
                return Err(ZapSchemaError::MissingMetadataField {
                    contract: contract.to_string(),
                    field: field.clone(),
                });
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MessageParts<'a> {
    pub kind: &'a str,
    pub subject: &'a str,
    pub content_type: Option<&'a str>,
    pub metadata: &'a [u8],
    pub body: &'a [u8],
}

fn default_subject_pattern() -> String {
    "*".to_string()
}

fn field_matches(pattern: &str, value: &str) -> bool {
    pattern == "*" || pattern.eq_ignore_ascii_case(value)
}

fn subject_pattern_matches(pattern: &str, value: &str) -> bool {
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

    fn parts(body: &'static [u8]) -> MessageParts<'static> {
        MessageParts {
            kind: "action",
            subject: "thermostat.setpoint",
            content_type: Some("application/json"),
            metadata: br#"{"source":"agent"}"#,
            body,
        }
    }

    #[test]
    fn validates_json_object_contract() {
        let contract = MessageContract {
            schema_version: 1,
            name: Some("setpoint".to_string()),
            kind: "action".to_string(),
            subject: "thermostat.*".to_string(),
            content_type: Some("application/json".to_string()),
            max_body_bytes: Some(128),
            body: BodyContract {
                format: BodyFormat::JsonObject,
                required_json_fields: vec!["temperature_c".to_string()],
                allowed_json_fields: vec!["temperature_c".to_string()],
            },
            metadata: MetadataContract {
                max_bytes: Some(64),
                json_object: true,
                required_json_fields: vec!["source".to_string()],
            },
        };

        contract
            .validate_message(&parts(br#"{"temperature_c":20}"#))
            .unwrap();
    }

    #[test]
    fn rejects_missing_required_body_field() {
        let mut contract = MessageContract::new("action", "thermostat.setpoint");
        contract.body.format = BodyFormat::JsonObject;
        contract
            .body
            .required_json_fields
            .push("temperature_c".to_string());

        let error = contract
            .validate_message(&parts(br#"{"c":20}"#))
            .unwrap_err();
        assert!(format!("{error}").contains("missing required body field"));
    }

    #[test]
    fn contract_set_can_require_a_match() {
        let set =
            MessageContractSet::new(true, vec![MessageContract::new("event", "sensor.*")]).unwrap();
        let error = set
            .validate_message(&parts(br#"{"temperature_c":20}"#))
            .unwrap_err();
        assert!(matches!(error, ZapSchemaError::NoMatchingContract { .. }));
    }

    #[test]
    fn parses_toml_contract() {
        let contract = MessageContract::from_toml_str(
            r#"
schema_version = 1
name = "machine command"
kind = "command"
subject = "machine.*"
content_type = "application/json"
max_body_bytes = 4096

[body]
format = "json_object"
required_json_fields = ["device_id"]
"#,
        )
        .unwrap();

        assert_eq!(contract.kind, "command");
        assert_eq!(contract.body.format, BodyFormat::JsonObject);
    }
}
