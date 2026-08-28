//! Signed driver manifests for the local RivunStore foundation.
//!
//! A manifest binds one driver action to one WASM/WAT artifact hash, an ABI
//! version, declared permissions, and an Ed25519 author identity. The signature
//! is over a deterministic JSON payload so TOML files can be distributed and
//! verified offline.

use base64::{Engine as _, engine::general_purpose::STANDARD_NO_PAD};
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::{fmt, str::FromStr};
use thiserror::Error;
use uuid::Uuid;
use rivun_capability::DriverPermissions;
use rivun_crypto::{Keypair, node_id_from_public_key};

pub mod audit;
pub mod bundle;
pub mod resolver;
pub mod validator;

pub use audit::*;
pub use bundle::*;
pub use resolver::*;
pub use validator::*;

pub const MANIFEST_SCHEMA_VERSION: u8 = 1;
pub const REGISTRY_SCHEMA_VERSION: u8 = 1;
pub const REGISTRY_INDEX_SYNC_SCHEMA_VERSION: u8 = 1;
pub const REGISTRY_PUBLICATION_SCHEMA_VERSION: u8 = 1;
pub const REGISTRY_BUNDLE_SCHEMA_VERSION: u8 = 1;
pub const REGISTRY_INSTALL_PLAN_SCHEMA_VERSION: u8 = 1;
pub const DOMAIN_PACK_REGISTRY_SCHEMA_VERSION: u8 = 1;
pub const DRIVER_ABI_VERSION: u16 = 1;
pub const DRIVER_HASH_PREFIX: &str = "blake3:";
pub const REGISTRY_INDEX_CONTENT_TYPE: &str = "application/rivun-registry-index+json";
pub const REGISTRY_BUNDLE_MANIFEST_CONTENT_TYPE: &str =
    "application/rivun-registry-bundle-manifest+json";
pub const REGISTRY_INDEX_REQUEST_SUBJECT: &str = "rivun.registry.index.request";
pub const REGISTRY_INDEX_RESPONSE_SUBJECT: &str = "rivun.registry.index.response";
pub const REGISTRY_BUNDLE_MANIFEST_REQUEST_SUBJECT: &str = "rivun.registry.bundle.manifest.request";
pub const REGISTRY_BUNDLE_MANIFEST_RESPONSE_SUBJECT: &str = "rivun.registry.bundle.manifest.response";

const MANIFEST_SIGNATURE_DOMAIN: &[u8] = b"Rivun-DRIVER-MANIFEST-v1";
const REGISTRY_SIGNATURE_DOMAIN: &[u8] = b"Rivun-DRIVER-REGISTRY-v1";
const REGISTRY_PUBLICATION_SIGNATURE_DOMAIN: &[u8] = b"Rivun-DRIVER-REGISTRY-PUBLICATION-v1";
const REGISTRY_INSTALL_PLAN_SIGNATURE_DOMAIN: &[u8] = b"Rivun-DRIVER-REGISTRY-INSTALL-PLAN-v1";
const DOMAIN_PACK_REGISTRY_SIGNATURE_DOMAIN: &[u8] = b"Rivun-DOMAIN-PACK-REGISTRY-v1";
const PUBLIC_KEY_LEN: usize = 32;
const SIGNATURE_LEN: usize = 64;

#[derive(Debug, Error)]
pub enum RivunStoreError {
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
    #[error("driver registry install plan schema version {0} is unsupported")]
    UnsupportedRegistryInstallPlanSchemaVersion(u8),
    #[error("domain pack registry schema version {0} is unsupported")]
    UnsupportedDomainPackRegistrySchemaVersion(u8),
    #[error("driver registry entry `{action}` version `{version}` is duplicated")]
    DuplicateRegistryEntry { action: String, version: String },
    #[error("domain pack registry entry `{id}` version `{version}` is duplicated")]
    DuplicateDomainPackRegistryEntry { id: String, version: String },
    #[error("driver registry entry version `{0}` is not a MAJOR.MINOR.PATCH version")]
    InvalidDriverVersion(String),
    #[error("driver registry version requirement `{0}` is invalid")]
    InvalidDriverVersionRequirement(String),
    #[error("driver registry ABI requirement `{0}` is invalid")]
    InvalidDriverAbiRequirement(String),
    #[error(
        "driver registry has no active compatible entry for action `{action}` requirement `{requirement}`"
    )]
    NoCompatibleRegistryEntry { action: String, requirement: String },
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
    #[error("registry install plan must contain at least one entry")]
    EmptyRegistryInstallPlan,
    #[error(
        "registry install plan planner public key derives node_id {derived}, but plan declares {declared}"
    )]
    RegistryInstallPlanPlannerNodeMismatch { declared: Uuid, derived: Uuid },
    #[error("registry install plan planner mismatch: expected node_id {expected}, actual {actual}")]
    RegistryInstallPlanPlannerPublicKeyMismatch { expected: Uuid, actual: Uuid },
    #[error("registry install plan field `{field}` mismatch")]
    RegistryInstallPlanFieldMismatch { field: &'static str },
    #[error(
        "registry install plan entry `{action}` selected version `{version}` does not satisfy `{requirement}`"
    )]
    RegistryInstallPlanRequirementMismatch {
        action: String,
        version: String,
        requirement: String,
    },
    #[error(
        "registry install plan entry `{action}` selected ABI `{abi_version}` does not satisfy `{requirement}`"
    )]
    RegistryInstallPlanAbiRequirementMismatch {
        action: String,
        abi_version: u16,
        requirement: String,
    },
    #[error("registry install plan entry `{action}` version `{version}` field `{field}` mismatch")]
    RegistryInstallPlanEntryMismatch {
        action: String,
        version: String,
        field: &'static str,
    },
    #[error("registry install plan signature verification failed")]
    InvalidRegistryInstallPlanSignature,
    #[error("domain pack id must not be empty")]
    EmptyDomainPackId,
    #[error("domain pack registry is not signed")]
    MissingDomainPackRegistrySignature,
    #[error(
        "domain pack registry operator public key derives node_id {derived}, but registry declares {declared}"
    )]
    DomainPackRegistryOperatorNodeMismatch { declared: Uuid, derived: Uuid },
    #[error("domain pack registry signature verification failed")]
    InvalidDomainPackRegistrySignature,
    #[error("domain pack registry operator mismatch: expected node_id {expected}, actual {actual}")]
    DomainPackRegistryOperatorPublicKeyMismatch { expected: Uuid, actual: Uuid },
    #[error("domain pack artifact path `{0}` is invalid")]
    InvalidDomainPackArtifactPath(String),
    #[error("domain pack artifact `{path}` hash mismatch: expected {expected}, actual {actual}")]
    DomainPackArtifactHashMismatch {
        path: String,
        expected: String,
        actual: String,
    },
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
    #[error("invalid domain pack bundle format: {0}")]
    InvalidDomainPackBundleFormat(String),
    #[error(
        "domain pack bundle digest mismatch for `{path}`: expected {expected}, actual {actual}"
    )]
    DomainPackBundleDigestMismatch {
        path: String,
        expected: String,
        actual: String,
    },
    #[error("domain pack signature missing or invalid")]
    InvalidDomainPackBundleSignature,
    #[error("domain pack signature signer `{signer}` is not in trusted public keys whitelist")]
    UntrustedDomainPackSigner { signer: String },
    #[error("unsatisfied domain pack dependency `{pack_id}` version requirement `{requirement}`")]
    UnsatisfiedDomainPackDependency {
        pack_id: String,
        requirement: String,
    },
    #[error("circular dependency detected in domain pack graph: {0}")]
    CircularDomainPackDependency(String),
    #[error("domain pack policy validation failed: {0}")]
    DomainPackPolicyValidationFailed(String),
    #[error("I/O error: {0}")]
    IoError(String),
}

