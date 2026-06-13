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
use zap_capability::DriverPermissions;
use zap_crypto::{Keypair, node_id_from_public_key};

pub const MANIFEST_SCHEMA_VERSION: u8 = 1;
pub const REGISTRY_SCHEMA_VERSION: u8 = 1;
pub const REGISTRY_INDEX_SYNC_SCHEMA_VERSION: u8 = 1;
pub const REGISTRY_PUBLICATION_SCHEMA_VERSION: u8 = 1;
pub const REGISTRY_BUNDLE_SCHEMA_VERSION: u8 = 1;
pub const DRIVER_ABI_VERSION: u16 = 1;
pub const DRIVER_HASH_PREFIX: &str = "blake3:";
pub const REGISTRY_INDEX_CONTENT_TYPE: &str = "application/zap-registry-index+json";
pub const REGISTRY_INDEX_REQUEST_SUBJECT: &str = "zap.registry.index.request";
pub const REGISTRY_INDEX_RESPONSE_SUBJECT: &str = "zap.registry.index.response";

const MANIFEST_SIGNATURE_DOMAIN: &[u8] = b"ZAP-DRIVER-MANIFEST-v1";
const REGISTRY_SIGNATURE_DOMAIN: &[u8] = b"ZAP-DRIVER-REGISTRY-v1";
const REGISTRY_PUBLICATION_SIGNATURE_DOMAIN: &[u8] = b"ZAP-DRIVER-REGISTRY-PUBLICATION-v1";
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
    #[error("driver registry index sync schema version {0} is unsupported")]
    UnsupportedRegistryIndexSyncSchemaVersion(u8),
    #[error("driver registry publication schema version {0} is unsupported")]
    UnsupportedRegistryPublicationSchemaVersion(u8),
    #[error("driver registry bundle schema version {0} is unsupported")]
    UnsupportedRegistryBundleSchemaVersion(u8),
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
    #[error("driver registry merge conflict for `{action}` version `{version}` field `{field}`")]
    RegistryMergeConflict {
        action: String,
        version: String,
        field: &'static str,
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
    #[error("registry operator public key mismatch: expected node_id {expected}, actual {actual}")]
    RegistryOperatorPublicKeyMismatch { expected: Uuid, actual: Uuid },
    #[error(
        "registry publication publisher public key derives node_id {derived}, but publication declares {declared}"
    )]
    RegistryPublicationPublisherNodeMismatch { declared: Uuid, derived: Uuid },
    #[error(
        "registry publication publisher mismatch: expected node_id {expected}, actual {actual}"
    )]
    RegistryPublicationPublisherPublicKeyMismatch { expected: Uuid, actual: Uuid },
    #[error("registry publication field `{field}` mismatch")]
    RegistryPublicationFieldMismatch { field: &'static str },
    #[error("registry publication signature verification failed")]
    InvalidRegistryPublicationSignature,
    #[error("registry bundle path `{0}` is invalid")]
    InvalidRegistryBundlePath(String),
    #[error("registry bundle publication path/hash metadata is incomplete")]
    RegistryBundlePublicationMetadataIncomplete,
    #[error(
        "registry bundle entry `{action}` version `{version}` has incomplete {artifact} path/hash metadata"
    )]
    RegistryBundleArtifactMetadataIncomplete {
        action: String,
        version: String,
        artifact: &'static str,
    },
    #[error(
        "registry bundle entry `{action}` version `{version}` driver hash does not match registry wasm hash"
    )]
    RegistryBundleDriverHashMismatch { action: String, version: String },
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RegistryIndexRequest {
    pub schema_version: u8,
    #[serde(default)]
    pub require_signature: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RegistryIndexResponse {
    pub schema_version: u8,
    pub node_id: Uuid,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub registry: Option<DriverRegistry>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unavailable_reason: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct DriverRegistryMergeReport {
    pub added: usize,
    pub unchanged: usize,
    pub revoked_overrides: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RegistryPublication {
    pub schema_version: u8,
    pub registry_hash: String,
    pub registry_entries: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub registry_operator_node_id: Option<Uuid>,
    pub published_at_micros: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub channel: Option<String>,
    #[serde(default)]
    pub labels: Vec<String>,
    pub publisher_node_id: Uuid,
    pub publisher_public_key: String,
    pub signature: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RegistryBundleManifest {
    pub schema_version: u8,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub generated_by: Option<String>,
    pub registry_path: String,
    pub registry_hash: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub publication_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub publication_hash: Option<String>,
    #[serde(default)]
    pub entries: Vec<RegistryBundleEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RegistryBundleEntry {
    pub action: String,
    pub version: String,
    pub name: String,
    pub abi_version: u16,
    pub wasm_hash: String,
    pub author_node_id: Uuid,
    pub status: DriverRegistryStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub manifest_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub manifest_hash: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub driver_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub driver_hash: Option<String>,
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
        self.clear_signature();
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

    pub fn revoke(&mut self, action: &str, version: &str, reason: impl Into<String>) -> Result<()> {
        self.validate()?;
        {
            let entry = self
                .entries
                .iter_mut()
                .find(|entry| entry.action == action && entry.version == version)
                .ok_or_else(|| ZapStoreError::MissingRegistryEntry {
                    action: action.to_string(),
                    version: version.to_string(),
                })?;
            entry.status = DriverRegistryStatus::Revoked;
            entry.revoked_reason = Some(reason.into());
        }
        self.clear_signature();
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

    pub fn merge_from(&mut self, other: &DriverRegistry) -> Result<DriverRegistryMergeReport> {
        self.validate()?;
        other.validate()?;
        let mut report = DriverRegistryMergeReport::default();
        let mut changed = false;
        for incoming in &other.entries {
            match self.entries.iter().position(|entry| {
                entry.action == incoming.action && entry.version == incoming.version
            }) {
                Some(index) => {
                    let existing = &mut self.entries[index];
                    validate_registry_merge_compatibility(existing, incoming)?;
                    if existing.status == DriverRegistryStatus::Revoked
                        || incoming.status == DriverRegistryStatus::Active
                    {
                        report.unchanged += 1;
                        continue;
                    }
                    *existing = incoming.clone();
                    report.revoked_overrides += 1;
                    changed = true;
                }
                None => {
                    self.entries.push(incoming.clone());
                    report.added += 1;
                    changed = true;
                }
            }
        }
        if changed {
            self.clear_signature();
            self.entries.sort_by(|left, right| {
                left.action
                    .cmp(&right.action)
                    .then_with(|| left.version.cmp(&right.version))
            });
        }
        self.validate()?;
        Ok(report)
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

    pub fn verify_signature_for_operator(&self, expected_operator_public_key: &str) -> Result<()> {
        self.verify_signature()?;
        let declared_public_key = self
            .operator_public_key
            .as_deref()
            .ok_or(ZapStoreError::MissingRegistrySignature)?;
        let expected_bytes =
            decode_fixed::<PUBLIC_KEY_LEN>(expected_operator_public_key, "expected_public_key")?;
        let declared_bytes =
            decode_fixed::<PUBLIC_KEY_LEN>(declared_public_key, "registry_operator_public_key")?;
        if declared_bytes != expected_bytes {
            return Err(ZapStoreError::RegistryOperatorPublicKeyMismatch {
                expected: node_id_from_public_key(&expected_bytes),
                actual: node_id_from_public_key(&declared_bytes),
            });
        }
        Ok(())
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

    fn clear_signature(&mut self) {
        self.operator_node_id = None;
        self.operator_public_key = None;
        self.signature = None;
    }
}

impl Default for RegistryIndexRequest {
    fn default() -> Self {
        Self {
            schema_version: REGISTRY_INDEX_SYNC_SCHEMA_VERSION,
            require_signature: false,
        }
    }
}

impl RegistryIndexRequest {
    pub fn validate(&self) -> Result<()> {
        if self.schema_version != REGISTRY_INDEX_SYNC_SCHEMA_VERSION {
            return Err(ZapStoreError::UnsupportedRegistryIndexSyncSchemaVersion(
                self.schema_version,
            ));
        }
        Ok(())
    }
}

impl RegistryIndexResponse {
    pub fn new(
        node_id: Uuid,
        registry: Option<DriverRegistry>,
        unavailable_reason: Option<String>,
    ) -> Self {
        Self {
            schema_version: REGISTRY_INDEX_SYNC_SCHEMA_VERSION,
            node_id,
            registry,
            unavailable_reason,
        }
    }

    pub fn verify(
        &self,
        require_signature: bool,
        expected_operator_public_key: Option<&str>,
    ) -> Result<()> {
        if self.schema_version != REGISTRY_INDEX_SYNC_SCHEMA_VERSION {
            return Err(ZapStoreError::UnsupportedRegistryIndexSyncSchemaVersion(
                self.schema_version,
            ));
        }
        if let Some(registry) = &self.registry {
            registry.validate()?;
            if let Some(expected_operator_public_key) = expected_operator_public_key {
                registry.verify_signature_for_operator(expected_operator_public_key)?;
            } else if require_signature {
                registry.verify_signature()?;
            }
        }
        Ok(())
    }
}

impl RegistryPublication {
    pub fn new(
        registry: &DriverRegistry,
        publisher: &Keypair,
        published_at_micros: u64,
        channel: Option<String>,
        labels: Vec<String>,
    ) -> Result<Self> {
        registry.verify_signature()?;
        let publisher_public_key = STANDARD_NO_PAD.encode(publisher.verifying_key().to_bytes());
        let mut publication = Self {
            schema_version: REGISTRY_PUBLICATION_SCHEMA_VERSION,
            registry_hash: registry_hash(registry)?,
            registry_entries: registry.entries.len(),
            registry_operator_node_id: registry.operator_node_id,
            published_at_micros,
            channel,
            labels,
            publisher_node_id: publisher.node_id(),
            publisher_public_key,
            signature: String::new(),
        };
        publication.validate_static()?;
        let signing_key = SigningKey::from_bytes(&publisher.secret_bytes());
        let signature: Signature = signing_key.sign(&publication.signing_message()?);
        publication.signature = STANDARD_NO_PAD.encode(signature.to_bytes());
        Ok(publication)
    }

    pub fn from_json_str(input: &str) -> Result<Self> {
        Ok(serde_json::from_str(input)?)
    }

    pub fn to_json_string(&self) -> Result<String> {
        Ok(serde_json::to_string_pretty(self)?)
    }

    pub fn verify_for_registry(
        &self,
        registry: &DriverRegistry,
        expected_publisher_public_key: Option<&str>,
    ) -> Result<()> {
        self.validate_static()?;
        registry.verify_signature()?;
        compare_registry_publication_field(
            "registry_hash",
            self.registry_hash.as_str(),
            registry_hash(registry)?.as_str(),
        )?;
        compare_registry_publication_field(
            "registry_entries",
            self.registry_entries,
            registry.entries.len(),
        )?;
        compare_registry_publication_field(
            "registry_operator_node_id",
            self.registry_operator_node_id,
            registry.operator_node_id,
        )?;
        let publisher_public_key =
            decode_fixed::<PUBLIC_KEY_LEN>(&self.publisher_public_key, "publisher_public_key")?;
        let derived_node_id = node_id_from_public_key(&publisher_public_key);
        if derived_node_id != self.publisher_node_id {
            return Err(ZapStoreError::RegistryPublicationPublisherNodeMismatch {
                declared: self.publisher_node_id,
                derived: derived_node_id,
            });
        }
        if let Some(expected_publisher_public_key) = expected_publisher_public_key {
            let expected = decode_fixed::<PUBLIC_KEY_LEN>(
                expected_publisher_public_key,
                "expected_publisher_public_key",
            )?;
            if expected != publisher_public_key {
                return Err(
                    ZapStoreError::RegistryPublicationPublisherPublicKeyMismatch {
                        expected: node_id_from_public_key(&expected),
                        actual: self.publisher_node_id,
                    },
                );
            }
        }
        let verifying_key = VerifyingKey::from_bytes(&publisher_public_key)?;
        let signature_bytes =
            decode_fixed::<SIGNATURE_LEN>(&self.signature, "publication_signature")?;
        let signature = Signature::from_bytes(&signature_bytes);
        verifying_key
            .verify(&self.signing_message()?, &signature)
            .map_err(|_| ZapStoreError::InvalidRegistryPublicationSignature)
    }

    fn validate_static(&self) -> Result<()> {
        if self.schema_version != REGISTRY_PUBLICATION_SCHEMA_VERSION {
            return Err(ZapStoreError::UnsupportedRegistryPublicationSchemaVersion(
                self.schema_version,
            ));
        }
        validate_driver_hash(&self.registry_hash)
    }

    fn signing_message(&self) -> Result<Vec<u8>> {
        let payload = RegistryPublicationSigningPayload {
            schema_version: self.schema_version,
            registry_hash: &self.registry_hash,
            registry_entries: self.registry_entries,
            registry_operator_node_id: self.registry_operator_node_id,
            published_at_micros: self.published_at_micros,
            channel: self.channel.as_deref(),
            labels: &self.labels,
            publisher_node_id: self.publisher_node_id,
            publisher_public_key: &self.publisher_public_key,
        };
        let encoded = serde_json::to_vec(&payload)?;
        let mut message =
            Vec::with_capacity(REGISTRY_PUBLICATION_SIGNATURE_DOMAIN.len() + encoded.len());
        message.extend_from_slice(REGISTRY_PUBLICATION_SIGNATURE_DOMAIN);
        message.extend_from_slice(&encoded);
        Ok(message)
    }
}

impl RegistryBundleManifest {
    pub fn new(
        generated_by: Option<String>,
        registry_path: String,
        registry_hash: String,
        publication_path: Option<String>,
        publication_hash: Option<String>,
        entries: Vec<RegistryBundleEntry>,
    ) -> Self {
        Self {
            schema_version: REGISTRY_BUNDLE_SCHEMA_VERSION,
            generated_by,
            registry_path,
            registry_hash,
            publication_path,
            publication_hash,
            entries,
        }
    }

    pub fn from_json_str(input: &str) -> Result<Self> {
        Ok(serde_json::from_str(input)?)
    }

    pub fn to_json_string(&self) -> Result<String> {
        Ok(serde_json::to_string_pretty(self)?)
    }

    pub fn validate(&self) -> Result<()> {
        if self.schema_version != REGISTRY_BUNDLE_SCHEMA_VERSION {
            return Err(ZapStoreError::UnsupportedRegistryBundleSchemaVersion(
                self.schema_version,
            ));
        }
        validate_bundle_path(&self.registry_path)?;
        validate_driver_hash(&self.registry_hash)?;
        if let Some(publication_path) = &self.publication_path {
            validate_bundle_path(publication_path)?;
        }
        if self.publication_path.is_some() != self.publication_hash.is_some() {
            return Err(ZapStoreError::RegistryBundlePublicationMetadataIncomplete);
        }
        if let Some(publication_hash) = &self.publication_hash {
            validate_driver_hash(publication_hash)?;
        }
        let mut seen = std::collections::HashSet::new();
        for entry in &self.entries {
            entry.validate()?;
            if !seen.insert((entry.action.clone(), entry.version.clone())) {
                return Err(ZapStoreError::DuplicateRegistryEntry {
                    action: entry.action.clone(),
                    version: entry.version.clone(),
                });
            }
        }
        Ok(())
    }
}

impl RegistryBundleEntry {
    pub fn from_registry_entry(entry: &DriverRegistryEntry) -> Self {
        Self {
            action: entry.action.clone(),
            version: entry.version.clone(),
            name: entry.name.clone(),
            abi_version: entry.abi_version,
            wasm_hash: entry.wasm_hash.clone(),
            author_node_id: entry.author_node_id,
            status: entry.status,
            manifest_path: None,
            manifest_hash: None,
            driver_path: None,
            driver_hash: None,
        }
    }

    pub fn validate(&self) -> Result<()> {
        if self.action.trim().is_empty() {
            return Err(ZapStoreError::EmptyAction);
        }
        validate_driver_hash(&self.wasm_hash)?;
        if let Some(manifest_path) = &self.manifest_path {
            validate_bundle_path(manifest_path)?;
        }
        if self.manifest_path.is_some() != self.manifest_hash.is_some() {
            return Err(ZapStoreError::RegistryBundleArtifactMetadataIncomplete {
                action: self.action.clone(),
                version: self.version.clone(),
                artifact: "manifest",
            });
        }
        if let Some(manifest_hash) = &self.manifest_hash {
            validate_driver_hash(manifest_hash)?;
        }
        if let Some(driver_path) = &self.driver_path {
            validate_bundle_path(driver_path)?;
        }
        if self.driver_path.is_some() != self.driver_hash.is_some() {
            return Err(ZapStoreError::RegistryBundleArtifactMetadataIncomplete {
                action: self.action.clone(),
                version: self.version.clone(),
                artifact: "driver",
            });
        }
        if let Some(driver_hash) = &self.driver_hash {
            validate_driver_hash(driver_hash)?;
            if driver_hash != &self.wasm_hash {
                return Err(ZapStoreError::RegistryBundleDriverHashMismatch {
                    action: self.action.clone(),
                    version: self.version.clone(),
                });
            }
        }
        Ok(())
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

#[derive(Debug, Serialize)]
struct RegistryPublicationSigningPayload<'a> {
    schema_version: u8,
    registry_hash: &'a str,
    registry_entries: usize,
    registry_operator_node_id: Option<Uuid>,
    published_at_micros: u64,
    channel: Option<&'a str>,
    labels: &'a [String],
    publisher_node_id: Uuid,
    publisher_public_key: &'a str,
}

pub fn driver_hash(wasm: &[u8]) -> String {
    artifact_hash(wasm)
}

pub fn artifact_hash(bytes: &[u8]) -> String {
    format!("{DRIVER_HASH_PREFIX}{}", blake3::hash(bytes).to_hex())
}

pub fn registry_hash(registry: &DriverRegistry) -> Result<String> {
    registry.validate()?;
    Ok(artifact_hash(&serde_json::to_vec(registry)?))
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

fn compare_registry_publication_field<T>(field: &'static str, expected: T, actual: T) -> Result<()>
where
    T: PartialEq,
{
    if expected != actual {
        return Err(ZapStoreError::RegistryPublicationFieldMismatch { field });
    }
    Ok(())
}

fn validate_bundle_path(path: &str) -> Result<()> {
    let path = std::path::Path::new(path);
    if path.is_absolute() || path.as_os_str().is_empty() {
        return Err(ZapStoreError::InvalidRegistryBundlePath(
            path.display().to_string(),
        ));
    }
    for component in path.components() {
        match component {
            std::path::Component::Normal(_) => {}
            _ => {
                return Err(ZapStoreError::InvalidRegistryBundlePath(
                    path.display().to_string(),
                ));
            }
        }
    }
    Ok(())
}

fn validate_registry_merge_compatibility(
    existing: &DriverRegistryEntry,
    incoming: &DriverRegistryEntry,
) -> Result<()> {
    compare_registry_merge_field(existing, incoming, "name", &existing.name, &incoming.name)?;
    compare_registry_merge_field(
        existing,
        incoming,
        "abi_version",
        existing.abi_version,
        incoming.abi_version,
    )?;
    compare_registry_merge_field(
        existing,
        incoming,
        "wasm_hash",
        &existing.wasm_hash,
        &incoming.wasm_hash,
    )?;
    compare_registry_merge_field(
        existing,
        incoming,
        "author_node_id",
        existing.author_node_id,
        incoming.author_node_id,
    )
}

fn compare_registry_merge_field<T>(
    existing: &DriverRegistryEntry,
    incoming: &DriverRegistryEntry,
    field: &'static str,
    expected: T,
    actual: T,
) -> Result<()>
where
    T: PartialEq,
{
    if expected != actual {
        return Err(ZapStoreError::RegistryMergeConflict {
            action: existing.action.clone(),
            version: incoming.version.clone(),
            field,
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
    fn registry_index_response_verifies_required_signature_and_operator() {
        let author = Keypair::generate();
        let operator = Keypair::generate();
        let other_operator = Keypair::generate();
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
        let operator_public_key = registry.operator_public_key.clone().unwrap();
        let other_public_key =
            base64::Engine::encode(&STANDARD_NO_PAD, other_operator.verifying_key().to_bytes());

        let response = RegistryIndexResponse::new(operator.node_id(), Some(registry), None);

        response.verify(false, None).unwrap();
        response.verify(true, None).unwrap();
        response
            .verify(true, Some(operator_public_key.as_str()))
            .unwrap();
        assert!(matches!(
            response.verify(true, Some(other_public_key.as_str())),
            Err(ZapStoreError::RegistryOperatorPublicKeyMismatch { .. })
        ));
    }

    #[test]
    fn registry_index_response_can_require_signed_registry() {
        let registry = DriverRegistry::empty(None);
        let response = RegistryIndexResponse::new(Uuid::nil(), Some(registry), None);

        response.verify(false, None).unwrap();
        assert!(matches!(
            response.verify(true, None),
            Err(ZapStoreError::MissingRegistrySignature)
        ));
    }

    #[test]
    fn registry_merge_adds_unique_entries_and_keeps_compatible_duplicates() {
        let author = Keypair::generate();
        let echo = DriverManifest::new(
            "echo",
            "0.1.0",
            "echo",
            wasm(),
            DriverPermissions::none(),
            None,
            &author,
        )
        .unwrap();
        let math = DriverManifest::new(
            "math",
            "0.1.0",
            "math",
            wasm(),
            DriverPermissions::none(),
            None,
            &author,
        )
        .unwrap();
        let mut left = DriverRegistry::empty(Some("left".to_string()));
        left.add_manifest(&echo, Some("left/echo.manifest.toml".to_string()))
            .unwrap();
        let mut right = DriverRegistry::empty(Some("right".to_string()));
        right
            .add_manifest(&echo, Some("right/echo.manifest.toml".to_string()))
            .unwrap();
        right.add_manifest(&math, None).unwrap();

        let report = left.merge_from(&right).unwrap();

        assert_eq!(report.added, 1);
        assert_eq!(report.unchanged, 1);
        assert_eq!(report.revoked_overrides, 0);
        assert_eq!(left.entries.len(), 2);
        assert_eq!(
            left.entries
                .iter()
                .find(|entry| entry.action == "echo")
                .unwrap()
                .manifest_path
                .as_deref(),
            Some("left/echo.manifest.toml")
        );
    }

    #[test]
    fn registry_merge_prefers_revoked_entry_for_same_driver_version() {
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
        let mut left = DriverRegistry::empty(Some("left".to_string()));
        left.add_manifest(&manifest, None).unwrap();
        let mut right = DriverRegistry::empty(Some("right".to_string()));
        right.add_manifest(&manifest, None).unwrap();
        right.revoke("echo", "0.1.0", "unsafe release").unwrap();

        let report = left.merge_from(&right).unwrap();

        assert_eq!(report.revoked_overrides, 1);
        assert_eq!(left.entries[0].status, DriverRegistryStatus::Revoked);
        assert_eq!(
            left.entries[0].revoked_reason.as_deref(),
            Some("unsafe release")
        );
    }

    #[test]
    fn registry_merge_rejects_conflicting_driver_identity() {
        let author = Keypair::generate();
        let echo = DriverManifest::new(
            "echo",
            "0.1.0",
            "echo",
            wasm(),
            DriverPermissions::none(),
            None,
            &author,
        )
        .unwrap();
        let echo_other = DriverManifest::new(
            "echo",
            "0.1.0",
            "echo",
            b"(module (memory (export \"memory\") 2))",
            DriverPermissions::none(),
            None,
            &author,
        )
        .unwrap();
        let mut left = DriverRegistry::empty(Some("left".to_string()));
        left.add_manifest(&echo, None).unwrap();
        let mut right = DriverRegistry::empty(Some("right".to_string()));
        right.add_manifest(&echo_other, None).unwrap();

        assert!(matches!(
            left.merge_from(&right),
            Err(ZapStoreError::RegistryMergeConflict {
                field: "wasm_hash",
                ..
            })
        ));
    }

    #[test]
    fn registry_publication_signs_and_verifies_registry_hash() {
        let author = Keypair::generate();
        let operator = Keypair::generate();
        let publisher = Keypair::generate();
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
        let publisher_public_key = STANDARD_NO_PAD.encode(publisher.verifying_key().to_bytes());

        let publication = RegistryPublication::new(
            &registry,
            &publisher,
            123,
            Some("stable".to_string()),
            vec!["factory-a".to_string()],
        )
        .unwrap();

        publication
            .verify_for_registry(&registry, Some(&publisher_public_key))
            .unwrap();
        assert_eq!(publication.registry_hash, registry_hash(&registry).unwrap());
        assert_eq!(publication.registry_entries, 1);
        assert_eq!(
            publication.registry_operator_node_id,
            Some(operator.node_id())
        );

        let encoded = publication.to_json_string().unwrap();
        let decoded = RegistryPublication::from_json_str(&encoded).unwrap();
        decoded.verify_for_registry(&registry, None).unwrap();
    }

    #[test]
    fn registry_publication_rejects_registry_mutation() {
        let author = Keypair::generate();
        let operator = Keypair::generate();
        let publisher = Keypair::generate();
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
        let publication =
            RegistryPublication::new(&registry, &publisher, 123, None, Vec::new()).unwrap();

        registry.generated_by = Some("tampered".to_string());
        registry.sign(&operator).unwrap();

        assert!(matches!(
            publication.verify_for_registry(&registry, None),
            Err(ZapStoreError::RegistryPublicationFieldMismatch {
                field: "registry_hash"
            })
        ));
    }

    #[test]
    fn registry_publication_requires_signed_registry() {
        let publisher = Keypair::generate();
        let registry = DriverRegistry::empty(None);

        assert!(matches!(
            RegistryPublication::new(&registry, &publisher, 123, None, Vec::new()),
            Err(ZapStoreError::MissingRegistrySignature)
        ));
    }

    #[test]
    fn registry_bundle_manifest_validates_relative_paths_and_hashes() {
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
        let entry = DriverRegistryEntry::from_manifest(&manifest, None);
        let mut bundle_entry = RegistryBundleEntry::from_registry_entry(&entry);
        bundle_entry.manifest_path = Some("manifests/echo.manifest.toml".to_string());
        bundle_entry.manifest_hash = Some(artifact_hash(b"manifest"));
        bundle_entry.driver_path = Some("drivers/echo.wat".to_string());
        bundle_entry.driver_hash = Some(entry.wasm_hash.clone());
        let bundle = RegistryBundleManifest::new(
            Some("test".to_string()),
            "registry.index.toml".to_string(),
            artifact_hash(b"registry"),
            Some("registry.publication.json".to_string()),
            Some(artifact_hash(b"publication")),
            vec![bundle_entry],
        );

        bundle.validate().unwrap();
    }

    #[test]
    fn registry_bundle_manifest_rejects_path_traversal() {
        let bundle = RegistryBundleManifest::new(
            None,
            "../registry.index.toml".to_string(),
            artifact_hash(b"registry"),
            None,
            None,
            Vec::new(),
        );

        assert!(matches!(
            bundle.validate(),
            Err(ZapStoreError::InvalidRegistryBundlePath(_))
        ));
    }

    #[test]
    fn registry_bundle_entry_rejects_driver_hash_mismatch() {
        let mut entry = RegistryBundleEntry {
            action: "echo".to_string(),
            version: "0.1.0".to_string(),
            name: "echo".to_string(),
            abi_version: DRIVER_ABI_VERSION,
            wasm_hash: artifact_hash(b"driver"),
            author_node_id: Uuid::nil(),
            status: DriverRegistryStatus::Active,
            manifest_path: None,
            manifest_hash: None,
            driver_path: Some("drivers/echo.wat".to_string()),
            driver_hash: Some(artifact_hash(b"other")),
        };

        assert!(matches!(
            entry.validate(),
            Err(ZapStoreError::RegistryBundleDriverHashMismatch { .. })
        ));
        entry.driver_hash = Some(entry.wasm_hash.clone());
        entry.validate().unwrap();
    }

    #[test]
    fn registry_bundle_entry_requires_artifact_hashes() {
        let entry = RegistryBundleEntry {
            action: "echo".to_string(),
            version: "0.1.0".to_string(),
            name: "echo".to_string(),
            abi_version: DRIVER_ABI_VERSION,
            wasm_hash: artifact_hash(b"driver"),
            author_node_id: Uuid::nil(),
            status: DriverRegistryStatus::Active,
            manifest_path: Some("manifests/echo.manifest.toml".to_string()),
            manifest_hash: None,
            driver_path: None,
            driver_hash: None,
        };

        assert!(matches!(
            entry.validate(),
            Err(ZapStoreError::RegistryBundleArtifactMetadataIncomplete {
                artifact: "manifest",
                ..
            })
        ));
    }

    #[test]
    fn registry_mutations_clear_signature() {
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
        assert!(registry.signature.is_some());

        registry
            .add_manifest(&manifest, Some("echo.manifest.toml".to_string()))
            .unwrap();

        assert!(registry.operator_node_id.is_none());
        assert!(registry.operator_public_key.is_none());
        assert!(registry.signature.is_none());
    }

    #[test]
    fn registry_revokes_manifest_version() {
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

        registry.revoke("echo", "0.1.0", "bad release").unwrap();

        assert_eq!(registry.entries[0].status, DriverRegistryStatus::Revoked);
        assert_eq!(
            registry.entries[0].revoked_reason.as_deref(),
            Some("bad release")
        );
        assert!(registry.signature.is_none());
        assert!(matches!(
            registry.verify_manifest(&manifest),
            Err(ZapStoreError::RevokedRegistryEntry { .. })
        ));
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
