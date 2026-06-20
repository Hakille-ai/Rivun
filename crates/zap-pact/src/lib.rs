//! ZAP PACT profile contracts.
//!
//! A PACT is a portable signed action record intended to travel inside a
//! `ZENV` envelope. The profile reuses ZAP identity, BLAKE3 hashing, and
//! Ed25519 domain signatures instead of defining a parallel trust stack.

use base64::{Engine as _, engine::general_purpose::STANDARD_NO_PAD};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use std::collections::BTreeMap;
use thiserror::Error;
use uuid::Uuid;
use zap_crypto::{Keypair, PublicKey, ZapCryptoError};

pub const PACT_SCHEMA_VERSION: u8 = 1;
pub const PACT_CONTENT_TYPE: &str = "application/zap-pact+json";
pub const PACT_RECORD_SUBJECT: &str = "zap.pact.record";
pub const PACT_VERIFY_SUBJECT: &str = "zap.pact.verify";
pub const PACT_REVOKE_SUBJECT: &str = "zap.pact.revoke";
pub const PACT_BUNDLE_SUBJECT: &str = "zap.pact.bundle";
pub const PACT_SIGNATURE_DOMAIN: &[u8] = b"ZAP-PACT-v1";
pub const PACT_REVOCATION_SIGNATURE_DOMAIN: &[u8] = b"ZAP-PACT-REVOCATION-v1";
pub const PACT_HASH_PREFIX: &str = "blake3:";

const ED25519_SIGNATURE_LEN: usize = 64;
const ED25519_PUBLIC_KEY_LEN: usize = 32;
const MAX_TEXT_BYTES: usize = 16 * 1024;

pub type Result<T> = std::result::Result<T, ZapPactError>;