pub type Result<T> = std::result::Result<T, RivunStoreError>;

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
    Deprecated,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deprecated_reason: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub migrations: Vec<DriverRegistryMigration>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DriverRegistryMigration {
    pub from_version_requirement: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from_abi_requirement: Option<String>,
    #[serde(default)]
    pub requires_operator_approval: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub migration_driver_action: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub migration_driver_version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RegistryBundleManifestRequest {
    pub schema_version: u8,
    #[serde(default)]
    pub require_publication: bool,
    #[serde(default)]
    pub require_drivers: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RegistryBundleManifestResponse {
    pub schema_version: u8,
    pub node_id: Uuid,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub manifest: Option<RegistryBundleManifest>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unavailable_reason: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct DriverRegistryMergeReport {
    pub added: usize,
    pub unchanged: usize,
    pub deprecated_overrides: usize,
    pub revoked_overrides: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct DriverVersion {
    pub major: u64,
    pub minor: u64,
    pub patch: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DriverVersionRequirement {
    raw: String,
    comparators: Vec<DriverVersionComparator>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DriverAbiRequirement {
    raw: String,
    comparators: Vec<DriverAbiComparator>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DriverVersionOperator {
    Equal,
    Greater,
    GreaterOrEqual,
    Less,
    LessOrEqual,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct DriverVersionComparator {
    operator: DriverVersionOperator,
    version: DriverVersion,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct DriverAbiComparator {
    operator: DriverVersionOperator,
    abi_version: u16,
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
pub struct RegistryInstallPlanRequest {
    pub action: String,
    pub requirement: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub abi_version: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub abi_requirement: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RegistryInstallPlan {
    pub schema_version: u8,
    pub registry_hash: String,
    pub registry_entries: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub registry_operator_node_id: Option<Uuid>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub publication_hash: Option<String>,
    pub requested_at_micros: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
    #[serde(default)]
    pub labels: Vec<String>,
    pub entries: Vec<RegistryInstallPlanEntry>,
    pub planner_node_id: Uuid,
    pub planner_public_key: String,
    pub signature: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RegistryInstallPlanEntry {
    pub action: String,
    pub requirement: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requested_abi_version: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requested_abi_requirement: Option<String>,
    pub selected_version: String,
    pub name: String,
    pub abi_version: u16,
    pub wasm_hash: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub manifest_path: Option<String>,
    pub author_node_id: Uuid,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub migrations: Vec<DriverRegistryMigration>,
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

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DomainPackStatus {
    #[default]
    Active,
    Deprecated,
    Revoked,
    Draft,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum DomainPackRisk {
    #[default]
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct DomainPackCompatibility {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_rivun_version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_rivun_version: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub runtimes: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub abi_versions: Vec<u16>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub rivun_version_req: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub abi_version_req: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub capabilities_required: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub capabilities_provided: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct DomainPackArtifact {
    #[serde(default)]
    pub path: String,
    #[serde(default)]
    pub hash: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size_bytes: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub relative_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sha256_hex: Option<String>,
}

impl DomainPackArtifact {
    pub fn path(&self) -> &str {
        if !self.path.is_empty() {
            &self.path
        } else if let Some(ref p) = self.relative_path {
            p.as_str()
        } else {
            ""
        }
    }

    pub fn hash(&self) -> &str {
        if !self.hash.is_empty() {
            &self.hash
        } else if let Some(ref h) = self.sha256_hex {
            h.as_str()
        } else {
            ""
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DomainPackRegistryEntry {
    pub id: String,
    pub name: String,
    pub version: String,
    pub status: DomainPackStatus,
    pub risk: DomainPackRisk,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deprecated_reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revoked_reason: Option<String>,
    #[serde(default)]
    pub author_node_id: Uuid,
    #[serde(default)]
    pub compatibility: DomainPackCompatibility,
    pub manifest: DomainPackArtifact,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub archive: Option<DomainPackArtifact>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub policies: Vec<DomainPackArtifact>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub schemas: Vec<DomainPackArtifact>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub drivers: Vec<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub metadata: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dependencies: Vec<DomainPackDependencySpec>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub labels: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DomainPackRegistry {
    pub schema_version: u8,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub generated_by: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub channel: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub operator_node_id: Option<Uuid>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub operator_public_key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signature: Option<String>,
    #[serde(default)]
    pub entries: Vec<DomainPackRegistryEntry>,
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
            return Err(RivunStoreError::ActionMismatch {
                manifest_action: self.action.clone(),
                configured_action: configured_action.to_string(),
            });
        }

        let actual = driver_hash(wasm);
        if self.wasm_hash != actual {
            return Err(RivunStoreError::HashMismatch {
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
            return Err(RivunStoreError::AuthorNodeMismatch {
                declared: self.author_node_id,
                derived: derived_node_id,
            });
        }

        let verifying_key = VerifyingKey::from_bytes(&public_key_bytes)?;
        let signature_bytes = decode_fixed::<SIGNATURE_LEN>(&self.signature, "signature")?;
        let signature = Signature::from_bytes(&signature_bytes);
        verifying_key
            .verify(&self.signing_message()?, &signature)
            .map_err(|_| RivunStoreError::InvalidSignature)
    }

    fn validate_static_fields(&self) -> Result<()> {
        if self.schema_version != MANIFEST_SCHEMA_VERSION {
            return Err(RivunStoreError::UnsupportedSchemaVersion(self.schema_version));
        }
        if self.abi_version != DRIVER_ABI_VERSION {
            return Err(RivunStoreError::UnsupportedAbiVersion(self.abi_version));
        }
        if self.action.trim().is_empty() {
            return Err(RivunStoreError::EmptyAction);
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

impl DriverVersion {
    pub fn parse(input: &str) -> Result<Self> {
        input.parse()
    }

    fn next_major(self, requirement: &str) -> Result<Self> {
        Ok(Self {
            major: self.major.checked_add(1).ok_or_else(|| {
                RivunStoreError::InvalidDriverVersionRequirement(requirement.to_string())
            })?,
            minor: 0,
            patch: 0,
        })
    }

    fn next_minor(self, requirement: &str) -> Result<Self> {
        Ok(Self {
            major: self.major,
            minor: self.minor.checked_add(1).ok_or_else(|| {
                RivunStoreError::InvalidDriverVersionRequirement(requirement.to_string())
            })?,
            patch: 0,
        })
    }

    fn next_patch(self, requirement: &str) -> Result<Self> {
        Ok(Self {
            major: self.major,
            minor: self.minor,
            patch: self.patch.checked_add(1).ok_or_else(|| {
                RivunStoreError::InvalidDriverVersionRequirement(requirement.to_string())
            })?,
        })
    }
}

impl FromStr for DriverVersion {
    type Err = RivunStoreError;

    fn from_str(input: &str) -> Result<Self> {
        fn parse_component(component: Option<&str>, input: &str) -> Result<u64> {
            let component =
                component.ok_or_else(|| RivunStoreError::InvalidDriverVersion(input.to_string()))?;
            if component.is_empty() || !component.bytes().all(|byte| byte.is_ascii_digit()) {
                return Err(RivunStoreError::InvalidDriverVersion(input.to_string()));
            }
            component
                .parse()
                .map_err(|_| RivunStoreError::InvalidDriverVersion(input.to_string()))
        }

        let input = input.trim();
        let mut parts = input.split('.');
        let major = parse_component(parts.next(), input)?;
        let minor = parse_component(parts.next(), input)?;
        let patch = parse_component(parts.next(), input)?;
        if parts.next().is_some() {
            return Err(RivunStoreError::InvalidDriverVersion(input.to_string()));
        }
        Ok(Self {
            major,
            minor,
            patch,
        })
    }
}

impl fmt::Display for DriverVersion {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

impl DriverVersionRequirement {
    pub fn parse(input: &str) -> Result<Self> {
        let raw = input.trim();
        if raw == "*" {
            return Ok(Self {
                raw: raw.to_string(),
                comparators: Vec::new(),
            });
        }
        if raw.is_empty() {
            return Err(RivunStoreError::InvalidDriverVersionRequirement(
                input.to_string(),
            ));
        }

        if let Some(base) = raw.strip_prefix('^') {
            let base = Self::parse_version_for_requirement(base, raw)?;
            let upper = if base.major > 0 {
                base.next_major(raw)?
            } else if base.minor > 0 {
                base.next_minor(raw)?
            } else {
                base.next_patch(raw)?
            };
            return Ok(Self::from_comparators(
                raw,
                vec![
                    DriverVersionComparator {
                        operator: DriverVersionOperator::GreaterOrEqual,
                        version: base,
                    },
                    DriverVersionComparator {
                        operator: DriverVersionOperator::Less,
                        version: upper,
                    },
                ],
            ));
        }

        if let Some(base) = raw.strip_prefix('~') {
            let base = Self::parse_version_for_requirement(base, raw)?;
            return Ok(Self::from_comparators(
                raw,
                vec![
                    DriverVersionComparator {
                        operator: DriverVersionOperator::GreaterOrEqual,
                        version: base,
                    },
                    DriverVersionComparator {
                        operator: DriverVersionOperator::Less,
                        version: base.next_minor(raw)?,
                    },
                ],
            ));
        }

        let comparators = raw
            .split(',')
            .map(str::trim)
            .map(|part| Self::parse_comparator(raw, part))
            .collect::<Result<Vec<_>>>()?;
        if comparators.is_empty() {
            return Err(RivunStoreError::InvalidDriverVersionRequirement(
                raw.to_string(),
            ));
        }
        Ok(Self::from_comparators(raw, comparators))
    }

    pub fn matches(&self, version: DriverVersion) -> bool {
        self.comparators
            .iter()
            .all(|comparator| match comparator.operator {
                DriverVersionOperator::Equal => version == comparator.version,
                DriverVersionOperator::Greater => version > comparator.version,
                DriverVersionOperator::GreaterOrEqual => version >= comparator.version,
                DriverVersionOperator::Less => version < comparator.version,
                DriverVersionOperator::LessOrEqual => version <= comparator.version,
            })
    }

    pub fn raw(&self) -> &str {
        &self.raw
    }

    fn from_comparators(raw: &str, comparators: Vec<DriverVersionComparator>) -> Self {
        Self {
            raw: raw.to_string(),
            comparators,
        }
    }

    fn parse_comparator(requirement: &str, part: &str) -> Result<DriverVersionComparator> {
        if part.is_empty() {
            return Err(RivunStoreError::InvalidDriverVersionRequirement(
                requirement.to_string(),
            ));
        }

        let (operator, version) = if let Some(version) = part.strip_prefix(">=") {
            (DriverVersionOperator::GreaterOrEqual, version)
        } else if let Some(version) = part.strip_prefix("<=") {
            (DriverVersionOperator::LessOrEqual, version)
        } else if let Some(version) = part.strip_prefix('>') {
            (DriverVersionOperator::Greater, version)
        } else if let Some(version) = part.strip_prefix('<') {
            (DriverVersionOperator::Less, version)
        } else if let Some(version) = part.strip_prefix('=') {
            (DriverVersionOperator::Equal, version)
        } else {
            (DriverVersionOperator::Equal, part)
        };

        Ok(DriverVersionComparator {
            operator,
            version: Self::parse_version_for_requirement(version, requirement)?,
        })
    }

    fn parse_version_for_requirement(version: &str, requirement: &str) -> Result<DriverVersion> {
        DriverVersion::parse(version.trim())
            .map_err(|_| RivunStoreError::InvalidDriverVersionRequirement(requirement.to_string()))
    }
}

impl DriverAbiRequirement {
    pub fn parse(input: &str) -> Result<Self> {
        let raw = input.trim();
        if raw == "*" {
            return Ok(Self {
                raw: raw.to_string(),
                comparators: Vec::new(),
            });
        }
        if raw.is_empty() {
            return Err(RivunStoreError::InvalidDriverAbiRequirement(
                input.to_string(),
            ));
        }

        let comparators = raw
            .split(',')
            .map(str::trim)
            .map(|part| Self::parse_comparator(raw, part))
            .collect::<Result<Vec<_>>>()?;
        if comparators.is_empty() {
            return Err(RivunStoreError::InvalidDriverAbiRequirement(raw.to_string()));
        }
        Ok(Self {
            raw: raw.to_string(),
            comparators,
        })
    }

    pub fn exact(abi_version: u16) -> Self {
        Self {
            raw: format!("={abi_version}"),
            comparators: vec![DriverAbiComparator {
                operator: DriverVersionOperator::Equal,
                abi_version,
            }],
        }
    }

    pub fn matches(&self, abi_version: u16) -> bool {
        self.comparators
            .iter()
            .all(|comparator| match comparator.operator {
                DriverVersionOperator::Equal => abi_version == comparator.abi_version,
                DriverVersionOperator::Greater => abi_version > comparator.abi_version,
                DriverVersionOperator::GreaterOrEqual => abi_version >= comparator.abi_version,
                DriverVersionOperator::Less => abi_version < comparator.abi_version,
                DriverVersionOperator::LessOrEqual => abi_version <= comparator.abi_version,
            })
    }

    pub fn raw(&self) -> &str {
        &self.raw
    }

    fn parse_comparator(requirement: &str, part: &str) -> Result<DriverAbiComparator> {
        if part.is_empty() {
            return Err(RivunStoreError::InvalidDriverAbiRequirement(
                requirement.to_string(),
            ));
        }

        let (operator, abi_version) = if let Some(abi_version) = part.strip_prefix(">=") {
            (DriverVersionOperator::GreaterOrEqual, abi_version)
        } else if let Some(abi_version) = part.strip_prefix("<=") {
            (DriverVersionOperator::LessOrEqual, abi_version)
        } else if let Some(abi_version) = part.strip_prefix('>') {
            (DriverVersionOperator::Greater, abi_version)
        } else if let Some(abi_version) = part.strip_prefix('<') {
            (DriverVersionOperator::Less, abi_version)
        } else if let Some(abi_version) = part.strip_prefix('=') {
            (DriverVersionOperator::Equal, abi_version)
        } else {
            (DriverVersionOperator::Equal, part)
        };

        Ok(DriverAbiComparator {
            operator,
            abi_version: Self::parse_abi_for_requirement(abi_version, requirement)?,
        })
    }

    fn parse_abi_for_requirement(abi_version: &str, requirement: &str) -> Result<u16> {
        let abi_version = abi_version.trim();
        if abi_version.is_empty() || !abi_version.bytes().all(|byte| byte.is_ascii_digit()) {
            return Err(RivunStoreError::InvalidDriverAbiRequirement(
                requirement.to_string(),
            ));
        }
        abi_version
            .parse()
            .map_err(|_| RivunStoreError::InvalidDriverAbiRequirement(requirement.to_string()))
    }
}

impl DriverRegistryMigration {
    pub fn new(
        from_version_requirement: impl Into<String>,
        from_abi_requirement: Option<String>,
        requires_operator_approval: bool,
        migration_driver_action: Option<String>,
        migration_driver_version: Option<String>,
        notes: Option<String>,
    ) -> Self {
        Self {
            from_version_requirement: from_version_requirement.into(),
            from_abi_requirement,
            requires_operator_approval,
            migration_driver_action,
            migration_driver_version,
            notes,
        }
    }

    fn validate(&self) -> Result<()> {
        DriverVersionRequirement::parse(&self.from_version_requirement)?;
        if let Some(requirement) = &self.from_abi_requirement {
            DriverAbiRequirement::parse(requirement)?;
        }
        if self.migration_driver_action.is_some() != self.migration_driver_version.is_some() {
            return Err(RivunStoreError::InvalidDriverVersionRequirement(
                self.from_version_requirement.clone(),
            ));
        }
        if let Some(action) = &self.migration_driver_action
            && action.trim().is_empty()
        {
            return Err(RivunStoreError::EmptyAction);
        }
        if let Some(version) = &self.migration_driver_version {
            DriverVersion::parse(version)?;
        }
        Ok(())
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
                .ok_or_else(|| RivunStoreError::MissingRegistryEntry {
                    action: action.to_string(),
                    version: version.to_string(),
                })?;
            entry.status = DriverRegistryStatus::Revoked;
            entry.revoked_reason = Some(reason.into());
        }
        self.clear_signature();
        self.validate()
    }

    pub fn deprecate(
        &mut self,
        action: &str,
        version: &str,
        reason: impl Into<String>,
    ) -> Result<()> {
        self.validate()?;
        {
            let entry = self
                .entries
                .iter_mut()
                .find(|entry| entry.action == action && entry.version == version)
                .ok_or_else(|| RivunStoreError::MissingRegistryEntry {
                    action: action.to_string(),
                    version: version.to_string(),
                })?;
            entry.status = DriverRegistryStatus::Deprecated;
            entry.deprecated_reason = Some(reason.into());
        }
        self.clear_signature();
        self.validate()
    }

    pub fn add_migration(
        &mut self,
        action: &str,
        version: &str,
        migration: DriverRegistryMigration,
    ) -> Result<()> {
        self.validate()?;
        migration.validate()?;
        {
            let entry = self
                .entries
                .iter_mut()
                .find(|entry| entry.action == action && entry.version == version)
                .ok_or_else(|| RivunStoreError::MissingRegistryEntry {
                    action: action.to_string(),
                    version: version.to_string(),
                })?;
            entry.migrations.push(migration);
        }
        self.clear_signature();
        self.validate()
    }

    pub fn validate(&self) -> Result<()> {
        if self.schema_version != REGISTRY_SCHEMA_VERSION {
            return Err(RivunStoreError::UnsupportedRegistrySchemaVersion(
                self.schema_version,
            ));
        }

        let mut seen = std::collections::HashSet::with_capacity(self.entries.len());
        for entry in &self.entries {
            if entry.action.trim().is_empty() {
                return Err(RivunStoreError::EmptyAction);
            }
            validate_driver_hash(&entry.wasm_hash)?;
            for migration in &entry.migrations {
                migration.validate()?;
            }
            if !seen.insert((entry.action.as_str(), entry.version.as_str())) {
                return Err(RivunStoreError::DuplicateRegistryEntry {
                    action: entry.action.clone(),
                    version: entry.version.clone(),
                });
            }
        }
        Ok(())
    }

    pub fn resolve(
        &self,
        action: &str,
        requirement: &str,
        abi_version: Option<u16>,
    ) -> Result<&DriverRegistryEntry> {
        let abi_requirement = abi_version.map(DriverAbiRequirement::exact);
        self.resolve_with_requirements(action, requirement, abi_requirement.as_ref())
    }

    pub fn resolve_compatible(
        &self,
        action: &str,
        requirement: &str,
        abi_requirement: Option<&str>,
    ) -> Result<&DriverRegistryEntry> {
        let abi_requirement = abi_requirement
            .map(DriverAbiRequirement::parse)
            .transpose()?;
        self.resolve_with_requirements(action, requirement, abi_requirement.as_ref())
    }

    fn resolve_with_requirements(
        &self,
        action: &str,
        requirement: &str,
        abi_requirement: Option<&DriverAbiRequirement>,
    ) -> Result<&DriverRegistryEntry> {
        self.validate()?;
        let requirement = DriverVersionRequirement::parse(requirement)?;
        let mut selected: Option<(&DriverRegistryEntry, DriverVersion)> = None;

        for entry in self.entries.iter().filter(|entry| {
            entry.action == action
                && entry.status == DriverRegistryStatus::Active
                && abi_requirement.is_none_or(|requirement| requirement.matches(entry.abi_version))
        }) {
            let version = DriverVersion::parse(&entry.version)?;
            if !requirement.matches(version) {
                continue;
            }
            let should_select = match selected {
                Some((_, selected_version)) => version > selected_version,
                None => true,
            };
            if should_select {
                selected = Some((entry, version));
            }
        }

        selected
            .map(|(entry, _)| entry)
            .ok_or_else(|| RivunStoreError::NoCompatibleRegistryEntry {
                action: action.to_string(),
                requirement: requirement.raw().to_string(),
            })
    }

    pub fn verify_manifest(&self, manifest: &DriverManifest) -> Result<()> {
        self.validate()?;
        manifest.verify_static_and_signature()?;
        let entry = self
            .entries
            .iter()
            .find(|entry| entry.action == manifest.action && entry.version == manifest.version)
            .ok_or_else(|| RivunStoreError::MissingRegistryEntry {
                action: manifest.action.clone(),
                version: manifest.version.clone(),
            })?;

        if entry.status == DriverRegistryStatus::Revoked {
            return Err(RivunStoreError::RevokedRegistryEntry {
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
                    if registry_status_rank(existing.status)
                        >= registry_status_rank(incoming.status)
                    {
                        report.unchanged += 1;
                        continue;
                    }
                    *existing = incoming.clone();
                    match incoming.status {
                        DriverRegistryStatus::Active => {}
                        DriverRegistryStatus::Deprecated => report.deprecated_overrides += 1,
                        DriverRegistryStatus::Revoked => report.revoked_overrides += 1,
                    }
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
            .ok_or(RivunStoreError::MissingRegistrySignature)?;
        let operator_public_key = self
            .operator_public_key
            .as_deref()
            .ok_or(RivunStoreError::MissingRegistrySignature)?;
        let signature = self
            .signature
            .as_deref()
            .ok_or(RivunStoreError::MissingRegistrySignature)?;

        let public_key_bytes =
            decode_fixed::<PUBLIC_KEY_LEN>(operator_public_key, "registry_public_key")?;
        let derived_node_id = node_id_from_public_key(&public_key_bytes);
        if derived_node_id != operator_node_id {
            return Err(RivunStoreError::RegistryOperatorNodeMismatch {
                declared: operator_node_id,
                derived: derived_node_id,
            });
        }

        let verifying_key = VerifyingKey::from_bytes(&public_key_bytes)?;
        let signature_bytes = decode_fixed::<SIGNATURE_LEN>(signature, "registry_signature")?;
        let signature = Signature::from_bytes(&signature_bytes);
        verifying_key
            .verify(&self.signing_message()?, &signature)
            .map_err(|_| RivunStoreError::InvalidRegistrySignature)
    }

    pub fn verify_signature_for_operator(&self, expected_operator_public_key: &str) -> Result<()> {
        self.verify_signature()?;
        let declared_public_key = self
            .operator_public_key
            .as_deref()
            .ok_or(RivunStoreError::MissingRegistrySignature)?;
        let expected_bytes =
            decode_fixed::<PUBLIC_KEY_LEN>(expected_operator_public_key, "expected_public_key")?;
        let declared_bytes =
            decode_fixed::<PUBLIC_KEY_LEN>(declared_public_key, "registry_operator_public_key")?;
        if declared_bytes != expected_bytes {
            return Err(RivunStoreError::RegistryOperatorPublicKeyMismatch {
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
                .ok_or(RivunStoreError::MissingRegistrySignature)?,
            operator_public_key: self
                .operator_public_key
                .as_deref()
                .ok_or(RivunStoreError::MissingRegistrySignature)?,
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
            return Err(RivunStoreError::UnsupportedRegistryIndexSyncSchemaVersion(
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
            return Err(RivunStoreError::UnsupportedRegistryIndexSyncSchemaVersion(
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

impl Default for RegistryBundleManifestRequest {
    fn default() -> Self {
        Self {
            schema_version: REGISTRY_BUNDLE_SCHEMA_VERSION,
            require_publication: false,
            require_drivers: false,
        }
    }
}

impl RegistryBundleManifestRequest {
    pub fn validate(&self) -> Result<()> {
        if self.schema_version != REGISTRY_BUNDLE_SCHEMA_VERSION {
            return Err(RivunStoreError::UnsupportedRegistryBundleSchemaVersion(
                self.schema_version,
            ));
        }
        Ok(())
    }
}

impl RegistryBundleManifestResponse {
    pub fn new(
        node_id: Uuid,
        manifest: Option<RegistryBundleManifest>,
        unavailable_reason: Option<String>,
    ) -> Self {
        Self {
            schema_version: REGISTRY_BUNDLE_SCHEMA_VERSION,
            node_id,
            manifest,
            unavailable_reason,
        }
    }

    pub fn verify(&self, request: &RegistryBundleManifestRequest) -> Result<()> {
        if self.schema_version != REGISTRY_BUNDLE_SCHEMA_VERSION {
            return Err(RivunStoreError::UnsupportedRegistryBundleSchemaVersion(
                self.schema_version,
            ));
        }
        request.validate()?;
        if let Some(manifest) = &self.manifest {
            manifest.validate()?;
            if request.require_publication
                && (manifest.publication_path.is_none() || manifest.publication_hash.is_none())
            {
                return Err(RivunStoreError::RegistryBundlePublicationMetadataIncomplete);
            }
            if request.require_drivers {
                for entry in &manifest.entries {
                    if entry.driver_path.is_none() || entry.driver_hash.is_none() {
                        return Err(RivunStoreError::RegistryBundleArtifactMetadataIncomplete {
                            action: entry.action.clone(),
                            version: entry.version.clone(),
                            artifact: "driver",
                        });
                    }
                }
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
            registry_hash_no_validate(registry)?.as_str(),
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
            return Err(RivunStoreError::RegistryPublicationPublisherNodeMismatch {
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
                    RivunStoreError::RegistryPublicationPublisherPublicKeyMismatch {
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
            .map_err(|_| RivunStoreError::InvalidRegistryPublicationSignature)
    }

    fn validate_static(&self) -> Result<()> {
        if self.schema_version != REGISTRY_PUBLICATION_SCHEMA_VERSION {
            return Err(RivunStoreError::UnsupportedRegistryPublicationSchemaVersion(
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

impl RegistryInstallPlanRequest {
    pub fn new(
        action: impl Into<String>,
        requirement: impl Into<String>,
        abi_version: Option<u16>,
    ) -> Self {
        Self {
            action: action.into(),
            requirement: requirement.into(),
            abi_version,
            abi_requirement: None,
        }
    }

    pub fn new_with_abi_requirement(
        action: impl Into<String>,
        requirement: impl Into<String>,
        abi_requirement: Option<String>,
    ) -> Self {
        Self {
            action: action.into(),
            requirement: requirement.into(),
            abi_version: None,
            abi_requirement,
        }
    }

    fn abi_requirement(&self) -> Result<Option<DriverAbiRequirement>> {
        match (self.abi_version, self.abi_requirement.as_deref()) {
            (Some(_), Some(requirement)) => Err(RivunStoreError::InvalidDriverAbiRequirement(
                requirement.to_string(),
            )),
            (Some(abi_version), None) => Ok(Some(DriverAbiRequirement::exact(abi_version))),
            (None, Some(requirement)) => Ok(Some(DriverAbiRequirement::parse(requirement)?)),
            (None, None) => Ok(None),
        }
    }
}

impl RegistryInstallPlan {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        registry: &DriverRegistry,
        requests: &[RegistryInstallPlanRequest],
        planner: &Keypair,
        requested_at_micros: u64,
        target: Option<String>,
        labels: Vec<String>,
        publication_hash: Option<String>,
    ) -> Result<Self> {
        registry.verify_signature()?;
        if requests.is_empty() {
            return Err(RivunStoreError::EmptyRegistryInstallPlan);
        }
        let planner_public_key = STANDARD_NO_PAD.encode(planner.verifying_key().to_bytes());
        let mut entries = requests
            .iter()
            .map(|request| {
                let abi_requirement = request.abi_requirement()?;
                let selected = registry.resolve_with_requirements(
                    &request.action,
                    &request.requirement,
                    abi_requirement.as_ref(),
                )?;
                Ok(RegistryInstallPlanEntry::from_registry_entry(
                    selected,
                    &request.requirement,
                    request.abi_version,
                    request.abi_requirement.clone(),
                ))
            })
            .collect::<Result<Vec<_>>>()?;
        entries.sort_by(|left, right| {
            left.action
                .cmp(&right.action)
                .then_with(|| left.selected_version.cmp(&right.selected_version))
        });

        let mut plan = Self {
            schema_version: REGISTRY_INSTALL_PLAN_SCHEMA_VERSION,
            registry_hash: registry_hash(registry)?,
            registry_entries: registry.entries.len(),
            registry_operator_node_id: registry.operator_node_id,
            publication_hash,
            requested_at_micros,
            target,
            labels,
            entries,
            planner_node_id: planner.node_id(),
            planner_public_key,
            signature: String::new(),
        };
        plan.validate_static()?;
        let signing_key = SigningKey::from_bytes(&planner.secret_bytes());
        let signature: Signature = signing_key.sign(&plan.signing_message()?);
        plan.signature = STANDARD_NO_PAD.encode(signature.to_bytes());
        Ok(plan)
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
        expected_planner_public_key: Option<&str>,
    ) -> Result<()> {
        self.validate_static()?;
        registry.verify_signature()?;
        compare_registry_install_plan_field(
            "registry_hash",
            self.registry_hash.as_str(),
            registry_hash_no_validate(registry)?.as_str(),
        )?;
        compare_registry_install_plan_field(
            "registry_entries",
            self.registry_entries,
            registry.entries.len(),
        )?;
        compare_registry_install_plan_field(
            "registry_operator_node_id",
            self.registry_operator_node_id,
            registry.operator_node_id,
        )?;

        let planner_public_key =
            decode_fixed::<PUBLIC_KEY_LEN>(&self.planner_public_key, "planner_public_key")?;
        let derived_node_id = node_id_from_public_key(&planner_public_key);
        if derived_node_id != self.planner_node_id {
            return Err(RivunStoreError::RegistryInstallPlanPlannerNodeMismatch {
                declared: self.planner_node_id,
                derived: derived_node_id,
            });
        }
        if let Some(expected_planner_public_key) = expected_planner_public_key {
            let expected =
                decode_fixed::<PUBLIC_KEY_LEN>(expected_planner_public_key, "expected_planner")?;
            if expected != planner_public_key {
                return Err(RivunStoreError::RegistryInstallPlanPlannerPublicKeyMismatch {
                    expected: node_id_from_public_key(&expected),
                    actual: self.planner_node_id,
                });
            }
        }

        for entry in &self.entries {
            let abi_requirement = entry.abi_requirement()?;
            let resolved = registry.resolve_with_requirements(
                &entry.action,
                &entry.requirement,
                abi_requirement.as_ref(),
            )?;
            entry.verify_against_registry_entry(resolved)?;
        }

        let verifying_key = VerifyingKey::from_bytes(&planner_public_key)?;
        let signature_bytes =
            decode_fixed::<SIGNATURE_LEN>(&self.signature, "install_plan_signature")?;
        let signature = Signature::from_bytes(&signature_bytes);
        verifying_key
            .verify(&self.signing_message()?, &signature)
            .map_err(|_| RivunStoreError::InvalidRegistryInstallPlanSignature)
    }

    fn validate_static(&self) -> Result<()> {
        if self.schema_version != REGISTRY_INSTALL_PLAN_SCHEMA_VERSION {
            return Err(RivunStoreError::UnsupportedRegistryInstallPlanSchemaVersion(
                self.schema_version,
            ));
        }
        validate_driver_hash(&self.registry_hash)?;
        if let Some(publication_hash) = &self.publication_hash {
            validate_driver_hash(publication_hash)?;
        }
        if self.entries.is_empty() {
            return Err(RivunStoreError::EmptyRegistryInstallPlan);
        }
        decode_fixed::<PUBLIC_KEY_LEN>(&self.planner_public_key, "planner_public_key")?;
        for entry in &self.entries {
            entry.validate()?;
        }
        Ok(())
    }

    fn signing_message(&self) -> Result<Vec<u8>> {
        let payload = RegistryInstallPlanSigningPayload {
            schema_version: self.schema_version,
            registry_hash: &self.registry_hash,
            registry_entries: self.registry_entries,
            registry_operator_node_id: self.registry_operator_node_id,
            publication_hash: self.publication_hash.as_deref(),
            requested_at_micros: self.requested_at_micros,
            target: self.target.as_deref(),
            labels: &self.labels,
            entries: &self.entries,
            planner_node_id: self.planner_node_id,
            planner_public_key: &self.planner_public_key,
        };
        let encoded = serde_json::to_vec(&payload)?;
        let mut message =
            Vec::with_capacity(REGISTRY_INSTALL_PLAN_SIGNATURE_DOMAIN.len() + encoded.len());
        message.extend_from_slice(REGISTRY_INSTALL_PLAN_SIGNATURE_DOMAIN);
        message.extend_from_slice(&encoded);
        Ok(message)
    }
}

impl RegistryInstallPlanEntry {
    pub fn from_registry_entry(
        entry: &DriverRegistryEntry,
        requirement: &str,
        requested_abi_version: Option<u16>,
        requested_abi_requirement: Option<String>,
    ) -> Self {
        Self {
            action: entry.action.clone(),
            requirement: requirement.to_string(),
            requested_abi_version,
            requested_abi_requirement,
            selected_version: entry.version.clone(),
            name: entry.name.clone(),
            abi_version: entry.abi_version,
            wasm_hash: entry.wasm_hash.clone(),
            manifest_path: entry.manifest_path.clone(),
            author_node_id: entry.author_node_id,
            migrations: entry.migrations.clone(),
        }
    }

    fn validate(&self) -> Result<()> {
        if self.action.trim().is_empty() {
            return Err(RivunStoreError::EmptyAction);
        }
        validate_driver_hash(&self.wasm_hash)?;
        let version = DriverVersion::parse(&self.selected_version)?;
        let requirement = DriverVersionRequirement::parse(&self.requirement)?;
        if !requirement.matches(version) {
            return Err(RivunStoreError::RegistryInstallPlanRequirementMismatch {
                action: self.action.clone(),
                version: self.selected_version.clone(),
                requirement: self.requirement.clone(),
            });
        }
        let abi_requirement = self.abi_requirement()?;
        if let Some(requirement) = &abi_requirement
            && !requirement.matches(self.abi_version)
        {
            return Err(RivunStoreError::RegistryInstallPlanAbiRequirementMismatch {
                action: self.action.clone(),
                abi_version: self.abi_version,
                requirement: requirement.raw().to_string(),
            });
        }
        for migration in &self.migrations {
            migration.validate()?;
        }
        Ok(())
    }

    fn verify_against_registry_entry(&self, entry: &DriverRegistryEntry) -> Result<()> {
        compare_registry_install_plan_entry_field(
            self,
            entry,
            "selected_version",
            self.selected_version.as_str(),
            entry.version.as_str(),
        )?;
        compare_registry_install_plan_entry_field(
            self,
            entry,
            "name",
            self.name.as_str(),
            entry.name.as_str(),
        )?;
        compare_registry_install_plan_entry_field(
            self,
            entry,
            "abi_version",
            self.abi_version,
            entry.abi_version,
        )?;
        compare_registry_install_plan_entry_field(
            self,
            entry,
            "wasm_hash",
            self.wasm_hash.as_str(),
            entry.wasm_hash.as_str(),
        )?;
        compare_registry_install_plan_entry_field(
            self,
            entry,
            "manifest_path",
            self.manifest_path.as_deref(),
            entry.manifest_path.as_deref(),
        )?;
        compare_registry_install_plan_entry_field(
            self,
            entry,
            "author_node_id",
            self.author_node_id,
            entry.author_node_id,
        )?;
        if self.migrations != entry.migrations {
            return Err(RivunStoreError::RegistryInstallPlanEntryMismatch {
                action: self.action.clone(),
                version: self.selected_version.clone(),
                field: "migrations",
            });
        }
        Ok(())
    }

    fn abi_requirement(&self) -> Result<Option<DriverAbiRequirement>> {
        match (
            self.requested_abi_version,
            self.requested_abi_requirement.as_deref(),
        ) {
            (Some(_), Some(requirement)) => Err(RivunStoreError::InvalidDriverAbiRequirement(
                requirement.to_string(),
            )),
            (Some(abi_version), None) => Ok(Some(DriverAbiRequirement::exact(abi_version))),
            (None, Some(requirement)) => Ok(Some(DriverAbiRequirement::parse(requirement)?)),
            (None, None) => Ok(None),
        }
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
            return Err(RivunStoreError::UnsupportedRegistryBundleSchemaVersion(
                self.schema_version,
            ));
        }
        validate_bundle_path(&self.registry_path)?;
        validate_driver_hash(&self.registry_hash)?;
        if let Some(publication_path) = &self.publication_path {
            validate_bundle_path(publication_path)?;
        }
        if self.publication_path.is_some() != self.publication_hash.is_some() {
            return Err(RivunStoreError::RegistryBundlePublicationMetadataIncomplete);
        }
        if let Some(publication_hash) = &self.publication_hash {
            validate_driver_hash(publication_hash)?;
        }
        let mut seen = std::collections::HashSet::new();
        for entry in &self.entries {
            entry.validate()?;
            if !seen.insert((entry.action.clone(), entry.version.clone())) {
                return Err(RivunStoreError::DuplicateRegistryEntry {
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
            return Err(RivunStoreError::EmptyAction);
        }
        validate_driver_hash(&self.wasm_hash)?;
        if let Some(manifest_path) = &self.manifest_path {
            validate_bundle_path(manifest_path)?;
        }
        if self.manifest_path.is_some() != self.manifest_hash.is_some() {
            return Err(RivunStoreError::RegistryBundleArtifactMetadataIncomplete {
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
            return Err(RivunStoreError::RegistryBundleArtifactMetadataIncomplete {
                action: self.action.clone(),
                version: self.version.clone(),
                artifact: "driver",
            });
        }
        if let Some(driver_hash) = &self.driver_hash {
            validate_driver_hash(driver_hash)?;
            if driver_hash != &self.wasm_hash {
                return Err(RivunStoreError::RegistryBundleDriverHashMismatch {
                    action: self.action.clone(),
                    version: self.version.clone(),
                });
            }
        }
        Ok(())
    }
}

impl DomainPackRegistry {
    pub fn empty(generated_by: Option<String>) -> Self {
        Self {
            schema_version: DOMAIN_PACK_REGISTRY_SCHEMA_VERSION,
            generated_by,
            channel: None,
            operator_node_id: None,
            operator_public_key: None,
            signature: None,
            entries: Vec::new(),
        }
    }

    pub fn from_json_str(input: &str) -> Result<Self> {
        Ok(serde_json::from_str(input)?)
    }

    pub fn to_json_string(&self) -> Result<String> {
        Ok(serde_json::to_string_pretty(self)?)
    }

    pub fn from_toml_str(input: &str) -> Result<Self> {
        Ok(toml::from_str(input)?)
    }

    pub fn to_toml_string(&self) -> Result<String> {
        Ok(toml::to_string_pretty(self)?)
    }

    pub fn add_entry(&mut self, entry: DomainPackRegistryEntry) -> Result<()> {
        self.validate()?;
        entry.validate()?;
        self.clear_signature();
        self.entries
            .retain(|existing| !(existing.id == entry.id && existing.version == entry.version));
        self.entries.push(entry);
        self.entries.sort_by(|left, right| {
            left.id
                .cmp(&right.id)
                .then_with(|| left.version.cmp(&right.version))
        });
        self.validate()
    }

    pub fn validate(&self) -> Result<()> {
        if self.schema_version != DOMAIN_PACK_REGISTRY_SCHEMA_VERSION {
            return Err(RivunStoreError::UnsupportedDomainPackRegistrySchemaVersion(
                self.schema_version,
            ));
        }

        let mut seen = std::collections::HashSet::with_capacity(self.entries.len());
        for entry in &self.entries {
            entry.validate()?;
            if !seen.insert((entry.id.as_str(), entry.version.as_str())) {
                return Err(RivunStoreError::DuplicateDomainPackRegistryEntry {
                    id: entry.id.clone(),
                    version: entry.version.clone(),
                });
            }
        }
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
            .ok_or(RivunStoreError::MissingDomainPackRegistrySignature)?;
        let operator_public_key = self
            .operator_public_key
            .as_deref()
            .ok_or(RivunStoreError::MissingDomainPackRegistrySignature)?;
        let signature = self
            .signature
            .as_deref()
            .ok_or(RivunStoreError::MissingDomainPackRegistrySignature)?;

        let public_key_bytes =
            decode_fixed::<PUBLIC_KEY_LEN>(operator_public_key, "domain_pack_registry_public_key")?;
        let derived_node_id = node_id_from_public_key(&public_key_bytes);
        if derived_node_id != operator_node_id {
            return Err(RivunStoreError::DomainPackRegistryOperatorNodeMismatch {
                declared: operator_node_id,
                derived: derived_node_id,
            });
        }

        let verifying_key = VerifyingKey::from_bytes(&public_key_bytes)?;
        let signature_bytes =
            decode_fixed::<SIGNATURE_LEN>(signature, "domain_pack_registry_signature")?;
        let signature = Signature::from_bytes(&signature_bytes);
        verifying_key
            .verify(&self.signing_message()?, &signature)
            .map_err(|_| RivunStoreError::InvalidDomainPackRegistrySignature)
    }

    pub fn verify_signature_for_operator(&self, expected_operator_public_key: &str) -> Result<()> {
        self.verify_signature()?;
        let declared_public_key = self
            .operator_public_key
            .as_deref()
            .ok_or(RivunStoreError::MissingDomainPackRegistrySignature)?;
        let expected_bytes =
            decode_fixed::<PUBLIC_KEY_LEN>(expected_operator_public_key, "expected_public_key")?;
        let declared_bytes = decode_fixed::<PUBLIC_KEY_LEN>(
            declared_public_key,
            "domain_pack_registry_operator_public_key",
        )?;
        if declared_bytes != expected_bytes {
            return Err(RivunStoreError::DomainPackRegistryOperatorPublicKeyMismatch {
                expected: node_id_from_public_key(&expected_bytes),
                actual: node_id_from_public_key(&declared_bytes),
            });
        }
        Ok(())
    }

    fn signing_message(&self) -> Result<Vec<u8>> {
        let payload = DomainPackRegistrySigningPayload {
            schema_version: self.schema_version,
            generated_by: self.generated_by.as_deref(),
            channel: self.channel.as_deref(),
            operator_node_id: self
                .operator_node_id
                .ok_or(RivunStoreError::MissingDomainPackRegistrySignature)?,
            operator_public_key: self
                .operator_public_key
                .as_deref()
                .ok_or(RivunStoreError::MissingDomainPackRegistrySignature)?,
            entries: &self.entries,
        };
        let encoded = serde_json::to_vec(&payload)?;
        let mut message =
            Vec::with_capacity(DOMAIN_PACK_REGISTRY_SIGNATURE_DOMAIN.len() + encoded.len());
        message.extend_from_slice(DOMAIN_PACK_REGISTRY_SIGNATURE_DOMAIN);
        message.extend_from_slice(&encoded);
        Ok(message)
    }

    fn clear_signature(&mut self) {
        self.operator_node_id = None;
        self.operator_public_key = None;
        self.signature = None;
    }
}

impl DomainPackRegistryEntry {
    pub fn validate(&self) -> Result<()> {
        if self.id.trim().is_empty() {
            return Err(RivunStoreError::EmptyDomainPackId);
        }
        DriverVersion::parse(&self.version)?;
        self.compatibility.validate()?;
        self.manifest.validate()?;
        if let Some(archive) = &self.archive {
            archive.validate()?;
        }
        for policy in &self.policies {
            policy.validate()?;
        }
        for schema in &self.schemas {
            schema.validate()?;
        }
        Ok(())
    }
}

impl DomainPackCompatibility {
    pub fn validate(&self) -> Result<()> {
        if let Some(version) = &self.min_rivun_version {
            DriverVersion::parse(version)?;
        }
        if let Some(version) = &self.max_rivun_version {
            DriverVersion::parse(version)?;
        }
        Ok(())
    }
}

impl DomainPackArtifact {
    pub fn new(path: impl Into<String>, bytes: &[u8], content_type: Option<String>) -> Self {
        Self {
            path: path.into(),
            hash: artifact_hash(bytes),
            content_type,
            size_bytes: Some(bytes.len() as u64),
            relative_path: None,
            sha256_hex: None,
        }
    }

    pub fn validate(&self) -> Result<()> {
        validate_domain_pack_artifact_path(&self.path)?;
        validate_driver_hash(&self.hash)
    }

    pub fn verify_bytes(&self, bytes: &[u8]) -> Result<()> {
        self.validate()?;
        let actual = artifact_hash(bytes);
        if actual != self.hash {
            return Err(RivunStoreError::DomainPackArtifactHashMismatch {
                path: self.path.clone(),
                expected: self.hash.clone(),
                actual,
            });
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
            deprecated_reason: None,
            migrations: Vec::new(),
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
struct DomainPackRegistrySigningPayload<'a> {
    schema_version: u8,
    generated_by: Option<&'a str>,
    channel: Option<&'a str>,
    operator_node_id: Uuid,
    operator_public_key: &'a str,
    entries: &'a [DomainPackRegistryEntry],
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

#[derive(Debug, Serialize)]
struct RegistryInstallPlanSigningPayload<'a> {
    schema_version: u8,
    registry_hash: &'a str,
    registry_entries: usize,
    registry_operator_node_id: Option<Uuid>,
    publication_hash: Option<&'a str>,
    requested_at_micros: u64,
    target: Option<&'a str>,
    labels: &'a [String],
    entries: &'a [RegistryInstallPlanEntry],
    planner_node_id: Uuid,
    planner_public_key: &'a str,
}

pub fn driver_hash(wasm: &[u8]) -> String {
    artifact_hash(wasm)
}

pub fn artifact_hash(bytes: &[u8]) -> String {
    format!("{DRIVER_HASH_PREFIX}{}", blake3::hash(bytes).to_hex())
}

pub fn registry_hash(registry: &DriverRegistry) -> Result<String> {
    registry.validate()?;
    registry_hash_no_validate(registry)
}

pub(crate) fn registry_hash_no_validate(registry: &DriverRegistry) -> Result<String> {
    Ok(artifact_hash(&serde_json::to_vec(registry)?))
}

pub fn domain_pack_registry_hash(registry: &DomainPackRegistry) -> Result<String> {
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
        return Err(RivunStoreError::InvalidHash(hash.to_string()));
    }
    Ok(())
}

fn validate_domain_pack_artifact_path(path: &str) -> Result<()> {
    validate_bundle_path(path).map_err(|error| match error {
        RivunStoreError::InvalidRegistryBundlePath(path) => {
            RivunStoreError::InvalidDomainPackArtifactPath(path)
        }
        other => other,
    })
}

fn compare_registry_field<T>(field: &'static str, expected: T, actual: T) -> Result<()>
where
    T: PartialEq + ToString,
{
    if expected != actual {
        return Err(RivunStoreError::RegistryFieldMismatch {
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
        return Err(RivunStoreError::RegistryPublicationFieldMismatch { field });
    }
    Ok(())
}

fn compare_registry_install_plan_field<T>(field: &'static str, expected: T, actual: T) -> Result<()>
where
    T: PartialEq,
{
    if expected != actual {
        return Err(RivunStoreError::RegistryInstallPlanFieldMismatch { field });
    }
    Ok(())
}

fn compare_registry_install_plan_entry_field<T>(
    plan_entry: &RegistryInstallPlanEntry,
    registry_entry: &DriverRegistryEntry,
    field: &'static str,
    expected: T,
    actual: T,
) -> Result<()>
where
    T: PartialEq,
{
    if expected != actual {
        return Err(RivunStoreError::RegistryInstallPlanEntryMismatch {
            action: plan_entry.action.clone(),
            version: registry_entry.version.clone(),
            field,
        });
    }
    Ok(())
}

fn validate_bundle_path(path: &str) -> Result<()> {
    let path = std::path::Path::new(path);
    if path.is_absolute() || path.as_os_str().is_empty() {
        return Err(RivunStoreError::InvalidRegistryBundlePath(
            path.display().to_string(),
        ));
    }
    for component in path.components() {
        match component {
            std::path::Component::Normal(_) => {}
            _ => {
                return Err(RivunStoreError::InvalidRegistryBundlePath(
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

fn registry_status_rank(status: DriverRegistryStatus) -> u8 {
    match status {
        DriverRegistryStatus::Active => 0,
        DriverRegistryStatus::Deprecated => 1,
        DriverRegistryStatus::Revoked => 2,
    }
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
        return Err(RivunStoreError::RegistryMergeConflict {
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
        return Err(RivunStoreError::InvalidKeyLength {
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

    fn domain_pack_entry(id: &str, version: &str) -> DomainPackRegistryEntry {
        DomainPackRegistryEntry {
            id: id.to_string(),
            name: format!("{id} pack"),
            version: version.to_string(),
            status: DomainPackStatus::Active,
            risk: DomainPackRisk::High,
            description: Some("Domain pack for integration tests".to_string()),
            deprecated_reason: None,
            revoked_reason: None,
            author_node_id: Uuid::nil(),
            compatibility: DomainPackCompatibility {
                min_rivun_version: Some("1.0.0".to_string()),
                max_rivun_version: None,
                runtimes: vec!["wasmtime".to_string()],
                abi_versions: vec![DRIVER_ABI_VERSION],
                rivun_version_req: String::new(),
                abi_version_req: String::new(),
                capabilities_required: vec![],
                capabilities_provided: vec![],
            },
            manifest: DomainPackArtifact::new(
                format!("{id}/pack.toml"),
                b"schema_version = 1\nid = \"test\"\n",
                Some("application/toml".to_string()),
            ),
            archive: Some(DomainPackArtifact::new(
                format!("{id}/{id}-pack.tar.zst"),
                b"archive",
                Some("application/zstd".to_string()),
            )),
            policies: vec![DomainPackArtifact::new(
                format!("{id}/policies/action-policy.toml"),
                b"[[rules]]\neffect = \"allow\"\n",
                Some("application/toml".to_string()),
            )],
            schemas: vec![DomainPackArtifact::new(
                format!("{id}/schemas/subjects.md"),
                b"# Subjects\n",
                Some("text/markdown".to_string()),
            )],
            drivers: vec![],
            metadata: BTreeMap::new(),
            dependencies: vec![],
            labels: vec!["phase-4".to_string(), "domain-pack".to_string()],
        }
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
            Err(RivunStoreError::RegistryOperatorPublicKeyMismatch { .. })
        ));
    }

    #[test]
    fn registry_index_response_can_require_signed_registry() {
        let registry = DriverRegistry::empty(None);
        let response = RegistryIndexResponse::new(Uuid::nil(), Some(registry), None);

        response.verify(false, None).unwrap();
        assert!(matches!(
            response.verify(true, None),
            Err(RivunStoreError::MissingRegistrySignature)
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
    fn registry_merge_prefers_deprecated_entry_over_active_version() {
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
        right.deprecate("echo", "0.1.0", "use >=0.2.0").unwrap();

        let report = left.merge_from(&right).unwrap();

        assert_eq!(report.deprecated_overrides, 1);
        assert_eq!(report.revoked_overrides, 0);
        assert_eq!(left.entries[0].status, DriverRegistryStatus::Deprecated);
        assert_eq!(
            left.entries[0].deprecated_reason.as_deref(),
            Some("use >=0.2.0")
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
            Err(RivunStoreError::RegistryMergeConflict {
                field: "wasm_hash",
                ..
            })
        ));
    }

    #[test]
    fn driver_version_requirement_matches_common_ranges() {
        let caret = DriverVersionRequirement::parse("^1.2.3").unwrap();
        assert!(caret.matches(DriverVersion::parse("1.2.3").unwrap()));
        assert!(caret.matches(DriverVersion::parse("1.9.9").unwrap()));
        assert!(!caret.matches(DriverVersion::parse("2.0.0").unwrap()));

        let tilde = DriverVersionRequirement::parse("~1.2.3").unwrap();
        assert!(tilde.matches(DriverVersion::parse("1.2.9").unwrap()));
        assert!(!tilde.matches(DriverVersion::parse("1.3.0").unwrap()));

        let comparators = DriverVersionRequirement::parse(">=1.0.0, <2.0.0").unwrap();
        assert!(comparators.matches(DriverVersion::parse("1.5.0").unwrap()));
        assert!(!comparators.matches(DriverVersion::parse("0.9.9").unwrap()));
        assert!(!comparators.matches(DriverVersion::parse("2.0.0").unwrap()));

        assert!(matches!(
            DriverVersionRequirement::parse("^1"),
            Err(RivunStoreError::InvalidDriverVersionRequirement(_))
        ));
    }

    #[test]
    fn driver_abi_requirement_parses_ranges() {
        let exact = DriverAbiRequirement::parse("1").unwrap();
        assert!(exact.matches(1));
        assert!(!exact.matches(2));

        let range = DriverAbiRequirement::parse(">=1, <=2").unwrap();
        assert!(range.matches(1));
        assert!(range.matches(2));
        assert!(!range.matches(3));

        assert!(matches!(
            DriverAbiRequirement::parse(">=one"),
            Err(RivunStoreError::InvalidDriverAbiRequirement(_))
        ));
    }

    #[test]
    fn registry_resolve_selects_highest_active_compatible_version() {
        let author = Keypair::generate();
        let mut registry = DriverRegistry::empty(Some("test".to_string()));
        for version in ["1.0.0", "1.2.0", "1.3.0", "2.0.0"] {
            let manifest = DriverManifest::new(
                "echo",
                version,
                "echo",
                wasm(),
                DriverPermissions::none(),
                None,
                &author,
            )
            .unwrap();
            registry.add_manifest(&manifest, None).unwrap();
        }
        registry.revoke("echo", "1.3.0", "bad release").unwrap();

        let selected = registry
            .resolve("echo", ">=1.0.0, <2.0.0", Some(DRIVER_ABI_VERSION))
            .unwrap();

        assert_eq!(selected.version, "1.2.0");
        assert_eq!(selected.status, DriverRegistryStatus::Active);
    }

    #[test]
    fn registry_resolve_can_filter_by_abi_version() {
        let author = Keypair::generate();
        let mut registry = DriverRegistry::empty(Some("test".to_string()));
        for version in ["1.0.0", "1.1.0"] {
            let manifest = DriverManifest::new(
                "echo",
                version,
                "echo",
                wasm(),
                DriverPermissions::none(),
                None,
                &author,
            )
            .unwrap();
            registry.add_manifest(&manifest, None).unwrap();
        }
        registry
            .entries
            .iter_mut()
            .find(|entry| entry.version == "1.1.0")
            .unwrap()
            .abi_version = DRIVER_ABI_VERSION + 1;

        let selected = registry
            .resolve("echo", "*", Some(DRIVER_ABI_VERSION))
            .unwrap();

        assert_eq!(selected.version, "1.0.0");
        assert_eq!(
            registry.resolve("echo", "*", None).unwrap().version,
            "1.1.0"
        );
    }

    #[test]
    fn registry_resolve_can_filter_by_abi_requirement() {
        let author = Keypair::generate();
        let mut registry = DriverRegistry::empty(Some("test".to_string()));
        for version in ["1.0.0", "1.1.0", "1.2.0"] {
            let manifest = DriverManifest::new(
                "echo",
                version,
                "echo",
                wasm(),
                DriverPermissions::none(),
                None,
                &author,
            )
            .unwrap();
            registry.add_manifest(&manifest, None).unwrap();
        }
        registry
            .entries
            .iter_mut()
            .find(|entry| entry.version == "1.1.0")
            .unwrap()
            .abi_version = DRIVER_ABI_VERSION + 1;
        registry
            .entries
            .iter_mut()
            .find(|entry| entry.version == "1.2.0")
            .unwrap()
            .abi_version = DRIVER_ABI_VERSION + 2;

        let selected = registry
            .resolve_compatible("echo", "*", Some(">=1,<=2"))
            .unwrap();

        assert_eq!(selected.version, "1.1.0");
        assert_eq!(selected.abi_version, DRIVER_ABI_VERSION + 1);
    }

    #[test]
    fn registry_resolve_rejects_invalid_versions_and_missing_matches() {
        let author = Keypair::generate();
        let manifest = DriverManifest::new(
            "echo",
            "1.0.0",
            "echo",
            wasm(),
            DriverPermissions::none(),
            None,
            &author,
        )
        .unwrap();
        let mut registry = DriverRegistry::empty(Some("test".to_string()));
        registry.add_manifest(&manifest, None).unwrap();

        assert!(matches!(
            registry.resolve("echo", "^2.0.0", None),
            Err(RivunStoreError::NoCompatibleRegistryEntry { .. })
        ));

        registry.entries[0].version = "latest".to_string();
        assert!(matches!(
            registry.resolve("echo", "*", None),
            Err(RivunStoreError::InvalidDriverVersion(version)) if version == "latest"
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
            Err(RivunStoreError::RegistryPublicationFieldMismatch {
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
            Err(RivunStoreError::MissingRegistrySignature)
        ));
    }

    #[test]
    fn registry_install_plan_signs_and_verifies_selected_versions() {
        let author = Keypair::generate();
        let operator = Keypair::generate();
        let planner = Keypair::generate();
        let mut registry = DriverRegistry::empty(Some("test".to_string()));
        for version in ["1.0.0", "1.2.0", "2.0.0"] {
            let manifest = DriverManifest::new(
                "echo",
                version,
                "echo",
                wasm(),
                DriverPermissions::none(),
                None,
                &author,
            )
            .unwrap();
            registry
                .add_manifest(&manifest, Some(format!("echo-{version}.manifest.toml")))
                .unwrap();
        }
        registry.sign(&operator).unwrap();
        let planner_public_key = STANDARD_NO_PAD.encode(planner.verifying_key().to_bytes());

        let plan = RegistryInstallPlan::new(
            &registry,
            &[RegistryInstallPlanRequest::new("echo", "^1.0.0", None)],
            &planner,
            4242,
            Some("factory-a".to_string()),
            vec!["stable".to_string()],
            Some(artifact_hash(b"publication")),
        )
        .unwrap();

        plan.verify_for_registry(&registry, Some(&planner_public_key))
            .unwrap();
        assert_eq!(plan.registry_hash, registry_hash(&registry).unwrap());
        assert_eq!(plan.registry_operator_node_id, Some(operator.node_id()));
        assert_eq!(plan.entries.len(), 1);
        assert_eq!(plan.entries[0].selected_version, "1.2.0");
        assert_eq!(plan.entries[0].requirement, "^1.0.0");
        assert_eq!(
            plan.entries[0].manifest_path.as_deref(),
            Some("echo-1.2.0.manifest.toml")
        );
    }

    #[test]
    fn registry_install_plan_records_abi_requirement_and_migrations() {
        let author = Keypair::generate();
        let operator = Keypair::generate();
        let planner = Keypair::generate();
        let mut registry = DriverRegistry::empty(Some("test".to_string()));
        for version in ["1.0.0", "2.0.0"] {
            let manifest = DriverManifest::new(
                "echo",
                version,
                "echo",
                wasm(),
                DriverPermissions::none(),
                None,
                &author,
            )
            .unwrap();
            registry.add_manifest(&manifest, None).unwrap();
        }
        registry
            .entries
            .iter_mut()
            .find(|entry| entry.version == "2.0.0")
            .unwrap()
            .abi_version = DRIVER_ABI_VERSION + 1;
        registry
            .add_migration(
                "echo",
                "2.0.0",
                DriverRegistryMigration::new(
                    "^1.0.0",
                    Some("=1".to_string()),
                    true,
                    Some("echo-migrate".to_string()),
                    Some("0.1.0".to_string()),
                    Some("copy persisted state before switching ABI".to_string()),
                ),
            )
            .unwrap();
        registry.sign(&operator).unwrap();

        let plan = RegistryInstallPlan::new(
            &registry,
            &[RegistryInstallPlanRequest::new_with_abi_requirement(
                "echo",
                "*",
                Some(">=1,<=2".to_string()),
            )],
            &planner,
            4242,
            None,
            Vec::new(),
            None,
        )
        .unwrap();

        plan.verify_for_registry(&registry, None).unwrap();
        assert_eq!(plan.entries[0].selected_version, "2.0.0");
        assert_eq!(
            plan.entries[0].requested_abi_requirement.as_deref(),
            Some(">=1,<=2")
        );
        assert_eq!(plan.entries[0].abi_version, DRIVER_ABI_VERSION + 1);
        assert_eq!(plan.entries[0].migrations.len(), 1);
        assert!(plan.entries[0].migrations[0].requires_operator_approval);
    }

    #[test]
    fn registry_install_plan_rejects_registry_mutation() {
        let author = Keypair::generate();
        let operator = Keypair::generate();
        let planner = Keypair::generate();
        let manifest = DriverManifest::new(
            "echo",
            "1.0.0",
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
        let plan = RegistryInstallPlan::new(
            &registry,
            &[RegistryInstallPlanRequest::new("echo", "^1.0.0", None)],
            &planner,
            4242,
            None,
            Vec::new(),
            None,
        )
        .unwrap();

        registry.generated_by = Some("mutated".to_string());
        registry.sign(&operator).unwrap();

        assert!(matches!(
            plan.verify_for_registry(&registry, None),
            Err(RivunStoreError::RegistryInstallPlanFieldMismatch {
                field: "registry_hash"
            })
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
            Err(RivunStoreError::InvalidRegistryBundlePath(_))
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
            Err(RivunStoreError::RegistryBundleDriverHashMismatch { .. })
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
            Err(RivunStoreError::RegistryBundleArtifactMetadataIncomplete {
                artifact: "manifest",
                ..
            })
        ));
    }

    #[test]
    fn registry_bundle_manifest_response_enforces_request_requirements() {
        let bundle = RegistryBundleManifest::new(
            Some("test".to_string()),
            "registry.index.toml".to_string(),
            artifact_hash(b"registry"),
            None,
            None,
            vec![RegistryBundleEntry {
                action: "echo".to_string(),
                version: "0.1.0".to_string(),
                name: "echo".to_string(),
                abi_version: DRIVER_ABI_VERSION,
                wasm_hash: artifact_hash(b"driver"),
                author_node_id: Uuid::nil(),
                status: DriverRegistryStatus::Active,
                manifest_path: Some("manifests/echo.toml".to_string()),
                manifest_hash: Some(artifact_hash(b"manifest")),
                driver_path: None,
                driver_hash: None,
            }],
        );
        let response = RegistryBundleManifestResponse::new(Uuid::nil(), Some(bundle), None);

        response
            .verify(&RegistryBundleManifestRequest::default())
            .unwrap();
        assert!(matches!(
            response.verify(&RegistryBundleManifestRequest {
                schema_version: REGISTRY_BUNDLE_SCHEMA_VERSION,
                require_publication: true,
                require_drivers: false,
            }),
            Err(RivunStoreError::RegistryBundlePublicationMetadataIncomplete)
        ));
        assert!(matches!(
            response.verify(&RegistryBundleManifestRequest {
                schema_version: REGISTRY_BUNDLE_SCHEMA_VERSION,
                require_publication: false,
                require_drivers: true,
            }),
            Err(RivunStoreError::RegistryBundleArtifactMetadataIncomplete {
                artifact: "driver",
                ..
            })
        ));
    }

    #[test]
    fn domain_pack_registry_round_trips_json_and_toml() {
        let mut registry = DomainPackRegistry::empty(Some("test".to_string()));
        registry.channel = Some("stable".to_string());
        registry
            .add_entry(domain_pack_entry("agentic-dev", "1.2.3"))
            .unwrap();

        let json = registry.to_json_string().unwrap();
        let from_json = DomainPackRegistry::from_json_str(&json).unwrap();
        assert_eq!(from_json, registry);

        let toml = registry.to_toml_string().unwrap();
        let from_toml = DomainPackRegistry::from_toml_str(&toml).unwrap();
        assert_eq!(from_toml, registry);
        assert_eq!(
            domain_pack_registry_hash(&registry).unwrap(),
            domain_pack_registry_hash(&from_toml).unwrap()
        );
    }

    #[test]
    fn domain_pack_registry_signs_and_detects_tampering() {
        let operator = Keypair::generate();
        let mut registry = DomainPackRegistry::empty(Some("test".to_string()));
        registry
            .add_entry(domain_pack_entry("cloud-ops", "0.3.0"))
            .unwrap();
        registry.sign(&operator).unwrap();
        let operator_public_key = registry.operator_public_key.clone().unwrap();

        registry.verify_signature().unwrap();
        registry
            .verify_signature_for_operator(&operator_public_key)
            .unwrap();

        registry.entries[0].risk = DomainPackRisk::Critical;
        assert!(matches!(
            registry.verify_signature(),
            Err(RivunStoreError::InvalidDomainPackRegistrySignature)
        ));
    }

    #[test]
    fn domain_pack_registry_add_entry_clears_signature() {
        let operator = Keypair::generate();
        let mut registry = DomainPackRegistry::empty(Some("test".to_string()));
        registry
            .add_entry(domain_pack_entry("industrial", "1.0.0"))
            .unwrap();
        registry.sign(&operator).unwrap();
        assert!(registry.signature.is_some());

        registry
            .add_entry(domain_pack_entry("personal-ai", "1.0.0"))
            .unwrap();
        assert!(registry.signature.is_none());
        assert_eq!(registry.entries.len(), 2);
    }

    #[test]
    fn domain_pack_registry_rejects_duplicates() {
        let entry = domain_pack_entry("smart-building", "2.0.0");
        let registry = DomainPackRegistry {
            schema_version: DOMAIN_PACK_REGISTRY_SCHEMA_VERSION,
            generated_by: None,
            channel: None,
            operator_node_id: None,
            operator_public_key: None,
            signature: None,
            entries: vec![entry.clone(), entry],
        };

        assert!(matches!(
            registry.validate(),
            Err(RivunStoreError::DuplicateDomainPackRegistryEntry {
                id,
                version
            }) if id == "smart-building" && version == "2.0.0"
        ));
    }

    #[test]
    fn domain_pack_registry_rejects_path_traversal() {
        let mut entry = domain_pack_entry("healthcare", "1.0.0");
        entry.manifest.path = "../pack.toml".to_string();

        assert!(matches!(
            entry.validate(),
            Err(RivunStoreError::InvalidDomainPackArtifactPath(_))
        ));
    }

    #[test]
    fn domain_pack_artifact_verifies_hash() {
        let artifact = DomainPackArtifact::new("packs/finance/pack.toml", b"expected", None);
        artifact.verify_bytes(b"expected").unwrap();

        assert!(matches!(
            artifact.verify_bytes(b"tampered"),
            Err(RivunStoreError::DomainPackArtifactHashMismatch { path, .. })
                if path == "packs/finance/pack.toml"
        ));
    }

    #[test]
    fn domain_pack_registry_validates_compatibility_versions() {
        let mut entry = domain_pack_entry("finance", "1.0.0");
        entry.compatibility.min_rivun_version = Some("v1".to_string());

        assert!(matches!(
            entry.validate(),
            Err(RivunStoreError::InvalidDriverVersion(version)) if version == "v1"
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
            Err(RivunStoreError::RevokedRegistryEntry { .. })
        ));
    }

    #[test]
    fn registry_deprecates_manifest_version_and_resolution_skips_it() {
        let author = Keypair::generate();
        let operator = Keypair::generate();
        let older = DriverManifest::new(
            "echo",
            "1.0.0",
            "echo",
            wasm(),
            DriverPermissions::none(),
            None,
            &author,
        )
        .unwrap();
        let newer = DriverManifest::new(
            "echo",
            "1.1.0",
            "echo",
            wasm(),
            DriverPermissions::none(),
            None,
            &author,
        )
        .unwrap();
        let mut registry = DriverRegistry::empty(None);
        registry.add_manifest(&older, None).unwrap();
        registry.add_manifest(&newer, None).unwrap();
        registry.sign(&operator).unwrap();

        registry
            .deprecate("echo", "1.1.0", "use 2.x migration")
            .unwrap();

        let deprecated = registry
            .entries
            .iter()
            .find(|entry| entry.version == "1.1.0")
            .unwrap();
        assert_eq!(deprecated.status, DriverRegistryStatus::Deprecated);
        assert_eq!(
            deprecated.deprecated_reason.as_deref(),
            Some("use 2.x migration")
        );
        assert!(registry.signature.is_none());
        registry.verify_manifest(&newer).unwrap();
        assert_eq!(
            registry.resolve("echo", "^1.0.0", None).unwrap().version,
            "1.0.0"
        );
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
            Err(RivunStoreError::InvalidRegistrySignature)
        ));
    }

    #[test]
    fn registry_signature_is_required_when_verified() {
        let registry = DriverRegistry::empty(None);

        assert!(matches!(
            registry.verify_signature(),
            Err(RivunStoreError::MissingRegistrySignature)
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
            Err(RivunStoreError::RevokedRegistryEntry { .. })
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
            Err(RivunStoreError::RegistryFieldMismatch {
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
            Err(RivunStoreError::HashMismatch { .. })
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
            Err(RivunStoreError::ActionMismatch { .. })
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
            Err(RivunStoreError::InvalidSignature)
        ));
    }
}
