//! Rivun node daemon.
//!
//! The node combines static peer discovery, encrypted UDP transport, optional
//! Ed25519 frame verification, and WASM action dispatch.

use anyhow::{Context, Result, anyhow, bail};
pub mod actors;
pub mod config;
pub mod durable_replay;
use base64::{Engine as _, engine::general_purpose::STANDARD_NO_PAD};
use bytes::Bytes;
pub use config::*;
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet, HashMap, HashSet, VecDeque},
    fs,
    fs::OpenOptions,
    io::{Read, Write},
    net::{SocketAddr, TcpListener, TcpStream},
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, SystemTime},
};
use tracing::{info, warn};
use uuid::Uuid;
use rivun_agent::{AGENT_CONTENT_TYPE, AgentMessage};
use rivun_capability::{
    CAPABILITY_CONTENT_TYPE, CAPABILITY_QUERY_SUBJECT, CAPABILITY_RESPONSE_SUBJECT,
    CapabilityAdvertisement, CapabilityGrant, CapabilityId, CapabilityQuery, CapabilityRequirement,
    CapabilityResponse, CapabilitySet, DriverPermissions, JsonlCapabilityCache,
    capabilities_for_driver,
};
use rivun_core::{ED25519_SIGNATURE_LEN, RivunFlags, RivunFrame, now_micros};
use rivun_crypto::{
    Keypair, POA_ATTESTATION_CONTENT_TYPE, POA_ATTESTATION_REQUEST_SUBJECT,
    POA_ATTESTATION_RESPONSE_SUBJECT, POA_VALIDATOR_SET_CONTENT_TYPE,
    POA_VALIDATOR_SET_REQUEST_SUBJECT, POA_VALIDATOR_SET_RESPONSE_SUBJECT, PoaValidatorSetRequest,
    PoaValidatorSetResponse, PublicKey, SignedPoaValidatorSet, sign_frame,
    sign_poa_attestation_request, verify_frame, verify_poa_certificate,
};
use rivun_envelope::{
    MAGIC_BYTES as ZENV_MAGIC_BYTES, MAX_CONTENT_TYPE_LEN as ZENV_MAX_CONTENT_TYPE_LEN,
    MAX_METADATA_LEN as ZENV_MAX_METADATA_LEN, MAX_SUBJECT_LEN as ZENV_MAX_SUBJECT_LEN,
    RivunEnvelope, RivunEnvelopeRef, RivunMessageKind,
};
use rivun_ledger::{
    PactReceiptReference, RECEIPT_REPLICATION_CONTENT_TYPE, RECEIPT_REPLICATION_REQUEST_SUBJECT,
    RECEIPT_REPLICATION_RESPONSE_SUBJECT, ReceiptJournalStore, ReceiptReplicationRequest,
    ReceiptReplicationResponse, SignedActionReceipt,
};
use rivun_memory::{MemoryJournalStore, MemoryPut, MemoryStore};
use rivun_net::{MAX_DATAGRAM_SIZE, Peer, TransportKey, RivunEndpoint, RivunEndpointConfig};
use rivun_pact::{PACT_CONTENT_TYPE, PACT_RECORD_SUBJECT, RivunPact};
use rivun_policy::{PolicyInput, PolicyRule, PolicySet};
use rivun_router::{RouteDecision, RouteMessage, RouteRule, RouteTable};
use rivun_runtime::{
    AsyncCompiledDriver, AsyncWasmExecutor, ExecutionLimits, HostCallKind, HostCallRecord,
    WasmDriver, WasmExecutor,
};
use rivun_schema::{MessageContract, MessageContractSet, MessageParts};
use rivun_store::{
    DriverManifest, DriverRegistry, REGISTRY_BUNDLE_MANIFEST_CONTENT_TYPE,
    REGISTRY_BUNDLE_MANIFEST_REQUEST_SUBJECT, REGISTRY_BUNDLE_MANIFEST_RESPONSE_SUBJECT,
    REGISTRY_INDEX_CONTENT_TYPE, REGISTRY_INDEX_REQUEST_SUBJECT, REGISTRY_INDEX_RESPONSE_SUBJECT,
    RegistryBundleManifest, RegistryBundleManifestRequest, RegistryBundleManifestResponse,
    RegistryIndexRequest, RegistryIndexResponse,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RivunNodeConfig {
    pub bind: String,
    pub key_file: PathBuf,
    #[serde(default = "default_require_signed")]
    pub require_signed: bool,
    #[serde(default)]
    pub max_datagram_size: Option<usize>,
    #[serde(default)]
    pub peers: Vec<PeerConfig>,
    #[serde(default)]
    pub drivers: Vec<DriverConfig>,
    #[serde(default)]
    pub runtime: RuntimeConfig,
    #[serde(default)]
    pub action_runtime_limits: BTreeMap<String, ActionRuntimeLimits>,
    #[serde(default)]
    pub security: SecurityConfig,
    #[serde(default)]
    pub trust: TrustConfig,
    #[serde(default)]
    pub poa: PoaConfig,
    #[serde(default)]
    pub receipts: ReceiptsConfig,
    #[serde(default)]
    pub registry: RegistryConfig,
    #[serde(default)]
    pub memory: MemoryConfig,
    #[serde(default)]
    pub capability_policy: CapabilityPolicyConfig,
    #[serde(default)]
    pub capability_cache: CapabilityCacheConfig,
    #[serde(default)]
    pub discovery: DiscoveryConfig,
    #[serde(default)]
    pub observability: ObservabilityConfig,
    #[serde(default)]
    pub message_policy: MessagePolicyConfig,
    #[serde(default)]
    pub message_schema: MessageSchemaConfig,
    #[serde(default)]
    pub routes: Vec<RouteRule>,
    #[serde(default)]
    pub swarm: SwarmConfig,
    #[serde(default)]
    pub gossip: GossipConfig,
    #[serde(default)]
    pub mesh: MeshConfig,
}

impl RivunNodeConfig {
    pub fn from_toml_str(input: &str) -> Result<Self> {
        Ok(toml::from_str(input)?)
    }

    pub fn from_path(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let input = fs::read_to_string(path)
            .with_context(|| format!("failed to read node config {}", path.display()))?;
        let config = Self::from_toml_str(&input)?;
        Ok(resolve_config_paths(config, path))
    }

    pub fn validate(&self) -> Result<ConfigValidationReport> {
        validate_config(self)
    }
}

fn default_require_signed() -> bool {
    true
}

fn resolve_config_paths(mut config: RivunNodeConfig, config_path: &Path) -> RivunNodeConfig {
    let base_dir = config_path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    config.key_file = resolve_relative_path(base_dir, &config.key_file);
    for driver in &mut config.drivers {
        driver.path = resolve_relative_path(base_dir, &driver.path);
        if let Some(manifest) = driver.manifest.take() {
            driver.manifest = Some(resolve_relative_path(base_dir, &manifest));
        }
    }
    if let Some(dir) = config.receipts.dir.take() {
        config.receipts.dir = Some(resolve_relative_path(base_dir, &dir));
    }
    if let Some(path) = config.receipts.path.take() {
        config.receipts.path = Some(resolve_relative_path(base_dir, &path));
    }
    if let Some(path) = config.registry.path.take() {
        config.registry.path = Some(resolve_relative_path(base_dir, &path));
    }
    if let Some(path) = config.registry.bundle_path.take() {
        config.registry.bundle_path = Some(resolve_relative_path(base_dir, &path));
    }
    if let Some(path) = config.poa.validator_set.take() {
        config.poa.validator_set = Some(resolve_relative_path(base_dir, &path));
    }
    if let Some(dir) = config.memory.dir.take() {
        config.memory.dir = Some(resolve_relative_path(base_dir, &dir));
    }
    if let Some(path) = config.memory.path.take() {
        config.memory.path = Some(resolve_relative_path(base_dir, &path));
    }
    if let Some(path) = config.capability_cache.path.take() {
        config.capability_cache.path = Some(resolve_relative_path(base_dir, &path));
    }
    if let Some(path) = config.security.durable_replay_store_path.take() {
        config.security.durable_replay_store_path = Some(resolve_relative_path(base_dir, &path));
    }
    if let Some(path) = config.discovery.announcement_cache.take() {
        config.discovery.announcement_cache = Some(resolve_relative_path(base_dir, &path));
    }
    for contract in &mut config.message_schema.contracts {
        contract.path = resolve_relative_path(base_dir, &contract.path);
    }
    config
}

fn resolve_relative_path(base_dir: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        base_dir.join(path)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerConfig {
    pub node_id: Uuid,
    pub addr: String,
    pub public_key: String,
    pub transport_key: String,
    #[serde(default)]
    pub transport_key_epoch: Option<u64>,
    #[serde(default)]
    pub transport_key_rotated_at_micros: Option<u64>,
    #[serde(default)]
    pub trust: PeerTrustConfig,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct TrustConfig {
    #[serde(default)]
    pub require_peer_expiry: bool,
    #[serde(default)]
    pub max_transport_key_age_micros: Option<u64>,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PeerTrustStatus {
    #[default]
    Trusted,
    Quarantined,
    Revoked,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PeerTrustConfig {
    #[serde(default)]
    pub status: PeerTrustStatus,
    #[serde(default = "default_true")]
    pub allow_send: bool,
    #[serde(default = "default_true")]
    pub allow_receive: bool,
    #[serde(default = "default_true")]
    pub allow_forward: bool,
    #[serde(default = "default_true")]
    pub allow_poa_attestation: bool,
    #[serde(default)]
    pub expires_at_micros: Option<u64>,
    #[serde(default)]
    pub labels: Vec<String>,
}

impl Default for PeerTrustConfig {
    fn default() -> Self {
        Self {
            status: PeerTrustStatus::Trusted,
            allow_send: true,
            allow_receive: true,
            allow_forward: true,
            allow_poa_attestation: true,
            expires_at_micros: None,
            labels: Vec::new(),
        }
    }
}

impl PeerTrustConfig {
    pub fn is_trusted(&self) -> bool {
        self.status == PeerTrustStatus::Trusted
    }

    pub fn allows_transport(&self) -> bool {
        self.is_trusted() && (self.allow_send || self.allow_receive || self.allow_forward)
    }

    pub fn allows_send(&self) -> bool {
        self.is_trusted() && self.allow_send
    }

    pub fn allows_receive(&self) -> bool {
        self.is_trusted() && self.allow_receive
    }

    pub fn allows_forward(&self) -> bool {
        self.is_trusted() && self.allow_forward && self.allow_send
    }

    pub fn allows_poa_attestation(&self) -> bool {
        self.is_trusted() && self.allow_poa_attestation && self.allow_send
    }
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoaValidatorConfig {
    pub node_id: Uuid,
    pub public_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoaConfig {
    #[serde(default = "default_poa_required_threshold")]
    pub required_threshold: u16,
    #[serde(default)]
    pub validators: Vec<PoaValidatorConfig>,
    #[serde(default)]
    pub validator_set: Option<PathBuf>,
    #[serde(default)]
    pub validator_set_authority: Option<String>,
}

impl Default for PoaConfig {
    fn default() -> Self {
        Self {
            required_threshold: default_poa_required_threshold(),
            validators: Vec::new(),
            validator_set: None,
            validator_set_authority: None,
        }
    }
}

fn default_poa_required_threshold() -> u16 {
    1
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriverConfig {
    pub action: String,
    pub path: PathBuf,
    #[serde(default)]
    pub manifest: Option<PathBuf>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct RuntimeConfig {
    pub max_memory_bytes: Option<usize>,
    pub fuel: Option<u64>,
    pub timeout_ms: Option<u64>,
    pub max_output_bytes: Option<usize>,
    /// Executes drivers through Tokio/Wasmtime async APIs while preserving the
    /// stable core driver ABI. Disabled by default for an explicit migration.
    #[serde(default)]
    pub async_execution: bool,
    #[serde(default)]
    pub permissions: DriverPermissions,
}

/// Optional per-action ceilings. Every configured value must be no greater
/// than the global runtime limit, so an action profile can only reduce scope.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct ActionRuntimeLimits {
    pub max_memory_bytes: Option<usize>,
    pub fuel: Option<u64>,
    pub timeout_ms: Option<u64>,
    pub max_output_bytes: Option<usize>,
}

impl ActionRuntimeLimits {
    fn apply(self, global: ExecutionLimits) -> ExecutionLimits {
        ExecutionLimits {
            max_memory_bytes: self.max_memory_bytes.unwrap_or(global.max_memory_bytes),
            fuel: self.fuel.unwrap_or(global.fuel),
            timeout_ms: self.timeout_ms.unwrap_or(global.timeout_ms),
            max_output_bytes: self.max_output_bytes.unwrap_or(global.max_output_bytes),
            permissions: global.permissions,
        }
    }
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            max_memory_bytes: None,
            fuel: None,
            timeout_ms: None,
            max_output_bytes: None,
            async_execution: false,
            permissions: DriverPermissions::none(),
        }
    }
}

impl RuntimeConfig {
    fn to_limits(self) -> ExecutionLimits {
        let defaults = ExecutionLimits::default();
        ExecutionLimits {
            max_memory_bytes: self.max_memory_bytes.unwrap_or(defaults.max_memory_bytes),
            fuel: self.fuel.unwrap_or(defaults.fuel),
            timeout_ms: self.timeout_ms.unwrap_or(defaults.timeout_ms),
            max_output_bytes: self.max_output_bytes.unwrap_or(defaults.max_output_bytes),
            permissions: self.permissions,
        }
    }
}

const DEFAULT_RECEIPT_FSYNC_INTERVAL_WRITES: u64 = 64;

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ReceiptFsyncPolicy {
    Always,
    Interval,
    #[default]
    Off,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReceiptsConfig {
    #[serde(default)]
    pub dir: Option<PathBuf>,
    #[serde(default)]
    pub path: Option<PathBuf>,
    #[serde(default)]
    pub fsync: ReceiptFsyncPolicy,
    #[serde(default)]
    pub fsync_interval_writes: Option<u64>,
}

impl Default for ReceiptsConfig {
    fn default() -> Self {
        Self {
            dir: None,
            path: None,
            fsync: ReceiptFsyncPolicy::Off,
            fsync_interval_writes: None,
        }
    }
}

impl ReceiptsConfig {
    fn fsync_interval_writes(&self) -> u64 {
        self.fsync_interval_writes
            .unwrap_or(DEFAULT_RECEIPT_FSYNC_INTERVAL_WRITES)
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RegistryConfig {
    #[serde(default)]
    pub path: Option<PathBuf>,
    #[serde(default)]
    pub require_signature: bool,
    #[serde(default)]
    pub bundle_path: Option<PathBuf>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MemoryConfig {
    #[serde(default)]
    pub dir: Option<PathBuf>,
    #[serde(default)]
    pub path: Option<PathBuf>,
    #[serde(default)]
    pub max_record_bytes: Option<usize>,
    #[serde(default)]
    pub allow_driver_read: bool,
    #[serde(default)]
    pub allow_driver_write: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CapabilityPolicyConfig {
    #[serde(default)]
    pub grants: Vec<CapabilityGrant>,
    #[serde(default)]
    pub requirements: Vec<CapabilityRequirement>,
    #[serde(default)]
    pub require_grants_for_advertised: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CapabilityCacheConfig {
    #[serde(default)]
    pub path: Option<PathBuf>,
    #[serde(default)]
    pub max_age_micros: Option<u64>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DiscoveryConfig {
    #[serde(default)]
    pub announcement_cache: Option<PathBuf>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ObservabilityConfig {
    #[serde(default)]
    pub http_bind: Option<String>,
}

pub const DISCOVERY_SCHEMA_VERSION: u8 = 1;
pub const DISCOVERY_QUERY_SUBJECT: &str = "rivun.discovery.query";
pub const DISCOVERY_RESPONSE_SUBJECT: &str = "rivun.discovery.response";
pub const DISCOVERY_ANNOUNCE_SUBJECT: &str = "rivun.discovery.announce";
pub const DISCOVERY_CONTENT_TYPE: &str = "application/rivun-discovery+json";
const DISCOVERY_SIGNATURE_DOMAIN: &[u8] = b"Rivun-DISCOVERY-ANNOUNCEMENT-v1";

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct DiscoveryService {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capability: Option<CapabilityId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subject: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DiscoveryPeer {
    pub node_id: Uuid,
    pub addr: String,
    pub public_key: String,
    pub status: PeerTrustStatus,
    pub allow_send: bool,
    pub allow_receive: bool,
    pub allow_forward: bool,
    pub allow_poa_attestation: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transport_key_epoch: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transport_key_rotated_at_micros: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires_at_micros: Option<u64>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub labels: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DiscoveryAdvertisement {
    pub schema_version: u8,
    pub node_id: Uuid,
    pub public_key: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub advertised_addr: Option<String>,
    pub capabilities: CapabilityAdvertisement,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub services: Vec<DiscoveryService>,
    pub issued_at_micros: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires_at_micros: Option<u64>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub labels: Vec<String>,
}

impl DiscoveryAdvertisement {
    pub fn validate(&self) -> Result<()> {
        if self.schema_version != DISCOVERY_SCHEMA_VERSION {
            bail!(
                "unsupported discovery advertisement schema_version {}",
                self.schema_version
            );
        }
        let public_key = decode_public_key(&self.public_key)?;
        if public_key.node_id() != self.node_id {
            bail!(
                "discovery advertisement public_key derives node_id {}, but declares {}",
                public_key.node_id(),
                self.node_id
            );
        }
        if self.capabilities.node_id != self.node_id {
            bail!(
                "discovery advertisement capability node_id {} does not match {}",
                self.capabilities.node_id,
                self.node_id
            );
        }
        if let Some(addr) = self.advertised_addr.as_deref() {
            validate_discovery_text("discovery advertised_addr", addr, 256, false)?;
        }
        if let Some(expires_at) = self.expires_at_micros
            && expires_at <= self.issued_at_micros
        {
            bail!("discovery advertisement expires_at_micros must be after issued_at_micros");
        }
        let mut service_ids = HashSet::new();
        for service in &self.services {
            validate_discovery_service(service)?;
            if !service_ids.insert(service.id.clone()) {
                bail!("duplicate discovery service `{}`", service.id);
            }
        }
        for label in &self.labels {
            validate_discovery_text("discovery label", label, 64, false)?;
        }
        Ok(())
    }

    fn signing_bytes(&self) -> Result<Vec<u8>> {
        Ok(serde_json::to_vec(self)?)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SignedDiscoveryAdvertisement {
    pub schema_version: u8,
    pub advertisement: DiscoveryAdvertisement,
    pub signature: String,
}

impl SignedDiscoveryAdvertisement {
    pub fn verify(&self, expected_public_key: Option<&PublicKey>) -> Result<()> {
        verify_discovery_advertisement(self, expected_public_key)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DiscoveryQuery {
    pub schema_version: u8,
    #[serde(default)]
    pub requested: Vec<CapabilityId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub service: Option<String>,
    #[serde(default = "default_true")]
    pub include_peers: bool,
    #[serde(default = "default_true")]
    pub include_known: bool,
}

impl Default for DiscoveryQuery {
    fn default() -> Self {
        Self {
            schema_version: DISCOVERY_SCHEMA_VERSION,
            requested: Vec::new(),
            service: None,
            include_peers: true,
            include_known: true,
        }
    }
}

impl DiscoveryQuery {
    pub fn validate(&self) -> Result<()> {
        if self.schema_version != DISCOVERY_SCHEMA_VERSION {
            bail!(
                "unsupported discovery query schema_version {}",
                self.schema_version
            );
        }
        if let Some(service) = self.service.as_deref() {
            validate_discovery_text("discovery query service", service, 128, false)?;
        }
        Ok(())
    }

    fn matches_advertisement(&self, advertisement: &DiscoveryAdvertisement) -> bool {
        let capability_match = self.requested.is_empty()
            || self
                .requested
                .iter()
                .any(|capability| advertisement.capabilities.capabilities.contains(capability));
        let service_match = self.service.as_deref().is_none_or(|requested| {
            advertisement
                .services
                .iter()
                .any(|service| service.id == requested)
        });
        capability_match && service_match
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DiscoveryResponse {
    pub schema_version: u8,
    pub node_id: Uuid,
    pub advertisement: SignedDiscoveryAdvertisement,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub peers: Vec<DiscoveryPeer>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub announcements: Vec<SignedDiscoveryAdvertisement>,
}

impl DiscoveryResponse {
    pub fn verify(&self, expected_node: Uuid, expected_public_key: &PublicKey) -> Result<()> {
        if self.schema_version != DISCOVERY_SCHEMA_VERSION {
            bail!(
                "unsupported discovery response schema_version {}",
                self.schema_version
            );
        }
        if self.node_id != expected_node {
            bail!(
                "discovery response from {} advertised node_id {}",
                expected_node,
                self.node_id
            );
        }
        self.advertisement.verify(Some(expected_public_key))?;
        if self.advertisement.advertisement.node_id != self.node_id {
            bail!(
                "discovery response advertisement node_id {} does not match {}",
                self.advertisement.advertisement.node_id,
                self.node_id
            );
        }
        for announcement in &self.announcements {
            announcement.verify(None)?;
        }
        Ok(())
    }
}

pub fn discovery_services_for_capabilities(
    advertisement: &CapabilityAdvertisement,
) -> Vec<DiscoveryService> {
    advertisement
        .capabilities
        .iter()
        .map(|capability| {
            let action = capability.driver_action();
            DiscoveryService {
                id: capability.to_string(),
                capability: Some(capability.clone()),
                kind: action
                    .map(|_| "action".to_string())
                    .or_else(|| Some("capability".to_string())),
                subject: action.map(ToString::to_string),
                content_type: None,
                description: None,
                tags: Vec::new(),
            }
        })
        .collect()
}

pub fn build_discovery_advertisement(
    keypair: &Keypair,
    advertised_addr: Option<String>,
    capability_advertisement: CapabilityAdvertisement,
    mut services: Vec<DiscoveryService>,
    labels: Vec<String>,
    expires_at_micros: Option<u64>,
) -> Result<DiscoveryAdvertisement> {
    if capability_advertisement.node_id != keypair.node_id() {
        bail!(
            "capability advertisement node_id {} does not match signing key {}",
            capability_advertisement.node_id,
            keypair.node_id()
        );
    }
    if services.is_empty() {
        services = discovery_services_for_capabilities(&capability_advertisement);
    }
    let advertisement = DiscoveryAdvertisement {
        schema_version: DISCOVERY_SCHEMA_VERSION,
        node_id: keypair.node_id(),
        public_key: STANDARD_NO_PAD.encode(keypair.verifying_key().to_bytes()),
        advertised_addr,
        capabilities: capability_advertisement,
        services,
        issued_at_micros: now_micros()?,
        expires_at_micros,
        labels,
    };
    advertisement.validate()?;
    Ok(advertisement)
}

pub fn sign_discovery_advertisement(
    keypair: &Keypair,
    advertisement: DiscoveryAdvertisement,
) -> Result<SignedDiscoveryAdvertisement> {
    if advertisement.node_id != keypair.node_id() {
        bail!(
            "discovery advertisement node_id {} does not match signing key {}",
            advertisement.node_id,
            keypair.node_id()
        );
    }
    advertisement.validate()?;
    let signature =
        keypair.sign_domain_message(DISCOVERY_SIGNATURE_DOMAIN, &advertisement.signing_bytes()?);
    Ok(SignedDiscoveryAdvertisement {
        schema_version: DISCOVERY_SCHEMA_VERSION,
        advertisement,
        signature: STANDARD_NO_PAD.encode(signature),
    })
}

pub fn verify_discovery_advertisement(
    signed: &SignedDiscoveryAdvertisement,
    expected_public_key: Option<&PublicKey>,
) -> Result<()> {
    if signed.schema_version != DISCOVERY_SCHEMA_VERSION {
        bail!(
            "unsupported signed discovery advertisement schema_version {}",
            signed.schema_version
        );
    }
    signed.advertisement.validate()?;
    let public_key = decode_public_key(&signed.advertisement.public_key)?;
    if let Some(expected) = expected_public_key
        && expected.to_bytes() != public_key.to_bytes()
    {
        bail!(
            "discovery advertisement expected public key for {}, got {}",
            expected.node_id(),
            public_key.node_id()
        );
    }
    let signature = decode_signature(&signed.signature)?;
    public_key.verify_domain_message(
        DISCOVERY_SIGNATURE_DOMAIN,
        &signed.advertisement.signing_bytes()?,
        &signature,
    )?;
    Ok(())
}

fn validate_discovery_service(service: &DiscoveryService) -> Result<()> {
    validate_discovery_text("discovery service id", &service.id, 128, false)?;
    if let Some(kind) = service.kind.as_deref() {
        validate_discovery_text("discovery service kind", kind, 64, false)?;
    }
    if let Some(subject) = service.subject.as_deref() {
        validate_discovery_text(
            "discovery service subject",
            subject,
            ZENV_MAX_SUBJECT_LEN,
            false,
        )?;
    }
    if let Some(content_type) = service.content_type.as_deref() {
        validate_discovery_text(
            "discovery service content_type",
            content_type,
            ZENV_MAX_CONTENT_TYPE_LEN,
            false,
        )?;
    }
    if let Some(description) = service.description.as_deref() {
        validate_discovery_text("discovery service description", description, 512, false)?;
    }
    for tag in &service.tags {
        validate_discovery_text("discovery service tag", tag, 64, false)?;
    }
    Ok(())
}

fn validate_discovery_text(
    label: &str,
    value: &str,
    max_len: usize,
    allow_empty: bool,
) -> Result<()> {
    validate_message_text(label, value, max_len, allow_empty)
}

fn validate_discovery_advertisement_time(advertisement: &DiscoveryAdvertisement) -> Result<()> {
    if let Some(expires_at) = advertisement.expires_at_micros
        && expires_at <= now_micros()?
    {
        bail!(
            "discovery advertisement for {} expired at {}",
            advertisement.node_id,
            expires_at
        );
    }
    Ok(())
}

fn decode_signature(encoded: &str) -> Result<[u8; ED25519_SIGNATURE_LEN]> {
    let bytes = STANDARD_NO_PAD.decode(encoded)?;
    if bytes.len() != ED25519_SIGNATURE_LEN {
        bail!(
            "invalid signature length: expected {}, got {}",
            ED25519_SIGNATURE_LEN,
            bytes.len()
        );
    }
    Ok(bytes.try_into().unwrap())
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MessagePolicyConfig {
    #[serde(default)]
    pub default_decision: MessagePolicyDecision,
    #[serde(default)]
    pub rules: Vec<MessagePolicyRule>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MessagePolicyRule {
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
    pub decision: MessagePolicyDecision,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub required_capability: Option<CapabilityId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

pub type MessagePolicyDecision = rivun_policy::PolicyDecision;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MessageSchemaConfig {
    #[serde(default)]
    pub require_match: bool,
    #[serde(default)]
    pub contracts: Vec<MessageContractConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageContractConfig {
    pub path: PathBuf,
}

impl From<MessagePolicyRule> for PolicyRule {
    fn from(rule: MessagePolicyRule) -> Self {
        Self {
            name: rule.name,
            kind: rule.kind,
            subject: rule.subject,
            source_node: rule.source_node,
            target_node: rule.target_node,
            content_type: rule.content_type,
            decision: rule.decision,
            required_capability: rule.required_capability,
            reason: rule.reason,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityConfig {
    #[serde(default = "default_max_clock_skew_micros")]
    pub max_clock_skew_micros: u64,
    #[serde(default = "default_replay_cache_capacity")]
    pub replay_cache_capacity: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub durable_replay_store_path: Option<PathBuf>,
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            max_clock_skew_micros: default_max_clock_skew_micros(),
            replay_cache_capacity: default_replay_cache_capacity(),
            durable_replay_store_path: None,
        }
    }
}

fn default_max_clock_skew_micros() -> u64 {
    5 * 60 * 1_000_000
}

fn default_replay_cache_capacity() -> usize {
    4096
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ConfigValidationReport {
    pub bind: SocketAddr,
    pub node_id: Uuid,
    pub observability_http_bind: Option<SocketAddr>,
    pub peer_count: usize,
    pub trusted_peer_count: usize,
    pub restricted_peer_count: usize,
    pub peer_send_enabled_count: usize,
    pub peer_receive_enabled_count: usize,
    pub peer_forward_enabled_count: usize,
    pub driver_count: usize,
    pub signed_driver_count: usize,
    pub receipt_journal_enabled: bool,
    pub registry_enabled: bool,
    pub registry_entry_count: usize,
    pub registry_signature_required: bool,
    pub registry_bundle_enabled: bool,
    pub require_signed: bool,
    pub poa_validator_count: usize,
    pub poa_required_threshold: u16,
    pub poa_validator_set_enabled: bool,
    pub poa_validator_set_epoch: Option<u64>,
    pub memory_enabled: bool,
    pub route_count: usize,
    pub capability_count: usize,
    pub capability_grant_count: usize,
    pub capability_requirement_count: usize,
    pub ungranted_capability_count: usize,
    pub capability_cache_enabled: bool,
    pub discovery_cache_enabled: bool,
    pub message_policy_default_decision: MessagePolicyDecision,
    pub message_policy_rule_count: usize,
    pub message_schema_contract_count: usize,
    pub message_schema_require_match: bool,
    pub peer_grant_route_count: usize,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ActionEnvelope {
    pub action: String,
    #[serde(default)]
    pub payload: String,
    #[serde(default)]
    pub payload_base64: Option<String>,
}

impl ActionEnvelope {
    pub fn new(action: impl Into<String>, payload: impl Into<String>) -> Self {
        Self {
            action: action.into(),
            payload: payload.into(),
            payload_base64: None,
        }
    }

    pub fn payload_bytes(&self) -> Result<Vec<u8>> {
        if let Some(encoded) = &self.payload_base64 {
            return Ok(STANDARD_NO_PAD.decode(encoded)?);
        }
        Ok(self.payload.as_bytes().to_vec())
    }

    pub fn to_payload_bytes(&self) -> Result<Vec<u8>> {
        Ok(serde_json::to_vec(self)?)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeEvent {
    pub source: Uuid,
    pub kind: RivunMessageKind,
    pub subject: String,
    /// Deprecated compatibility alias for legacy action-oriented consumers.
    pub action: String,
    pub output: Option<Vec<u8>>,
}

#[derive(Debug, Clone)]
struct InboundMessage {
    kind: RivunMessageKind,
    subject: String,
    content_type: Option<String>,
    body: Vec<u8>,
    metadata: Vec<u8>,
}

impl InboundMessage {
    fn from_universal(envelope: RivunEnvelopeRef<'_>) -> Self {
        Self {
            kind: envelope.kind(),
            subject: envelope.subject().to_string(),
            content_type: Some(envelope.content_type().to_string()),
            body: envelope.body().to_vec(),
            metadata: envelope.metadata().to_vec(),
        }
    }

    fn legacy_action(envelope: ActionEnvelope) -> Result<Self> {
        let body = envelope.payload_bytes()?;
        Ok(Self {
            kind: RivunMessageKind::Action,
            subject: envelope.action,
            content_type: None,
            body,
            metadata: Vec::new(),
        })
    }

    fn raw_data(payload: &[u8]) -> Self {
        Self {
            kind: RivunMessageKind::Data,
            subject: String::new(),
            content_type: None,
            body: payload.to_vec(),
            metadata: Vec::new(),
        }
    }
}

fn parse_inbound_message(payload: &[u8]) -> Result<InboundMessage> {
    if payload.starts_with(&ZENV_MAGIC_BYTES) {
        let envelope = RivunEnvelopeRef::parse(payload).context("invalid ZENV envelope")?;
        return Ok(InboundMessage::from_universal(envelope));
    }

    if let Ok(envelope) = serde_json::from_slice::<ActionEnvelope>(payload) {
        return InboundMessage::legacy_action(envelope);
    }

    Ok(InboundMessage::raw_data(payload))
}

fn validate_inbound_message(message: &InboundMessage) -> Result<()> {
    if message.subject.trim().is_empty() && message.kind.requires_subject() {
        bail!("{} message subject must not be empty", message.kind);
    }
    validate_message_text(
        "message subject",
        &message.subject,
        ZENV_MAX_SUBJECT_LEN,
        true,
    )?;
    if let Some(content_type) = &message.content_type {
        validate_message_text(
            "message content_type",
            content_type,
            ZENV_MAX_CONTENT_TYPE_LEN,
            true,
        )?;
    }
    if message.metadata.len() > ZENV_MAX_METADATA_LEN {
        bail!(
            "message metadata length {} exceeds max {}",
            message.metadata.len(),
            ZENV_MAX_METADATA_LEN,
        );
    }
    Ok(())
}

fn validate_agent_message_envelope(message: &InboundMessage) -> Result<()> {
    if message.content_type.as_deref() != Some(AGENT_CONTENT_TYPE) {
        return Ok(());
    }
    let agent_message =
        AgentMessage::from_json_slice(&message.body).context("invalid agent protocol body")?;
    if agent_message.subject() != message.subject {
        bail!(
            "agent protocol subject mismatch: envelope subject `{}` carries `{}`",
            message.subject,
            agent_message.subject()
        );
    }
    Ok(())
}

fn validate_message_text(
    label: &str,
    value: &str,
    max_len: usize,
    allow_empty: bool,
) -> Result<()> {
    if !allow_empty && value.trim().is_empty() {
        bail!("{label} must not be empty");
    }
    if value.len() > max_len {
        bail!("{label} length {} exceeds max {}", value.len(), max_len);
    }
    if value.chars().any(char::is_control) {
        bail!("{label} must not contain control characters");
    }
    Ok(())
}

fn load_discovery_announcement_cache(
    path: Option<&Path>,
) -> Result<HashMap<Uuid, SignedDiscoveryAdvertisement>> {
    let Some(path) = path else {
        return Ok(HashMap::new());
    };
    if !path.exists() {
        return Ok(HashMap::new());
    }
    let input = fs::read_to_string(path)
        .with_context(|| format!("failed to read discovery cache {}", path.display()))?;
    let mut announcements = HashMap::new();
    for (index, line) in input.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let signed: SignedDiscoveryAdvertisement =
            serde_json::from_str(line).with_context(|| {
                format!(
                    "failed to parse discovery cache {} line {}",
                    path.display(),
                    index + 1
                )
            })?;
        signed.verify(None).with_context(|| {
            format!(
                "invalid discovery announcement in {} line {}",
                path.display(),
                index + 1
            )
        })?;
        if validate_discovery_advertisement_time(&signed.advertisement).is_ok() {
            announcements.insert(signed.advertisement.node_id, signed);
        }
    }
    Ok(announcements)
}

fn persist_discovery_announcement(
    path: Option<&Path>,
    signed: &SignedDiscoveryAdvertisement,
) -> Result<()> {
    let Some(path) = path else {
        return Ok(());
    };
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent).with_context(|| {
            format!(
                "failed to create discovery cache directory {}",
                parent.display()
            )
        })?;
    }
    let mut options = OpenOptions::new();
    options.create(true).append(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(path)
        .with_context(|| format!("failed to open discovery cache {}", path.display()))?;
    let line = serde_json::to_string(signed)?;
    writeln!(file, "{line}")
        .with_context(|| format!("failed to write discovery cache {}", path.display()))
}

pub struct RivunNode {
    endpoint: RivunEndpoint,
    keypair: Keypair,
    public_keys: HashMap<Uuid, PublicKey>,
    peer_trust: HashMap<Uuid, PeerTrustConfig>,
    drivers: HashMap<String, DriverRegistration>,
    runtime: WasmExecutor,
    async_runtime: Option<AsyncWasmExecutor>,
    limits: ExecutionLimits,
    action_limits: HashMap<String, ExecutionLimits>,
    require_signed: bool,
    replay_guard: Mutex<ReplayGuard>,
    security: SecurityConfig,
    poa_validators: Vec<(Uuid, PublicKey)>,
    poa_required_threshold: u16,
    poa_validator_set_path: Option<PathBuf>,
    poa_validator_set_authority: Option<PublicKey>,
    receipt_journal: Option<ReceiptJournalStore>,
    receipt_fsync: ReceiptFsyncPolicy,
    receipt_fsync_interval_writes: u64,
    receipt_durability: Mutex<ReceiptDurabilityState>,
    registry_path: Option<PathBuf>,
    registry_bundle_path: Option<PathBuf>,
    registry_require_signature: bool,
    capability_cache_path: Option<PathBuf>,
    capability_cache_max_age_micros: Option<u64>,
    memory: MemoryConfig,
    route_table: RouteTable,
    message_policy: MessagePolicyConfig,
    message_contracts: MessageContractSet,
    peer_ids: Vec<Uuid>,
    capability_advertisement: CapabilityAdvertisement,
    discovery_peers: Vec<DiscoveryPeer>,
    discovery_announcement_cache: Option<PathBuf>,
    discovery_announcements: Mutex<HashMap<Uuid, SignedDiscoveryAdvertisement>>,
    metrics: Mutex<NodeMetricsCounters>,
}

struct DriverRegistration {
    driver: WasmDriver,
    async_driver: Option<AsyncCompiledDriver>,
    permissions: DriverPermissions,
}

#[derive(Debug, Default)]
struct ReceiptDurabilityState {
    writes_since_sync: u64,
}

impl ReceiptDurabilityState {
    fn record_write(&mut self, policy: ReceiptFsyncPolicy, interval_writes: u64) -> bool {
        match policy {
            ReceiptFsyncPolicy::Always => true,
            ReceiptFsyncPolicy::Off => false,
            ReceiptFsyncPolicy::Interval => {
                self.writes_since_sync = self.writes_since_sync.saturating_add(1);
                if self.writes_since_sync >= interval_writes {
                    self.writes_since_sync = 0;
                    true
                } else {
                    false
                }
            }
        }
    }
}

pub use rivun_telemetry::{
    ActionCounter, PeerCounter, PeerTrustGauge, ReasonCounter, TransportCounter,
    RivunNodeMetricsSnapshot,
};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RivunNodeHealthStatus {
    Healthy,
    Degraded,
    Critical,
}

impl RivunNodeHealthStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            RivunNodeHealthStatus::Healthy => "healthy",
            RivunNodeHealthStatus::Degraded => "degraded",
            RivunNodeHealthStatus::Critical => "critical",
        }
    }

    pub fn merge(self, other: Self) -> Self {
        match (self, other) {
            (RivunNodeHealthStatus::Critical, _) | (_, RivunNodeHealthStatus::Critical) => {
                RivunNodeHealthStatus::Critical
            }
            (RivunNodeHealthStatus::Degraded, _) | (_, RivunNodeHealthStatus::Degraded) => {
                RivunNodeHealthStatus::Degraded
            }
            _ => RivunNodeHealthStatus::Healthy,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RivunNodeHealthCheck {
    pub name: String,
    pub status: RivunNodeHealthStatus,
    pub summary: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

impl RivunNodeHealthCheck {
    pub fn new(
        name: impl Into<String>,
        status: RivunNodeHealthStatus,
        summary: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            status,
            summary: summary.into(),
            detail: None,
        }
    }

    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RivunNodeHealthSnapshot {
    pub node_id: Uuid,
    pub status: RivunNodeHealthStatus,
    pub checks: Vec<RivunNodeHealthCheck>,
}

impl RivunNodeHealthSnapshot {
    pub fn to_json(&self) -> Result<String> {
        Ok(serde_json::to_string_pretty(self)?)
    }

    pub fn to_healthz_text(&self) -> String {
        let mut output = format!(
            "status={}\nnode_id={}\n",
            self.status.as_str(),
            self.node_id
        );
        for check in &self.checks {
            output.push_str(&format!(
                "check{{name=\"{}\"}}={}\n",
                health_text_escape(&check.name),
                check.status.as_str()
            ));
        }
        output
    }
}

impl PeerTrustStatus {
    fn as_metric_label(self) -> &'static str {
        match self {
            PeerTrustStatus::Trusted => "trusted",
            PeerTrustStatus::Quarantined => "quarantined",
            PeerTrustStatus::Revoked => "revoked",
        }
    }
}

fn health_text_escape(value: &str) -> String {
    value
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.'))
        .collect()
}

fn handle_observability_http_connection(node: &RivunNode, mut stream: TcpStream) -> Result<()> {
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .context("failed to set observability HTTP read timeout")?;
    stream
        .set_write_timeout(Some(Duration::from_secs(2)))
        .context("failed to set observability HTTP write timeout")?;

    let mut buffer = [0_u8; 2048];
    let read = stream
        .read(&mut buffer)
        .context("failed to read observability HTTP request")?;
    if read == 0 {
        return Ok(());
    }
    let request = std::str::from_utf8(&buffer[..read]).unwrap_or("");
    let mut request_parts = request.lines().next().unwrap_or("").split_whitespace();
    let method = request_parts.next().unwrap_or("");
    let path = request_parts
        .next()
        .unwrap_or("/")
        .split('?')
        .next()
        .unwrap_or("/");

    if method != "GET" && method != "HEAD" {
        return write_observability_http_response(
            &mut stream,
            method,
            405,
            "text/plain; charset=utf-8",
            "method not allowed\n".to_string(),
        );
    }

    match path {
        "/metrics" => write_observability_http_response(
            &mut stream,
            method,
            200,
            "text/plain; version=0.0.4; charset=utf-8",
            node.metrics_prometheus_text(),
        ),
        "/healthz" => {
            let snapshot = node.health_snapshot();
            let status_code = if snapshot.status == RivunNodeHealthStatus::Critical {
                503
            } else {
                200
            };
            write_observability_http_response(
                &mut stream,
                method,
                status_code,
                "text/plain; charset=utf-8",
                snapshot.to_healthz_text(),
            )
        }
        "/healthz.json" => {
            let snapshot = node.health_snapshot();
            let status_code = if snapshot.status == RivunNodeHealthStatus::Critical {
                503
            } else {
                200
            };
            write_observability_http_response(
                &mut stream,
                method,
                status_code,
                "application/json; charset=utf-8",
                snapshot.to_json()?,
            )
        }
        _ => write_observability_http_response(
            &mut stream,
            method,
            404,
            "text/plain; charset=utf-8",
            "not found\n".to_string(),
        ),
    }
}

fn write_observability_http_response(
    stream: &mut TcpStream,
    method: &str,
    status_code: u16,
    content_type: &str,
    body: String,
) -> Result<()> {
    let reason = match status_code {
        200 => "OK",
        404 => "Not Found",
        405 => "Method Not Allowed",
        503 => "Service Unavailable",
        _ => "OK",
    };
    let headers = format!(
        "HTTP/1.1 {status_code} {reason}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n",
        body.len()
    );
    stream
        .write_all(headers.as_bytes())
        .context("failed to write observability HTTP response headers")?;
    if method != "HEAD" {
        stream
            .write_all(body.as_bytes())
            .context("failed to write observability HTTP response body")?;
    }
    stream
        .flush()
        .context("failed to flush observability HTTP response")
}

fn classify_processing_error(error: &anyhow::Error) -> &'static str {
    let details = format!("{error:#}");
    if details.contains("anti-replay") || details.contains("stale frame timestamp") {
        "anti_replay"
    } else if details.contains("signature") || details.contains("public key") {
        "signature"
    } else if details.contains("Proof-of-Action") || details.contains("PoA") {
        "poa"
    } else if details.contains("message policy") {
        "message_policy"
    } else if details.contains("message contract") || details.contains("schema") {
        "message_contract"
    } else if details.contains("not permitted") || details.contains("missing trust contract") {
        "peer_trust"
    } else if details.contains("invalid ZENV envelope") || details.contains("message subject") {
        "message_parse"
    } else {
        "processing_error"
    }
}

#[derive(Debug, Default)]
struct NodeMetricsCounters {
    frames_sent_by_peer: BTreeMap<Uuid, u64>,
    frames_received_by_peer: BTreeMap<Uuid, u64>,
    frames_rejected_by_reason: BTreeMap<String, u64>,
    driver_execution_errors_by_action: BTreeMap<String, u64>,
    receipt_log_verify_failures_total: u64,
    poa_attestation_failures_total: u64,
    replay_rejections_total: u64,
    replay_drops_total: u64,
    journal_segment_rotations_total: u64,
    segment_manifest_errors_total: u64,
    pack_verification_failures_total: u64,
    store_verifications_total: u64,
    agent_gateway_requests_total: BTreeMap<(String, String), u64>,
    agent_sessions_active: i64,
    provenance_verification_failures_total: u64,
    peers_active: u64,
}

#[derive(Debug)]
pub struct RivunNodeObservabilityHttpServer {
    addr: SocketAddr,
    shutdown: Arc<AtomicBool>,
    handle: Option<thread::JoinHandle<()>>,
}

impl RivunNodeObservabilityHttpServer {
    pub fn local_addr(&self) -> SocketAddr {
        self.addr
    }
}

impl Drop for RivunNodeObservabilityHttpServer {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::Relaxed);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

impl RivunNode {
    pub async fn from_config(config: RivunNodeConfig) -> Result<Self> {
        let runtime = WasmExecutor::new()?;
        validate_config_with_executor(&config, &runtime)?;
        let async_runtime = config
            .runtime
            .async_execution
            .then(AsyncWasmExecutor::new)
            .transpose()?;
        let action_limits = resolved_action_limits(config.runtime, &config.action_runtime_limits);
        let keypair = load_keypair(&config.key_file)?;
        let node_id = keypair.node_id();
        let bind = config
            .bind
            .parse::<SocketAddr>()
            .with_context(|| format!("invalid bind address {}", config.bind))?;

        let mut endpoint_config = RivunEndpointConfig::new(bind, node_id);
        if let Some(max_datagram_size) = config.max_datagram_size {
            endpoint_config.max_datagram_size = max_datagram_size;
        }
        endpoint_config.inbound_nonce_cache_capacity = config.security.replay_cache_capacity;

        let mut public_keys = HashMap::new();
        let mut peer_trust = HashMap::new();
        let mut peer_ids = Vec::with_capacity(config.peers.len());
        let mut discovery_peers = Vec::with_capacity(config.peers.len());
        for peer in &config.peers {
            let peer_addr = peer
                .addr
                .parse::<SocketAddr>()
                .with_context(|| format!("invalid peer address {}", peer.addr))?;
            let transport_key = TransportKey::from_hex(&peer.transport_key)?;
            if peer.trust.allows_transport() {
                endpoint_config.peers.push(Peer {
                    node_id: peer.node_id,
                    addr: peer_addr,
                    transport_key,
                });
            }
            public_keys.insert(peer.node_id, decode_public_key(&peer.public_key)?);
            peer_trust.insert(peer.node_id, peer.trust.clone());
            peer_ids.push(peer.node_id);
            discovery_peers.push(DiscoveryPeer {
                node_id: peer.node_id,
                addr: peer.addr.clone(),
                public_key: peer.public_key.clone(),
                status: peer.trust.status,
                allow_send: peer.trust.allow_send,
                allow_receive: peer.trust.allow_receive,
                allow_forward: peer.trust.allow_forward,
                allow_poa_attestation: peer.trust.allow_poa_attestation,
                transport_key_epoch: peer.transport_key_epoch,
                transport_key_rotated_at_micros: peer.transport_key_rotated_at_micros,
                expires_at_micros: peer.trust.expires_at_micros,
                labels: peer.trust.labels.clone(),
            });
        }
        let poa_verifier = load_poa_verifier(&config.poa)?;
        let poa_validator_set_path = config.poa.validator_set.clone();
        let poa_validator_set_authority = config
            .poa
            .validator_set_authority
            .as_deref()
            .map(decode_public_key)
            .transpose()
            .context("invalid poa.validator_set_authority")?;

        let registry_path = config.registry.path.clone();
        let registry_bundle_path = config.registry.bundle_path.clone();
        let registry_require_signature = config.registry.require_signature;
        let capability_cache_path = config.capability_cache.path.clone();
        let capability_cache_max_age_micros = config.capability_cache.max_age_micros;
        let registry = load_driver_registry_optional(&config.registry)?;
        let drivers = load_drivers(
            &runtime,
            async_runtime.as_ref(),
            &config.drivers,
            registry.as_ref(),
        )?;
        let route_table = RouteTable::new(config.routes.clone())?;
        let capability_advertisement = describe_capabilities(&config)?;
        let message_contracts = load_message_contract_set(&config.message_schema)?;
        let discovery_announcement_cache = config.discovery.announcement_cache.clone();
        let discovery_announcements =
            load_discovery_announcement_cache(discovery_announcement_cache.as_deref())?;
        let receipt_fsync = config.receipts.fsync;
        let receipt_fsync_interval_writes = config.receipts.fsync_interval_writes();
        let endpoint = RivunEndpoint::bind(endpoint_config).await?;
        let replay_guard = if let Some(path) = &config.security.durable_replay_store_path {
            let store = durable_replay::DurableReplayStore::open(
                path,
                config.security.replay_cache_capacity,
                config.security.max_clock_skew_micros,
            )?;
            ReplayGuard::with_durable_store(config.security.replay_cache_capacity, store)
        } else {
            ReplayGuard::new(config.security.replay_cache_capacity)
        };

        Ok(Self {
            endpoint,
            keypair,
            public_keys,
            peer_trust,
            drivers,
            runtime,
            async_runtime,
            limits: config.runtime.to_limits(),
            action_limits,
            require_signed: config.require_signed,
            replay_guard: Mutex::new(replay_guard),
            security: config.security,
            poa_validators: poa_verifier.validators,
            poa_required_threshold: poa_verifier.required_threshold,
            poa_validator_set_path,
            poa_validator_set_authority,
            receipt_journal: config.receipts.dir.map(ReceiptJournalStore::open),
            receipt_fsync,
            receipt_fsync_interval_writes,
            receipt_durability: Mutex::new(ReceiptDurabilityState::default()),
            registry_path,
            registry_bundle_path,
            registry_require_signature,
            capability_cache_path,
            capability_cache_max_age_micros,
            memory: config.memory,
            route_table,
            message_policy: config.message_policy,
            message_contracts,
            peer_ids,
            capability_advertisement,
            discovery_peers,
            discovery_announcement_cache,
            discovery_announcements: Mutex::new(discovery_announcements),
            metrics: Mutex::new(NodeMetricsCounters::default()),
        })
    }

    pub fn local_addr(&self) -> Result<SocketAddr> {
        Ok(self.endpoint.local_addr()?)
    }

    pub async fn run_forever(&self) -> Result<()> {
        info!(node_id = %self.endpoint.node_id(), addr = %self.local_addr()?, "Rivun node running");
        loop {
            match self.handle_once().await {
                Ok(event) => {
                    info!(
                        source = %event.source,
                        kind = %event.kind,
                        subject = %event.subject,
                        output_bytes = event.output.as_ref().map(|bytes| bytes.len()).unwrap_or(0),
                        "processed Rivun message"
                    );
                }
                Err(error) => warn!(%error, "failed to process inbound Rivun frame"),
            }
        }
    }

    pub fn spawn_observability_http(
        self: Arc<Self>,
        bind: SocketAddr,
    ) -> Result<RivunNodeObservabilityHttpServer> {
        let listener = TcpListener::bind(bind)
            .with_context(|| format!("failed to bind observability HTTP listener on {bind}"))?;
        listener.set_nonblocking(true).with_context(|| {
            format!("failed to set observability HTTP listener nonblocking on {bind}")
        })?;
        let addr = listener
            .local_addr()
            .context("failed to read observability HTTP listener address")?;
        let shutdown = Arc::new(AtomicBool::new(false));
        let thread_shutdown = Arc::clone(&shutdown);
        let handle = thread::Builder::new()
            .name("rivun-observability-http".to_string())
            .spawn(move || {
                info!(addr = %addr, "Rivun observability HTTP listener running");
                while !thread_shutdown.load(Ordering::Relaxed) {
                    match listener.accept() {
                        Ok((stream, peer_addr)) => {
                            if let Err(error) = handle_observability_http_connection(&self, stream)
                            {
                                warn!(%error, %peer_addr, "observability HTTP request failed");
                            }
                        }
                        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                            thread::sleep(Duration::from_millis(20));
                        }
                        Err(error) => {
                            warn!(%error, "observability HTTP accept failed");
                            thread::sleep(Duration::from_millis(100));
                        }
                    }
                }
            })
            .context("failed to spawn observability HTTP listener thread")?;
        Ok(RivunNodeObservabilityHttpServer {
            addr,
            shutdown,
            handle: Some(handle),
        })
    }

    pub async fn handle_once(&self) -> Result<NodeEvent> {
        match self.handle_once_inner().await {
            Ok(event) => Ok(event),
            Err(error) => {
                self.record_rejected_frame(classify_processing_error(&error));
                Err(error)
            }
        }
    }

    async fn handle_once_inner(&self) -> Result<NodeEvent> {
        let inbound = self.endpoint.recv().await?;
        self.record_received_frame(inbound.peer.node_id);
        self.ensure_peer_can_receive(inbound.peer.node_id)?;
        if self.require_signed {
            let public_key = self
                .public_keys
                .get(&inbound.peer.node_id)
                .ok_or_else(|| anyhow!("missing public key for peer {}", inbound.peer.node_id))?;
            verify_frame(public_key, &inbound.frame)?;
        }
        if inbound
            .frame
            .header
            .flags
            .contains(rivun_core::RivunFlags::REQUIRES_CONSENSUS)
        {
            self.verify_consensus(&inbound.frame)?;
        }
        self.validate_fresh_frame(&inbound.frame)
            .context("inbound frame failed anti-replay validation")?;

        let message = parse_inbound_message(&inbound.frame.payload)?;
        validate_inbound_message(&message)?;
        validate_agent_message_envelope(&message)?;
        self.validate_message_contracts(&message)?;
        self.apply_message_policy(&inbound.frame, &message)?;
        let output = if message.kind == RivunMessageKind::Control
            && message.subject == POA_ATTESTATION_REQUEST_SUBJECT
        {
            if let Err(error) = self
                .respond_to_poa_attestation_request(inbound.peer.node_id, &message.body)
                .await
            {
                self.record_poa_attestation_failure();
                return Err(error);
            }
            None
        } else if message.kind == RivunMessageKind::Control
            && message.subject == DISCOVERY_ANNOUNCE_SUBJECT
        {
            self.record_discovery_announcement(inbound.peer.node_id, &message.body)?;
            None
        } else if message.kind == RivunMessageKind::Control
            && message.subject == DISCOVERY_QUERY_SUBJECT
        {
            self.respond_to_discovery_query(inbound.peer.node_id, &message.body)
                .await?;
            None
        } else if message.kind == RivunMessageKind::Control
            && message.subject == CAPABILITY_QUERY_SUBJECT
        {
            self.respond_to_capability_query(inbound.peer.node_id, &message.body)
                .await?;
            None
        } else if message.kind == RivunMessageKind::Control
            && message.subject == RECEIPT_REPLICATION_REQUEST_SUBJECT
        {
            self.respond_to_receipt_replication_request(inbound.peer.node_id, &message.body)
                .await?;
            None
        } else if message.kind == RivunMessageKind::Control
            && message.subject == POA_VALIDATOR_SET_REQUEST_SUBJECT
        {
            self.respond_to_poa_validator_set_request(inbound.peer.node_id, &message.body)
                .await?;
            None
        } else if message.kind == RivunMessageKind::Control
            && message.subject == REGISTRY_INDEX_REQUEST_SUBJECT
        {
            self.respond_to_registry_index_request(inbound.peer.node_id, &message.body)
                .await?;
            None
        } else if message.kind == RivunMessageKind::Control
            && message.subject == REGISTRY_BUNDLE_MANIFEST_REQUEST_SUBJECT
        {
            self.respond_to_registry_bundle_manifest_request(inbound.peer.node_id, &message.body)
                .await?;
            None
        } else {
            self.route_message(&inbound, &message).await?
        };
        self.write_receipt(&inbound.frame, &message, output.as_deref())?;

        Ok(NodeEvent {
            source: inbound.peer.node_id,
            kind: message.kind,
            subject: message.subject.clone(),
            action: message.subject,
            output,
        })
    }

    pub fn metrics_snapshot(&self) -> RivunNodeMetricsSnapshot {
        let counters = self.metrics.lock().expect("node metrics mutex poisoned");
        RivunNodeMetricsSnapshot {
            node_id: self.keypair.node_id(),
            frames_sent_total: counters
                .frames_sent_by_peer
                .iter()
                .map(|(peer, value)| PeerCounter {
                    peer: *peer,
                    value: *value,
                })
                .collect(),
            frames_received_total: counters
                .frames_received_by_peer
                .iter()
                .map(|(peer, value)| PeerCounter {
                    peer: *peer,
                    value: *value,
                })
                .collect(),
            frames_rejected_total: counters
                .frames_rejected_by_reason
                .iter()
                .map(|(reason, value)| ReasonCounter {
                    reason: reason.clone(),
                    value: *value,
                })
                .collect(),
            driver_execution_errors_total: counters
                .driver_execution_errors_by_action
                .iter()
                .map(|(action, value)| ActionCounter {
                    action: action.clone(),
                    value: *value,
                })
                .collect(),
            peer_trust_status: self.peer_trust_gauges(),
            registry_signature_valid: self.registry_signature_valid(),
            capability_cache_age_seconds: self.capability_cache_age_seconds(),
            receipt_log_verify_failures_total: counters.receipt_log_verify_failures_total,
            poa_attestation_failures_total: counters.poa_attestation_failures_total,
            replay_rejections_total: counters.replay_rejections_total,
            replay_drops_total: counters.replay_drops_total,
            journal_segment_rotations_total: counters.journal_segment_rotations_total,
            segment_manifest_errors_total: counters.segment_manifest_errors_total,
            pack_verification_failures_total: counters.pack_verification_failures_total,
            store_verifications_total: counters.store_verifications_total,
            agent_gateway_requests_total: counters
                .agent_gateway_requests_total
                .iter()
                .map(|((t, s), value)| TransportCounter {
                    transport: t.clone(),
                    status: s.clone(),
                    value: *value,
                })
                .collect(),
            agent_sessions_active: counters.agent_sessions_active,
            provenance_verification_failures_total: counters.provenance_verification_failures_total,
            peers_active: counters.peers_active,
        }
    }

    pub fn metrics_prometheus_text(&self) -> String {
        self.metrics_snapshot().to_prometheus_text()
    }

    pub fn health_snapshot(&self) -> RivunNodeHealthSnapshot {
        let metrics = self.metrics_snapshot();
        let checks = self.health_checks_from_metrics(&metrics);
        let status = checks
            .iter()
            .fold(RivunNodeHealthStatus::Healthy, |status, check| {
                status.merge(check.status)
            });
        RivunNodeHealthSnapshot {
            node_id: self.keypair.node_id(),
            status,
            checks,
        }
    }

    pub fn health_json(&self) -> Result<String> {
        self.health_snapshot().to_json()
    }

    pub fn healthz_text(&self) -> String {
        self.health_snapshot().to_healthz_text()
    }

    fn health_checks_from_metrics(
        &self,
        metrics: &RivunNodeMetricsSnapshot,
    ) -> Vec<RivunNodeHealthCheck> {
        let mut checks = Vec::new();
        checks.push(match self.local_addr() {
            Ok(addr) => RivunNodeHealthCheck::new(
                "endpoint_bound",
                RivunNodeHealthStatus::Healthy,
                "node UDP endpoint is bound",
            )
            .with_detail(addr.to_string()),
            Err(error) => RivunNodeHealthCheck::new(
                "endpoint_bound",
                RivunNodeHealthStatus::Critical,
                "node UDP endpoint is not reachable",
            )
            .with_detail(format!("{error:#}")),
        });

        checks.push(self.registry_health_check());
        checks.push(self.registry_bundle_health_check());
        checks.push(self.receipt_log_health_check(metrics.receipt_log_verify_failures_total));
        checks.push(self.capability_cache_health_check(metrics.capability_cache_age_seconds));
        checks.push(self.message_policy_health_check());
        checks.push(self.peer_trust_health_check());
        checks.push(self.runtime_error_health_check(metrics));
        checks
    }

    fn registry_health_check(&self) -> RivunNodeHealthCheck {
        let Some(path) = &self.registry_path else {
            return RivunNodeHealthCheck::new(
                "registry_signature",
                RivunNodeHealthStatus::Healthy,
                "driver registry is not configured",
            );
        };
        match self.registry_signature_valid() {
            Some(1) => RivunNodeHealthCheck::new(
                "registry_signature",
                RivunNodeHealthStatus::Healthy,
                "driver registry signature verifies",
            )
            .with_detail(path.display().to_string()),
            Some(_) => RivunNodeHealthCheck::new(
                "registry_signature",
                RivunNodeHealthStatus::Critical,
                "driver registry signature verification failed",
            )
            .with_detail(path.display().to_string()),
            None => RivunNodeHealthCheck::new(
                "registry_signature",
                RivunNodeHealthStatus::Critical,
                "driver registry signature status is unavailable",
            )
            .with_detail(path.display().to_string()),
        }
    }

    fn registry_bundle_health_check(&self) -> RivunNodeHealthCheck {
        let Some(path) = &self.registry_bundle_path else {
            return RivunNodeHealthCheck::new(
                "registry_bundle",
                RivunNodeHealthStatus::Healthy,
                "registry bundle manifest is not configured",
            );
        };
        match load_registry_bundle_manifest_optional(
            path,
            &RegistryBundleManifestRequest::default(),
        ) {
            Ok(Some(_)) => RivunNodeHealthCheck::new(
                "registry_bundle",
                RivunNodeHealthStatus::Healthy,
                "registry bundle manifest loads",
            )
            .with_detail(path.display().to_string()),
            Ok(None) => RivunNodeHealthCheck::new(
                "registry_bundle",
                RivunNodeHealthStatus::Critical,
                "registry bundle manifest is configured but unavailable",
            )
            .with_detail(path.display().to_string()),
            Err(error) => RivunNodeHealthCheck::new(
                "registry_bundle",
                RivunNodeHealthStatus::Critical,
                "registry bundle manifest failed validation",
            )
            .with_detail(format!("{error:#}")),
        }
    }

    fn receipt_log_health_check(&self, verify_failures: u64) -> RivunNodeHealthCheck {
        if verify_failures > 0 {
            return RivunNodeHealthCheck::new(
                "receipt_log",
                RivunNodeHealthStatus::Critical,
                "receipt verification failures were observed",
            )
            .with_detail(format!("verify_failures={verify_failures}"));
        }
        let Some(store) = &self.receipt_journal else {
            return RivunNodeHealthCheck::new(
                "receipt_log",
                RivunNodeHealthStatus::Degraded,
                "receipt log is not configured",
            );
        };
        let path = store.dir();
        if !path.exists() {
            return RivunNodeHealthCheck::new(
                "receipt_log",
                RivunNodeHealthStatus::Healthy,
                "receipt journal is configured and will be created on first receipt",
            )
            .with_detail(path.display().to_string());
        }
        match store.verify() {
            Ok(_) => RivunNodeHealthCheck::new(
                "receipt_log",
                RivunNodeHealthStatus::Healthy,
                "receipt journal verifies",
            )
            .with_detail(path.display().to_string()),
            Err(error) => RivunNodeHealthCheck::new(
                "receipt_log",
                RivunNodeHealthStatus::Critical,
                "receipt journal verification failed",
            )
            .with_detail(format!("{error:#}")),
        }
    }

    fn capability_cache_health_check(&self, age_seconds: Option<u64>) -> RivunNodeHealthCheck {
        let Some(path) = &self.capability_cache_path else {
            let status = if self
                .route_table
                .routes
                .iter()
                .any(|route| route.requires_peer_grant.is_some())
            {
                RivunNodeHealthStatus::Critical
            } else {
                RivunNodeHealthStatus::Healthy
            };
            return RivunNodeHealthCheck::new(
                "capability_cache",
                status,
                "capability cache is not configured",
            );
        };

        let Some(age_seconds) = age_seconds else {
            return RivunNodeHealthCheck::new(
                "capability_cache",
                RivunNodeHealthStatus::Critical,
                "capability cache is configured but cannot be read",
            )
            .with_detail(path.display().to_string());
        };

        if let Some(max_age_micros) = self.capability_cache_max_age_micros {
            let age_micros = age_seconds.saturating_mul(1_000_000);
            if age_micros > max_age_micros {
                return RivunNodeHealthCheck::new(
                    "capability_cache",
                    RivunNodeHealthStatus::Degraded,
                    "capability cache is stale",
                )
                .with_detail(format!(
                    "path={} age_seconds={} max_age_seconds={}",
                    path.display(),
                    age_seconds,
                    max_age_micros / 1_000_000
                ));
            }
        }

        RivunNodeHealthCheck::new(
            "capability_cache",
            RivunNodeHealthStatus::Healthy,
            "capability cache is fresh enough",
        )
        .with_detail(format!("path={} age_seconds={age_seconds}", path.display()))
    }

    fn message_policy_health_check(&self) -> RivunNodeHealthCheck {
        if self.message_policy.default_decision == MessagePolicyDecision::Allow {
            return RivunNodeHealthCheck::new(
                "message_policy",
                RivunNodeHealthStatus::Degraded,
                "message policy default decision is allow",
            )
            .with_detail(format!("rules={}", self.message_policy.rules.len()));
        }
        RivunNodeHealthCheck::new(
            "message_policy",
            RivunNodeHealthStatus::Healthy,
            "message policy default decision is fail-closed",
        )
        .with_detail(format!(
            "default_decision={:?} rules={}",
            self.message_policy.default_decision,
            self.message_policy.rules.len()
        ))
    }

    fn peer_trust_health_check(&self) -> RivunNodeHealthCheck {
        let quarantined = self
            .peer_trust
            .values()
            .filter(|trust| trust.status == PeerTrustStatus::Quarantined)
            .count();
        let revoked = self
            .peer_trust
            .values()
            .filter(|trust| trust.status == PeerTrustStatus::Revoked)
            .count();
        let status = if revoked > 0 {
            RivunNodeHealthStatus::Critical
        } else if quarantined > 0 {
            RivunNodeHealthStatus::Degraded
        } else {
            RivunNodeHealthStatus::Healthy
        };
        RivunNodeHealthCheck::new("peer_trust", status, "peer trust table evaluated").with_detail(
            format!(
                "peers={} quarantined={} revoked={}",
                self.peer_trust.len(),
                quarantined,
                revoked
            ),
        )
    }

    fn runtime_error_health_check(&self, metrics: &RivunNodeMetricsSnapshot) -> RivunNodeHealthCheck {
        let rejected_total: u64 = metrics
            .frames_rejected_total
            .iter()
            .map(|counter| counter.value)
            .sum();
        let driver_errors_total: u64 = metrics
            .driver_execution_errors_total
            .iter()
            .map(|counter| counter.value)
            .sum();
        if metrics.poa_attestation_failures_total > 0 {
            return RivunNodeHealthCheck::new(
                "runtime_errors",
                RivunNodeHealthStatus::Critical,
                "PoA attestation failures were observed",
            )
            .with_detail(format!(
                "poa_attestation_failures={}",
                metrics.poa_attestation_failures_total
            ));
        }
        if driver_errors_total > 0 || rejected_total > 0 {
            return RivunNodeHealthCheck::new(
                "runtime_errors",
                RivunNodeHealthStatus::Degraded,
                "runtime rejection or driver error counters are nonzero",
            )
            .with_detail(format!(
                "rejected_frames={rejected_total} driver_errors={driver_errors_total}"
            ));
        }
        RivunNodeHealthCheck::new(
            "runtime_errors",
            RivunNodeHealthStatus::Healthy,
            "runtime error counters are clear",
        )
    }

    pub fn record_sent_frame(&self, peer: Uuid) {
        if let Ok(mut counters) = self.metrics.lock() {
            *counters.frames_sent_by_peer.entry(peer).or_default() += 1;
        }
    }

    pub fn record_received_frame(&self, peer: Uuid) {
        if let Ok(mut counters) = self.metrics.lock() {
            *counters.frames_received_by_peer.entry(peer).or_default() += 1;
        }
    }

    pub fn record_rejected_frame(&self, reason: &'static str) {
        if let Ok(mut counters) = self.metrics.lock() {
            *counters
                .frames_rejected_by_reason
                .entry(reason.to_string())
                .or_default() += 1;
        }
    }

    pub fn record_driver_execution_error(&self, action: &str) {
        if let Ok(mut counters) = self.metrics.lock() {
            *counters
                .driver_execution_errors_by_action
                .entry(action.to_string())
                .or_default() += 1;
        }
    }

    pub fn record_receipt_log_verify_failure(&self) {
        if let Ok(mut counters) = self.metrics.lock() {
            counters.receipt_log_verify_failures_total += 1;
        }
    }

    pub fn record_poa_attestation_failure(&self) {
        if let Ok(mut counters) = self.metrics.lock() {
            counters.poa_attestation_failures_total += 1;
        }
    }

    pub fn record_replay_drop(&self) {
        if let Ok(mut counters) = self.metrics.lock() {
            counters.replay_drops_total += 1;
            counters.replay_rejections_total += 1;
        }
    }

    pub fn record_replay_rejection(&self) {
        if let Ok(mut counters) = self.metrics.lock() {
            counters.replay_rejections_total += 1;
        }
    }

    pub fn record_segment_rotation(&self) {
        if let Ok(mut counters) = self.metrics.lock() {
            counters.journal_segment_rotations_total += 1;
        }
    }

    pub fn record_segment_manifest_error(&self) {
        if let Ok(mut counters) = self.metrics.lock() {
            counters.segment_manifest_errors_total += 1;
        }
    }

    pub fn record_pack_verification_failure(&self) {
        if let Ok(mut counters) = self.metrics.lock() {
            counters.pack_verification_failures_total += 1;
        }
    }

    pub fn record_store_verification(&self) {
        if let Ok(mut counters) = self.metrics.lock() {
            counters.store_verifications_total += 1;
        }
    }

    pub fn record_agent_gateway_request(&self, transport: &str, status: &str) {
        if let Ok(mut counters) = self.metrics.lock() {
            *counters
                .agent_gateway_requests_total
                .entry((transport.to_string(), status.to_string()))
                .or_default() += 1;
        }
    }

    pub fn inc_agent_session(&self) {
        if let Ok(mut counters) = self.metrics.lock() {
            counters.agent_sessions_active += 1;
        }
    }

    pub fn dec_agent_session(&self) {
        if let Ok(mut counters) = self.metrics.lock() {
            counters.agent_sessions_active = (counters.agent_sessions_active - 1).max(0);
        }
    }

    pub fn record_provenance_verification_failure(&self) {
        if let Ok(mut counters) = self.metrics.lock() {
            counters.provenance_verification_failures_total += 1;
        }
    }

    pub fn set_peers_active(&self, count: usize) {
        if let Ok(mut counters) = self.metrics.lock() {
            counters.peers_active = count as u64;
        }
    }

    fn peer_trust_gauges(&self) -> Vec<PeerTrustGauge> {
        let mut gauges = Vec::new();
        for (peer, trust) in &self.peer_trust {
            for status in [
                PeerTrustStatus::Trusted,
                PeerTrustStatus::Quarantined,
                PeerTrustStatus::Revoked,
            ] {
                gauges.push(PeerTrustGauge {
                    peer: *peer,
                    status: status.as_metric_label().to_string(),
                    value: u64::from(trust.status == status),
                });
            }
        }
        gauges.sort_by_key(|gauge| (gauge.peer, gauge.status.clone()));
        gauges
    }

    fn registry_signature_valid(&self) -> Option<u8> {
        let path = self.registry_path.clone()?;
        let config = RegistryConfig {
            path: Some(path),
            require_signature: true,
            bundle_path: None,
        };
        Some(u8::from(load_driver_registry_optional(&config).is_ok()))
    }

    fn capability_cache_age_seconds(&self) -> Option<u64> {
        let path = self.capability_cache_path.as_ref()?;
        let modified = fs::metadata(path).ok()?.modified().ok()?;
        SystemTime::now()
            .duration_since(modified)
            .ok()
            .map(|duration| duration.as_secs())
    }

    async fn respond_to_poa_attestation_request(
        &self,
        requester_node: Uuid,
        body: &[u8],
    ) -> Result<()> {
        self.ensure_peer_can_receive_poa_attestation(requester_node)?;
        self.ensure_peer_can_send(requester_node)?;
        let request = serde_json::from_slice(body).context("invalid PoA attestation request")?;
        let response = sign_poa_attestation_request(&self.keypair, &request)?;
        let envelope = RivunEnvelope::new(
            RivunMessageKind::Control,
            POA_ATTESTATION_RESPONSE_SUBJECT,
            POA_ATTESTATION_CONTENT_TYPE,
            Bytes::from(serde_json::to_vec(&response)?),
        )?;
        let frame = RivunFrame::new(
            self.keypair.node_id(),
            requester_node,
            RivunFlags::ENCRYPTED,
            envelope.encode(),
        )?;
        let frame = sign_frame(&self.keypair, &frame)?;
        self.endpoint.send_frame(requester_node, &frame).await?;
        self.record_sent_frame(requester_node);
        Ok(())
    }

    async fn respond_to_capability_query(&self, requester_node: Uuid, body: &[u8]) -> Result<()> {
        self.ensure_peer_can_send(requester_node)?;
        let query = if body.is_empty() {
            CapabilityQuery::default()
        } else {
            serde_json::from_slice::<CapabilityQuery>(body).context("invalid capability query")?
        };
        let response = CapabilityResponse {
            advertisement: self.capability_advertisement.filtered(&query.requested),
        };
        let envelope = RivunEnvelope::new(
            RivunMessageKind::Control,
            CAPABILITY_RESPONSE_SUBJECT,
            CAPABILITY_CONTENT_TYPE,
            Bytes::from(serde_json::to_vec(&response)?),
        )?;
        let frame = RivunFrame::new(
            self.keypair.node_id(),
            requester_node,
            RivunFlags::ENCRYPTED,
            envelope.encode(),
        )?;
        let frame = sign_frame(&self.keypair, &frame)?;
        self.endpoint.send_frame(requester_node, &frame).await?;
        self.record_sent_frame(requester_node);
        Ok(())
    }

    fn record_discovery_announcement(&self, announcer_node: Uuid, body: &[u8]) -> Result<()> {
        let signed: SignedDiscoveryAdvertisement =
            serde_json::from_slice(body).context("invalid discovery announcement")?;
        signed.verify(None)?;
        if signed.advertisement.node_id != announcer_node {
            bail!(
                "discovery announcement node_id {} does not match frame source {}",
                signed.advertisement.node_id,
                announcer_node
            );
        }
        validate_discovery_advertisement_time(&signed.advertisement)?;
        let mut announcements = self
            .discovery_announcements
            .lock()
            .map_err(|_| anyhow!("discovery announcement mutex poisoned"))?;
        announcements.insert(announcer_node, signed.clone());
        drop(announcements);
        persist_discovery_announcement(self.discovery_announcement_cache.as_deref(), &signed)?;
        Ok(())
    }

    async fn respond_to_discovery_query(&self, requester_node: Uuid, body: &[u8]) -> Result<()> {
        self.ensure_peer_can_send(requester_node)?;
        let query = if body.is_empty() {
            DiscoveryQuery::default()
        } else {
            serde_json::from_slice::<DiscoveryQuery>(body).context("invalid discovery query")?
        };
        query.validate()?;
        let response = self.discovery_response(&query)?;
        let envelope = RivunEnvelope::new(
            RivunMessageKind::Control,
            DISCOVERY_RESPONSE_SUBJECT,
            DISCOVERY_CONTENT_TYPE,
            Bytes::from(serde_json::to_vec(&response)?),
        )?;
        let frame = RivunFrame::new(
            self.keypair.node_id(),
            requester_node,
            RivunFlags::ENCRYPTED,
            envelope.encode(),
        )?;
        let frame = sign_frame(&self.keypair, &frame)?;
        self.endpoint.send_frame(requester_node, &frame).await?;
        self.record_sent_frame(requester_node);
        Ok(())
    }

    fn discovery_response(&self, query: &DiscoveryQuery) -> Result<DiscoveryResponse> {
        let advertisement = self.signed_discovery_advertisement(None)?;
        let peers = if query.include_peers {
            self.discovery_peers.clone()
        } else {
            Vec::new()
        };
        let announcements = if query.include_known {
            self.discovery_announcements_for_query(query)?
        } else {
            Vec::new()
        };
        let response = DiscoveryResponse {
            schema_version: DISCOVERY_SCHEMA_VERSION,
            node_id: self.keypair.node_id(),
            advertisement,
            peers,
            announcements,
        };
        response.verify(self.keypair.node_id(), &self.keypair.verifying_key())?;
        Ok(response)
    }

    fn signed_discovery_advertisement(
        &self,
        expires_at_micros: Option<u64>,
    ) -> Result<SignedDiscoveryAdvertisement> {
        let advertised_addr = Some(self.local_addr()?.to_string());
        let advertisement = build_discovery_advertisement(
            &self.keypair,
            advertised_addr,
            self.capability_advertisement.clone(),
            Vec::new(),
            Vec::new(),
            expires_at_micros,
        )?;
        sign_discovery_advertisement(&self.keypair, advertisement)
    }

    fn discovery_announcements_for_query(
        &self,
        query: &DiscoveryQuery,
    ) -> Result<Vec<SignedDiscoveryAdvertisement>> {
        let announcements = self
            .discovery_announcements
            .lock()
            .map_err(|_| anyhow!("discovery announcement mutex poisoned"))?;
        let mut matches = Vec::new();
        for announcement in announcements.values() {
            if validate_discovery_advertisement_time(&announcement.advertisement).is_err() {
                continue;
            }
            if query.matches_advertisement(&announcement.advertisement) {
                matches.push(announcement.clone());
            }
        }
        matches.sort_by_key(|announcement| announcement.advertisement.node_id);
        Ok(matches)
    }

    async fn respond_to_receipt_replication_request(
        &self,
        requester_node: Uuid,
        body: &[u8],
    ) -> Result<()> {
        self.ensure_peer_can_send(requester_node)?;
        let request = if body.is_empty() {
            ReceiptReplicationRequest::default()
        } else {
            serde_json::from_slice::<ReceiptReplicationRequest>(body)
                .context("invalid receipt replication request")?
        };
        request.validate()?;
        let limit = request.effective_limit()?;
        let mut receipts = match &self.receipt_journal {
            Some(store) => match store.query_with_limit(&request, limit.saturating_add(1)) {
                Ok(receipts) => receipts,
                Err(error) => {
                    self.record_receipt_log_verify_failure();
                    return Err(error.into());
                }
            },
            None => Vec::new(),
        };
        let truncated = receipts.len() > limit;
        receipts.truncate(limit);
        let next_after_processed_at_micros = truncated
            .then(|| {
                receipts
                    .last()
                    .map(|receipt| receipt.receipt.processed_at_micros)
            })
            .flatten();
        let response = ReceiptReplicationResponse::new_with_cursor(
            self.keypair.node_id(),
            receipts,
            truncated,
            next_after_processed_at_micros,
        );
        let envelope = RivunEnvelope::new(
            RivunMessageKind::Control,
            RECEIPT_REPLICATION_RESPONSE_SUBJECT,
            RECEIPT_REPLICATION_CONTENT_TYPE,
            Bytes::from(serde_json::to_vec(&response)?),
        )?;
        let frame = RivunFrame::new(
            self.keypair.node_id(),
            requester_node,
            RivunFlags::ENCRYPTED,
            envelope.encode(),
        )?;
        let frame = sign_frame(&self.keypair, &frame)?;
        self.endpoint.send_frame(requester_node, &frame).await?;
        self.record_sent_frame(requester_node);
        Ok(())
    }

    async fn respond_to_poa_validator_set_request(
        &self,
        requester_node: Uuid,
        body: &[u8],
    ) -> Result<()> {
        self.ensure_peer_can_send(requester_node)?;
        let request = if body.is_empty() {
            PoaValidatorSetRequest::default()
        } else {
            serde_json::from_slice::<PoaValidatorSetRequest>(body)
                .context("invalid PoA validator-set request")?
        };
        request.validate()?;
        let (validator_set, unavailable_reason) = match &self.poa_validator_set_path {
            Some(path) => {
                let signed = load_signed_poa_validator_set(path)?;
                signed.verify(self.poa_validator_set_authority.as_ref())?;
                validate_poa_validator_set_time(&signed)?;
                if let Some(min_epoch) = request.min_epoch
                    && signed.set.epoch < min_epoch
                {
                    (
                        None,
                        Some(format!(
                            "validator set epoch {} is below requested minimum {}",
                            signed.set.epoch, min_epoch
                        )),
                    )
                } else {
                    (Some(signed), None)
                }
            }
            None => (
                None,
                Some("node has no configured poa.validator_set".to_string()),
            ),
        };
        let response =
            PoaValidatorSetResponse::new(self.keypair.node_id(), validator_set, unavailable_reason);
        let envelope = RivunEnvelope::new(
            RivunMessageKind::Control,
            POA_VALIDATOR_SET_RESPONSE_SUBJECT,
            POA_VALIDATOR_SET_CONTENT_TYPE,
            Bytes::from(serde_json::to_vec(&response)?),
        )?;
        let frame = RivunFrame::new(
            self.keypair.node_id(),
            requester_node,
            RivunFlags::ENCRYPTED,
            envelope.encode(),
        )?;
        let frame = sign_frame(&self.keypair, &frame)?;
        self.endpoint.send_frame(requester_node, &frame).await?;
        self.record_sent_frame(requester_node);
        Ok(())
    }

    async fn respond_to_registry_index_request(
        &self,
        requester_node: Uuid,
        body: &[u8],
    ) -> Result<()> {
        self.ensure_peer_can_send(requester_node)?;
        let request = if body.is_empty() {
            RegistryIndexRequest::default()
        } else {
            serde_json::from_slice::<RegistryIndexRequest>(body)
                .context("invalid registry index request")?
        };
        request.validate()?;
        let (registry, unavailable_reason) = match &self.registry_path {
            Some(path) => {
                let require_signature =
                    self.registry_require_signature || request.require_signature;
                let registry_config = RegistryConfig {
                    path: Some(path.clone()),
                    require_signature,
                    bundle_path: None,
                };
                match load_driver_registry_optional(&registry_config) {
                    Ok(Some(registry)) => (Some(registry), None),
                    Ok(None) => (
                        None,
                        Some("node has no configured registry.path".to_string()),
                    ),
                    Err(error) => (None, Some(format!("{error:#}"))),
                }
            }
            None => (
                None,
                Some("node has no configured registry.path".to_string()),
            ),
        };
        let response =
            RegistryIndexResponse::new(self.keypair.node_id(), registry, unavailable_reason);
        let envelope = RivunEnvelope::new(
            RivunMessageKind::Control,
            REGISTRY_INDEX_RESPONSE_SUBJECT,
            REGISTRY_INDEX_CONTENT_TYPE,
            Bytes::from(serde_json::to_vec(&response)?),
        )?;
        let frame = RivunFrame::new(
            self.keypair.node_id(),
            requester_node,
            RivunFlags::ENCRYPTED,
            envelope.encode(),
        )?;
        let frame = sign_frame(&self.keypair, &frame)?;
        self.endpoint.send_frame(requester_node, &frame).await?;
        self.record_sent_frame(requester_node);
        Ok(())
    }

    async fn respond_to_registry_bundle_manifest_request(
        &self,
        requester_node: Uuid,
        body: &[u8],
    ) -> Result<()> {
        self.ensure_peer_can_send(requester_node)?;
        let request = if body.is_empty() {
            RegistryBundleManifestRequest::default()
        } else {
            serde_json::from_slice::<RegistryBundleManifestRequest>(body)
                .context("invalid registry bundle manifest request")?
        };
        request.validate()?;
        let (manifest, unavailable_reason) = match &self.registry_bundle_path {
            Some(path) => match load_registry_bundle_manifest_optional(path, &request) {
                Ok(Some(manifest)) => (Some(manifest), None),
                Ok(None) => (
                    None,
                    Some("node has no configured registry.bundle_path".to_string()),
                ),
                Err(error) => (None, Some(format!("{error:#}"))),
            },
            None => (
                None,
                Some("node has no configured registry.bundle_path".to_string()),
            ),
        };
        let response = RegistryBundleManifestResponse::new(
            self.keypair.node_id(),
            manifest,
            unavailable_reason,
        );
        let envelope = RivunEnvelope::new(
            RivunMessageKind::Control,
            REGISTRY_BUNDLE_MANIFEST_RESPONSE_SUBJECT,
            REGISTRY_BUNDLE_MANIFEST_CONTENT_TYPE,
            Bytes::from(serde_json::to_vec(&response)?),
        )?;
        let frame = RivunFrame::new(
            self.keypair.node_id(),
            requester_node,
            RivunFlags::ENCRYPTED,
            envelope.encode(),
        )?;
        let frame = sign_frame(&self.keypair, &frame)?;
        self.endpoint.send_frame(requester_node, &frame).await?;
        self.record_sent_frame(requester_node);
        Ok(())
    }

    async fn route_message(
        &self,
        inbound: &rivun_net::InboundRivun,
        message: &InboundMessage,
    ) -> Result<Option<Vec<u8>>> {
        let route_message = RouteMessage {
            source_node: inbound.peer.node_id,
            target_node: inbound.frame.header.target_node,
            kind: message.kind.as_str().to_string(),
            subject: message.subject.clone(),
            content_type: message.content_type.clone(),
        };
        let decision = self.route_table.decide(&route_message);
        self.apply_route_decision(decision, &inbound.frame, message)
            .await
    }

    async fn apply_route_decision(
        &self,
        decision: RouteDecision,
        frame: &RivunFrame,
        message: &InboundMessage,
    ) -> Result<Option<Vec<u8>>> {
        let target = decision.target;
        if target.drop {
            info!(
                kind = %message.kind,
                subject = %message.subject,
                reason = %decision.reason,
                "dropped Rivun message by route"
            );
            return Ok(None);
        }
        if let Some(peer) = target.peer {
            self.forward_message(peer, frame, message).await?;
            return Ok(None);
        }
        if target.broadcast {
            for peer in &self.peer_ids {
                self.forward_message(*peer, frame, message).await?;
            }
            return Ok(None);
        }
        if let Some(capability) = target.capability {
            return self.dispatch_capability(capability, message, frame).await;
        }
        if let Some(action) = target.local_driver {
            return self.dispatch_local_driver(&action, message, frame).await;
        }
        Ok(None)
    }

    async fn dispatch_capability(
        &self,
        capability: CapabilityId,
        message: &InboundMessage,
        frame: &RivunFrame,
    ) -> Result<Option<Vec<u8>>> {
        if let Some(action) = capability.driver_action() {
            return self.dispatch_local_driver(action, message, frame).await;
        }
        warn!(
            capability = %capability,
            "capability route has no local executor; message acknowledged only"
        );
        Ok(None)
    }

    async fn dispatch_local_driver(
        &self,
        action: &str,
        message: &InboundMessage,
        frame: &RivunFrame,
    ) -> Result<Option<Vec<u8>>> {
        if message.kind != RivunMessageKind::Action {
            info!(
                kind = %message.kind,
                subject = %message.subject,
                action,
                "local driver route ignored for non-action message"
            );
            return Ok(None);
        }
        match self.drivers.get(action) {
            Some(driver) => {
                let mut limits = self.action_limits.get(action).copied().unwrap_or(self.limits);
                limits.permissions = merge_permissions(limits.permissions, driver.permissions);
                let execution = if let (Some(async_runtime), Some(async_driver)) =
                    (&self.async_runtime, &driver.async_driver)
                {
                    async_runtime
                        .execute_async(async_driver, action, &message.body, limits)
                        .await
                        .map(|result| (result.output, result.host_calls))
                } else {
                    self.runtime
                        .execute(&driver.driver, action, &message.body, limits)
                        .map(|result| (result.output, result.host_calls))
                };
                let (output, host_calls) = match execution {
                    Ok(result) => result,
                    Err(error) => {
                        self.record_driver_execution_error(action);
                        return Err(error.into());
                    }
                };
                self.record_host_calls(action, message, frame, &host_calls)?;
                Ok(Some(output))
            }
            None => {
                warn!(
                    action,
                    "no WASM driver registered; action acknowledged only"
                );
                Ok(None)
            }
        }
    }

    fn record_host_calls(
        &self,
        action: &str,
        message: &InboundMessage,
        frame: &RivunFrame,
        host_calls: &[HostCallRecord],
    ) -> Result<()> {
        for call in host_calls {
            match call.kind {
                HostCallKind::MemoryWrite => {
                    if !self.memory.allow_driver_write {
                        bail!(
                            "driver `{}` called rivun.memory_write but memory.allow_driver_write=false",
                            action
                        );
                    }
                    let dir = self.memory.dir.as_ref().ok_or_else(|| {
                        anyhow!(
                            "driver `{}` called rivun.memory_write but memory.dir is not configured",
                            action
                        )
                    })?;
                    let mut store = MemoryJournalStore::open(dir);
                    if let Some(max_record_bytes) = self.memory.max_record_bytes {
                        store = store.with_max_record_bytes(max_record_bytes);
                    }
                    store
                        .put(MemoryPut {
                            namespace: "driver".to_string(),
                            subject: format!("driver.{action}.memory_write"),
                            content_type: "application/octet-stream".to_string(),
                            body: call.payload.clone(),
                            metadata: serde_json::json!({
                                "host_call": "memory_write",
                                "action": action,
                                "message_kind": message.kind.as_str(),
                                "message_subject": message.subject,
                            }),
                            source_node: Some(frame.header.source_node),
                            frame_hash: Some(hash_frame(frame)),
                        })
                        .with_context(|| {
                            format!(
                                "failed to append driver `{}` memory_write host call",
                                action
                            )
                        })?;
                }
                HostCallKind::EmitEvent => {
                    info!(
                        action,
                        payload_bytes = call.payload.len(),
                        "driver emitted host event"
                    );
                }
                HostCallKind::DeviceCall => {
                    warn!(
                        action,
                        payload_bytes = call.payload.len(),
                        "driver requested device_call but no device bridge is configured in ABI v2 foundation"
                    );
                }
            }
        }
        Ok(())
    }

    fn validate_message_contracts(&self, message: &InboundMessage) -> Result<()> {
        self.message_contracts
            .validate_message(&MessageParts {
                kind: message.kind.as_str(),
                subject: &message.subject,
                content_type: message.content_type.as_deref(),
                metadata: &message.metadata,
                body: &message.body,
            })
            .with_context(|| {
                format!(
                    "message contract validation failed for {} {}",
                    message.kind, message.subject
                )
            })?;
        Ok(())
    }

    fn apply_message_policy(&self, frame: &RivunFrame, message: &InboundMessage) -> Result<()> {
        let policy = PolicySet::new_with_default(
            self.message_policy.default_decision,
            self.message_policy
                .rules
                .iter()
                .cloned()
                .map(PolicyRule::from)
                .collect(),
        )?;
        let granted_capabilities = self
            .capability_advertisement
            .grants
            .iter()
            .map(|grant| grant.capability.clone())
            .collect::<BTreeSet<_>>();
        let evaluation = policy.evaluate(&PolicyInput {
            kind: message.kind.as_str(),
            subject: &message.subject,
            source_node: Some(frame.header.source_node),
            target_node: Some(frame.header.target_node),
            content_type: message.content_type.as_deref(),
            consensus_protected: frame.header.flags.contains(RivunFlags::REQUIRES_CONSENSUS),
            granted_capabilities: &granted_capabilities,
            human_approved: false,
            simulation_passed: false,
        });
        if evaluation.allowed {
            return Ok(());
        }

        match evaluation.decision {
            MessagePolicyDecision::Allow => Ok(()),
            MessagePolicyDecision::Deny => bail!(
                "message policy denied {} {}: {}",
                message.kind,
                message.subject,
                evaluation.reason
            ),
            MessagePolicyDecision::RequirePoa => bail!(
                "message policy requires Proof-of-Action for {} {}: {}",
                message.kind,
                message.subject,
                evaluation.reason
            ),
            MessagePolicyDecision::RequireGrant => bail!(
                "message policy requires capability grant {:?} for {} {}: {}",
                evaluation.required_capability,
                message.kind,
                message.subject,
                evaluation.reason
            ),
            MessagePolicyDecision::HumanApproval => bail!(
                "message policy requires external human approval for {} {}: {}",
                message.kind,
                message.subject,
                evaluation.reason
            ),
            MessagePolicyDecision::SimulateFirst => bail!(
                "message policy requires simulation evidence for {} {}: {}",
                message.kind,
                message.subject,
                evaluation.reason
            ),
        }
    }

    async fn forward_message(
        &self,
        peer: Uuid,
        frame: &RivunFrame,
        message: &InboundMessage,
    ) -> Result<()> {
        self.ensure_peer_can_forward(peer)?;
        if frame.header.flags.contains(RivunFlags::REQUIRES_CONSENSUS) {
            bail!("route forwarding of consensus-protected frames is not supported in v1");
        }
        let forwarded = RivunFrame::new(
            self.keypair.node_id(),
            peer,
            RivunFlags::ENCRYPTED,
            Bytes::from(frame.payload.to_vec()),
        )
        .with_context(|| {
            format!(
                "failed to build routed frame for {} {}",
                message.kind, message.subject
            )
        })?;
        let forwarded = sign_frame(&self.keypair, &forwarded)?;
        self.endpoint.send_frame(peer, &forwarded).await?;
        self.record_sent_frame(peer);
        Ok(())
    }

    fn peer_trust(&self, peer: Uuid) -> Result<&PeerTrustConfig> {
        self.peer_trust
            .get(&peer)
            .ok_or_else(|| anyhow!("missing trust contract for peer {}", peer))
    }

    fn ensure_peer_can_receive(&self, peer: Uuid) -> Result<()> {
        let trust = self.peer_trust(peer)?;
        if trust.allows_receive() {
            return Ok(());
        }
        bail!("peer {} is not permitted to send inbound frames", peer)
    }

    fn ensure_peer_can_send(&self, peer: Uuid) -> Result<()> {
        let trust = self.peer_trust(peer)?;
        if trust.allows_send() {
            return Ok(());
        }
        bail!("peer {} is not permitted to receive outbound frames", peer)
    }

    fn ensure_peer_can_forward(&self, peer: Uuid) -> Result<()> {
        let trust = self.peer_trust(peer)?;
        if trust.allows_forward() {
            return Ok(());
        }
        bail!(
            "peer {} is not permitted as a route forwarding target",
            peer
        )
    }

    fn ensure_peer_can_receive_poa_attestation(&self, peer: Uuid) -> Result<()> {
        let trust = self.peer_trust(peer)?;
        if trust.allows_poa_attestation() {
            return Ok(());
        }
        bail!(
            "peer {} is not permitted to receive Proof-of-Action attestations",
            peer
        )
    }

    fn validate_fresh_frame(&self, frame: &rivun_core::RivunFrame) -> Result<()> {
        let now = now_micros()?;
        let timestamp = frame.header.timestamp_micros;
        let skew = self.security.max_clock_skew_micros;
        if timestamp.saturating_add(skew) < now {
            bail!(
                "stale frame timestamp: timestamp_micros={}, now_micros={}, max_clock_skew_micros={}",
                timestamp,
                now,
                skew
            );
        }
        if timestamp > now.saturating_add(skew) {
            bail!(
                "future frame timestamp: timestamp_micros={}, now_micros={}, max_clock_skew_micros={}",
                timestamp,
                now,
                skew
            );
        }

        let mut guard = self
            .replay_guard
            .lock()
            .map_err(|_| anyhow!("anti-replay guard mutex poisoned"))?;
        guard.remember(frame)
    }

    fn verify_consensus(&self, frame: &rivun_core::RivunFrame) -> Result<()> {
        if self.poa_validators.is_empty() {
            self.record_poa_attestation_failure();
            bail!("frame requires Proof-of-Action, but no PoA validators are configured");
        }
        match verify_poa_certificate(frame, &self.poa_validators, self.poa_required_threshold)
            .context("inbound frame failed Proof-of-Action validation")
        {
            Ok(()) => Ok(()),
            Err(error) => {
                self.record_poa_attestation_failure();
                Err(error)
            }
        }
    }

    fn write_receipt(
        &self,
        frame: &rivun_core::RivunFrame,
        message: &InboundMessage,
        output: Option<&[u8]>,
    ) -> Result<()> {
        let Some(store) = &self.receipt_journal else {
            return Ok(());
        };
        let processed_at_micros = now_micros()?;
        let required_threshold = frame
            .header
            .flags
            .contains(rivun_core::RivunFlags::REQUIRES_CONSENSUS)
            .then_some(self.poa_required_threshold);
        let pact = pact_receipt_reference(message, frame)?;
        let receipt = SignedActionReceipt::new_message_with_pact(
            &self.keypair,
            frame,
            message.kind.as_str(),
            &message.subject,
            output,
            processed_at_micros,
            required_threshold,
            pact,
        )?;
        let should_sync = self
            .receipt_durability
            .lock()
            .unwrap()
            .record_write(self.receipt_fsync, self.receipt_fsync_interval_writes);
        store.append(&receipt, should_sync).with_context(|| {
            format!("failed to write receipt journal {}", store.dir().display())
        })?;
        Ok(())
    }
}

fn pact_receipt_reference(
    message: &InboundMessage,
    frame: &rivun_core::RivunFrame,
) -> Result<Option<PactReceiptReference>> {
    if message.kind != RivunMessageKind::Action
        || message.subject != PACT_RECORD_SUBJECT
        || message.content_type.as_deref() != Some(PACT_CONTENT_TYPE)
    {
        return Ok(None);
    }

    let pact: RivunPact = serde_json::from_slice(&message.body)
        .context("failed to parse rivun.pact.record body for receipt reference")?;
    let verification = pact
        .verify(None)
        .context("failed to verify rivun.pact.record body for receipt reference")?;
    let poa_summary = frame.poa.as_ref().map(|poa| {
        format!(
            "threshold={} attestations={}",
            poa.threshold,
            poa.attestations.len()
        )
    });
    Ok(Some(PactReceiptReference {
        pact_id: pact.pact_id,
        intent: pact.intent,
        hash: verification.hash,
        status: format!("{:?}", verification.status).to_ascii_lowercase(),
        policy_decision: None,
        poa_summary,
        output_hash: None,
    }))
}

fn validate_config(config: &RivunNodeConfig) -> Result<ConfigValidationReport> {
    let executor = WasmExecutor::new()?;
    validate_config_with_executor(config, &executor)
}

fn validate_config_with_executor(
    config: &RivunNodeConfig,
    executor: &WasmExecutor,
) -> Result<ConfigValidationReport> {
    let bind = config
        .bind
        .parse::<SocketAddr>()
        .with_context(|| format!("invalid bind address {}", config.bind))?;
    let keypair = load_keypair(&config.key_file)?;
    let node_id = keypair.node_id();
    let mut warnings = Vec::new();

    if !config.require_signed {
        warnings.push("require_signed=false allows unauthenticated action frames".to_string());
    }
    if config.peers.is_empty() {
        warnings.push(
            "no peers configured; node can receive only after peers are added programmatically"
                .to_string(),
        );
    }
    if config.security.max_clock_skew_micros == 0 {
        bail!("security.max_clock_skew_micros must be greater than zero");
    }
    if config.security.replay_cache_capacity == 0 {
        warnings.push(
            "security.replay_cache_capacity=0 disables frame and datagram nonce replay detection"
                .to_string(),
        );
    }
    if let Some(max_datagram_size) = config.max_datagram_size
        && max_datagram_size < 128
    {
        bail!("max_datagram_size must be at least 128 bytes");
    }
    if let Some(max_datagram_size) = config.max_datagram_size
        && max_datagram_size > MAX_DATAGRAM_SIZE
    {
        bail!("max_datagram_size must be at most {MAX_DATAGRAM_SIZE} bytes");
    }

    warn_key_file_permissions(&config.key_file, &mut warnings)?;
    validate_receipts(config)?;
    validate_memory(config)?;
    validate_capability_cache_config(config)?;
    let observability_http_bind = validate_observability(config)?;
    validate_message_policy(config, &mut warnings)?;
    let message_contracts = load_message_contract_set(&config.message_schema)?;
    validate_runtime(config.runtime, &config.action_runtime_limits, &config.drivers)?;
    let peer_trust_summary = validate_peers(config, bind, node_id, &mut warnings)?;
    let poa_summary = validate_poa(config, &mut warnings)?;
    let registry = load_driver_registry_optional(&config.registry)?;
    let registry_entry_count = registry
        .as_ref()
        .map(|registry| registry.entries.len())
        .unwrap_or(0);
    let registry_bundle_enabled =
        load_registry_bundle_manifest_from_config(&config.registry)?.is_some();
    let signed_driver_count = validate_drivers(
        executor,
        &config.drivers,
        config.runtime,
        &config.memory,
        registry.as_ref(),
        &mut warnings,
    )?;
    let peer_grant_route_count = validate_routes(config, &mut warnings)?;
    let advertisement = describe_capabilities(config)?;
    let capability_count = advertisement.capabilities.capabilities.len();
    let ungranted_capability_count =
        validate_capability_policy(config, &advertisement, &mut warnings)?;

    Ok(ConfigValidationReport {
        bind,
        node_id,
        observability_http_bind,
        peer_count: config.peers.len(),
        trusted_peer_count: peer_trust_summary.trusted_peer_count,
        restricted_peer_count: peer_trust_summary.restricted_peer_count,
        peer_send_enabled_count: peer_trust_summary.peer_send_enabled_count,
        peer_receive_enabled_count: peer_trust_summary.peer_receive_enabled_count,
        peer_forward_enabled_count: peer_trust_summary.peer_forward_enabled_count,
        driver_count: config.drivers.len(),
        signed_driver_count,
        receipt_journal_enabled: config.receipts.dir.is_some(),
        registry_enabled: config.registry.path.is_some(),
        registry_entry_count,
        registry_signature_required: config.registry.require_signature,
        registry_bundle_enabled,
        require_signed: config.require_signed,
        poa_validator_count: poa_summary.validator_count,
        poa_required_threshold: poa_summary.required_threshold,
        poa_validator_set_enabled: poa_summary.validator_set_enabled,
        poa_validator_set_epoch: poa_summary.validator_set_epoch,
        memory_enabled: config.memory.dir.is_some(),
        route_count: config.routes.len(),
        capability_count,
        capability_grant_count: advertisement.grants.len(),
        capability_requirement_count: advertisement.requirements.len(),
        ungranted_capability_count,
        capability_cache_enabled: config.capability_cache.path.is_some(),
        discovery_cache_enabled: config.discovery.announcement_cache.is_some(),
        message_policy_default_decision: config.message_policy.default_decision,
        message_policy_rule_count: config.message_policy.rules.len(),
        message_schema_contract_count: message_contracts.contracts.len(),
        message_schema_require_match: message_contracts.require_match,
        peer_grant_route_count,
        warnings,
    })
}

fn validate_observability(config: &RivunNodeConfig) -> Result<Option<SocketAddr>> {
    let Some(bind) = config.observability.http_bind.as_deref() else {
        return Ok(None);
    };
    let addr = bind
        .parse::<SocketAddr>()
        .with_context(|| format!("invalid observability.http_bind address {bind}"))?;
    Ok(Some(addr))
}

fn validate_runtime(
    runtime: RuntimeConfig,
    action_limits: &BTreeMap<String, ActionRuntimeLimits>,
    drivers: &[DriverConfig],
) -> Result<()> {
    if matches!(runtime.max_memory_bytes, Some(0)) {
        bail!("runtime.max_memory_bytes must be greater than zero");
    }
    if matches!(runtime.fuel, Some(0)) {
        bail!("runtime.fuel must be greater than zero");
    }
    if matches!(runtime.timeout_ms, Some(0)) {
        bail!("runtime.timeout_ms must be greater than zero");
    }
    if matches!(runtime.max_output_bytes, Some(0)) {
        bail!("runtime.max_output_bytes must be greater than zero");
    }
    let global = runtime.to_limits();
    for (action, limits) in action_limits {
        if action.trim().is_empty() {
            bail!("action_runtime_limits action must not be empty");
        }
        if !drivers.iter().any(|driver| driver.action == *action) {
            bail!("action_runtime_limits references unknown driver action {action}");
        }
        for (name, value, maximum) in [
            ("max_memory_bytes", limits.max_memory_bytes, global.max_memory_bytes),
            ("fuel", limits.fuel.map(|value| value as usize), global.fuel as usize),
            ("timeout_ms", limits.timeout_ms.map(|value| value as usize), global.timeout_ms as usize),
            ("max_output_bytes", limits.max_output_bytes, global.max_output_bytes),
        ] {
            if matches!(value, Some(0)) {
                bail!("action_runtime_limits.{action}.{name} must be greater than zero");
            }
            if value.is_some_and(|value| value > maximum) {
                bail!("action_runtime_limits.{action}.{name} must not exceed global runtime limit");
            }
        }
    }
    Ok(())
}

fn resolved_action_limits(
    runtime: RuntimeConfig,
    configured: &BTreeMap<String, ActionRuntimeLimits>,
) -> HashMap<String, ExecutionLimits> {
    let global = runtime.to_limits();
    configured
        .iter()
        .map(|(action, limits)| (action.clone(), limits.apply(global)))
        .collect()
}

fn validate_receipts(config: &RivunNodeConfig) -> Result<()> {
    if config.receipts.path.is_some() {
        bail!(
            "receipts.path is no longer supported; use receipts.dir for the binary receipt journal"
        );
    }
    if config.receipts.fsync == ReceiptFsyncPolicy::Interval
        && config.receipts.fsync_interval_writes() == 0
    {
        bail!("receipts.fsync_interval_writes must be greater than 0 when receipts.fsync=interval");
    }
    let Some(receipt_dir) = &config.receipts.dir else {
        return Ok(());
    };
    if receipt_dir == &config.key_file {
        bail!(
            "receipts.dir must not point at key_file {}",
            config.key_file.display()
        );
    }
    if let Some(registry_path) = &config.registry.path
        && receipt_dir == registry_path
    {
        bail!(
            "receipts.dir must not point at registry.path {}",
            registry_path.display()
        );
    }
    for driver in &config.drivers {
        if receipt_dir == &driver.path {
            bail!(
                "receipts.dir must not point at driver `{}` path {}",
                driver.action,
                driver.path.display()
            );
        }
        if let Some(manifest_path) = &driver.manifest
            && receipt_dir == manifest_path
        {
            bail!(
                "receipts.dir must not point at driver `{}` manifest {}",
                driver.action,
                manifest_path.display()
            );
        }
    }
    Ok(())
}

fn validate_memory(config: &RivunNodeConfig) -> Result<()> {
    if config.memory.path.is_some() {
        bail!("memory.path is no longer supported; use memory.dir for the binary memory journal");
    }
    if matches!(config.memory.max_record_bytes, Some(0)) {
        bail!("memory.max_record_bytes must be greater than zero");
    }
    if (config.memory.allow_driver_read || config.memory.allow_driver_write)
        && config.memory.dir.is_none()
    {
        bail!("memory driver access requires memory.dir");
    }
    if let Some(memory_dir) = &config.memory.dir {
        if memory_dir == &config.key_file {
            bail!(
                "memory.dir must not point at key_file {}",
                config.key_file.display()
            );
        }
        if let Some(receipt_dir) = &config.receipts.dir
            && memory_dir == receipt_dir
        {
            bail!(
                "memory.dir must not point at receipts.dir {}",
                receipt_dir.display()
            );
        }
        if let Some(registry_path) = &config.registry.path
            && memory_dir == registry_path
        {
            bail!(
                "memory.dir must not point at registry.path {}",
                registry_path.display()
            );
        }
        for driver in &config.drivers {
            if memory_dir == &driver.path {
                bail!(
                    "memory.dir must not point at driver `{}` path {}",
                    driver.action,
                    driver.path.display()
                );
            }
            if let Some(manifest_path) = &driver.manifest
                && memory_dir == manifest_path
            {
                bail!(
                    "memory.dir must not point at driver `{}` manifest {}",
                    driver.action,
                    manifest_path.display()
                );
            }
        }
    }
    Ok(())
}

fn validate_capability_cache_config(config: &RivunNodeConfig) -> Result<()> {
    if matches!(config.capability_cache.max_age_micros, Some(0)) {
        bail!("capability_cache.max_age_micros must be greater than zero");
    }
    let Some(cache_path) = &config.capability_cache.path else {
        return Ok(());
    };
    if cache_path == &config.key_file {
        bail!(
            "capability_cache.path must not point at key_file {}",
            config.key_file.display()
        );
    }
    if let Some(receipt_dir) = &config.receipts.dir
        && cache_path == receipt_dir
    {
        bail!(
            "capability_cache.path must not point at receipts.dir {}",
            receipt_dir.display()
        );
    }
    if let Some(registry_path) = &config.registry.path
        && cache_path == registry_path
    {
        bail!(
            "capability_cache.path must not point at registry.path {}",
            registry_path.display()
        );
    }
    if let Some(memory_dir) = &config.memory.dir
        && cache_path == memory_dir
    {
        bail!(
            "capability_cache.path must not point at memory.dir {}",
            memory_dir.display()
        );
    }
    for driver in &config.drivers {
        if cache_path == &driver.path {
            bail!(
                "capability_cache.path must not point at driver `{}` path {}",
                driver.action,
                driver.path.display()
            );
        }
        if let Some(manifest_path) = &driver.manifest
            && cache_path == manifest_path
        {
            bail!(
                "capability_cache.path must not point at driver `{}` manifest {}",
                driver.action,
                manifest_path.display()
            );
        }
    }
    Ok(())
}

fn validate_message_policy(config: &RivunNodeConfig, warnings: &mut Vec<String>) -> Result<()> {
    if !matches!(
        config.message_policy.default_decision,
        MessagePolicyDecision::Allow | MessagePolicyDecision::Deny
    ) {
        bail!("message_policy.default_decision must be allow or deny");
    }
    for (index, rule) in config.message_policy.rules.iter().enumerate() {
        let rule_name = format!("message_policy.rules[{index}]");
        if let Some(kind) = rule.kind.as_deref()
            && kind != "*"
        {
            validate_message_text(
                &format!("{rule_name}.kind"),
                kind,
                ZENV_MAX_CONTENT_TYPE_LEN,
                false,
            )?;
            kind.parse::<RivunMessageKind>()
                .with_context(|| format!("invalid {rule_name}.kind `{kind}`"))?;
        }
        if let Some(subject) = rule.subject.as_deref() {
            validate_message_text(
                &format!("{rule_name}.subject"),
                subject,
                ZENV_MAX_SUBJECT_LEN,
                false,
            )?;
        }
        if let Some(content_type) = rule.content_type.as_deref()
            && content_type != "*"
        {
            validate_message_text(
                &format!("{rule_name}.content_type"),
                content_type,
                ZENV_MAX_CONTENT_TYPE_LEN,
                false,
            )?;
        }
        if rule.decision == MessagePolicyDecision::RequireGrant
            && rule.required_capability.is_none()
        {
            bail!("{rule_name}.required_capability is required for decision=require_grant");
        }
        if matches!(
            rule.decision,
            MessagePolicyDecision::HumanApproval | MessagePolicyDecision::SimulateFirst
        ) {
            warnings.push(format!(
                "{rule_name} uses decision={:?}; this gate fails closed until an approval or simulation subsystem supplies trusted evidence",
                rule.decision
            ));
        }
        if let Some(reason) = rule.reason.as_deref()
            && reason.trim().is_empty()
        {
            bail!("{rule_name}.reason must not be empty when provided");
        }
    }
    Ok(())
}

fn load_message_contract_set(config: &MessageSchemaConfig) -> Result<MessageContractSet> {
    let contracts = config
        .contracts
        .iter()
        .map(|contract| load_message_contract(&contract.path))
        .collect::<Result<Vec<_>>>()?;
    MessageContractSet::new(config.require_match, contracts).map_err(Into::into)
}

fn load_message_contract(path: &Path) -> Result<MessageContract> {
    let input = fs::read_to_string(path)
        .with_context(|| format!("failed to read message contract {}", path.display()))?;
    let contract = match path.extension().and_then(|extension| extension.to_str()) {
        Some(extension) if extension.eq_ignore_ascii_case("json") => {
            MessageContract::from_json_str(&input)
        }
        _ => MessageContract::from_toml_str(&input),
    }
    .with_context(|| format!("invalid message contract {}", path.display()))?;
    Ok(contract)
}

fn validate_routes(config: &RivunNodeConfig, warnings: &mut Vec<String>) -> Result<usize> {
    let table = RouteTable::new(config.routes.clone())?;
    let peer_ids = config
        .peers
        .iter()
        .map(|peer| (peer.node_id, peer))
        .collect::<HashMap<_, _>>();
    let driver_actions = config
        .drivers
        .iter()
        .map(|driver| driver.action.as_str())
        .collect::<HashSet<_>>();
    let mut peer_grant_route_count = 0;
    for (index, route) in table.routes.iter().enumerate() {
        let route_name = route.name.clone().unwrap_or_else(|| format!("#{index}"));
        if let Some(peer) = route.target.peer {
            let Some(peer_config) = peer_ids.get(&peer) else {
                bail!("route `{}` targets unknown peer {}", route_name, peer);
            };
            if !peer_config.trust.allows_forward() {
                bail!(
                    "route `{}` targets peer {} but peer trust does not allow forwarding",
                    route_name,
                    peer
                );
            }
        }
        if route.target.broadcast {
            for peer_config in peer_ids.values() {
                if !peer_config.trust.allows_forward() {
                    bail!(
                        "route `{}` broadcasts but peer {} trust does not allow forwarding",
                        route_name,
                        peer_config.node_id
                    );
                }
            }
        }
        if let Some(action) = route.target.local_driver.as_deref()
            && !driver_actions.contains(action)
        {
            bail!(
                "route `{}` targets unknown local driver `{}`",
                route_name,
                action
            );
        }
        if let Some(capability) = &route.target.capability {
            if let Some(action) = capability.driver_action() {
                if !driver_actions.contains(action) {
                    bail!(
                        "route `{}` targets unknown driver capability `{}`",
                        route_name,
                        capability
                    );
                }
            } else {
                warnings.push(format!(
                    "route `{}` targets capability `{}` but v1 can only execute driver.execute:* capabilities locally; message will be acknowledged without execution",
                    route_name, capability
                ));
            }
        }
        if let Some(required_capability) = &route.requires_peer_grant {
            let peer = route.target.peer.ok_or_else(|| {
                anyhow!("route `{route_name}` requires a peer grant but does not target a peer")
            })?;
            validate_route_peer_grant(config, &route_name, peer, required_capability)?;
            peer_grant_route_count += 1;
        }
    }
    Ok(peer_grant_route_count)
}

fn validate_route_peer_grant(
    config: &RivunNodeConfig,
    route_name: &str,
    peer: Uuid,
    required_capability: &CapabilityId,
) -> Result<()> {
    let cache_path = config.capability_cache.path.as_ref().ok_or_else(|| {
        anyhow!(
            "route `{}` requires peer grant `{}` but capability_cache.path is not configured",
            route_name,
            required_capability
        )
    })?;
    let cache = JsonlCapabilityCache::open(cache_path);
    let entry = cache.latest_for_peer(peer)?.ok_or_else(|| {
        anyhow!(
            "route `{}` requires peer grant `{}` but cache {} has no advertisement for peer {}",
            route_name,
            required_capability,
            cache_path.display(),
            peer
        )
    })?;
    if let Some(max_age_micros) = config.capability_cache.max_age_micros {
        let now = now_micros()?;
        let age = now.saturating_sub(entry.observed_at_micros);
        if age > max_age_micros {
            bail!(
                "route `{}` requires peer grant `{}` but cached advertisement for peer {} is too old: age_micros={} max_age_micros={}",
                route_name,
                required_capability,
                peer,
                age,
                max_age_micros
            );
        }
    }
    if !entry.advertisement.grants_capability(required_capability) {
        bail!(
            "route `{}` requires peer {} to grant `{}`, but latest cache entry {} does not prove that grant",
            route_name,
            peer,
            required_capability,
            entry.id
        );
    }
    Ok(())
}

fn validate_capability_policy(
    config: &RivunNodeConfig,
    advertisement: &CapabilityAdvertisement,
    warnings: &mut Vec<String>,
) -> Result<usize> {
    let mut granted = HashSet::new();
    for grant in &config.capability_policy.grants {
        if !granted.insert(grant.capability.clone()) {
            bail!("duplicate capability grant `{}`", grant.capability);
        }
        validate_policy_reason(
            "capability grant",
            &grant.capability,
            grant.reason.as_deref(),
        )?;
        if !advertisement.capabilities.contains(&grant.capability) {
            bail!(
                "capability grant `{}` is not advertised by this node",
                grant.capability
            );
        }
    }

    let mut required = HashSet::new();
    for requirement in &config.capability_policy.requirements {
        if !required.insert(requirement.capability.clone()) {
            bail!(
                "duplicate capability requirement `{}`",
                requirement.capability
            );
        }
        validate_policy_reason(
            "capability requirement",
            &requirement.capability,
            requirement.reason.as_deref(),
        )?;
    }

    let ungranted = advertisement
        .capabilities
        .iter()
        .filter(|capability| !granted.contains(*capability))
        .cloned()
        .collect::<Vec<_>>();
    if !ungranted.is_empty() && config.capability_policy.require_grants_for_advertised {
        bail!(
            "capability_policy.require_grants_for_advertised=true but missing grants for {}",
            ungranted
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(", ")
        );
    }
    if !ungranted.is_empty() && !config.capability_policy.grants.is_empty() {
        warnings.push(format!(
            "capability_policy grants only {} of {} advertised capabilities",
            config.capability_policy.grants.len(),
            advertisement.capabilities.capabilities.len()
        ));
    }

    Ok(ungranted.len())
}

fn validate_policy_reason(
    label: &str,
    capability: &CapabilityId,
    reason: Option<&str>,
) -> Result<()> {
    if matches!(reason, Some(reason) if reason.trim().is_empty()) {
        bail!("{label} `{capability}` reason must not be empty when provided");
    }
    Ok(())
}

#[cfg(unix)]
fn warn_key_file_permissions(path: &Path, warnings: &mut Vec<String>) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;

    let mode = fs::metadata(path)
        .with_context(|| format!("failed to inspect key file permissions {}", path.display()))?
        .permissions()
        .mode()
        & 0o777;
    if mode & 0o077 != 0 {
        warnings.push(format!(
            "key_file {} permissions {mode:03o} allow group/other access; expected 600",
            path.display()
        ));
    }
    Ok(())
}

#[cfg(not(unix))]
fn warn_key_file_permissions(_path: &Path, _warnings: &mut Vec<String>) -> Result<()> {
    Ok(())
}

#[derive(Debug, Clone, Copy, Default)]
struct PeerTrustSummary {
    trusted_peer_count: usize,
    restricted_peer_count: usize,
    peer_send_enabled_count: usize,
    peer_receive_enabled_count: usize,
    peer_forward_enabled_count: usize,
}

fn validate_peers(
    config: &RivunNodeConfig,
    bind: SocketAddr,
    local_node_id: Uuid,
    warnings: &mut Vec<String>,
) -> Result<PeerTrustSummary> {
    if matches!(config.trust.max_transport_key_age_micros, Some(0)) {
        bail!("trust.max_transport_key_age_micros must be greater than zero");
    }

    let mut peer_ids = HashSet::new();
    let mut peer_addrs = HashSet::new();
    let mut transport_keys: HashMap<[u8; 32], Uuid> = HashMap::new();
    let mut summary = PeerTrustSummary::default();
    let now = now_micros()?;
    for peer in &config.peers {
        if peer.node_id == local_node_id {
            bail!("peer list contains local node_id {}", local_node_id);
        }
        if !peer_ids.insert(peer.node_id) {
            bail!("duplicate peer node_id {}", peer.node_id);
        }
        let peer_addr = peer
            .addr
            .parse::<SocketAddr>()
            .with_context(|| format!("invalid peer address {}", peer.addr))?;
        if !peer_addrs.insert(peer_addr) {
            bail!("duplicate peer address {}", peer_addr);
        }
        if peer_addr == bind {
            bail!("peer {} uses the local bind address {}", peer.node_id, bind);
        }

        let public_key = decode_public_key(&peer.public_key)
            .with_context(|| format!("invalid public key for peer {}", peer.node_id))?;
        if public_key.node_id() != peer.node_id {
            bail!(
                "peer public_key derives node_id {}, but config declares {}",
                public_key.node_id(),
                peer.node_id
            );
        }

        let transport_key = TransportKey::from_hex(&peer.transport_key)
            .with_context(|| format!("invalid transport key for peer {}", peer.node_id))?;
        if transport_key.0 == [0_u8; 32] {
            bail!("peer {} transport_key must not be all zeros", peer.node_id);
        }
        if let Some(existing_peer) = transport_keys.insert(transport_key.0, peer.node_id) {
            warnings.push(format!(
                "transport_key is reused by peers {} and {}; use unique per-peer transport keys in production",
                existing_peer, peer.node_id
            ));
        }
        validate_peer_trust(config, peer, now, warnings, &mut summary)?;
    }
    Ok(summary)
}

fn validate_peer_trust(
    config: &RivunNodeConfig,
    peer: &PeerConfig,
    now_micros: u64,
    warnings: &mut Vec<String>,
    summary: &mut PeerTrustSummary,
) -> Result<()> {
    if matches!(peer.transport_key_epoch, Some(0)) {
        bail!(
            "peer {} transport_key_epoch must be greater than zero",
            peer.node_id
        );
    }
    if peer.trust.status == PeerTrustStatus::Revoked {
        bail!(
            "peer {} trust.status=revoked; remove it from active peers or re-enroll it with rotated keys",
            peer.node_id
        );
    }
    if config.trust.require_peer_expiry && peer.trust.expires_at_micros.is_none() {
        bail!(
            "trust.require_peer_expiry=true but peer {} has no trust.expires_at_micros",
            peer.node_id
        );
    }
    if let Some(expires_at_micros) = peer.trust.expires_at_micros
        && expires_at_micros <= now_micros
    {
        bail!(
            "peer {} trust expired at {}",
            peer.node_id,
            expires_at_micros
        );
    }
    if let Some(max_age_micros) = config.trust.max_transport_key_age_micros {
        let Some(rotated_at_micros) = peer.transport_key_rotated_at_micros else {
            bail!(
                "trust.max_transport_key_age_micros requires peer {} transport_key_rotated_at_micros",
                peer.node_id
            );
        };
        if rotated_at_micros > now_micros {
            bail!(
                "peer {} transport_key_rotated_at_micros {} is in the future",
                peer.node_id,
                rotated_at_micros
            );
        }
        let age = now_micros.saturating_sub(rotated_at_micros);
        if age > max_age_micros {
            bail!(
                "peer {} transport key age_micros={} exceeds trust.max_transport_key_age_micros={}",
                peer.node_id,
                age,
                max_age_micros
            );
        }
    }
    let mut labels = HashSet::new();
    for label in &peer.trust.labels {
        validate_message_text("peer trust label", label, 64, false)?;
        if !labels.insert(label) {
            bail!(
                "peer {} has duplicate trust label `{}`",
                peer.node_id,
                label
            );
        }
    }

    if peer.trust.is_trusted() {
        summary.trusted_peer_count += 1;
    }
    if peer.trust.allows_send() {
        summary.peer_send_enabled_count += 1;
    }
    if peer.trust.allows_receive() {
        summary.peer_receive_enabled_count += 1;
    }
    if peer.trust.allows_forward() {
        summary.peer_forward_enabled_count += 1;
    }
    if !peer.trust.is_trusted()
        || !peer.trust.allow_send
        || !peer.trust.allow_receive
        || !peer.trust.allow_forward
        || !peer.trust.allow_poa_attestation
    {
        summary.restricted_peer_count += 1;
    }

    if peer.trust.status == PeerTrustStatus::Quarantined {
        warnings.push(format!(
            "peer {} is quarantined; transport, receive, send, and route forwarding are disabled",
            peer.node_id
        ));
    } else {
        if !peer.trust.allow_receive {
            warnings.push(format!(
                "peer {} trust.allow_receive=false; inbound frames will be rejected after transport authentication",
                peer.node_id
            ));
        }
        if !peer.trust.allow_send {
            warnings.push(format!(
                "peer {} trust.allow_send=false; outbound CLI sends and node responses are disabled",
                peer.node_id
            ));
        }
        if !peer.trust.allow_forward {
            warnings.push(format!(
                "peer {} trust.allow_forward=false; route forwarding to this peer is disabled",
                peer.node_id
            ));
        }
        if !peer.trust.allow_poa_attestation {
            warnings.push(format!(
                "peer {} trust.allow_poa_attestation=false; this node will not issue PoA attestations to it",
                peer.node_id
            ));
        }
    }
    Ok(())
}

struct PoaVerifierConfig {
    validators: Vec<(Uuid, PublicKey)>,
    required_threshold: u16,
    validator_set_epoch: Option<u64>,
}

struct PoaValidationSummary {
    validator_count: usize,
    required_threshold: u16,
    validator_set_enabled: bool,
    validator_set_epoch: Option<u64>,
}

fn validate_poa(
    config: &RivunNodeConfig,
    warnings: &mut Vec<String>,
) -> Result<PoaValidationSummary> {
    if config.poa.validator_set.is_some() && !config.poa.validators.is_empty() {
        warnings.push(
            "poa.validator_set is configured; inline poa.validators are ignored for verification"
                .to_string(),
        );
    }
    if config.poa.validator_set.is_none() && config.poa.validator_set_authority.is_some() {
        warnings.push(
            "poa.validator_set_authority is configured without poa.validator_set".to_string(),
        );
    }
    let verifier = load_poa_verifier(&config.poa)?;
    if verifier.validators.is_empty() {
        warnings.push(
            "no PoA validators configured; frames marked REQUIRES_CONSENSUS will be rejected"
                .to_string(),
        );
    }
    Ok(PoaValidationSummary {
        validator_count: verifier.validators.len(),
        required_threshold: verifier.required_threshold,
        validator_set_enabled: config.poa.validator_set.is_some(),
        validator_set_epoch: verifier.validator_set_epoch,
    })
}

fn load_poa_verifier(config: &PoaConfig) -> Result<PoaVerifierConfig> {
    if config.required_threshold == 0 {
        bail!("poa.required_threshold must be greater than zero");
    }
    if let Some(path) = &config.validator_set {
        let signed = load_signed_poa_validator_set(path)?;
        let authority = config
            .validator_set_authority
            .as_deref()
            .map(decode_public_key)
            .transpose()
            .context("invalid poa.validator_set_authority")?;
        signed.verify(authority.as_ref())?;
        validate_poa_validator_set_time(&signed)?;
        let validators = signed
            .set
            .validators
            .iter()
            .map(|validator| {
                Ok((
                    validator.node_id,
                    decode_public_key(&validator.public_key).with_context(|| {
                        format!("invalid PoA validator public key {}", validator.node_id)
                    })?,
                ))
            })
            .collect::<Result<Vec<_>>>()?;
        let required_threshold = config.required_threshold.max(signed.set.required_threshold);
        if required_threshold as usize > validators.len() {
            bail!(
                "effective PoA required_threshold {} exceeds validator-set count {}",
                required_threshold,
                validators.len()
            );
        }
        return Ok(PoaVerifierConfig {
            validators,
            required_threshold,
            validator_set_epoch: Some(signed.set.epoch),
        });
    }
    let validators = load_static_poa_validators(config)?;
    if validators.is_empty() {
        return Ok(PoaVerifierConfig {
            validators,
            required_threshold: config.required_threshold,
            validator_set_epoch: None,
        });
    }
    if config.required_threshold as usize > validators.len() {
        bail!(
            "poa.required_threshold {} exceeds configured validator count {}",
            config.required_threshold,
            validators.len()
        );
    }
    Ok(PoaVerifierConfig {
        validators,
        required_threshold: config.required_threshold,
        validator_set_epoch: None,
    })
}

fn load_static_poa_validators(config: &PoaConfig) -> Result<Vec<(Uuid, PublicKey)>> {
    let mut validator_ids = HashSet::new();
    let mut validators = Vec::with_capacity(config.validators.len());
    for validator in &config.validators {
        if !validator_ids.insert(validator.node_id) {
            bail!("duplicate PoA validator node_id {}", validator.node_id);
        }
        let public_key = decode_public_key(&validator.public_key)
            .with_context(|| format!("invalid PoA validator public key {}", validator.node_id))?;
        if public_key.node_id() != validator.node_id {
            bail!(
                "PoA validator public_key derives node_id {}, but config declares {}",
                public_key.node_id(),
                validator.node_id
            );
        }
        validators.push((validator.node_id, public_key));
    }
    Ok(validators)
}

fn load_signed_poa_validator_set(path: &Path) -> Result<SignedPoaValidatorSet> {
    let input = fs::read_to_string(path)
        .with_context(|| format!("failed to read PoA validator set {}", path.display()))?;
    serde_json::from_str(&input)
        .with_context(|| format!("failed to parse PoA validator set {}", path.display()))
}

fn validate_poa_validator_set_time(signed: &SignedPoaValidatorSet) -> Result<()> {
    let now = now_micros()?;
    if let Some(valid_from) = signed.set.valid_from_micros
        && valid_from > now
    {
        bail!(
            "PoA validator set {} epoch {} is not valid until {}",
            signed.set.set_id,
            signed.set.epoch,
            valid_from
        );
    }
    if let Some(expires_at) = signed.set.expires_at_micros
        && expires_at <= now
    {
        bail!(
            "PoA validator set {} epoch {} expired at {}",
            signed.set.set_id,
            signed.set.epoch,
            expires_at
        );
    }
    Ok(())
}

pub fn describe_capabilities(config: &RivunNodeConfig) -> Result<CapabilityAdvertisement> {
    let keypair = load_keypair(&config.key_file)?;
    let mut advertisement = CapabilityAdvertisement::new(keypair.node_id());
    let mut capabilities = CapabilitySet::new();

    for driver in &config.drivers {
        let manifest_permissions = match &driver.manifest {
            Some(manifest_path) => load_driver_manifest(manifest_path)?.permissions,
            None => DriverPermissions::none(),
        };
        let permissions = merge_permissions(config.runtime.permissions, manifest_permissions);
        for capability in capabilities_for_driver(&driver.action, permissions)?.capabilities {
            capabilities.insert(capability);
        }
    }

    if config.memory.dir.is_some() {
        capabilities.insert(CapabilityId::new("memory.local")?);
        if config.memory.allow_driver_read {
            capabilities.insert(CapabilityId::new("memory.read")?);
        }
        if config.memory.allow_driver_write {
            capabilities.insert(CapabilityId::new("memory.write")?);
        }
    }

    advertisement.capabilities = capabilities;
    advertisement.grants = config.capability_policy.grants.clone();
    advertisement.requirements = config.capability_policy.requirements.clone();
    Ok(advertisement)
}

fn validate_drivers(
    executor: &WasmExecutor,
    drivers: &[DriverConfig],
    runtime: RuntimeConfig,
    memory: &MemoryConfig,
    registry: Option<&DriverRegistry>,
    warnings: &mut Vec<String>,
) -> Result<usize> {
    let mut actions = HashSet::new();
    let mut signed_driver_count = 0;
    for driver in drivers {
        if driver.action.trim().is_empty() {
            bail!("driver action must not be empty");
        }
        if !actions.insert(driver.action.clone()) {
            bail!("duplicate driver action {}", driver.action);
        }
        let wasm = fs::read(&driver.path)
            .with_context(|| format!("failed to read driver {}", driver.path.display()))?;
        executor
            .compile_and_validate_cached(&wasm)
            .with_context(|| format!("invalid driver ABI {}", driver.path.display()))?;

        let manifest_permissions = if let Some(manifest_path) = &driver.manifest {
            signed_driver_count += 1;
            let manifest = load_driver_manifest(manifest_path)?;
            manifest
                .verify_for_driver(&driver.action, &wasm)
                .with_context(|| {
                    format!("invalid signed driver manifest {}", manifest_path.display())
                })?;
            if let Some(registry) = registry {
                registry.verify_manifest(&manifest).with_context(|| {
                    format!(
                        "driver `{}` is not active in configured registry",
                        driver.action
                    )
                })?;
            }
            manifest.permissions
        } else {
            warnings.push(format!(
                "driver `{}` has no signed RivunStore manifest; provenance validation disabled",
                driver.action
            ));
            if registry.is_some() {
                warnings.push(format!(
                    "driver `{}` cannot be checked against registry without a signed manifest",
                    driver.action
                ));
            }
            DriverPermissions::none()
        };
        validate_effective_driver_permissions(driver, runtime, memory, manifest_permissions)?;
    }
    Ok(signed_driver_count)
}

fn load_drivers(
    executor: &WasmExecutor,
    async_executor: Option<&AsyncWasmExecutor>,
    drivers: &[DriverConfig],
    registry: Option<&DriverRegistry>,
) -> Result<HashMap<String, DriverRegistration>> {
    let mut compiled = HashMap::with_capacity(drivers.len());
    for driver in drivers {
        let wasm = fs::read(&driver.path)
            .with_context(|| format!("failed to read driver {}", driver.path.display()))?;
        let permissions = match &driver.manifest {
            Some(manifest_path) => {
                let manifest = load_driver_manifest(manifest_path)?;
                manifest
                    .verify_for_driver(&driver.action, &wasm)
                    .with_context(|| {
                        format!("invalid signed driver manifest {}", manifest_path.display())
                    })?;
                if let Some(registry) = registry {
                    registry.verify_manifest(&manifest).with_context(|| {
                        format!(
                            "driver `{}` is not active in configured registry",
                            driver.action
                        )
                    })?;
                }
                manifest.permissions
            }
            None => DriverPermissions::none(),
        };
        let wasm_driver = executor
            .compile_and_validate_cached(&wasm)
            .with_context(|| format!("invalid driver ABI {}", driver.path.display()))?;
        let async_driver = async_executor
            .map(|executor| executor.compile_and_validate(&wasm))
            .transpose()
            .with_context(|| format!("invalid async driver ABI {}", driver.path.display()))?;
        compiled.insert(
            driver.action.clone(),
            DriverRegistration {
                driver: wasm_driver,
                async_driver,
                permissions,
            },
        );
    }
    Ok(compiled)
}

fn load_driver_manifest(path: &Path) -> Result<DriverManifest> {
    let input = fs::read_to_string(path)
        .with_context(|| format!("failed to read driver manifest {}", path.display()))?;
    DriverManifest::from_toml_str(&input)
        .with_context(|| format!("failed to parse driver manifest {}", path.display()))
}

fn load_driver_registry_optional(config: &RegistryConfig) -> Result<Option<DriverRegistry>> {
    let Some(path) = config.path.as_deref() else {
        if config.require_signature {
            bail!("registry.require_signature=true requires registry.path");
        }
        return Ok(None);
    };
    let input = fs::read_to_string(path)
        .with_context(|| format!("failed to read driver registry {}", path.display()))?;
    let registry = DriverRegistry::from_toml_str(&input)
        .with_context(|| format!("failed to parse driver registry {}", path.display()))?;
    registry
        .validate()
        .with_context(|| format!("invalid driver registry {}", path.display()))?;
    if config.require_signature {
        registry
            .verify_signature()
            .with_context(|| format!("invalid driver registry signature {}", path.display()))?;
    }
    Ok(Some(registry))
}

fn load_registry_bundle_manifest_from_config(
    config: &RegistryConfig,
) -> Result<Option<RegistryBundleManifest>> {
    let Some(path) = config.bundle_path.as_deref() else {
        return Ok(None);
    };
    load_registry_bundle_manifest_optional(path, &RegistryBundleManifestRequest::default())
}

fn load_registry_bundle_manifest_optional(
    bundle_path: &Path,
    request: &RegistryBundleManifestRequest,
) -> Result<Option<RegistryBundleManifest>> {
    let path = bundle_path.join("rivunstore.bundle.json");
    let input = fs::read_to_string(&path)
        .with_context(|| format!("failed to read registry bundle manifest {}", path.display()))?;
    let manifest = RegistryBundleManifest::from_json_str(&input).with_context(|| {
        format!(
            "failed to parse registry bundle manifest {}",
            path.display()
        )
    })?;
    let response = RegistryBundleManifestResponse::new(Uuid::nil(), Some(manifest.clone()), None);
    response
        .verify(request)
        .with_context(|| format!("invalid registry bundle manifest {}", path.display()))?;
    Ok(Some(manifest))
}

fn hash_frame(frame: &RivunFrame) -> String {
    format!("blake3:{}", blake3::hash(&frame.encode()).to_hex())
}

fn validate_effective_driver_permissions(
    driver: &DriverConfig,
    runtime: RuntimeConfig,
    memory: &MemoryConfig,
    manifest_permissions: DriverPermissions,
) -> Result<()> {
    let permissions = merge_permissions(runtime.permissions, manifest_permissions);
    if permissions.max_host_call_bytes == 0 {
        bail!(
            "driver `{}` sets max_host_call_bytes=0; host call byte limit must be greater than zero",
            driver.action
        );
    }
    if permissions.network {
        bail!(
            "driver `{}` requests network permission, but host network capabilities are not implemented in ABI v1",
            driver.action
        );
    }
    if permissions.filesystem {
        bail!(
            "driver `{}` requests filesystem permission, but host filesystem capabilities are not implemented in ABI v1",
            driver.action
        );
    }
    if permissions.clock {
        bail!(
            "driver `{}` requests clock permission, but host clock capabilities are not implemented in ABI v1",
            driver.action
        );
    }
    if permissions.environment {
        bail!(
            "driver `{}` requests environment permission, but host environment capabilities are not implemented in ABI v1",
            driver.action
        );
    }
    if permissions.memory_read && !(memory.dir.is_some() && memory.allow_driver_read) {
        bail!(
            "driver `{}` requests memory_read permission, but memory.dir and memory.allow_driver_read=true are required",
            driver.action
        );
    }
    if permissions.memory_write && !(memory.dir.is_some() && memory.allow_driver_write) {
        bail!(
            "driver `{}` requests memory_write permission, but memory.dir and memory.allow_driver_write=true are required",
            driver.action
        );
    }
    Ok(())
}

fn merge_permissions(a: DriverPermissions, b: DriverPermissions) -> DriverPermissions {
    a.merge(b)
}

#[derive(Debug)]
struct ReplayGuard {
    capacity: usize,
    durable_store: Option<durable_replay::DurableReplayStore>,
    seen: HashSet<[u8; 16]>,
    order: VecDeque<[u8; 16]>,
}

impl ReplayGuard {
    fn new(capacity: usize) -> Self {
        Self {
            capacity,
            durable_store: None,
            seen: HashSet::with_capacity(capacity.min(4096)),
            order: VecDeque::with_capacity(capacity.min(4096)),
        }
    }

    fn with_durable_store(
        capacity: usize,
        durable_store: durable_replay::DurableReplayStore,
    ) -> Self {
        Self {
            capacity,
            durable_store: Some(durable_store),
            seen: HashSet::with_capacity(capacity.min(4096)),
            order: VecDeque::with_capacity(capacity.min(4096)),
        }
    }

    fn remember(&mut self, frame: &rivun_core::RivunFrame) -> Result<()> {
        if self.capacity == 0 {
            return Ok(());
        }

        if let Some(durable) = &mut self.durable_store {
            durable.check_and_insert(frame, now_micros()?)?;
        } else {
            let fingerprint = frame_fingerprint(frame);
            if self.seen.contains(&fingerprint) {
                bail!(
                    "replayed frame rejected: source_node={}, timestamp_micros={}, signature_hint={}",
                    frame.header.source_node,
                    frame.header.timestamp_micros,
                    hex_hint(frame.header.rivun_sign)
                );
            }

            self.seen.insert(fingerprint);
            self.order.push_back(fingerprint);
            while self.order.len() > self.capacity {
                if let Some(expired) = self.order.pop_front() {
                    self.seen.remove(&expired);
                }
            }
        }
        Ok(())
    }
}

fn frame_fingerprint(frame: &rivun_core::RivunFrame) -> [u8; 16] {
    let hash = blake3::hash(&frame.encode());
    hash.as_bytes()[..16].try_into().unwrap()
}

fn hex_hint(hint: [u8; 8]) -> String {
    hint.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn load_keypair(path: &Path) -> Result<Keypair> {
    let input = fs::read_to_string(path)
        .with_context(|| format!("failed to read key file {}", path.display()))?;
    Ok(Keypair::from_key_file_toml(&input)?)
}

fn decode_public_key(encoded: &str) -> Result<PublicKey> {
    let bytes = STANDARD_NO_PAD.decode(encoded)?;
    if bytes.len() != 32 {
        bail!(
            "invalid public key length: expected 32 bytes, got {}",
            bytes.len()
        );
    }
    Ok(PublicKey::from_bytes(bytes.try_into().unwrap())?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::engine::general_purpose::STANDARD_NO_PAD;
    use bytes::Bytes;
    use tokio::time::{Duration, timeout};
    use rivun_core::{RivunFlags, RivunFrame, now_micros};
    use rivun_crypto::{
        POA_VALIDATOR_SET_SCHEMA_VERSION, PoaValidatorDescriptor, PoaValidatorSet, certify_frame,
        sign_frame, sign_poa_validator_set, verify_frame,
    };
    use rivun_envelope::{RivunEnvelope, RivunMessageKind};
    use rivun_memory::MemoryQuery;
    use rivun_net::{Peer, RivunEndpoint, RivunEndpointConfig};
    use rivun_router::{RouteMatch, RouteTarget};
    use rivun_store::{DriverManifest, DriverRegistry, DriverRegistryStatus};

    fn public_key_string(keypair: &Keypair) -> String {
        base64::Engine::encode(&STANDARD_NO_PAD, keypair.verifying_key().to_bytes())
    }

    fn echo_driver_wat() -> &'static str {
        r#"
        (module
          (memory (export "memory") 1)
          (global $heap (mut i32) (i32.const 1024))
          (func (export "rivun_alloc") (param $len i32) (result i32)
            global.get $heap
            global.get $heap
            local.get $len
            i32.add
            global.set $heap)
          (func (export "rivun_dealloc") (param i32 i32))
          (func (export "rivun_execute")
            (param $action_ptr i32) (param $action_len i32)
            (param $payload_ptr i32) (param $payload_len i32)
            (result i64)
            local.get $payload_ptr
            i64.extend_i32_u
            i64.const 32
            i64.shl
            local.get $payload_len
            i64.extend_i32_u
            i64.or))
        "#
    }

    fn missing_execute_driver_wat() -> &'static str {
        r#"
        (module
          (memory (export "memory") 1)
          (func (export "rivun_alloc") (param i32) (result i32) i32.const 0)
          (func (export "rivun_dealloc") (param i32 i32)))
        "#
    }

    fn memory_write_driver_wat() -> &'static str {
        r#"
        (module
          (import "rivun" "memory_write" (func $memory_write (param i32 i32) (result i32)))
          (memory (export "memory") 1)
          (global $heap (mut i32) (i32.const 4096))
          (data (i32.const 1024) "machine-note")
          (func (export "rivun_alloc") (param $len i32) (result i32)
            global.get $heap
            global.get $heap
            local.get $len
            i32.add
            global.set $heap)
          (func (export "rivun_dealloc") (param i32 i32))
          (func (export "rivun_execute")
            (param $action_ptr i32) (param $action_len i32)
            (param $payload_ptr i32) (param $payload_len i32)
            (result i64)
            i32.const 1024
            i32.const 12
            call $memory_write
            drop
            local.get $payload_ptr
            i64.extend_i32_u
            i64.const 32
            i64.shl
            local.get $payload_len
            i64.extend_i32_u
            i64.or))
        "#
    }

    fn signed_driver_manifest_toml(
        action: &str,
        wasm: &[u8],
        author: &Keypair,
        permissions: DriverPermissions,
    ) -> String {
        DriverManifest::new(
            format!("{action}-driver"),
            "0.1.0",
            action,
            wasm,
            permissions,
            Some("test driver manifest".to_string()),
            author,
        )
        .unwrap()
        .to_toml_string()
        .unwrap()
    }

    fn zenv_payload(kind: RivunMessageKind, subject: &str, body: &[u8]) -> Vec<u8> {
        RivunEnvelope::new(
            kind,
            subject,
            "application/octet-stream",
            Bytes::copy_from_slice(body),
        )
        .unwrap()
        .with_metadata(Bytes::from_static(b"fixture=rivun-node"))
        .unwrap()
        .encode()
        .to_vec()
    }

    #[test]
    fn action_envelope_round_trips() {
        let envelope = ActionEnvelope::new("echo", "hello");
        let bytes = envelope.to_payload_bytes().unwrap();
        let decoded: ActionEnvelope = serde_json::from_slice(&bytes).unwrap();

        assert_eq!(decoded.action, "echo");
        assert_eq!(decoded.payload_bytes().unwrap(), b"hello");
    }

    #[test]
    fn raw_unknown_payload_becomes_data_message() {
        let message = parse_inbound_message(b"\x00raw payload").unwrap();

        validate_inbound_message(&message).unwrap();
        assert_eq!(message.kind, RivunMessageKind::Data);
        assert_eq!(message.subject, "");
        assert_eq!(message.body, b"\x00raw payload");
    }

    #[test]
    fn parses_config() {
        let toml = r#"
            bind = "127.0.0.1:7000"
            key_file = ".rivun/node.key"
            require_signed = true

            [[peers]]
            node_id = "01010101-0101-0101-0101-010101010101"
            addr = "127.0.0.1:7001"
            public_key = "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"
            transport_key = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"

            [[drivers]]
            action = "echo"
            path = "examples/wasm-drivers/echo/echo.wat"

            [runtime]
            fuel = 100000
            timeout_ms = 250
            async_execution = true

            [action_runtime_limits.echo]
            fuel = 50000
            timeout_ms = 100
        "#;

        let config = RivunNodeConfig::from_toml_str(toml).unwrap();
        assert_eq!(config.bind, "127.0.0.1:7000");
        assert_eq!(config.peers.len(), 1);
        assert_eq!(config.drivers[0].action, "echo");
        assert_eq!(config.runtime.fuel, Some(100_000));
        assert!(config.runtime.async_execution);
        assert_eq!(
            config.action_runtime_limits["echo"].fuel,
            Some(50_000)
        );
    }

    #[test]
    fn action_runtime_limits_only_reduce_global_budgets() {
        let global = ExecutionLimits::default();
        let constrained = ActionRuntimeLimits {
            fuel: Some(50_000),
            timeout_ms: Some(10),
            ..ActionRuntimeLimits::default()
        }
        .apply(global);

        assert_eq!(constrained.fuel, 50_000);
        assert_eq!(constrained.timeout_ms, 10);
        assert_eq!(constrained.max_memory_bytes, global.max_memory_bytes);
    }

    struct NodeHarness {
        _temp: tempfile::TempDir,
        driver_path: PathBuf,
        node: RivunNode,
        sender_endpoint: RivunEndpoint,
        receiver_key: Keypair,
        sender_key: Keypair,
    }

    async fn node_harness(security: SecurityConfig) -> NodeHarness {
        node_harness_with_poa(security, PoaConfig::default()).await
    }

    async fn node_harness_with_poa(security: SecurityConfig, poa: PoaConfig) -> NodeHarness {
        node_harness_with_poa_and_message_policy(security, poa, MessagePolicyConfig::default())
            .await
    }

    async fn node_harness_with_poa_and_message_policy(
        security: SecurityConfig,
        poa: PoaConfig,
        message_policy: MessagePolicyConfig,
    ) -> NodeHarness {
        node_harness_with_poa_policy_and_schema(
            security,
            poa,
            message_policy,
            MessageSchemaConfig::default(),
        )
        .await
    }

    async fn node_harness_with_poa_policy_and_schema(
        security: SecurityConfig,
        poa: PoaConfig,
        message_policy: MessagePolicyConfig,
        message_schema: MessageSchemaConfig,
    ) -> NodeHarness {
        node_harness_with_poa_policy_schema_and_runtime(
            security,
            poa,
            message_policy,
            message_schema,
            RuntimeConfig::default(),
        )
        .await
    }

    async fn node_harness_with_poa_policy_schema_and_runtime(
        security: SecurityConfig,
        poa: PoaConfig,
        message_policy: MessagePolicyConfig,
        message_schema: MessageSchemaConfig,
        runtime: RuntimeConfig,
    ) -> NodeHarness {
        let temp = tempfile::tempdir().unwrap();
        let receiver_key = Keypair::generate();
        let sender_key = Keypair::generate();
        let receiver_key_path = temp.path().join("receiver.key");
        let driver_path = temp.path().join("echo.wat");
        std::fs::write(&receiver_key_path, receiver_key.to_key_file_toml().unwrap()).unwrap();
        std::fs::write(&driver_path, echo_driver_wat()).unwrap();

        let sender_endpoint = RivunEndpoint::bind(RivunEndpointConfig::new(
            "127.0.0.1:0".parse().unwrap(),
            sender_key.node_id(),
        ))
        .await
        .unwrap();
        let transport_key = [0x55_u8; 32];
        let config = RivunNodeConfig {
            bind: "127.0.0.1:0".to_string(),
            key_file: receiver_key_path,
            require_signed: true,
            max_datagram_size: None,
            peers: vec![PeerConfig {
                node_id: sender_key.node_id(),
                addr: sender_endpoint.local_addr().unwrap().to_string(),
                public_key: public_key_string(&sender_key),
                transport_key: transport_key
                    .iter()
                    .map(|byte| format!("{byte:02x}"))
                    .collect(),
                transport_key_epoch: None,
                transport_key_rotated_at_micros: None,
                trust: PeerTrustConfig::default(),
            }],
            drivers: vec![DriverConfig {
                action: "echo".to_string(),
                path: driver_path.clone(),
                manifest: None,
            }],
            runtime,
            action_runtime_limits: BTreeMap::new(),
            security,
            trust: TrustConfig::default(),
            poa,
            receipts: ReceiptsConfig::default(),
            registry: RegistryConfig::default(),
            memory: MemoryConfig::default(),
            capability_policy: CapabilityPolicyConfig::default(),
            capability_cache: CapabilityCacheConfig::default(),
            discovery: DiscoveryConfig::default(),
            observability: ObservabilityConfig::default(),
            message_policy,
            message_schema,
            routes: Vec::new(),
            swarm: SwarmConfig::default(),
            gossip: GossipConfig::default(),
            mesh: MeshConfig::default(),
        };
        let node = RivunNode::from_config(config).await.unwrap();
        sender_endpoint
            .add_peer(Peer::new(
                receiver_key.node_id(),
                node.local_addr().unwrap(),
                transport_key,
            ))
            .await;

        NodeHarness {
            _temp: temp,
            driver_path,
            node,
            sender_endpoint,
            receiver_key,
            sender_key,
        }
    }

    fn validation_config(
        temp: &tempfile::TempDir,
        local_key: &Keypair,
        peer_key: &Keypair,
        peer_public_key: String,
        transport_key: String,
    ) -> RivunNodeConfig {
        let key_path = temp.path().join("node.key");
        let driver_path = temp.path().join("echo.wat");
        std::fs::write(&key_path, local_key.to_key_file_toml().unwrap()).unwrap();
        std::fs::write(&driver_path, echo_driver_wat()).unwrap();
        RivunNodeConfig {
            bind: "127.0.0.1:0".to_string(),
            key_file: key_path,
            require_signed: true,
            max_datagram_size: None,
            peers: vec![PeerConfig {
                node_id: peer_key.node_id(),
                addr: "127.0.0.1:9".to_string(),
                public_key: peer_public_key,
                transport_key,
                transport_key_epoch: None,
                transport_key_rotated_at_micros: None,
                trust: PeerTrustConfig::default(),
            }],
            drivers: vec![DriverConfig {
                action: "echo".to_string(),
                path: driver_path,
                manifest: None,
            }],
            runtime: RuntimeConfig::default(),
            action_runtime_limits: BTreeMap::new(),
            security: SecurityConfig::default(),
            trust: TrustConfig::default(),
            poa: PoaConfig::default(),
            receipts: ReceiptsConfig::default(),
            registry: RegistryConfig::default(),
            memory: MemoryConfig::default(),
            capability_policy: CapabilityPolicyConfig::default(),
            capability_cache: CapabilityCacheConfig::default(),
            discovery: DiscoveryConfig::default(),
            observability: ObservabilityConfig::default(),
            message_policy: MessagePolicyConfig::default(),
            message_schema: MessageSchemaConfig::default(),
            routes: Vec::new(),
            swarm: SwarmConfig::default(),
            gossip: GossipConfig::default(),
            mesh: MeshConfig::default(),
        }
    }

    fn signed_echo_frame(harness: &NodeHarness, timestamp_micros: u64) -> RivunFrame {
        let envelope = ActionEnvelope::new("echo", "hello-node");
        let unsigned = RivunFrame::with_timestamp(
            harness.sender_key.node_id(),
            harness.receiver_key.node_id(),
            RivunFlags::ENCRYPTED,
            timestamp_micros,
            Bytes::from(envelope.to_payload_bytes().unwrap()),
        )
        .unwrap();
        sign_frame(&harness.sender_key, &unsigned).unwrap()
    }

    fn signed_zenv_frame(
        harness: &NodeHarness,
        kind: RivunMessageKind,
        subject: &str,
        body: &[u8],
        timestamp_micros: u64,
    ) -> RivunFrame {
        let unsigned = RivunFrame::with_timestamp(
            harness.sender_key.node_id(),
            harness.receiver_key.node_id(),
            RivunFlags::ENCRYPTED,
            timestamp_micros,
            Bytes::from(zenv_payload(kind, subject, body)),
        )
        .unwrap();
        sign_frame(&harness.sender_key, &unsigned).unwrap()
    }

    fn signed_consensus_echo_frame(harness: &NodeHarness, timestamp_micros: u64) -> RivunFrame {
        let envelope = ActionEnvelope::new("echo", "critical-node");
        let unsigned = RivunFrame::with_timestamp(
            harness.sender_key.node_id(),
            harness.receiver_key.node_id(),
            RivunFlags::ENCRYPTED | RivunFlags::REQUIRES_CONSENSUS,
            timestamp_micros,
            Bytes::from(envelope.to_payload_bytes().unwrap()),
        )
        .unwrap();
        sign_frame(&harness.sender_key, &unsigned).unwrap()
    }

    #[test]
    fn config_validation_accepts_valid_config() {
        let temp = tempfile::tempdir().unwrap();
        let local = Keypair::generate();
        let peer = Keypair::generate();
        let config = validation_config(
            &temp,
            &local,
            &peer,
            public_key_string(&peer),
            "4242424242424242424242424242424242424242424242424242424242424242".to_string(),
        );

        let report = config.validate().unwrap();
        assert_eq!(report.node_id, local.node_id());
        assert_eq!(report.peer_count, 1);
        assert_eq!(report.driver_count, 1);
        assert_eq!(report.signed_driver_count, 0);
        assert!(
            report
                .warnings
                .iter()
                .any(|warning| warning.contains("no PoA validators configured"))
        );
    }

    #[test]
    fn config_validation_reports_memory_routes_and_capabilities() {
        let temp = tempfile::tempdir().unwrap();
        let local = Keypair::generate();
        let peer = Keypair::generate();
        let mut config = validation_config(
            &temp,
            &local,
            &peer,
            public_key_string(&peer),
            "4242424242424242424242424242424242424242424242424242424242424242".to_string(),
        );
        config.memory.dir = Some(temp.path().join("memory"));
        config.memory.allow_driver_read = true;
        config.routes.push(RouteRule {
            name: Some("echo-local".to_string()),
            description: None,
            requires_peer_grant: None,
            matches: RouteMatch {
                kind: Some("action".to_string()),
                subject: Some("echo".to_string()),
                ..RouteMatch::default()
            },
            target: RouteTarget::local_driver("echo"),
        });

        let report = config.validate().unwrap();
        assert!(report.memory_enabled);
        assert_eq!(report.route_count, 1);
        assert_eq!(report.capability_count, 3);

        let advertisement = describe_capabilities(&config).unwrap();
        assert!(
            advertisement
                .capabilities
                .contains(&CapabilityId::new("driver.execute:echo").unwrap())
        );
        assert!(
            advertisement
                .capabilities
                .contains(&CapabilityId::new("memory.read").unwrap())
        );
    }

    #[test]
    fn config_validation_reports_message_policy_rules() {
        let temp = tempfile::tempdir().unwrap();
        let local = Keypair::generate();
        let peer = Keypair::generate();
        let mut config = validation_config(
            &temp,
            &local,
            &peer,
            public_key_string(&peer),
            "4242424242424242424242424242424242424242424242424242424242424242".to_string(),
        );
        config.message_policy.rules.push(MessagePolicyRule {
            name: None,
            kind: Some("action".to_string()),
            subject: Some("safety.*".to_string()),
            source_node: None,
            target_node: None,
            content_type: None,
            decision: MessagePolicyDecision::RequirePoa,
            required_capability: None,
            reason: Some("safety actions require validator quorum".to_string()),
        });

        let report = config.validate().unwrap();
        assert_eq!(
            report.message_policy_default_decision,
            MessagePolicyDecision::Allow
        );
        assert_eq!(report.message_policy_rule_count, 1);
    }

    #[test]
    fn config_validation_reports_message_policy_default_deny() {
        let temp = tempfile::tempdir().unwrap();
        let local = Keypair::generate();
        let peer = Keypair::generate();
        let mut config = validation_config(
            &temp,
            &local,
            &peer,
            public_key_string(&peer),
            "4242424242424242424242424242424242424242424242424242424242424242".to_string(),
        );
        config.message_policy.default_decision = MessagePolicyDecision::Deny;

        let report = config.validate().unwrap();
        assert_eq!(
            report.message_policy_default_decision,
            MessagePolicyDecision::Deny
        );
        assert_eq!(report.message_policy_rule_count, 0);
    }

    #[test]
    fn config_validation_rejects_non_terminal_message_policy_default() {
        let temp = tempfile::tempdir().unwrap();
        let local = Keypair::generate();
        let peer = Keypair::generate();
        let mut config = validation_config(
            &temp,
            &local,
            &peer,
            public_key_string(&peer),
            "4242424242424242424242424242424242424242424242424242424242424242".to_string(),
        );
        config.message_policy.default_decision = MessagePolicyDecision::RequirePoa;

        let error = config.validate().unwrap_err();
        assert!(format!("{error:#}").contains("message_policy.default_decision"));
    }

    #[test]
    fn config_validation_rejects_invalid_message_policy_kind() {
        let temp = tempfile::tempdir().unwrap();
        let local = Keypair::generate();
        let peer = Keypair::generate();
        let mut config = validation_config(
            &temp,
            &local,
            &peer,
            public_key_string(&peer),
            "4242424242424242424242424242424242424242424242424242424242424242".to_string(),
        );
        config.message_policy.rules.push(MessagePolicyRule {
            name: None,
            kind: Some("actions".to_string()),
            subject: Some("echo".to_string()),
            source_node: None,
            target_node: None,
            content_type: None,
            decision: MessagePolicyDecision::Allow,
            required_capability: None,
            reason: None,
        });

        let error = config.validate().unwrap_err();
        assert!(format!("{error:#}").contains("invalid message_policy.rules[0].kind"));
    }

    #[test]
    fn config_validation_rejects_require_grant_without_capability() {
        let temp = tempfile::tempdir().unwrap();
        let local = Keypair::generate();
        let peer = Keypair::generate();
        let mut config = validation_config(
            &temp,
            &local,
            &peer,
            public_key_string(&peer),
            "4242424242424242424242424242424242424242424242424242424242424242".to_string(),
        );
        config.message_policy.rules.push(MessagePolicyRule {
            name: Some("grant gate".to_string()),
            kind: Some("action".to_string()),
            subject: Some("echo".to_string()),
            source_node: None,
            target_node: None,
            content_type: None,
            decision: MessagePolicyDecision::RequireGrant,
            required_capability: None,
            reason: None,
        });

        let error = config.validate().unwrap_err();
        assert!(format!("{error:#}").contains("required_capability is required"));
    }

    #[test]
    fn config_validation_rejects_memory_write_permission_without_memory_gate() {
        let temp = tempfile::tempdir().unwrap();
        let local = Keypair::generate();
        let peer = Keypair::generate();
        let mut config = validation_config(
            &temp,
            &local,
            &peer,
            public_key_string(&peer),
            "4242424242424242424242424242424242424242424242424242424242424242".to_string(),
        );
        config.runtime.permissions.memory_write = true;

        let error = config.validate().unwrap_err();
        assert!(format!("{error:#}").contains("memory.allow_driver_write=true"));
    }

    #[test]
    fn config_validation_reports_message_schema_contracts() {
        let temp = tempfile::tempdir().unwrap();
        let local = Keypair::generate();
        let peer = Keypair::generate();
        let contract_path = temp.path().join("echo.contract.toml");
        std::fs::write(
            &contract_path,
            r#"
schema_version = 1
name = "echo json"
kind = "action"
subject = "echo"
content_type = "application/octet-stream"

[body]
format = "json_object"
required_json_fields = ["message"]
"#,
        )
        .unwrap();
        let mut config = validation_config(
            &temp,
            &local,
            &peer,
            public_key_string(&peer),
            "4242424242424242424242424242424242424242424242424242424242424242".to_string(),
        );
        config.message_schema.require_match = true;
        config.message_schema.contracts.push(MessageContractConfig {
            path: contract_path,
        });

        let report = config.validate().unwrap();
        assert_eq!(report.message_schema_contract_count, 1);
        assert!(report.message_schema_require_match);
    }

    #[test]
    fn config_validation_rejects_unknown_driver_capability_route() {
        let temp = tempfile::tempdir().unwrap();
        let local = Keypair::generate();
        let peer = Keypair::generate();
        let mut config = validation_config(
            &temp,
            &local,
            &peer,
            public_key_string(&peer),
            "4242424242424242424242424242424242424242424242424242424242424242".to_string(),
        );
        config.routes.push(RouteRule {
            name: Some("missing-capability".to_string()),
            description: None,
            requires_peer_grant: None,
            matches: RouteMatch {
                kind: Some("action".to_string()),
                subject: Some("missing".to_string()),
                ..RouteMatch::default()
            },
            target: RouteTarget::capability(CapabilityId::new("driver.execute:missing").unwrap()),
        });

        let error = config.validate().unwrap_err();
        assert!(format!("{error:#}").contains("targets unknown driver capability"));
    }

    #[test]
    fn config_validation_warns_on_non_executable_capability_route() {
        let temp = tempfile::tempdir().unwrap();
        let local = Keypair::generate();
        let peer = Keypair::generate();
        let mut config = validation_config(
            &temp,
            &local,
            &peer,
            public_key_string(&peer),
            "4242424242424242424242424242424242424242424242424242424242424242".to_string(),
        );
        config.memory.dir = Some(temp.path().join("memory"));
        config.routes.push(RouteRule {
            name: Some("memory-capability".to_string()),
            description: None,
            requires_peer_grant: None,
            matches: RouteMatch {
                kind: Some("query".to_string()),
                subject: Some("memory.*".to_string()),
                ..RouteMatch::default()
            },
            target: RouteTarget::capability(CapabilityId::new("memory.local").unwrap()),
        });

        let report = config.validate().unwrap();
        assert!(
            report
                .warnings
                .iter()
                .any(|warning| warning.contains("v1 can only execute driver.execute:*"))
        );
    }

    #[test]
    fn config_validation_reports_capability_policy() {
        let temp = tempfile::tempdir().unwrap();
        let local = Keypair::generate();
        let peer = Keypair::generate();
        let mut config = validation_config(
            &temp,
            &local,
            &peer,
            public_key_string(&peer),
            "4242424242424242424242424242424242424242424242424242424242424242".to_string(),
        );
        config.capability_policy.grants.push(CapabilityGrant {
            capability: CapabilityId::new("driver.execute:echo").unwrap(),
            reason: Some("operator-approved local echo driver".to_string()),
        });
        config
            .capability_policy
            .requirements
            .push(CapabilityRequirement {
                capability: CapabilityId::new("poa.validator").unwrap(),
                required: true,
                reason: Some("critical frames require validator quorum".to_string()),
            });

        let report = config.validate().unwrap();
        assert_eq!(report.capability_count, 1);
        assert_eq!(report.capability_grant_count, 1);
        assert_eq!(report.capability_requirement_count, 1);
        assert_eq!(report.ungranted_capability_count, 0);

        let advertisement = describe_capabilities(&config).unwrap();
        assert_eq!(advertisement.grants.len(), 1);
        assert_eq!(
            advertisement.grants[0].capability,
            CapabilityId::new("driver.execute:echo").unwrap()
        );
        assert_eq!(advertisement.requirements.len(), 1);
    }

    #[test]
    fn config_validation_rejects_unknown_capability_grant() {
        let temp = tempfile::tempdir().unwrap();
        let local = Keypair::generate();
        let peer = Keypair::generate();
        let mut config = validation_config(
            &temp,
            &local,
            &peer,
            public_key_string(&peer),
            "4242424242424242424242424242424242424242424242424242424242424242".to_string(),
        );
        config.capability_policy.grants.push(CapabilityGrant {
            capability: CapabilityId::new("driver.execute:missing").unwrap(),
            reason: None,
        });

        let error = config.validate().unwrap_err();
        assert!(format!("{error:#}").contains("is not advertised by this node"));
    }

    #[test]
    fn config_validation_can_require_grants_for_all_advertised_capabilities() {
        let temp = tempfile::tempdir().unwrap();
        let local = Keypair::generate();
        let peer = Keypair::generate();
        let mut config = validation_config(
            &temp,
            &local,
            &peer,
            public_key_string(&peer),
            "4242424242424242424242424242424242424242424242424242424242424242".to_string(),
        );
        config.memory.dir = Some(temp.path().join("memory"));
        config.capability_policy.require_grants_for_advertised = true;
        config.capability_policy.grants.push(CapabilityGrant {
            capability: CapabilityId::new("driver.execute:echo").unwrap(),
            reason: Some("operator-approved".to_string()),
        });

        let error = config.validate().unwrap_err();
        assert!(
            format!("{error:#}").contains(
                "capability_policy.require_grants_for_advertised=true but missing grants"
            )
        );
        assert!(format!("{error:#}").contains("memory.local"));
    }

    #[test]
    fn config_validation_accepts_peer_route_with_cached_grant() {
        let temp = tempfile::tempdir().unwrap();
        let local = Keypair::generate();
        let peer = Keypair::generate();
        let mut config = validation_config(
            &temp,
            &local,
            &peer,
            public_key_string(&peer),
            "4242424242424242424242424242424242424242424242424242424242424242".to_string(),
        );
        let cache_path = temp.path().join("capabilities.jsonl");
        let capability = CapabilityId::new("driver.execute:thermostat.setpoint").unwrap();
        let mut advertisement = CapabilityAdvertisement::new(peer.node_id());
        advertisement.capabilities.insert(capability.clone());
        advertisement.grants.push(CapabilityGrant {
            capability: capability.clone(),
            reason: Some("peer-approved thermostat driver".to_string()),
        });
        JsonlCapabilityCache::open(&cache_path)
            .put(peer.node_id(), advertisement)
            .unwrap();
        config.capability_cache.path = Some(cache_path);
        config.routes.push(RouteRule {
            name: Some("thermostat-peer".to_string()),
            description: None,
            requires_peer_grant: Some(capability),
            matches: RouteMatch {
                kind: Some("action".to_string()),
                subject: Some("thermostat.setpoint".to_string()),
                ..RouteMatch::default()
            },
            target: RouteTarget::peer(peer.node_id()),
        });

        let report = config.validate().unwrap();
        assert!(report.capability_cache_enabled);
        assert_eq!(report.peer_grant_route_count, 1);
    }

    #[test]
    fn config_validation_rejects_peer_grant_route_without_cache() {
        let temp = tempfile::tempdir().unwrap();
        let local = Keypair::generate();
        let peer = Keypair::generate();
        let mut config = validation_config(
            &temp,
            &local,
            &peer,
            public_key_string(&peer),
            "4242424242424242424242424242424242424242424242424242424242424242".to_string(),
        );
        config.routes.push(RouteRule {
            name: Some("thermostat-peer".to_string()),
            description: None,
            requires_peer_grant: Some(
                CapabilityId::new("driver.execute:thermostat.setpoint").unwrap(),
            ),
            matches: RouteMatch {
                kind: Some("action".to_string()),
                subject: Some("thermostat.setpoint".to_string()),
                ..RouteMatch::default()
            },
            target: RouteTarget::peer(peer.node_id()),
        });

        let error = config.validate().unwrap_err();
        assert!(format!("{error:#}").contains("capability_cache.path is not configured"));
    }

    #[test]
    fn config_validation_rejects_peer_route_without_cached_grant() {
        let temp = tempfile::tempdir().unwrap();
        let local = Keypair::generate();
        let peer = Keypair::generate();
        let mut config = validation_config(
            &temp,
            &local,
            &peer,
            public_key_string(&peer),
            "4242424242424242424242424242424242424242424242424242424242424242".to_string(),
        );
        let cache_path = temp.path().join("capabilities.jsonl");
        let capability = CapabilityId::new("driver.execute:thermostat.setpoint").unwrap();
        let mut advertisement = CapabilityAdvertisement::new(peer.node_id());
        advertisement.capabilities.insert(capability.clone());
        JsonlCapabilityCache::open(&cache_path)
            .put(peer.node_id(), advertisement)
            .unwrap();
        config.capability_cache.path = Some(cache_path);
        config.routes.push(RouteRule {
            name: Some("thermostat-peer".to_string()),
            description: None,
            requires_peer_grant: Some(capability),
            matches: RouteMatch {
                kind: Some("action".to_string()),
                subject: Some("thermostat.setpoint".to_string()),
                ..RouteMatch::default()
            },
            target: RouteTarget::peer(peer.node_id()),
        });

        let error = config.validate().unwrap_err();
        assert!(format!("{error:#}").contains("does not prove that grant"));
    }

    #[test]
    fn config_from_path_resolves_relative_paths_from_config_directory() {
        let temp = tempfile::tempdir().unwrap();
        let config_dir = temp.path().join("conf");
        let key_dir = config_dir.join("keys");
        let driver_dir = config_dir.join("drivers");
        std::fs::create_dir_all(&key_dir).unwrap();
        std::fs::create_dir_all(&driver_dir).unwrap();
        let local = Keypair::generate();
        let peer = Keypair::generate();
        let key_path = key_dir.join("node.key");
        let driver_path = driver_dir.join("echo.wat");
        let manifest_path = driver_dir.join("echo.manifest.toml");
        let receipt_path = config_dir.join("logs").join("receipts");
        let config_path = config_dir.join("node.toml");
        let author = Keypair::generate();
        std::fs::write(&key_path, local.to_key_file_toml().unwrap()).unwrap();
        std::fs::write(&driver_path, echo_driver_wat()).unwrap();
        std::fs::write(
            &manifest_path,
            signed_driver_manifest_toml(
                "echo",
                echo_driver_wat().as_bytes(),
                &author,
                DriverPermissions::none(),
            ),
        )
        .unwrap();
        std::fs::write(
            &config_path,
            format!(
                r#"
bind = "127.0.0.1:0"
key_file = "keys/node.key"
require_signed = true

[[peers]]
node_id = "{}"
addr = "127.0.0.1:9"
public_key = "{}"
transport_key = "4242424242424242424242424242424242424242424242424242424242424242"

[[drivers]]
action = "echo"
path = "drivers/echo.wat"
manifest = "drivers/echo.manifest.toml"

[receipts]
dir = "logs/receipts"
fsync = "always"
"#,
                peer.node_id(),
                public_key_string(&peer),
            ),
        )
        .unwrap();

        let config = RivunNodeConfig::from_path(&config_path).unwrap();
        assert_eq!(config.key_file, key_path);
        assert_eq!(config.drivers[0].path, driver_path);
        assert_eq!(
            config.drivers[0].manifest.as_deref(),
            Some(manifest_path.as_path())
        );
        assert_eq!(config.receipts.dir.as_deref(), Some(receipt_path.as_path()));
        assert_eq!(config.receipts.fsync, ReceiptFsyncPolicy::Always);
        let report = config.validate().unwrap();
        assert_eq!(report.node_id, local.node_id());
        assert_eq!(report.driver_count, 1);
        assert_eq!(report.signed_driver_count, 1);
    }

    #[test]
    fn config_validation_accepts_signed_driver_manifest() {
        let temp = tempfile::tempdir().unwrap();
        let local = Keypair::generate();
        let peer = Keypair::generate();
        let author = Keypair::generate();
        let mut config = validation_config(
            &temp,
            &local,
            &peer,
            public_key_string(&peer),
            "4242424242424242424242424242424242424242424242424242424242424242".to_string(),
        );
        let manifest_path = temp.path().join("echo.manifest.toml");
        std::fs::write(
            &manifest_path,
            signed_driver_manifest_toml(
                "echo",
                echo_driver_wat().as_bytes(),
                &author,
                DriverPermissions::none(),
            ),
        )
        .unwrap();
        config.drivers[0].manifest = Some(manifest_path);

        let report = config.validate().unwrap();
        assert_eq!(report.signed_driver_count, 1);
        assert!(
            !report
                .warnings
                .iter()
                .any(|warning| warning.contains("no signed RivunStore manifest"))
        );
    }

    #[test]
    fn config_validation_rejects_revoked_registry_entry() {
        let temp = tempfile::tempdir().unwrap();
        let local = Keypair::generate();
        let peer = Keypair::generate();
        let author = Keypair::generate();
        let mut config = validation_config(
            &temp,
            &local,
            &peer,
            public_key_string(&peer),
            "4242424242424242424242424242424242424242424242424242424242424242".to_string(),
        );
        let manifest_path = temp.path().join("echo.manifest.toml");
        let manifest = DriverManifest::new(
            "echo-driver",
            "0.1.0",
            "echo",
            echo_driver_wat().as_bytes(),
            DriverPermissions::none(),
            Some("test driver manifest".to_string()),
            &author,
        )
        .unwrap();
        std::fs::write(&manifest_path, manifest.to_toml_string().unwrap()).unwrap();
        config.drivers[0].manifest = Some(manifest_path.clone());

        let mut registry = DriverRegistry::empty(Some("test".to_string()));
        registry
            .add_manifest(&manifest, Some(manifest_path.display().to_string()))
            .unwrap();
        registry.entries[0].status = DriverRegistryStatus::Revoked;
        registry.entries[0].revoked_reason = Some("bad release".to_string());
        let registry_path = temp.path().join("registry.index.toml");
        std::fs::write(&registry_path, registry.to_toml_string().unwrap()).unwrap();
        config.registry.path = Some(registry_path);

        let error = config.validate().unwrap_err();
        assert!(format!("{error:#}").contains("revoked"));
        assert!(format!("{error:#}").contains("bad release"));
    }

    #[test]
    fn config_validation_can_require_signed_registry() {
        let temp = tempfile::tempdir().unwrap();
        let local = Keypair::generate();
        let peer = Keypair::generate();
        let author = Keypair::generate();
        let operator = Keypair::generate();
        let mut config = validation_config(
            &temp,
            &local,
            &peer,
            public_key_string(&peer),
            "4242424242424242424242424242424242424242424242424242424242424242".to_string(),
        );
        let manifest_path = temp.path().join("echo.manifest.toml");
        let manifest = DriverManifest::new(
            "echo-driver",
            "0.1.0",
            "echo",
            echo_driver_wat().as_bytes(),
            DriverPermissions::none(),
            Some("test driver manifest".to_string()),
            &author,
        )
        .unwrap();
        std::fs::write(&manifest_path, manifest.to_toml_string().unwrap()).unwrap();
        config.drivers[0].manifest = Some(manifest_path.clone());

        let mut registry = DriverRegistry::empty(Some("test".to_string()));
        registry
            .add_manifest(&manifest, Some(manifest_path.display().to_string()))
            .unwrap();
        let registry_path = temp.path().join("registry.index.toml");
        std::fs::write(&registry_path, registry.to_toml_string().unwrap()).unwrap();
        config.registry.path = Some(registry_path.clone());
        config.registry.require_signature = true;

        let error = config.validate().unwrap_err();
        assert!(format!("{error:#}").contains("driver registry is not signed"));

        registry.sign(&operator).unwrap();
        std::fs::write(&registry_path, registry.to_toml_string().unwrap()).unwrap();
        let report = config.validate().unwrap();
        assert!(report.registry_enabled);
        assert!(report.registry_signature_required);
        assert_eq!(report.registry_entry_count, 1);
    }

    #[test]
    fn config_validation_rejects_driver_manifest_hash_mismatch() {
        let temp = tempfile::tempdir().unwrap();
        let local = Keypair::generate();
        let peer = Keypair::generate();
        let author = Keypair::generate();
        let mut config = validation_config(
            &temp,
            &local,
            &peer,
            public_key_string(&peer),
            "4242424242424242424242424242424242424242424242424242424242424242".to_string(),
        );
        let manifest_path = temp.path().join("echo.manifest.toml");
        std::fs::write(
            &manifest_path,
            signed_driver_manifest_toml(
                "echo",
                echo_driver_wat().as_bytes(),
                &author,
                DriverPermissions::none(),
            ),
        )
        .unwrap();
        std::fs::write(
            &config.drivers[0].path,
            format!("{}\n;; changed after manifest signing\n", echo_driver_wat()),
        )
        .unwrap();
        config.drivers[0].manifest = Some(manifest_path);

        let error = config.validate().unwrap_err();
        assert!(format!("{error:#}").contains("driver hash mismatch"));
    }

    #[test]
    fn config_validation_rejects_manifest_requesting_unimplemented_permission() {
        let temp = tempfile::tempdir().unwrap();
        let local = Keypair::generate();
        let peer = Keypair::generate();
        let author = Keypair::generate();
        let mut config = validation_config(
            &temp,
            &local,
            &peer,
            public_key_string(&peer),
            "4242424242424242424242424242424242424242424242424242424242424242".to_string(),
        );
        let manifest_path = temp.path().join("echo.manifest.toml");
        let mut permissions = DriverPermissions::none();
        permissions.network = true;
        std::fs::write(
            &manifest_path,
            signed_driver_manifest_toml("echo", echo_driver_wat().as_bytes(), &author, permissions),
        )
        .unwrap();
        config.drivers[0].manifest = Some(manifest_path);

        let error = config.validate().unwrap_err();
        assert!(format!("{error:#}").contains("requests network permission"));
    }

    #[test]
    fn config_validation_rejects_poa_threshold_above_validator_count() {
        let temp = tempfile::tempdir().unwrap();
        let local = Keypair::generate();
        let peer = Keypair::generate();
        let validator = Keypair::generate();
        let mut config = validation_config(
            &temp,
            &local,
            &peer,
            public_key_string(&peer),
            "4242424242424242424242424242424242424242424242424242424242424242".to_string(),
        );
        config.poa = PoaConfig {
            required_threshold: 2,
            validators: vec![PoaValidatorConfig {
                node_id: validator.node_id(),
                public_key: public_key_string(&validator),
            }],
            ..PoaConfig::default()
        };

        let error = config.validate().unwrap_err();
        assert!(format!("{error:#}").contains("poa.required_threshold 2 exceeds"));
    }

    #[test]
    fn config_validation_accepts_signed_poa_validator_set() {
        let temp = tempfile::tempdir().unwrap();
        let local = Keypair::generate();
        let peer = Keypair::generate();
        let authority = Keypair::generate();
        let validator = Keypair::generate();
        let mut config = validation_config(
            &temp,
            &local,
            &peer,
            public_key_string(&peer),
            "4242424242424242424242424242424242424242424242424242424242424242".to_string(),
        );
        let validator_set_path = temp.path().join("poa-validators.json");
        let signed = sign_poa_validator_set(
            &authority,
            PoaValidatorSet {
                schema_version: POA_VALIDATOR_SET_SCHEMA_VERSION,
                set_id: Uuid::from_bytes([7_u8; 16]),
                epoch: 5,
                required_threshold: 1,
                validators: vec![PoaValidatorDescriptor {
                    node_id: validator.node_id(),
                    public_key: public_key_string(&validator),
                }],
                valid_from_micros: None,
                expires_at_micros: None,
                labels: vec!["factory".to_string()],
            },
        )
        .unwrap();
        std::fs::write(
            &validator_set_path,
            serde_json::to_string_pretty(&signed).unwrap(),
        )
        .unwrap();
        config.poa.validator_set = Some(validator_set_path);
        config.poa.validator_set_authority = Some(public_key_string(&authority));

        let report = config.validate().unwrap();

        assert_eq!(report.poa_validator_count, 1);
        assert_eq!(report.poa_required_threshold, 1);
        assert!(report.poa_validator_set_enabled);
        assert_eq!(report.poa_validator_set_epoch, Some(5));
    }

    #[test]
    fn config_validation_rejects_peer_public_key_mismatch() {
        let temp = tempfile::tempdir().unwrap();
        let local = Keypair::generate();
        let peer = Keypair::generate();
        let wrong_peer = Keypair::generate();
        let config = validation_config(
            &temp,
            &local,
            &peer,
            public_key_string(&wrong_peer),
            "4242424242424242424242424242424242424242424242424242424242424242".to_string(),
        );

        let error = config.validate().unwrap_err();
        assert!(format!("{error:#}").contains("peer public_key derives node_id"));
    }

    #[test]
    fn config_validation_rejects_zero_transport_key() {
        let temp = tempfile::tempdir().unwrap();
        let local = Keypair::generate();
        let peer = Keypair::generate();
        let config = validation_config(
            &temp,
            &local,
            &peer,
            public_key_string(&peer),
            "0000000000000000000000000000000000000000000000000000000000000000".to_string(),
        );

        let error = config.validate().unwrap_err();
        assert!(format!("{error:#}").contains("transport_key must not be all zeros"));
    }

    #[test]
    fn config_validation_rejects_oversized_datagram_buffer() {
        let temp = tempfile::tempdir().unwrap();
        let local = Keypair::generate();
        let peer = Keypair::generate();
        let mut config = validation_config(
            &temp,
            &local,
            &peer,
            public_key_string(&peer),
            "4242424242424242424242424242424242424242424242424242424242424242".to_string(),
        );
        config.max_datagram_size = Some(MAX_DATAGRAM_SIZE + 1);

        let error = config.validate().unwrap_err();
        assert!(format!("{error:#}").contains("max_datagram_size must be at most"));
    }

    #[test]
    fn config_validation_rejects_local_node_as_peer() {
        let temp = tempfile::tempdir().unwrap();
        let local = Keypair::generate();
        let config = validation_config(
            &temp,
            &local,
            &local,
            public_key_string(&local),
            "4242424242424242424242424242424242424242424242424242424242424242".to_string(),
        );

        let error = config.validate().unwrap_err();
        assert!(format!("{error:#}").contains("peer list contains local node_id"));
    }

    #[test]
    fn config_validation_warns_on_reused_transport_key() {
        let temp = tempfile::tempdir().unwrap();
        let local = Keypair::generate();
        let peer_a = Keypair::generate();
        let peer_b = Keypair::generate();
        let transport_key =
            "4242424242424242424242424242424242424242424242424242424242424242".to_string();
        let mut config = validation_config(
            &temp,
            &local,
            &peer_a,
            public_key_string(&peer_a),
            transport_key.clone(),
        );
        config.peers.push(PeerConfig {
            node_id: peer_b.node_id(),
            addr: "127.0.0.1:10".to_string(),
            public_key: public_key_string(&peer_b),
            transport_key,
            transport_key_epoch: None,
            transport_key_rotated_at_micros: None,
            trust: PeerTrustConfig::default(),
        });

        let report = config.validate().unwrap();
        assert!(
            report
                .warnings
                .iter()
                .any(|warning| warning.contains("transport_key is reused"))
        );
    }

    #[test]
    fn config_validation_reports_peer_trust_restrictions() {
        let temp = tempfile::tempdir().unwrap();
        let local = Keypair::generate();
        let peer = Keypair::generate();
        let mut config = validation_config(
            &temp,
            &local,
            &peer,
            public_key_string(&peer),
            "4242424242424242424242424242424242424242424242424242424242424242".to_string(),
        );
        config.peers[0].trust.allow_forward = false;
        config.peers[0].trust.labels = vec!["edge".to_string(), "lab".to_string()];

        let report = config.validate().unwrap();
        assert_eq!(report.trusted_peer_count, 1);
        assert_eq!(report.restricted_peer_count, 1);
        assert_eq!(report.peer_send_enabled_count, 1);
        assert_eq!(report.peer_receive_enabled_count, 1);
        assert_eq!(report.peer_forward_enabled_count, 0);
        assert!(
            report
                .warnings
                .iter()
                .any(|warning| warning.contains("trust.allow_forward=false"))
        );
    }

    #[test]
    fn config_validation_rejects_revoked_peer() {
        let temp = tempfile::tempdir().unwrap();
        let local = Keypair::generate();
        let peer = Keypair::generate();
        let mut config = validation_config(
            &temp,
            &local,
            &peer,
            public_key_string(&peer),
            "4242424242424242424242424242424242424242424242424242424242424242".to_string(),
        );
        config.peers[0].trust.status = PeerTrustStatus::Revoked;

        let error = config.validate().unwrap_err();
        assert!(format!("{error:#}").contains("trust.status=revoked"));
    }

    #[test]
    fn config_validation_enforces_transport_key_rotation_age() {
        let temp = tempfile::tempdir().unwrap();
        let local = Keypair::generate();
        let peer = Keypair::generate();
        let mut config = validation_config(
            &temp,
            &local,
            &peer,
            public_key_string(&peer),
            "4242424242424242424242424242424242424242424242424242424242424242".to_string(),
        );
        config.trust.max_transport_key_age_micros = Some(1);
        config.peers[0].transport_key_rotated_at_micros = Some(now_micros().unwrap() - 10);

        let error = config.validate().unwrap_err();
        assert!(format!("{error:#}").contains("transport key age_micros"));
    }

    #[test]
    fn config_validation_rejects_receipt_log_over_key_file() {
        let temp = tempfile::tempdir().unwrap();
        let local = Keypair::generate();
        let peer = Keypair::generate();
        let mut config = validation_config(
            &temp,
            &local,
            &peer,
            public_key_string(&peer),
            "4242424242424242424242424242424242424242424242424242424242424242".to_string(),
        );
        config.receipts.dir = Some(config.key_file.clone());

        let error = config.validate().unwrap_err();
        assert!(format!("{error:#}").contains("receipts.dir must not point at key_file"));
    }

    #[test]
    fn config_validation_rejects_zero_receipt_fsync_interval() {
        let temp = tempfile::tempdir().unwrap();
        let local = Keypair::generate();
        let peer = Keypair::generate();
        let mut config = validation_config(
            &temp,
            &local,
            &peer,
            public_key_string(&peer),
            "4242424242424242424242424242424242424242424242424242424242424242".to_string(),
        );
        config.receipts.fsync = ReceiptFsyncPolicy::Interval;
        config.receipts.fsync_interval_writes = Some(0);

        let error = config.validate().unwrap_err();
        assert!(
            format!("{error:#}").contains("receipts.fsync_interval_writes must be greater than 0")
        );
    }

    #[test]
    fn receipt_durability_state_decides_when_to_fsync() {
        let mut state = ReceiptDurabilityState::default();
        assert!(!state.record_write(ReceiptFsyncPolicy::Off, 2));
        assert!(!state.record_write(ReceiptFsyncPolicy::Interval, 2));
        assert!(state.record_write(ReceiptFsyncPolicy::Interval, 2));
        assert!(!state.record_write(ReceiptFsyncPolicy::Interval, 2));
        assert!(state.record_write(ReceiptFsyncPolicy::Always, 2));
    }

    #[test]
    fn config_validation_rejects_driver_with_invalid_abi() {
        let temp = tempfile::tempdir().unwrap();
        let local = Keypair::generate();
        let peer = Keypair::generate();
        let invalid_driver_path = temp.path().join("invalid-driver.wat");
        std::fs::write(&invalid_driver_path, missing_execute_driver_wat()).unwrap();
        let mut config = validation_config(
            &temp,
            &local,
            &peer,
            public_key_string(&peer),
            "4242424242424242424242424242424242424242424242424242424242424242".to_string(),
        );
        config.drivers[0].path = invalid_driver_path;

        let error = config.validate().unwrap_err();
        assert!(format!("{error:#}").contains("invalid driver ABI"));
        assert!(format!("{error:#}").contains("missing required export `rivun_execute`"));
    }

    #[tokio::test]
    async fn node_handles_legacy_action_envelope_and_executes_wasm_driver() {
        let harness = node_harness(SecurityConfig::default()).await;
        let signed = signed_echo_frame(&harness, now_micros().unwrap());
        harness
            .sender_endpoint
            .send_frame(harness.receiver_key.node_id(), &signed)
            .await
            .unwrap();

        let event = timeout(Duration::from_secs(2), harness.node.handle_once())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(event.source, harness.sender_key.node_id());
        assert_eq!(event.kind, RivunMessageKind::Action);
        assert_eq!(event.subject, "echo");
        assert_eq!(event.action, "echo");
        assert_eq!(event.output.as_deref(), Some(b"hello-node".as_slice()));
    }

    #[tokio::test]
    async fn node_async_runtime_executes_existing_driver_abi() {
        let runtime = RuntimeConfig {
            async_execution: true,
            ..RuntimeConfig::default()
        };
        let harness = node_harness_with_poa_policy_schema_and_runtime(
            SecurityConfig::default(),
            PoaConfig::default(),
            MessagePolicyConfig::default(),
            MessageSchemaConfig::default(),
            runtime,
        )
        .await;
        let signed = signed_echo_frame(&harness, now_micros().unwrap());
        harness
            .sender_endpoint
            .send_frame(harness.receiver_key.node_id(), &signed)
            .await
            .unwrap();

        let event = timeout(Duration::from_secs(2), harness.node.handle_once())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(event.output.as_deref(), Some(b"hello-node".as_slice()));
    }

    #[tokio::test]
    async fn node_metrics_snapshot_tracks_received_frames_and_prometheus_text() {
        let harness = node_harness(SecurityConfig::default()).await;
        let signed = signed_echo_frame(&harness, now_micros().unwrap());
        harness
            .sender_endpoint
            .send_frame(harness.receiver_key.node_id(), &signed)
            .await
            .unwrap();

        timeout(Duration::from_secs(2), harness.node.handle_once())
            .await
            .unwrap()
            .unwrap();

        let snapshot = harness.node.metrics_snapshot();
        assert_eq!(
            snapshot.frames_received_total,
            vec![PeerCounter {
                peer: harness.sender_key.node_id(),
                value: 1
            }]
        );
        assert!(
            snapshot
                .peer_trust_status
                .iter()
                .any(|gauge| gauge.peer == harness.sender_key.node_id()
                    && gauge.status == "trusted"
                    && gauge.value == 1)
        );

        let prometheus = harness.node.metrics_prometheus_text();
        assert!(prometheus.contains("rivun_frames_received_total"));
        assert!(prometheus.contains("rivun_peer_trust_status"));
    }

    #[tokio::test]
    async fn node_metrics_snapshot_tracks_rejected_frames() {
        let harness = node_harness(SecurityConfig {
            max_clock_skew_micros: 1_000,
            replay_cache_capacity: 4096,
            durable_replay_store_path: None,
        })
        .await;
        let signed = signed_echo_frame(&harness, 1);
        harness
            .sender_endpoint
            .send_frame(harness.receiver_key.node_id(), &signed)
            .await
            .unwrap();

        let error = harness.node.handle_once().await.unwrap_err();
        assert!(format!("{error:#}").contains("stale frame timestamp"));

        let snapshot = harness.node.metrics_snapshot();
        assert_eq!(
            snapshot.frames_rejected_total,
            vec![ReasonCounter {
                reason: "anti_replay".to_string(),
                value: 1
            }]
        );
    }

    #[tokio::test]
    async fn node_health_snapshot_reports_healthy_embedding_surface() {
        let mut harness = node_harness_with_poa_and_message_policy(
            SecurityConfig::default(),
            PoaConfig::default(),
            MessagePolicyConfig {
                default_decision: MessagePolicyDecision::Deny,
                rules: Vec::new(),
            },
        )
        .await;
        harness.node.receipt_journal = Some(ReceiptJournalStore::open(
            harness._temp.path().join("receipts"),
        ));

        let snapshot = harness.node.health_snapshot();
        assert_eq!(snapshot.status, RivunNodeHealthStatus::Healthy);
        assert!(snapshot.checks.iter().any(|check| {
            check.name == "endpoint_bound" && check.status == RivunNodeHealthStatus::Healthy
        }));
        assert!(snapshot.checks.iter().any(|check| {
            check.name == "message_policy" && check.status == RivunNodeHealthStatus::Healthy
        }));

        let json = harness.node.health_json().unwrap();
        assert!(json.contains("\"status\": \"healthy\""));
        let healthz = harness.node.healthz_text();
        assert!(healthz.contains("status=healthy"));
        assert!(healthz.contains("check{name=\"endpoint_bound\"}=healthy"));
    }

    fn http_get(addr: SocketAddr, path: &str) -> String {
        let mut stream = TcpStream::connect(addr).unwrap();
        let request = format!("GET {path} HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\n\r\n");
        stream.write_all(request.as_bytes()).unwrap();
        let mut response = String::new();
        stream.read_to_string(&mut response).unwrap();
        response
    }

    #[tokio::test]
    async fn node_observability_http_serves_metrics_and_healthz() {
        let mut harness = node_harness_with_poa_and_message_policy(
            SecurityConfig::default(),
            PoaConfig::default(),
            MessagePolicyConfig {
                default_decision: MessagePolicyDecision::Deny,
                rules: Vec::new(),
            },
        )
        .await;
        harness.node.receipt_journal = Some(ReceiptJournalStore::open(
            harness._temp.path().join("receipts"),
        ));
        let node = Arc::new(harness.node);
        let server = node
            .clone()
            .spawn_observability_http("127.0.0.1:0".parse().unwrap())
            .unwrap();

        let metrics = http_get(server.local_addr(), "/metrics");
        assert!(metrics.starts_with("HTTP/1.1 200 OK"));
        assert!(metrics.contains("Content-Type: text/plain; version=0.0.4"));
        assert!(metrics.contains("rivun_frames_received_total"));

        let healthz = http_get(server.local_addr(), "/healthz");
        assert!(healthz.starts_with("HTTP/1.1 200 OK"));
        assert!(healthz.contains("status=healthy"));
        assert!(healthz.contains("check{name=\"endpoint_bound\"}=healthy"));

        let health_json = http_get(server.local_addr(), "/healthz.json");
        assert!(health_json.starts_with("HTTP/1.1 200 OK"));
        assert!(health_json.contains("Content-Type: application/json"));
        assert!(health_json.contains("\"status\": \"healthy\""));
    }

    #[tokio::test]
    async fn node_health_snapshot_reports_degraded_runtime_signals() {
        let mut harness = node_harness_with_poa_and_message_policy(
            SecurityConfig::default(),
            PoaConfig::default(),
            MessagePolicyConfig {
                default_decision: MessagePolicyDecision::Deny,
                rules: Vec::new(),
            },
        )
        .await;
        harness.node.receipt_journal = Some(ReceiptJournalStore::open(
            harness._temp.path().join("receipts"),
        ));
        harness.node.record_rejected_frame("anti_replay");

        let snapshot = harness.node.health_snapshot();
        assert_eq!(snapshot.status, RivunNodeHealthStatus::Degraded);
        assert!(snapshot.checks.iter().any(|check| {
            check.name == "runtime_errors" && check.status == RivunNodeHealthStatus::Degraded
        }));
    }

    #[tokio::test]
    async fn node_health_snapshot_reports_critical_trust_and_audit_failures() {
        let mut harness = node_harness_with_poa_and_message_policy(
            SecurityConfig::default(),
            PoaConfig::default(),
            MessagePolicyConfig {
                default_decision: MessagePolicyDecision::Deny,
                rules: Vec::new(),
            },
        )
        .await;
        let registry_path = harness._temp.path().join("registry.index.toml");
        let receipt_path = harness._temp.path().join("receipts");
        std::fs::write(&registry_path, "not = [valid").unwrap();
        std::fs::create_dir_all(&receipt_path).unwrap();
        std::fs::write(
            receipt_path.join("00000000000000000000.zjseg"),
            "not-a-rivun-journal",
        )
        .unwrap();
        harness.node.registry_path = Some(registry_path);
        harness.node.receipt_journal = Some(ReceiptJournalStore::open(receipt_path));

        let snapshot = harness.node.health_snapshot();
        assert_eq!(snapshot.status, RivunNodeHealthStatus::Critical);
        assert!(snapshot.checks.iter().any(|check| {
            check.name == "registry_signature" && check.status == RivunNodeHealthStatus::Critical
        }));
        assert!(snapshot.checks.iter().any(|check| {
            check.name == "receipt_log" && check.status == RivunNodeHealthStatus::Critical
        }));
    }

    #[tokio::test]
    async fn node_handles_zenv_action_and_executes_wasm_driver() {
        let harness = node_harness(SecurityConfig::default()).await;
        let signed = signed_zenv_frame(
            &harness,
            RivunMessageKind::Action,
            "echo",
            b"hello-zenv",
            now_micros().unwrap(),
        );
        harness
            .sender_endpoint
            .send_frame(harness.receiver_key.node_id(), &signed)
            .await
            .unwrap();

        let event = timeout(Duration::from_secs(2), harness.node.handle_once())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(event.source, harness.sender_key.node_id());
        assert_eq!(event.kind, RivunMessageKind::Action);
        assert_eq!(event.subject, "echo");
        assert_eq!(event.action, "echo");
        assert_eq!(event.output.as_deref(), Some(b"hello-zenv".as_slice()));
    }

    #[tokio::test]
    async fn node_records_driver_memory_write_host_call() {
        let temp = tempfile::tempdir().unwrap();
        let receiver_key = Keypair::generate();
        let sender_key = Keypair::generate();
        let receiver_key_path = temp.path().join("receiver.key");
        let driver_path = temp.path().join("memory-write.wat");
        let memory_path = temp.path().join("driver-memory.jsonl");
        std::fs::write(&receiver_key_path, receiver_key.to_key_file_toml().unwrap()).unwrap();
        std::fs::write(&driver_path, memory_write_driver_wat()).unwrap();

        let sender_endpoint = RivunEndpoint::bind(RivunEndpointConfig::new(
            "127.0.0.1:0".parse().unwrap(),
            sender_key.node_id(),
        ))
        .await
        .unwrap();
        let transport_key = [0x61_u8; 32];
        let mut runtime = RuntimeConfig::default();
        runtime.permissions.memory_write = true;
        let config = RivunNodeConfig {
            bind: "127.0.0.1:0".to_string(),
            key_file: receiver_key_path,
            require_signed: true,
            max_datagram_size: None,
            peers: vec![PeerConfig {
                node_id: sender_key.node_id(),
                addr: sender_endpoint.local_addr().unwrap().to_string(),
                public_key: public_key_string(&sender_key),
                transport_key: transport_key
                    .iter()
                    .map(|byte| format!("{byte:02x}"))
                    .collect(),
                transport_key_epoch: None,
                transport_key_rotated_at_micros: None,
                trust: PeerTrustConfig::default(),
            }],
            drivers: vec![DriverConfig {
                action: "machine.note".to_string(),
                path: driver_path,
                manifest: None,
            }],
            runtime,
            action_runtime_limits: BTreeMap::new(),
            security: SecurityConfig::default(),
            trust: TrustConfig::default(),
            poa: PoaConfig::default(),
            receipts: ReceiptsConfig::default(),
            registry: RegistryConfig::default(),
            memory: MemoryConfig {
                dir: Some(memory_path.clone()),
                path: None,
                max_record_bytes: None,
                allow_driver_read: false,
                allow_driver_write: true,
            },
            capability_policy: CapabilityPolicyConfig::default(),
            capability_cache: CapabilityCacheConfig::default(),
            discovery: DiscoveryConfig::default(),
            observability: ObservabilityConfig::default(),
            message_policy: MessagePolicyConfig::default(),
            message_schema: MessageSchemaConfig::default(),
            routes: Vec::new(),
            swarm: SwarmConfig::default(),
            gossip: GossipConfig::default(),
            mesh: MeshConfig::default(),
        };
        let node = RivunNode::from_config(config).await.unwrap();
        sender_endpoint
            .add_peer(Peer::new(
                receiver_key.node_id(),
                node.local_addr().unwrap(),
                transport_key,
            ))
            .await;

        let payload = RivunEnvelope::action("machine.note", Bytes::from_static(b"driver-output"))
            .unwrap()
            .encode();
        let frame = RivunFrame::with_timestamp(
            sender_key.node_id(),
            receiver_key.node_id(),
            RivunFlags::ENCRYPTED,
            now_micros().unwrap(),
            payload,
        )
        .unwrap();
        let signed = sign_frame(&sender_key, &frame).unwrap();
        sender_endpoint
            .send_frame(receiver_key.node_id(), &signed)
            .await
            .unwrap();

        let event = timeout(Duration::from_secs(2), node.handle_once())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(event.output.as_deref(), Some(b"driver-output".as_slice()));

        let store = MemoryJournalStore::open(&memory_path);
        let records = store
            .query(&MemoryQuery {
                namespace: Some("driver".to_string()),
                subject: Some("driver.machine.note.memory_write".to_string()),
                ..MemoryQuery::default()
            })
            .unwrap();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].body_bytes().unwrap(), b"machine-note");
        assert_eq!(records[0].source_node, Some(sender_key.node_id()));
        assert!(records[0].frame_hash.is_some());
    }

    #[tokio::test]
    async fn node_responds_to_capability_query() {
        let harness = node_harness(SecurityConfig::default()).await;
        let query = CapabilityQuery::default();
        let envelope = RivunEnvelope::new(
            RivunMessageKind::Control,
            CAPABILITY_QUERY_SUBJECT,
            CAPABILITY_CONTENT_TYPE,
            Bytes::from(serde_json::to_vec(&query).unwrap()),
        )
        .unwrap();
        let unsigned = RivunFrame::with_timestamp(
            harness.sender_key.node_id(),
            harness.receiver_key.node_id(),
            RivunFlags::ENCRYPTED,
            now_micros().unwrap(),
            envelope.encode(),
        )
        .unwrap();
        let signed = sign_frame(&harness.sender_key, &unsigned).unwrap();
        harness
            .sender_endpoint
            .send_frame(harness.receiver_key.node_id(), &signed)
            .await
            .unwrap();

        let event = timeout(Duration::from_secs(2), harness.node.handle_once())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(event.kind, RivunMessageKind::Control);
        assert_eq!(event.subject, CAPABILITY_QUERY_SUBJECT);

        let response = timeout(Duration::from_secs(2), harness.sender_endpoint.recv())
            .await
            .unwrap()
            .unwrap();
        verify_frame(&harness.receiver_key.verifying_key(), &response.frame).unwrap();
        let response_envelope = RivunEnvelopeRef::parse(&response.frame.payload).unwrap();
        assert_eq!(response_envelope.kind(), RivunMessageKind::Control);
        assert_eq!(response_envelope.subject(), CAPABILITY_RESPONSE_SUBJECT);
        let response: CapabilityResponse =
            serde_json::from_slice(response_envelope.body()).unwrap();
        assert!(
            response
                .advertisement
                .capabilities
                .contains(&CapabilityId::new("driver.execute:echo").unwrap())
        );
    }

    #[tokio::test]
    async fn node_responds_to_discovery_query_with_signed_services_and_peers() {
        let harness = node_harness(SecurityConfig::default()).await;
        let query = DiscoveryQuery::default();
        let envelope = RivunEnvelope::new(
            RivunMessageKind::Control,
            DISCOVERY_QUERY_SUBJECT,
            DISCOVERY_CONTENT_TYPE,
            Bytes::from(serde_json::to_vec(&query).unwrap()),
        )
        .unwrap();
        let unsigned = RivunFrame::with_timestamp(
            harness.sender_key.node_id(),
            harness.receiver_key.node_id(),
            RivunFlags::ENCRYPTED,
            now_micros().unwrap(),
            envelope.encode(),
        )
        .unwrap();
        let signed = sign_frame(&harness.sender_key, &unsigned).unwrap();
        harness
            .sender_endpoint
            .send_frame(harness.receiver_key.node_id(), &signed)
            .await
            .unwrap();

        let event = timeout(Duration::from_secs(2), harness.node.handle_once())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(event.kind, RivunMessageKind::Control);
        assert_eq!(event.subject, DISCOVERY_QUERY_SUBJECT);

        let response = timeout(Duration::from_secs(2), harness.sender_endpoint.recv())
            .await
            .unwrap()
            .unwrap();
        verify_frame(&harness.receiver_key.verifying_key(), &response.frame).unwrap();
        let response_envelope = RivunEnvelopeRef::parse(&response.frame.payload).unwrap();
        assert_eq!(response_envelope.kind(), RivunMessageKind::Control);
        assert_eq!(response_envelope.subject(), DISCOVERY_RESPONSE_SUBJECT);
        let response: DiscoveryResponse = serde_json::from_slice(response_envelope.body()).unwrap();
        response
            .verify(
                harness.receiver_key.node_id(),
                &harness.receiver_key.verifying_key(),
            )
            .unwrap();
        assert!(
            response
                .advertisement
                .advertisement
                .services
                .iter()
                .any(|service| service.id == "driver.execute:echo")
        );
        assert!(
            response
                .peers
                .iter()
                .any(|peer| peer.node_id == harness.sender_key.node_id())
        );
    }

    #[tokio::test]
    async fn node_records_discovery_announcement_for_later_queries() {
        let harness = node_harness(SecurityConfig::default()).await;
        let mut capabilities = CapabilityAdvertisement::new(harness.sender_key.node_id());
        capabilities
            .capabilities
            .insert(CapabilityId::new("driver.execute:remote").unwrap());
        let advertisement = build_discovery_advertisement(
            &harness.sender_key,
            Some(harness.sender_endpoint.local_addr().unwrap().to_string()),
            capabilities,
            vec![DiscoveryService {
                id: "remote.echo".to_string(),
                capability: Some(CapabilityId::new("driver.execute:remote").unwrap()),
                kind: Some("action".to_string()),
                subject: Some("remote".to_string()),
                content_type: None,
                description: Some("remote echo test service".to_string()),
                tags: vec!["test".to_string()],
            }],
            vec!["dynamic".to_string()],
            None,
        )
        .unwrap();
        let announcement =
            sign_discovery_advertisement(&harness.sender_key, advertisement).unwrap();
        let announce_envelope = RivunEnvelope::new(
            RivunMessageKind::Control,
            DISCOVERY_ANNOUNCE_SUBJECT,
            DISCOVERY_CONTENT_TYPE,
            Bytes::from(serde_json::to_vec(&announcement).unwrap()),
        )
        .unwrap();
        let unsigned = RivunFrame::with_timestamp(
            harness.sender_key.node_id(),
            harness.receiver_key.node_id(),
            RivunFlags::ENCRYPTED,
            now_micros().unwrap(),
            announce_envelope.encode(),
        )
        .unwrap();
        let signed = sign_frame(&harness.sender_key, &unsigned).unwrap();
        harness
            .sender_endpoint
            .send_frame(harness.receiver_key.node_id(), &signed)
            .await
            .unwrap();

        let event = timeout(Duration::from_secs(2), harness.node.handle_once())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(event.subject, DISCOVERY_ANNOUNCE_SUBJECT);

        let query_envelope = RivunEnvelope::new(
            RivunMessageKind::Control,
            DISCOVERY_QUERY_SUBJECT,
            DISCOVERY_CONTENT_TYPE,
            Bytes::from(serde_json::to_vec(&DiscoveryQuery::default()).unwrap()),
        )
        .unwrap();
        let unsigned = RivunFrame::with_timestamp(
            harness.sender_key.node_id(),
            harness.receiver_key.node_id(),
            RivunFlags::ENCRYPTED,
            now_micros().unwrap(),
            query_envelope.encode(),
        )
        .unwrap();
        let signed = sign_frame(&harness.sender_key, &unsigned).unwrap();
        harness
            .sender_endpoint
            .send_frame(harness.receiver_key.node_id(), &signed)
            .await
            .unwrap();

        timeout(Duration::from_secs(2), harness.node.handle_once())
            .await
            .unwrap()
            .unwrap();
        let response = timeout(Duration::from_secs(2), harness.sender_endpoint.recv())
            .await
            .unwrap()
            .unwrap();
        verify_frame(&harness.receiver_key.verifying_key(), &response.frame).unwrap();
        let response_envelope = RivunEnvelopeRef::parse(&response.frame.payload).unwrap();
        let response: DiscoveryResponse = serde_json::from_slice(response_envelope.body()).unwrap();
        assert_eq!(response.announcements.len(), 1);
        let known = &response.announcements[0];
        known
            .verify(Some(&harness.sender_key.verifying_key()))
            .unwrap();
        assert_eq!(known.advertisement.node_id, harness.sender_key.node_id());
        assert_eq!(known.advertisement.services[0].id, "remote.echo");
    }

    #[tokio::test]
    async fn node_routes_action_to_configured_peer() {
        let temp = tempfile::tempdir().unwrap();
        let router_key = Keypair::generate();
        let sender_key = Keypair::generate();
        let target_key = Keypair::generate();
        let router_key_path = temp.path().join("router.key");
        std::fs::write(&router_key_path, router_key.to_key_file_toml().unwrap()).unwrap();

        let sender_endpoint = RivunEndpoint::bind(RivunEndpointConfig::new(
            "127.0.0.1:0".parse().unwrap(),
            sender_key.node_id(),
        ))
        .await
        .unwrap();
        let target_endpoint = RivunEndpoint::bind(RivunEndpointConfig::new(
            "127.0.0.1:0".parse().unwrap(),
            target_key.node_id(),
        ))
        .await
        .unwrap();
        let sender_transport_key = [0x31_u8; 32];
        let target_transport_key = [0x32_u8; 32];
        let config = RivunNodeConfig {
            bind: "127.0.0.1:0".to_string(),
            key_file: router_key_path,
            require_signed: true,
            max_datagram_size: None,
            peers: vec![
                PeerConfig {
                    node_id: sender_key.node_id(),
                    addr: sender_endpoint.local_addr().unwrap().to_string(),
                    public_key: public_key_string(&sender_key),
                    transport_key: sender_transport_key
                        .iter()
                        .map(|byte| format!("{byte:02x}"))
                        .collect(),
                    transport_key_epoch: None,
                    transport_key_rotated_at_micros: None,
                    trust: PeerTrustConfig::default(),
                },
                PeerConfig {
                    node_id: target_key.node_id(),
                    addr: target_endpoint.local_addr().unwrap().to_string(),
                    public_key: public_key_string(&target_key),
                    transport_key: target_transport_key
                        .iter()
                        .map(|byte| format!("{byte:02x}"))
                        .collect(),
                    transport_key_epoch: None,
                    transport_key_rotated_at_micros: None,
                    trust: PeerTrustConfig::default(),
                },
            ],
            drivers: Vec::new(),
            runtime: RuntimeConfig::default(),
            action_runtime_limits: BTreeMap::new(),
            security: SecurityConfig::default(),
            trust: TrustConfig::default(),
            poa: PoaConfig::default(),
            receipts: ReceiptsConfig::default(),
            registry: RegistryConfig::default(),
            memory: MemoryConfig::default(),
            capability_policy: CapabilityPolicyConfig::default(),
            capability_cache: CapabilityCacheConfig::default(),
            discovery: DiscoveryConfig::default(),
            observability: ObservabilityConfig::default(),
            message_policy: MessagePolicyConfig::default(),
            message_schema: MessageSchemaConfig::default(),
            routes: vec![RouteRule {
                name: Some("echo-to-peer".to_string()),
                description: None,
                requires_peer_grant: None,
                matches: RouteMatch {
                    kind: Some("action".to_string()),
                    subject: Some("echo".to_string()),
                    ..RouteMatch::default()
                },
                target: RouteTarget::peer(target_key.node_id()),
            }],
            swarm: SwarmConfig::default(),
            gossip: GossipConfig::default(),
            mesh: MeshConfig::default(),
        };
        let router = RivunNode::from_config(config).await.unwrap();
        sender_endpoint
            .add_peer(Peer::new(
                router_key.node_id(),
                router.local_addr().unwrap(),
                sender_transport_key,
            ))
            .await;
        target_endpoint
            .add_peer(Peer::new(
                router_key.node_id(),
                router.local_addr().unwrap(),
                target_transport_key,
            ))
            .await;

        let payload = zenv_payload(RivunMessageKind::Action, "echo", b"forward-me");
        let unsigned = RivunFrame::with_timestamp(
            sender_key.node_id(),
            router_key.node_id(),
            RivunFlags::ENCRYPTED,
            now_micros().unwrap(),
            Bytes::from(payload),
        )
        .unwrap();
        let signed = sign_frame(&sender_key, &unsigned).unwrap();
        sender_endpoint
            .send_frame(router_key.node_id(), &signed)
            .await
            .unwrap();

        let event = timeout(Duration::from_secs(2), router.handle_once())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(event.kind, RivunMessageKind::Action);
        assert_eq!(event.output, None);

        let forwarded = timeout(Duration::from_secs(2), target_endpoint.recv())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(forwarded.peer.node_id, router_key.node_id());
        verify_frame(&router_key.verifying_key(), &forwarded.frame).unwrap();
        let envelope = RivunEnvelopeRef::parse(&forwarded.frame.payload).unwrap();
        assert_eq!(envelope.kind(), RivunMessageKind::Action);
        assert_eq!(envelope.subject(), "echo");
        assert_eq!(envelope.body(), b"forward-me");
    }

    #[tokio::test]
    async fn node_accepts_zenv_event_without_wasm_dispatch() {
        let mut harness = node_harness(SecurityConfig::default()).await;
        let receipt_path = harness._temp.path().join("receipts").join("events");
        harness.node.receipt_journal = Some(ReceiptJournalStore::open(&receipt_path));
        let signed = signed_zenv_frame(
            &harness,
            RivunMessageKind::Event,
            "echo",
            b"event-body",
            now_micros().unwrap(),
        );
        harness
            .sender_endpoint
            .send_frame(harness.receiver_key.node_id(), &signed)
            .await
            .unwrap();

        let event = timeout(Duration::from_secs(2), harness.node.handle_once())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(event.source, harness.sender_key.node_id());
        assert_eq!(event.kind, RivunMessageKind::Event);
        assert_eq!(event.subject, "echo");
        assert_eq!(event.action, "echo");
        assert_eq!(event.output, None);

        let receipts = ReceiptJournalStore::open(&receipt_path).all().unwrap();
        let receipt = receipts.first().unwrap();
        receipt.verify().unwrap();
        assert_eq!(receipt.receipt.kind, "event");
        assert_eq!(receipt.receipt.subject, "echo");
        assert!(receipt.receipt.output_hash.is_none());
    }

    #[tokio::test]
    async fn node_rejects_zenv_message_that_violates_configured_schema() {
        let contract_temp = tempfile::tempdir().unwrap();
        let contract_path = contract_temp.path().join("echo.contract.toml");
        std::fs::write(
            &contract_path,
            r#"
schema_version = 1
name = "echo json"
kind = "action"
subject = "echo"
content_type = "application/octet-stream"

[body]
format = "json_object"
required_json_fields = ["message"]
"#,
        )
        .unwrap();
        let harness = node_harness_with_poa_policy_and_schema(
            SecurityConfig::default(),
            PoaConfig::default(),
            MessagePolicyConfig::default(),
            MessageSchemaConfig {
                require_match: false,
                contracts: vec![MessageContractConfig {
                    path: contract_path,
                }],
            },
        )
        .await;
        let signed = signed_zenv_frame(
            &harness,
            RivunMessageKind::Action,
            "echo",
            b"not-json",
            now_micros().unwrap(),
        );
        harness
            .sender_endpoint
            .send_frame(harness.receiver_key.node_id(), &signed)
            .await
            .unwrap();

        let error = harness.node.handle_once().await.unwrap_err();
        assert!(format!("{error:#}").contains("message contract validation failed"));
        assert!(format!("{error:#}").contains("expected JSON body"));
    }

    #[tokio::test]
    async fn node_writes_signed_action_receipt_when_enabled() {
        let mut harness = node_harness(SecurityConfig::default()).await;
        let receipt_path = harness._temp.path().join("receipts").join("actions");
        harness.node.receipt_journal = Some(ReceiptJournalStore::open(&receipt_path));
        let signed = signed_echo_frame(&harness, now_micros().unwrap());
        harness
            .sender_endpoint
            .send_frame(harness.receiver_key.node_id(), &signed)
            .await
            .unwrap();

        let event = timeout(Duration::from_secs(2), harness.node.handle_once())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(event.action, "echo");

        let receipts = ReceiptJournalStore::open(&receipt_path).all().unwrap();
        let receipt = receipts.first().unwrap();
        receipt.verify().unwrap();
        assert_eq!(receipt.receipt.node_id, harness.receiver_key.node_id());
        assert_eq!(receipt.receipt.source_node, harness.sender_key.node_id());
        assert_eq!(receipt.receipt.kind, "action");
        assert_eq!(receipt.receipt.subject, "echo");
        assert_eq!(receipt.receipt.action, "echo");
        assert!(receipt.receipt.output_hash.is_some());
        assert!(receipt.receipt.poa.is_none());
    }

    #[tokio::test]
    async fn node_receipt_references_verified_pact_record() {
        let mut harness = node_harness(SecurityConfig::default()).await;
        let receipt_path = harness._temp.path().join("receipts").join("pact");
        harness.node.receipt_journal = Some(ReceiptJournalStore::open(&receipt_path));

        let mut pact = RivunPact::new(
            "agent.alpha",
            "driver.valve",
            "valve.open",
            1_893_456_000_000_000,
        );
        pact.pact_id = Uuid::parse_str("33333333-3333-4333-8333-333333333333").unwrap();
        pact.object = serde_json::json!({"valve": "v-7"});
        pact.terms = serde_json::json!({"max_runtime_ms": 5000});
        pact.consent = serde_json::json!({"operator": "ops.lead", "approved": true});
        pact.proof = serde_json::json!({"kind": "policy", "decision": "allow"});
        pact.sign(&harness.sender_key).unwrap();
        let body = serde_json::to_vec(&pact).unwrap();
        let payload = RivunEnvelope::new(
            RivunMessageKind::Action,
            PACT_RECORD_SUBJECT,
            PACT_CONTENT_TYPE,
            Bytes::from(body),
        )
        .unwrap()
        .encode()
        .to_vec();
        let unsigned = RivunFrame::with_timestamp(
            harness.sender_key.node_id(),
            harness.receiver_key.node_id(),
            RivunFlags::ENCRYPTED,
            now_micros().unwrap(),
            Bytes::from(payload),
        )
        .unwrap();
        let signed = sign_frame(&harness.sender_key, &unsigned).unwrap();

        harness
            .sender_endpoint
            .send_frame(harness.receiver_key.node_id(), &signed)
            .await
            .unwrap();

        let event = timeout(Duration::from_secs(2), harness.node.handle_once())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(event.kind, RivunMessageKind::Action);
        assert_eq!(event.subject, PACT_RECORD_SUBJECT);

        let receipts = ReceiptJournalStore::open(&receipt_path).all().unwrap();
        let receipt = receipts.first().unwrap();
        receipt.verify().unwrap();
        let pact_ref = receipt.receipt.pact.as_ref().unwrap();
        assert_eq!(pact_ref.pact_id, pact.pact_id);
        assert_eq!(pact_ref.intent, "valve.open");
        assert_eq!(pact_ref.hash, pact.hash.unwrap());
        assert_eq!(pact_ref.status, "active");
    }

    #[tokio::test]
    async fn node_policy_rejects_message_missing_required_poa() {
        let harness = node_harness_with_poa_and_message_policy(
            SecurityConfig::default(),
            PoaConfig::default(),
            MessagePolicyConfig {
                rules: vec![MessagePolicyRule {
                    name: None,
                    kind: Some("action".to_string()),
                    subject: Some("echo".to_string()),
                    source_node: None,
                    target_node: None,
                    content_type: None,
                    decision: MessagePolicyDecision::RequirePoa,
                    required_capability: None,
                    reason: Some("echo requires operator approval".to_string()),
                }],
                ..MessagePolicyConfig::default()
            },
        )
        .await;
        let signed = signed_zenv_frame(
            &harness,
            RivunMessageKind::Action,
            "echo",
            b"policy-body",
            now_micros().unwrap(),
        );
        harness
            .sender_endpoint
            .send_frame(harness.receiver_key.node_id(), &signed)
            .await
            .unwrap();

        let error = harness.node.handle_once().await.unwrap_err();
        assert!(format!("{error:#}").contains("message policy requires Proof-of-Action"));
        assert!(format!("{error:#}").contains("echo requires operator approval"));
    }

    #[tokio::test]
    async fn node_policy_denies_matching_message() {
        let harness = node_harness_with_poa_and_message_policy(
            SecurityConfig::default(),
            PoaConfig::default(),
            MessagePolicyConfig {
                rules: vec![MessagePolicyRule {
                    name: None,
                    kind: Some("action".to_string()),
                    subject: Some("echo".to_string()),
                    source_node: None,
                    target_node: None,
                    content_type: None,
                    decision: MessagePolicyDecision::Deny,
                    required_capability: None,
                    reason: Some("echo disabled".to_string()),
                }],
                ..MessagePolicyConfig::default()
            },
        )
        .await;
        let signed = signed_zenv_frame(
            &harness,
            RivunMessageKind::Action,
            "echo",
            b"denied-body",
            now_micros().unwrap(),
        );
        harness
            .sender_endpoint
            .send_frame(harness.receiver_key.node_id(), &signed)
            .await
            .unwrap();

        let error = harness.node.handle_once().await.unwrap_err();
        assert!(format!("{error:#}").contains("message policy denied action echo"));
        assert!(format!("{error:#}").contains("echo disabled"));
    }

    #[tokio::test]
    async fn node_policy_default_deny_rejects_unmatched_message() {
        let harness = node_harness_with_poa_and_message_policy(
            SecurityConfig::default(),
            PoaConfig::default(),
            MessagePolicyConfig {
                default_decision: MessagePolicyDecision::Deny,
                rules: Vec::new(),
            },
        )
        .await;
        let signed = signed_zenv_frame(
            &harness,
            RivunMessageKind::Action,
            "echo",
            b"default-denied-body",
            now_micros().unwrap(),
        );
        harness
            .sender_endpoint
            .send_frame(harness.receiver_key.node_id(), &signed)
            .await
            .unwrap();

        let error = harness.node.handle_once().await.unwrap_err();
        assert!(format!("{error:#}").contains("message policy denied action echo"));
        assert!(format!("{error:#}").contains("default deny"));
    }

    #[tokio::test]
    async fn node_policy_accepts_message_with_required_poa() {
        let validator = Keypair::generate();
        let harness = node_harness_with_poa_and_message_policy(
            SecurityConfig::default(),
            PoaConfig {
                required_threshold: 1,
                validators: vec![PoaValidatorConfig {
                    node_id: validator.node_id(),
                    public_key: public_key_string(&validator),
                }],
                ..PoaConfig::default()
            },
            MessagePolicyConfig {
                rules: vec![MessagePolicyRule {
                    name: None,
                    kind: Some("action".to_string()),
                    subject: Some("echo".to_string()),
                    source_node: None,
                    target_node: None,
                    content_type: None,
                    decision: MessagePolicyDecision::RequirePoa,
                    required_capability: None,
                    reason: Some("echo requires operator approval".to_string()),
                }],
                ..MessagePolicyConfig::default()
            },
        )
        .await;
        let signed = signed_consensus_echo_frame(&harness, now_micros().unwrap());
        let certified = certify_frame(&signed, 1, std::slice::from_ref(&validator)).unwrap();
        harness
            .sender_endpoint
            .send_frame(harness.receiver_key.node_id(), &certified)
            .await
            .unwrap();

        let event = timeout(Duration::from_secs(2), harness.node.handle_once())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(event.action, "echo");
        assert_eq!(event.output.as_deref(), Some(b"critical-node".as_slice()));
    }

    #[tokio::test]
    async fn node_rejects_consensus_frame_without_poa_certificate() {
        let validator = Keypair::generate();
        let harness = node_harness_with_poa(
            SecurityConfig::default(),
            PoaConfig {
                required_threshold: 1,
                validators: vec![PoaValidatorConfig {
                    node_id: validator.node_id(),
                    public_key: public_key_string(&validator),
                }],
                ..PoaConfig::default()
            },
        )
        .await;
        let signed = signed_consensus_echo_frame(&harness, now_micros().unwrap());
        harness
            .sender_endpoint
            .send_frame(harness.receiver_key.node_id(), &signed)
            .await
            .unwrap();

        let error = harness.node.handle_once().await.unwrap_err();
        assert!(format!("{error:#}").contains("Proof-of-Action"));
        assert!(format!("{error:#}").contains("missing a Proof-of-Action certificate"));
    }

    #[tokio::test]
    async fn node_accepts_consensus_frame_with_valid_poa_certificate() {
        let validator = Keypair::generate();
        let harness = node_harness_with_poa(
            SecurityConfig::default(),
            PoaConfig {
                required_threshold: 1,
                validators: vec![PoaValidatorConfig {
                    node_id: validator.node_id(),
                    public_key: public_key_string(&validator),
                }],
                ..PoaConfig::default()
            },
        )
        .await;
        let signed = signed_consensus_echo_frame(&harness, now_micros().unwrap());
        let certified = certify_frame(&signed, 1, std::slice::from_ref(&validator)).unwrap();
        harness
            .sender_endpoint
            .send_frame(harness.receiver_key.node_id(), &certified)
            .await
            .unwrap();

        let event = timeout(Duration::from_secs(2), harness.node.handle_once())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(event.action, "echo");
        assert_eq!(event.output.as_deref(), Some(b"critical-node".as_slice()));
    }

    #[tokio::test]
    async fn node_accepts_consensus_frame_with_signed_validator_set() {
        let temp = tempfile::tempdir().unwrap();
        let authority = Keypair::generate();
        let validator = Keypair::generate();
        let validator_set_path = temp.path().join("poa-validators.json");
        let signed = sign_poa_validator_set(
            &authority,
            PoaValidatorSet {
                schema_version: POA_VALIDATOR_SET_SCHEMA_VERSION,
                set_id: Uuid::from_bytes([6_u8; 16]),
                epoch: 2,
                required_threshold: 1,
                validators: vec![PoaValidatorDescriptor {
                    node_id: validator.node_id(),
                    public_key: public_key_string(&validator),
                }],
                valid_from_micros: None,
                expires_at_micros: None,
                labels: Vec::new(),
            },
        )
        .unwrap();
        std::fs::write(
            &validator_set_path,
            serde_json::to_string_pretty(&signed).unwrap(),
        )
        .unwrap();
        let harness = node_harness_with_poa(
            SecurityConfig::default(),
            PoaConfig {
                required_threshold: 1,
                validator_set: Some(validator_set_path),
                validator_set_authority: Some(public_key_string(&authority)),
                ..PoaConfig::default()
            },
        )
        .await;
        let signed = signed_consensus_echo_frame(&harness, now_micros().unwrap());
        let certified = certify_frame(&signed, 1, std::slice::from_ref(&validator)).unwrap();
        harness
            .sender_endpoint
            .send_frame(harness.receiver_key.node_id(), &certified)
            .await
            .unwrap();

        let event = timeout(Duration::from_secs(2), harness.node.handle_once())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(event.action, "echo");
    }

    #[tokio::test]
    async fn node_reuses_precompiled_driver_after_driver_file_is_removed() {
        let harness = node_harness(SecurityConfig::default()).await;
        std::fs::remove_file(&harness.driver_path).unwrap();
        let signed = signed_echo_frame(&harness, now_micros().unwrap());
        harness
            .sender_endpoint
            .send_frame(harness.receiver_key.node_id(), &signed)
            .await
            .unwrap();

        let event = timeout(Duration::from_secs(2), harness.node.handle_once())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(event.source, harness.sender_key.node_id());
        assert_eq!(event.action, "echo");
        assert_eq!(event.output.as_deref(), Some(b"hello-node".as_slice()));
    }

    #[tokio::test]
    async fn node_rejects_replayed_signed_frame() {
        let harness = node_harness(SecurityConfig::default()).await;
        let signed = signed_echo_frame(&harness, now_micros().unwrap());

        harness
            .sender_endpoint
            .send_frame(harness.receiver_key.node_id(), &signed)
            .await
            .unwrap();
        harness.node.handle_once().await.unwrap();

        harness
            .sender_endpoint
            .send_frame(harness.receiver_key.node_id(), &signed)
            .await
            .unwrap();
        let error = harness.node.handle_once().await.unwrap_err();
        assert!(error.to_string().contains("anti-replay"));
        assert!(format!("{error:#}").contains("replayed frame rejected"));
    }

    #[tokio::test]
    async fn node_rejects_inbound_from_receive_denied_peer() {
        let temp = tempfile::tempdir().unwrap();
        let receiver_key = Keypair::generate();
        let sender_key = Keypair::generate();
        let receiver_key_path = temp.path().join("receiver.key");
        std::fs::write(&receiver_key_path, receiver_key.to_key_file_toml().unwrap()).unwrap();

        let sender_endpoint = RivunEndpoint::bind(RivunEndpointConfig::new(
            "127.0.0.1:0".parse().unwrap(),
            sender_key.node_id(),
        ))
        .await
        .unwrap();
        let transport_key = [0x73_u8; 32];
        let trust = PeerTrustConfig {
            allow_receive: false,
            ..PeerTrustConfig::default()
        };
        let config = RivunNodeConfig {
            bind: "127.0.0.1:0".to_string(),
            key_file: receiver_key_path,
            require_signed: true,
            max_datagram_size: None,
            peers: vec![PeerConfig {
                node_id: sender_key.node_id(),
                addr: sender_endpoint.local_addr().unwrap().to_string(),
                public_key: public_key_string(&sender_key),
                transport_key: transport_key
                    .iter()
                    .map(|byte| format!("{byte:02x}"))
                    .collect(),
                transport_key_epoch: None,
                transport_key_rotated_at_micros: None,
                trust,
            }],
            drivers: Vec::new(),
            runtime: RuntimeConfig::default(),
            action_runtime_limits: BTreeMap::new(),
            security: SecurityConfig::default(),
            trust: TrustConfig::default(),
            poa: PoaConfig::default(),
            receipts: ReceiptsConfig::default(),
            registry: RegistryConfig::default(),
            memory: MemoryConfig::default(),
            capability_policy: CapabilityPolicyConfig::default(),
            capability_cache: CapabilityCacheConfig::default(),
            discovery: DiscoveryConfig::default(),
            observability: ObservabilityConfig::default(),
            message_policy: MessagePolicyConfig::default(),
            message_schema: MessageSchemaConfig::default(),
            routes: Vec::new(),
            swarm: SwarmConfig::default(),
            gossip: GossipConfig::default(),
            mesh: MeshConfig::default(),
        };
        let node = RivunNode::from_config(config).await.unwrap();
        sender_endpoint
            .add_peer(Peer::new(
                receiver_key.node_id(),
                node.local_addr().unwrap(),
                transport_key,
            ))
            .await;

        let frame = RivunFrame::new(
            sender_key.node_id(),
            receiver_key.node_id(),
            RivunFlags::ENCRYPTED,
            Bytes::from_static(b"hello"),
        )
        .unwrap();
        let frame = sign_frame(&sender_key, &frame).unwrap();
        sender_endpoint
            .send_frame(receiver_key.node_id(), &frame)
            .await
            .unwrap();

        let error = node.handle_once().await.unwrap_err();
        assert!(format!("{error:#}").contains("not permitted to send inbound frames"));
    }

    #[tokio::test]
    async fn node_rejects_stale_frame_timestamp() {
        let harness = node_harness(SecurityConfig {
            max_clock_skew_micros: 1_000,
            replay_cache_capacity: 4096,
            durable_replay_store_path: None,
        })
        .await;
        let signed = signed_echo_frame(&harness, 1);
        harness
            .sender_endpoint
            .send_frame(harness.receiver_key.node_id(), &signed)
            .await
            .unwrap();

        let error = harness.node.handle_once().await.unwrap_err();
        assert!(format!("{error:#}").contains("stale frame timestamp"));
    }

    #[tokio::test]
    async fn node_rejects_future_frame_timestamp() {
        let harness = node_harness(SecurityConfig {
            max_clock_skew_micros: 1_000,
            replay_cache_capacity: 4096,
            durable_replay_store_path: None,
        })
        .await;
        let signed = signed_echo_frame(&harness, now_micros().unwrap() + 10_000_000);
        harness
            .sender_endpoint
            .send_frame(harness.receiver_key.node_id(), &signed)
            .await
            .unwrap();

        let error = harness.node.handle_once().await.unwrap_err();
        assert!(format!("{error:#}").contains("future frame timestamp"));
    }
}