#[derive(Debug, Error)]
pub enum ZapPactError {
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
    #[error("{entity} field `{field}` is required for verification")]
    MissingField {
        entity: &'static str,
        field: &'static str,
    },
    #[error("{entity} field `{field}` must match {expected}")]
    InvalidField {
        entity: &'static str,
        field: &'static str,
        expected: &'static str,
    },
    #[error("PACT hash mismatch: expected {expected}, got {actual}")]
    HashMismatch { expected: String, actual: String },
    #[error("PACT is expired")]
    Expired,
    #[error("PACT is revoked")]
    Revoked,
    #[error("PACT signature is invalid")]
    InvalidSignature,
    #[error("failed to decode base64 field `{field}`: {source}")]
    Base64 {
        field: &'static str,
        #[source]
        source: base64::DecodeError,
    },
    #[error("crypto error: {0}")]
    Crypto(#[from] ZapCryptoError),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

pub trait Validate {
    fn validate(&self) -> Result<()>;
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ZapPactStatus {
    Draft,
    #[default]
    Active,
    Expired,
    Revoked,
    Invalid,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ZapPactProof {
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub digest: Option<String>,
    #[serde(default)]
    pub evidence: Value,
    #[serde(default)]
    pub metadata: BTreeMap<String, Value>,
}

impl ZapPactProof {
    pub fn new(kind: impl Into<String>) -> Self {
        Self {
            kind: kind.into(),
            digest: None,
            evidence: Value::Null,
            metadata: BTreeMap::new(),
        }
    }
}

impl Validate for ZapPactProof {
    fn validate(&self) -> Result<()> {
        validate_text("pact_proof", "kind", &self.kind)?;
        if let Some(digest) = &self.digest {
            validate_text("pact_proof", "digest", digest)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ZapPact {
    pub schema_version: u8,
    pub pact_id: Uuid,
    pub actor: String,
    pub target: String,
    pub intent: String,
    #[serde(default)]
    pub object: Value,
    #[serde(default)]
    pub terms: Value,
    #[serde(default)]
    pub consent: Value,
    #[serde(default)]
    pub proof: Value,
    pub created_at_micros: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires_at_micros: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub actor_public_key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hash: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signature: Option<String>,
    #[serde(default)]
    pub status: ZapPactStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verification: Option<ZapPactVerification>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revocation: Option<ZapPactRevocation>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub timeline: Vec<ZapPactTimelineEntry>,
}

impl ZapPact {
    pub fn new(
        actor: impl Into<String>,
        target: impl Into<String>,
        intent: impl Into<String>,
        created_at_micros: u64,
    ) -> Self {
        Self {
            schema_version: PACT_SCHEMA_VERSION,
            pact_id: Uuid::new_v4(),
            actor: actor.into(),
            target: target.into(),
            intent: intent.into(),
            object: Value::Null,
            terms: Value::Null,
            consent: Value::Null,
            proof: Value::Null,
            created_at_micros,
            expires_at_micros: None,
            actor_public_key: None,
            hash: None,
            signature: None,
            status: ZapPactStatus::Draft,
            verification: None,
            revocation: None,
            timeline: Vec::new(),
        }
    }

    pub fn signing_payload(&self) -> Value {
        serde_json::to_value(self.signing_payload_ordered()).expect("PACT signing payload is JSON")
    }

    pub fn canonical_signing_bytes(&self) -> Result<Vec<u8>> {
        Ok(serde_json::to_vec(&self.signing_payload_ordered())?)
    }

    pub fn canonical_hash(&self) -> Result<String> {
        Ok(hash_bytes(&self.canonical_signing_bytes()?))
    }

    pub fn sign(&mut self, keypair: &Keypair) -> Result<()> {
        self.validate()?;
        let bytes = self.canonical_signing_bytes()?;
        let signature = keypair.sign_domain_message(PACT_SIGNATURE_DOMAIN, &bytes);
        let public_key = keypair.verifying_key();
        self.actor_public_key = Some(STANDARD_NO_PAD.encode(public_key.to_bytes()));
        self.hash = Some(hash_bytes(&bytes));
        self.signature = Some(STANDARD_NO_PAD.encode(signature));
        self.status = ZapPactStatus::Active;
        Ok(())
    }

    pub fn verify(&self, now_micros: Option<u64>) -> Result<ZapPactVerification> {
        self.validate()?;
        if self.status == ZapPactStatus::Revoked || self.revocation.is_some() {
            return Err(ZapPactError::Revoked);
        }
        if self.expires_at_micros.zip(now_micros).is_some_and(|(expires, now)| now > expires) {
            return Err(ZapPactError::Expired);
        }
        let expected_hash = self.canonical_hash()?;
        let actual_hash = self.hash.as_ref().ok_or(ZapPactError::MissingField {
            entity: "pact",
            field: "hash",
        })?;
        if actual_hash != &expected_hash {
            return Err(ZapPactError::HashMismatch {
                expected: expected_hash,
                actual: actual_hash.clone(),
            });
        }
        let public_key = self.public_key()?;
        let signature = decode_fixed::<ED25519_SIGNATURE_LEN>(
            self.signature
                .as_deref()
                .ok_or(ZapPactError::MissingField {
                    entity: "pact",
                    field: "signature",
                })?,
            "signature",
        )?;
        public_key
            .verify_domain_message(
                PACT_SIGNATURE_DOMAIN,
                &self.canonical_signing_bytes()?,
                &signature,
            )
            .map_err(|_| ZapPactError::InvalidSignature)?;
        Ok(ZapPactVerification {
            schema_version: PACT_SCHEMA_VERSION,
            pact_id: self.pact_id,
            valid: true,
            status: self.status,
            hash: actual_hash.clone(),
            verified_at_micros: now_micros,
            reason: None,
        })
    }

    pub fn public_key(&self) -> Result<PublicKey> {
        let public_key = self
            .actor_public_key
            .as_deref()
            .ok_or(ZapPactError::MissingField {
                entity: "pact",
                field: "actor_public_key",
            })?;
        let bytes = decode_fixed::<ED25519_PUBLIC_KEY_LEN>(public_key, "actor_public_key")?;
        PublicKey::from_bytes(bytes).map_err(Into::into)
    }

    fn signing_payload_ordered(&self) -> ZapPactSigningPayload<'_> {
        ZapPactSigningPayload {
            pact_id: self.pact_id,
            actor: self.actor.as_str(),
            target: self.target.as_str(),
            intent: self.intent.as_str(),
            object: normalize_json_value(&self.object),
            terms: normalize_json_value(&self.terms),
            consent: normalize_json_value(&self.consent),
            proof: normalize_json_value(&self.proof),
            created_at_micros: self.created_at_micros,
            expires_at_micros: self.expires_at_micros,
        }
    }
}

#[derive(Debug, Serialize)]
struct ZapPactSigningPayload<'a> {
    pact_id: Uuid,
    actor: &'a str,
    target: &'a str,
    intent: &'a str,
    object: Value,
    terms: Value,
    consent: Value,
    proof: Value,
    created_at_micros: u64,
    expires_at_micros: Option<u64>,
}

impl Validate for ZapPact {
    fn validate(&self) -> Result<()> {
        validate_schema_version("pact", self.schema_version)?;
        validate_text("pact", "actor", &self.actor)?;
        validate_text("pact", "target", &self.target)?;
        validate_text("pact", "intent", &self.intent)?;
        if self.expires_at_micros.is_some_and(|expires| expires <= self.created_at_micros) {
            return Err(ZapPactError::InvalidField {
                entity: "pact",
                field: "expires_at_micros",
                expected: "a timestamp greater than created_at_micros",
            });
        }
        if let Some(hash) = &self.hash {
            validate_hash("pact", "hash", hash)?;
        }
        if let Some(public_key) = &self.actor_public_key {
            decode_fixed::<ED25519_PUBLIC_KEY_LEN>(public_key, "actor_public_key")?;
        }
        if let Some(signature) = &self.signature {
            decode_fixed::<ED25519_SIGNATURE_LEN>(signature, "signature")?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ZapPactVerification {
    pub schema_version: u8,
    pub pact_id: Uuid,
    pub valid: bool,
    pub status: ZapPactStatus,
    pub hash: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verified_at_micros: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

impl Validate for ZapPactVerification {
    fn validate(&self) -> Result<()> {
        validate_schema_version("pact_verification", self.schema_version)?;
        validate_hash("pact_verification", "hash", &self.hash)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ZapPactRevocation {
    pub schema_version: u8,
    pub pact_id: Uuid,
    pub revoked_by: String,
    pub reason: String,
    pub revoked_at_micros: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signature: Option<String>,
}

impl ZapPactRevocation {
    pub fn new(
        pact_id: Uuid,
        revoked_by: impl Into<String>,
        reason: impl Into<String>,
        revoked_at_micros: u64,
    ) -> Self {
        Self {
            schema_version: PACT_SCHEMA_VERSION,
            pact_id,
            revoked_by: revoked_by.into(),
            reason: reason.into(),
            revoked_at_micros,
            signature: None,
        }
    }

    pub fn signing_bytes(&self) -> Result<Vec<u8>> {
        let payload = json!({
            "pact_id": self.pact_id,
            "revoked_by": self.revoked_by,
            "reason": self.reason,
            "revoked_at_micros": self.revoked_at_micros,
        });
        Ok(serde_json::to_vec(&payload)?)
    }

    pub fn sign(&mut self, keypair: &Keypair) -> Result<()> {
        self.validate()?;
        let signature =
            keypair.sign_domain_message(PACT_REVOCATION_SIGNATURE_DOMAIN, &self.signing_bytes()?);
        self.signature = Some(STANDARD_NO_PAD.encode(signature));
        Ok(())
    }
}

impl Validate for ZapPactRevocation {
    fn validate(&self) -> Result<()> {
        validate_schema_version("pact_revocation", self.schema_version)?;
        validate_text("pact_revocation", "revoked_by", &self.revoked_by)?;
        validate_text("pact_revocation", "reason", &self.reason)?;
        if let Some(signature) = &self.signature {
            decode_fixed::<ED25519_SIGNATURE_LEN>(signature, "signature")?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ZapPactTimelineEntry {
    pub at_micros: u64,
    pub status: ZapPactStatus,
    pub note: String,
}

impl Validate for ZapPactTimelineEntry {
    fn validate(&self) -> Result<()> {
        validate_text("pact_timeline_entry", "note", &self.note)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ZapPactBundle {
    pub schema_version: u8,
    pub pact: ZapPact,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub verifications: Vec<ZapPactVerification>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub revocations: Vec<ZapPactRevocation>,
    #[serde(default)]
    pub metadata: BTreeMap<String, Value>,
}

impl ZapPactBundle {
    pub fn new(pact: ZapPact) -> Self {
        Self {
            schema_version: PACT_SCHEMA_VERSION,
            pact,
            verifications: Vec::new(),
            revocations: Vec::new(),
            metadata: BTreeMap::new(),
        }
    }

    pub fn verify(&self, now_micros: Option<u64>) -> Result<ZapPactVerification> {
        self.validate()?;
        if self
            .revocations
            .iter()
            .any(|revocation| revocation.pact_id == self.pact.pact_id)
        {
            return Err(ZapPactError::Revoked);
        }
        self.pact.verify(now_micros)
    }
}

impl Validate for ZapPactBundle {
    fn validate(&self) -> Result<()> {
        validate_schema_version("pact_bundle", self.schema_version)?;
        self.pact.validate()?;
        for verification in &self.verifications {
            verification.validate()?;
            if verification.pact_id != self.pact.pact_id {
                return Err(ZapPactError::InvalidField {
                    entity: "pact_bundle",
                    field: "verifications[].pact_id",
                    expected: "bundle pact_id",
                });
            }
        }
        for revocation in &self.revocations {
            revocation.validate()?;
            if revocation.pact_id != self.pact.pact_id {
                return Err(ZapPactError::InvalidField {
                    entity: "pact_bundle",
                    field: "revocations[].pact_id",
                    expected: "bundle pact_id",
                });
            }
        }
        Ok(())
    }
}

pub fn normalize_json_value(value: &Value) -> Value {
    match value {
        Value::Array(items) => Value::Array(items.iter().map(normalize_json_value).collect()),
        Value::Object(map) => {
            let sorted = map
                .iter()
                .map(|(key, value)| (key.clone(), normalize_json_value(value)))
                .collect::<BTreeMap<_, _>>();
            Value::Object(sorted.into_iter().collect::<Map<_, _>>())
        }
        other => other.clone(),
    }
}

pub fn hash_bytes(bytes: &[u8]) -> String {
    format!("{PACT_HASH_PREFIX}{}", blake3::hash(bytes).to_hex())
}

pub fn pact_json_schema() -> Value {
    json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "title": "ZAP PACT v1",
        "type": "object",
        "required": [
            "schema_version",
            "pact_id",
            "actor",
            "target",
            "intent",
            "object",
            "terms",
            "consent",
            "proof",
            "created_at_micros",
            "hash",
            "signature",
            "status"
        ],
        "properties": {
            "schema_version": { "const": PACT_SCHEMA_VERSION },
            "pact_id": { "type": "string", "format": "uuid" },
            "actor": { "type": "string" },
            "target": { "type": "string" },
            "intent": { "type": "string" },
            "object": true,
            "terms": true,
            "consent": true,
            "proof": true,
            "created_at_micros": { "type": "integer", "minimum": 0 },
            "expires_at_micros": { "type": "integer", "minimum": 0 },
            "actor_public_key": { "type": "string" },
            "hash": { "type": "string", "pattern": "^blake3:[0-9a-f]{64}$" },
            "signature": { "type": "string" },
            "status": {
                "enum": ["draft", "active", "expired", "revoked", "invalid"]
            }
        }
    })
}

fn validate_schema_version(entity: &'static str, version: u8) -> Result<()> {
    if version != PACT_SCHEMA_VERSION {
        return Err(ZapPactError::UnsupportedSchemaVersion { entity, version });
    }
    Ok(())
}

fn validate_text(entity: &'static str, field: &'static str, value: &str) -> Result<()> {
    if value.trim().is_empty() {
        return Err(ZapPactError::EmptyField { entity, field });
    }
    if value.len() > MAX_TEXT_BYTES {
        return Err(ZapPactError::FieldTooLong {
            entity,
            field,
            max: MAX_TEXT_BYTES,
        });
    }
    Ok(())
}

fn validate_hash(entity: &'static str, field: &'static str, value: &str) -> Result<()> {
    if !value.starts_with(PACT_HASH_PREFIX)
        || value.len() != PACT_HASH_PREFIX.len() + 64
        || !value[PACT_HASH_PREFIX.len()..]
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return Err(ZapPactError::InvalidField {
            entity,
            field,
            expected: "blake3:<64 lowercase hex characters>",
        });
    }
    Ok(())
}

fn decode_fixed<const N: usize>(input: &str, field: &'static str) -> Result<[u8; N]> {
    let decoded = STANDARD_NO_PAD
        .decode(input)
        .map_err(|source| ZapPactError::Base64 { field, source })?;
    decoded
        .try_into()
        .map_err(|bytes: Vec<u8>| ZapPactError::InvalidField {
            entity: "pact",
            field,
            expected: if bytes.len() < N {
                "a longer base64-encoded byte string"
            } else {
                "a shorter base64-encoded byte string"
            },
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn sample_pact() -> ZapPact {
        let mut pact = ZapPact::new("agent.alpha", "driver.valve", "open_valve", 1_700_000);
        pact.pact_id = Uuid::parse_str("11111111-1111-4111-8111-111111111111").unwrap();
        pact.object = json!({"z": 1, "a": {"b": 2, "a": 1}});
        pact.terms = json!({"max_runtime_ms": 5000});
        pact.consent = json!({"operator": "ops", "approved": true});
        pact.proof = json!({"kind": "policy", "decision": "allow"});
        pact.expires_at_micros = Some(1_800_000);
        pact
    }

    #[test]
    fn canonical_payload_excludes_mutable_fields() {
        let mut pact = sample_pact();
        let before = pact.canonical_signing_bytes().unwrap();
        pact.status = ZapPactStatus::Revoked;
        pact.hash =
            Some("blake3:0000000000000000000000000000000000000000000000000000000000000000".into());
        pact.signature = Some(STANDARD_NO_PAD.encode([7u8; 64]));
        pact.verification = Some(ZapPactVerification {
            schema_version: PACT_SCHEMA_VERSION,
            pact_id: pact.pact_id,
            valid: false,
            status: ZapPactStatus::Invalid,
            hash: "blake3:1111111111111111111111111111111111111111111111111111111111111111".into(),
            verified_at_micros: Some(2),
            reason: Some("mutated".into()),
        });
        pact.timeline.push(ZapPactTimelineEntry {
            at_micros: 2,
            status: ZapPactStatus::Revoked,
            note: "revoked".into(),
        });
        assert_eq!(before, pact.canonical_signing_bytes().unwrap());
    }

    #[test]
    fn canonical_payload_uses_protocol_field_order() {
        let pact = sample_pact();
        let bytes = String::from_utf8(pact.canonical_signing_bytes().unwrap()).unwrap();
        assert!(bytes.starts_with(
            r#"{"pact_id":"11111111-1111-4111-8111-111111111111","actor":"agent.alpha","target":"driver.valve","intent":"open_valve","object":"#
        ));
        assert!(bytes.contains(
            r#","terms":{"max_runtime_ms":5000},"consent":{"approved":true,"operator":"ops"},"proof":{"decision":"allow","kind":"policy"},"created_at_micros":1700000,"expires_at_micros":1800000}"#
        ));
    }

    #[test]
    fn nested_json_keys_are_normalized_for_hashing() {
        let mut left = sample_pact();
        let mut right = sample_pact();
        left.object = json!({"z": 1, "a": {"b": 2, "a": 1}});
        right.object = json!({"a": {"a": 1, "b": 2}, "z": 1});
        assert_eq!(
            left.canonical_hash().unwrap(),
            right.canonical_hash().unwrap()
        );
    }

    #[test]
    fn signed_pact_verifies_offline() {
        let keypair = Keypair::generate();
        let mut pact = sample_pact();
        pact.sign(&keypair).unwrap();
        let verification = pact.verify(Some(1_750_000)).unwrap();
        assert!(verification.valid);
        assert_eq!(verification.pact_id, pact.pact_id);
    }

    #[test]
    fn tampered_terms_fail_verification() {
        let keypair = Keypair::generate();
        let mut pact = sample_pact();
        pact.sign(&keypair).unwrap();
        pact.terms = json!({"max_runtime_ms": 1});
        assert!(matches!(
            pact.verify(Some(1_750_000)),
            Err(ZapPactError::HashMismatch { .. })
        ));
    }

    #[test]
    fn expired_and_revoked_pacts_fail_verification() {
        let keypair = Keypair::generate();
        let mut pact = sample_pact();
        pact.sign(&keypair).unwrap();
        assert!(matches!(
            pact.verify(Some(1_900_000)),
            Err(ZapPactError::Expired)
        ));
        pact.revocation = Some(ZapPactRevocation::new(
            pact.pact_id,
            "ops",
            "stop",
            1_760_000,
        ));
        assert!(matches!(
            pact.verify(Some(1_760_001)),
            Err(ZapPactError::Revoked)
        ));
    }

    #[test]
    fn valid_signed_bundle_verifies_offline() {
        let keypair = Keypair::generate();
        let mut pact = sample_pact();
        pact.sign(&keypair).unwrap();
        let bundle = ZapPactBundle::new(pact);
        assert!(bundle.verify(Some(1_750_000)).unwrap().valid);
    }
}
