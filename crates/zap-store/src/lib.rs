//! Signed driver manifests for the local ZapStore foundation.
//!
//! A manifest binds one driver action to one WASM/WAT artifact hash, an ABI
//! version, declared permissions, and an Ed25519 author identity. The signature
//! is over a deterministic JSON payload so TOML files can be distributed and
//! verified offline.

use base64::{Engine as _, engine::general_purpose::STANDARD_NO_PAD};
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;
use zap_crypto::{Keypair, node_id_from_public_key};
use zap_runtime::DriverPermissions;

pub const MANIFEST_SCHEMA_VERSION: u8 = 1;
pub const REGISTRY_SCHEMA_VERSION: u8 = 1;
pub const DRIVER_ABI_VERSION: u16 = 1;
pub const DRIVER_HASH_PREFIX: &str = "blake3:";

const MANIFEST_SIGNATURE_DOMAIN: &[u8] = b"ZAP-DRIVER-MANIFEST-v1";
const REGISTRY_SIGNATURE_DOMAIN: &[u8] = b"ZAP-DRIVER-REGISTRY-v1";
const PUBLIC_KEY_LEN: usize = 32;
const SIGNATURE_LEN: usize = 64;

#[derive(Debug, Error)]
pub enum ZapStoreError {
    #[error("driver manifest schema version {0} is unsupported")]
    UnsupportedSchemaVersion(u8),
    #[error("driver ABI version {0} is unsupported")]
    UnsupportedAbiVersion(u16),
    #[error("driver manifest action must not be empty")]
    EmptyAction,
    #[error("driver registry schema version {0} is unsupported")]
    UnsupportedRegistrySchemaVersion(u8),
    #[error("driver registry entry `{action}` version `{version}` is duplicated")]
    DuplicateRegistryEntry { action: String, version: String },
    #[error("driver registry has no entry for action `{action}` version `{version}`")]
    MissingRegistryEntry { action: String, version: String },
    #[error("driver registry entry `{action}` version `{version}` is revoked: {reason}")]
    RevokedRegistryEntry {
        action: String,
        version: String,
        reason: String,
    },
    #[error("driver registry field `{field}` mismatch: expected {expected}, actual {actual}")]
    RegistryFieldMismatch {
        field: &'static str,
        expected: String,
        actual: String,
    },
    #[error("driver registry is not signed")]
    MissingRegistrySignature,
    #[error(
        "registry operator public key derives node_id {derived}, but registry declares {declared}"
    )]
    RegistryOperatorNodeMismatch { declared: Uuid, derived: Uuid },
    #[error("registry signature verification failed")]
    InvalidRegistrySignature,
    #[error(
        "driver manifest action `{manifest_action}` does not match configured action `{configured_action}`"
    )]
    ActionMismatch {
        manifest_action: String,
        configured_action: String,
    },
    #[error("driver hash mismatch: manifest expected {expected}, actual {actual}")]
    HashMismatch { expected: String, actual: String },
    #[error("driver hash `{0}` is not a blake3 hash")]
    InvalidHash(String),
    #[error(
        "manifest author public key derives node_id {derived}, but manifest declares {declared}"
    )]
    AuthorNodeMismatch { declared: Uuid, derived: Uuid },
    #[error("manifest signature verification failed")]
    InvalidSignature,
    #[error("invalid manifest key material length for {kind}: expected {expected}, got {actual}")]
    InvalidKeyLength {
        kind: &'static str,
        expected: usize,
        actual: usize,
    },
    #[error("failed to decode base64 manifest key material: {0}")]
    Base64(#[from] base64::DecodeError),
    #[error("failed to parse Ed25519 manifest key material: {0}")]
    Ed25519(#[from] ed25519_dalek::SignatureError),
    #[error("failed to serialize signing payload: {0}")]
    Json(#[from] serde_json::Error),
    #[error("failed to parse TOML driver manifest: {0}")]
    TomlDecode(#[from] toml::de::Error),
    #[error("failed to serialize TOML driver manifest: {0}")]
    TomlEncode(#[from] toml::ser::Error),
}

pub type Result<T> = std::result::Result<T, ZapStoreError>;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DriverManifest {
    pub schema_version: u8,
    pub name: String,
    pub version: String,
    pub action: String,
    pub abi_version: u16,
    pub wasm_hash: String,
    #[serde(default)]
    pub permissions: DriverPermissions,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub author_node_id: Uuid,
    pub author_public_key: String,
    pub signature: String,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DriverRegistryStatus {
    #[default]
    Active,
    Revoked,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DriverRegistryEntry {
    pub name: String,
    pub version: String,
    pub action: String,
    pub abi_version: u16,
    pub wasm_hash: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub manifest_path: Option<String>,
    pub author_node_id: Uuid,
    #[serde(default)]
    pub status: DriverRegistryStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revoked_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DriverRegistry {
    pub schema_version: u8,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub generated_by: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub operator_node_id: Option<Uuid>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub operator_public_key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signature: Option<String>,
    #[serde(default)]
    pub entries: Vec<DriverRegistryEntry>,
}

impl DriverManifest {
    pub fn new(
        name: impl Into<String>,
        version: impl Into<String>,
        action: impl Into<String>,
        wasm: &[u8],
        permissions: DriverPermissions,
        description: Option<String>,
        author: &Keypair,
    ) -> Result<Self> {
        let author_public_key = author.verifying_key().to_bytes();
        let mut manifest = Self {
            schema_version: MANIFEST_SCHEMA_VERSION,
            name: name.into(),
            version: version.into(),
            action: action.into(),
            abi_version: DRIVER_ABI_VERSION,
            wasm_hash: driver_hash(wasm),
            permissions,
            description,
            author_node_id: author.node_id(),
            author_public_key: STANDARD_NO_PAD.encode(author_public_key),
            signature: String::new(),
        };
        manifest.validate_static_fields()?;

        let signing_key = SigningKey::from_bytes(&author.secret_bytes());
        let signature: Signature = signing_key.sign(&manifest.signing_message()?);
        manifest.signature = STANDARD_NO_PAD.encode(signature.to_bytes());
        Ok(manifest)
    }

    pub fn from_toml_str(input: &str) -> Result<Self> {
        Ok(toml::from_str(input)?)
    }

    pub fn to_toml_string(&self) -> Result<String> {
        Ok(toml::to_string_pretty(self)?)
    }

    pub fn verify_for_driver(&self, configured_action: &str, wasm: &[u8]) -> Result<()> {
        self.verify_static_and_signature()?;
        if self.action != configured_action {
            return Err(ZapStoreError::ActionMismatch {
                manifest_action: self.action.clone(),
                configured_action: configured_action.to_string(),
            });
        }

        let actual = driver_hash(wasm);
        if self.wasm_hash != actual {
            return Err(ZapStoreError::HashMismatch {
                expected: self.wasm_hash.clone(),
                actual,
            });
        }
        Ok(())
    }

    pub fn verify_static_and_signature(&self) -> Result<()> {
        self.validate_static_fields()?;
        let public_key_bytes =
            decode_fixed::<PUBLIC_KEY_LEN>(&self.author_public_key, "public_key")?;
        let derived_node_id = node_id_from_public_key(&public_key_bytes);
        if derived_node_id != self.author_node_id {
            return Err(ZapStoreError::AuthorNodeMismatch {
                declared: self.author_node_id,
                derived: derived_node_id,
            });
        }

        let verifying_key = VerifyingKey::from_bytes(&public_key_bytes)?;
        let signature_bytes = decode_fixed::<SIGNATURE_LEN>(&self.signature, "signature")?;
        let signature = Signature::from_bytes(&signature_bytes);
        verifying_key
            .verify(&self.signing_message()?, &signature)
            .map_err(|_| ZapStoreError::InvalidSignature)
    }

    fn validate_static_fields(&self) -> Result<()> {
        if self.schema_version != MANIFEST_SCHEMA_VERSION {
            return Err(ZapStoreError::UnsupportedSchemaVersion(self.schema_version));
        }
        if self.abi_version != DRIVER_ABI_VERSION {
            return Err(ZapStoreError::UnsupportedAbiVersion(self.abi_version));
        }
        if self.action.trim().is_empty() {
            return Err(ZapStoreError::EmptyAction);
        }
        validate_driver_hash(&self.wasm_hash)
    }

    fn signing_message(&self) -> Result<Vec<u8>> {
        let payload = ManifestSigningPayload {
            schema_version: self.schema_version,
            name: &self.name,
            version: &self.version,
            action: &self.action,
            abi_version: self.abi_version,
            wasm_hash: &self.wasm_hash,
            permissions: self.permissions,
            description: self.description.as_deref(),
            author_node_id: self.author_node_id,
            author_public_key: &self.author_public_key,
        };
        let encoded = serde_json::to_vec(&payload)?;
        let mut message = Vec::with_capacity(MANIFEST_SIGNATURE_DOMAIN.len() + encoded.len());
        message.extend_from_slice(MANIFEST_SIGNATURE_DOMAIN);
        message.extend_from_slice(&encoded);
        Ok(message)
    }
}

impl DriverRegistry {
    pub fn empty(generated_by: Option<String>) -> Self {
        Self {
            schema_version: REGISTRY_SCHEMA_VERSION,
            generated_by,
            operator_node_id: None,
            operator_public_key: None,
            signature: None,
            entries: Vec::new(),
        }
    }

    pub fn from_toml_str(input: &str) -> Result<Self> {
        Ok(toml::from_str(input)?)
    }

    pub fn to_toml_string(&self) -> Result<String> {
        Ok(toml::to_string_pretty(self)?)
    }

    pub fn add_manifest(
        &mut self,
        manifest: &DriverManifest,
        manifest_path: Option<String>,
    ) -> Result<()> {
        self.validate()?;
        manifest.verify_static_and_signature()?;
        self.entries.retain(|entry| {
            !(entry.action == manifest.action && entry.version == manifest.version)
        });
        self.entries
            .push(DriverRegistryEntry::from_manifest(manifest, manifest_path));
        self.entries.sort_by(|left, right| {
            left.action
                .cmp(&right.action)
                .then_with(|| left.version.cmp(&right.version))
        });
        self.validate()
    }

    pub fn validate(&self) -> Result<()> {
        if self.schema_version != REGISTRY_SCHEMA_VERSION {
            return Err(ZapStoreError::UnsupportedRegistrySchemaVersion(
                self.schema_version,
            ));
        }

        let mut seen = std::collections::HashSet::new();
        for entry in &self.entries {
            if entry.action.trim().is_empty() {
                return Err(ZapStoreError::EmptyAction);
            }
            validate_driver_hash(&entry.wasm_hash)?;
            if !seen.insert((entry.action.clone(), entry.version.clone())) {
                return Err(ZapStoreError::DuplicateRegistryEntry {
                    action: entry.action.clone(),
                    version: entry.version.clone(),
                });
            }
        }
        Ok(())
    }

    pub fn verify_manifest(&self, manifest: &DriverManifest) -> Result<()> {
        self.validate()?;
        manifest.verify_static_and_signature()?;
        let entry = self
            .entries
            .iter()
            .find(|entry| entry.action == manifest.action && entry.version == manifest.version)
            .ok_or_else(|| ZapStoreError::MissingRegistryEntry {
                action: manifest.action.clone(),
                version: manifest.version.clone(),
            })?;

        if entry.status == DriverRegistryStatus::Revoked {
            return Err(ZapStoreError::RevokedRegistryEntry {
                action: entry.action.clone(),
                version: entry.version.clone(),
                reason: entry
                    .revoked_reason
                    .clone()
                    .unwrap_or_else(|| "no reason provided".to_string()),
            });
        }

        compare_registry_field("name", &entry.name, &manifest.name)?;
        compare_registry_field("abi_version", entry.abi_version, manifest.abi_version)?;
        compare_registry_field("wasm_hash", &entry.wasm_hash, &manifest.wasm_hash)?;
        compare_registry_field(
            "author_node_id",
            entry.author_node_id,
            manifest.author_node_id,
        )?;
        Ok(())
    }

    pub fn sign(&mut self, operator: &Keypair) -> Result<()> {
        self.validate()?;
        self.operator_node_id = Some(operator.node_id());
        self.operator_public_key =
            Some(STANDARD_NO_PAD.encode(operator.verifying_key().to_bytes()));
        self.signature = None;

        let signing_key = SigningKey::from_bytes(&operator.secret_bytes());
        let signature: Signature = signing_key.sign(&self.signing_message()?);
        self.signature = Some(STANDARD_NO_PAD.encode(signature.to_bytes()));
        Ok(())
    }

    pub fn verify_signature(&self) -> Result<()> {
        self.validate()?;
        let operator_node_id = self
            .operator_node_id
            .ok_or(ZapStoreError::MissingRegistrySignature)?;
        let operator_public_key = self
            .operator_public_key
            .as_deref()
            .ok_or(ZapStoreError::MissingRegistrySignature)?;
        let signature = self
            .signature
            .as_deref()
            .ok_or(ZapStoreError::MissingRegistrySignature)?;

        let public_key_bytes =
            decode_fixed::<PUBLIC_KEY_LEN>(operator_public_key, "registry_public_key")?;
        let derived_node_id = node_id_from_public_key(&public_key_bytes);
        if derived_node_id != operator_node_id {
            return Err(ZapStoreError::RegistryOperatorNodeMismatch {
                declared: operator_node_id,
                derived: derived_node_id,
            });
        }

        let verifying_key = VerifyingKey::from_bytes(&public_key_bytes)?;
        let signature_bytes = decode_fixed::<SIGNATURE_LEN>(signature, "registry_signature")?;
        let signature = Signature::from_bytes(&signature_bytes);
        verifying_key
            .verify(&self.signing_message()?, &signature)
            .map_err(|_| ZapStoreError::InvalidRegistrySignature)
    }

    fn signing_message(&self) -> Result<Vec<u8>> {
        let payload = RegistrySigningPayload {
            schema_version: self.schema_version,
            generated_by: self.generated_by.as_deref(),
            operator_node_id: self
                .operator_node_id
                .ok_or(ZapStoreError::MissingRegistrySignature)?,
            operator_public_key: self
                .operator_public_key
                .as_deref()
                .ok_or(ZapStoreError::MissingRegistrySignature)?,
            entries: &self.entries,
        };
        let encoded = serde_json::to_vec(&payload)?;
        let mut message = Vec::with_capacity(REGISTRY_SIGNATURE_DOMAIN.len() + encoded.len());
        message.extend_from_slice(REGISTRY_SIGNATURE_DOMAIN);
        message.extend_from_slice(&encoded);
        Ok(message)
    }
}

impl DriverRegistryEntry {
    pub fn from_manifest(manifest: &DriverManifest, manifest_path: Option<String>) -> Self {
        Self {
            name: manifest.name.clone(),
            version: manifest.version.clone(),
            action: manifest.action.clone(),
            abi_version: manifest.abi_version,
            wasm_hash: manifest.wasm_hash.clone(),
            manifest_path,
            author_node_id: manifest.author_node_id,
            status: DriverRegistryStatus::Active,
            revoked_reason: None,
        }
    }
}

#[derive(Debug, Serialize)]
struct ManifestSigningPayload<'a> {
    schema_version: u8,
    name: &'a str,
    version: &'a str,
    action: &'a str,
    abi_version: u16,
    wasm_hash: &'a str,
    permissions: DriverPermissions,
    description: Option<&'a str>,
    author_node_id: Uuid,
    author_public_key: &'a str,
}

#[derive(Debug, Serialize)]
struct RegistrySigningPayload<'a> {
    schema_version: u8,
    generated_by: Option<&'a str>,
    operator_node_id: Uuid,
    operator_public_key: &'a str,
    entries: &'a [DriverRegistryEntry],
}

pub fn driver_hash(wasm: &[u8]) -> String {
    format!("{DRIVER_HASH_PREFIX}{}", blake3::hash(wasm).to_hex())
}

fn validate_driver_hash(hash: &str) -> Result<()> {
    if !hash.starts_with(DRIVER_HASH_PREFIX)
        || hash.len() != DRIVER_HASH_PREFIX.len() + 64
        || !hash[DRIVER_HASH_PREFIX.len()..]
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit())
    {
        return Err(ZapStoreError::InvalidHash(hash.to_string()));
    }
    Ok(())
}

fn compare_registry_field<T>(field: &'static str, expected: T, actual: T) -> Result<()>
where
    T: PartialEq + ToString,
{
    if expected != actual {
        return Err(ZapStoreError::RegistryFieldMismatch {
            field,
            expected: expected.to_string(),
            actual: actual.to_string(),
        });
    }
    Ok(())
}

fn decode_fixed<const N: usize>(encoded: &str, kind: &'static str) -> Result<[u8; N]> {
    let decoded = STANDARD_NO_PAD.decode(encoded)?;
    if decoded.len() != N {
        return Err(ZapStoreError::InvalidKeyLength {
            kind,
            expected: N,
            actual: decoded.len(),
        });
    }
    Ok(decoded.try_into().unwrap())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wasm() -> &'static [u8] {
        b"(module (memory (export \"memory\") 1))"
    }

    #[test]
    fn manifest_signs_and_verifies_driver() {
        let author = Keypair::generate();
        let manifest = DriverManifest::new(
            "echo",
            "0.1.0",
            "echo",
            wasm(),
            DriverPermissions::none(),
            Some("Echo driver".to_string()),
            &author,
        )
        .unwrap();

        manifest.verify_for_driver("echo", wasm()).unwrap();
        assert_eq!(manifest.author_node_id, author.node_id());
        assert_eq!(manifest.abi_version, DRIVER_ABI_VERSION);
    }

    #[test]
    fn manifest_round_trips_toml() {
        let author = Keypair::generate();
        let manifest = DriverManifest::new(
            "echo",
            "0.1.0",
            "echo",
            wasm(),
            DriverPermissions::none(),
            None,
            &author,
        )
        .unwrap();
        let encoded = manifest.to_toml_string().unwrap();
        let decoded = DriverManifest::from_toml_str(&encoded).unwrap();

        assert_eq!(decoded, manifest);
        decoded.verify_for_driver("echo", wasm()).unwrap();
    }

    #[test]
    fn registry_adds_manifest_and_round_trips_toml() {
        let author = Keypair::generate();
        let manifest = DriverManifest::new(
            "echo",
            "0.1.0",
            "echo",
            wasm(),
            DriverPermissions::none(),
            None,
            &author,
        )
        .unwrap();
        let mut registry = DriverRegistry::empty(Some("test".to_string()));
        registry
            .add_manifest(&manifest, Some("echo.manifest.toml".to_string()))
            .unwrap();

        registry.verify_manifest(&manifest).unwrap();
        assert_eq!(registry.entries.len(), 1);
        assert_eq!(
            registry.entries[0].manifest_path.as_deref(),
            Some("echo.manifest.toml")
        );

        let encoded = registry.to_toml_string().unwrap();
        let decoded = DriverRegistry::from_toml_str(&encoded).unwrap();
        assert_eq!(decoded, registry);
    }

    #[test]
    fn registry_signs_and_verifies_index() {
        let author = Keypair::generate();
        let operator = Keypair::generate();
        let manifest = DriverManifest::new(
            "echo",
            "0.1.0",
            "echo",
            wasm(),
            DriverPermissions::none(),
            None,
            &author,
        )
        .unwrap();
        let mut registry = DriverRegistry::empty(Some("test".to_string()));
        registry.add_manifest(&manifest, None).unwrap();
        registry.sign(&operator).unwrap();

        registry.verify_signature().unwrap();
        assert_eq!(registry.operator_node_id, Some(operator.node_id()));
        assert!(registry.operator_public_key.is_some());
        assert!(registry.signature.is_some());

        let encoded = registry.to_toml_string().unwrap();
        let decoded = DriverRegistry::from_toml_str(&encoded).unwrap();
        decoded.verify_signature().unwrap();
    }

    #[test]
    fn registry_signature_rejects_entry_mutation() {
        let author = Keypair::generate();
        let operator = Keypair::generate();
        let manifest = DriverManifest::new(
            "echo",
            "0.1.0",
            "echo",
            wasm(),
            DriverPermissions::none(),
            None,
            &author,
        )
        .unwrap();
        let mut registry = DriverRegistry::empty(None);
        registry.add_manifest(&manifest, None).unwrap();
        registry.sign(&operator).unwrap();
        registry.entries[0].wasm_hash = driver_hash(b"tampered");

        assert!(matches!(
            registry.verify_signature(),
            Err(ZapStoreError::InvalidRegistrySignature)
        ));
    }

    #[test]
    fn registry_signature_is_required_when_verified() {
        let registry = DriverRegistry::empty(None);

        assert!(matches!(
            registry.verify_signature(),
            Err(ZapStoreError::MissingRegistrySignature)
        ));
    }

    #[test]
    fn registry_rejects_revoked_manifest() {
        let author = Keypair::generate();
        let manifest = DriverManifest::new(
            "echo",
            "0.1.0",
            "echo",
            wasm(),
            DriverPermissions::none(),
            None,
            &author,
        )
        .unwrap();
        let mut registry = DriverRegistry::empty(None);
        registry.add_manifest(&manifest, None).unwrap();
        registry.entries[0].status = DriverRegistryStatus::Revoked;
        registry.entries[0].revoked_reason = Some("superseded".to_string());

        assert!(matches!(
            registry.verify_manifest(&manifest),
            Err(ZapStoreError::RevokedRegistryEntry { .. })
        ));
    }

    #[test]
    fn registry_detects_manifest_mismatch() {
        let author = Keypair::generate();
        let manifest = DriverManifest::new(
            "echo",
            "0.1.0",
            "echo",
            wasm(),
            DriverPermissions::none(),
            None,
            &author,
        )
        .unwrap();
        let mut registry = DriverRegistry::empty(None);
        registry.add_manifest(&manifest, None).unwrap();
        registry.entries[0].wasm_hash = driver_hash(b"different");

        assert!(matches!(
            registry.verify_manifest(&manifest),
            Err(ZapStoreError::RegistryFieldMismatch {
                field: "wasm_hash",
                ..
            })
        ));
    }

    #[test]
    fn hash_mismatch_rejects_tampered_driver() {
        let author = Keypair::generate();
        let manifest = DriverManifest::new(
            "echo",
            "0.1.0",
            "echo",
            wasm(),
            DriverPermissions::none(),
            None,
            &author,
        )
        .unwrap();

        assert!(matches!(
            manifest.verify_for_driver("echo", b"tampered"),
            Err(ZapStoreError::HashMismatch { .. })
        ));
    }

    #[test]
    fn action_mismatch_rejects_wrong_config() {
        let author = Keypair::generate();
        let manifest = DriverManifest::new(
            "echo",
            "0.1.0",
            "echo",
            wasm(),
            DriverPermissions::none(),
            None,
            &author,
        )
        .unwrap();

        assert!(matches!(
            manifest.verify_for_driver("thermostat.setpoint", wasm()),
            Err(ZapStoreError::ActionMismatch { .. })
        ));
    }

    #[test]
    fn signature_rejects_mutated_manifest_fields() {
        let author = Keypair::generate();
        let mut manifest = DriverManifest::new(
            "echo",
            "0.1.0",
            "echo",
            wasm(),
            DriverPermissions::none(),
            None,
            &author,
        )
        .unwrap();
        manifest.version = "9.9.9".to_string();

        assert!(matches!(
            manifest.verify_static_and_signature(),
            Err(ZapStoreError::InvalidSignature)
        ));
    }
}
