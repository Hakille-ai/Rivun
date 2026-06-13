//! ZAP node daemon.
//!
//! The node combines static peer discovery, encrypted UDP transport, optional
//! Ed25519 frame verification, and WASM action dispatch.

use anyhow::{Context, Result, anyhow, bail};
use base64::{Engine as _, engine::general_purpose::STANDARD_NO_PAD};
use bytes::Bytes;
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeSet, HashMap, HashSet, VecDeque},
    fs,
    fs::OpenOptions,
    io::Write,
    net::SocketAddr,
    path::{Path, PathBuf},
    sync::Mutex,
};
use tracing::{info, warn};
use uuid::Uuid;
use zap_capability::{
    CAPABILITY_CONTENT_TYPE, CAPABILITY_QUERY_SUBJECT, CAPABILITY_RESPONSE_SUBJECT,
    CapabilityAdvertisement, CapabilityGrant, CapabilityId, CapabilityQuery, CapabilityRequirement,
    CapabilityResponse, CapabilitySet, DriverPermissions, JsonlCapabilityCache,
    capabilities_for_driver,
};
use zap_core::{ZapFlags, ZapFrame, now_micros};
use zap_crypto::{
    Keypair, POA_ATTESTATION_CONTENT_TYPE, POA_ATTESTATION_REQUEST_SUBJECT,
    POA_ATTESTATION_RESPONSE_SUBJECT, POA_VALIDATOR_SET_CONTENT_TYPE,
    POA_VALIDATOR_SET_REQUEST_SUBJECT, POA_VALIDATOR_SET_RESPONSE_SUBJECT, PoaValidatorSetRequest,
    PoaValidatorSetResponse, PublicKey, SignedPoaValidatorSet, sign_frame,
    sign_poa_attestation_request, verify_frame, verify_poa_certificate,
};
use zap_envelope::{
    MAGIC_BYTES as ZENV_MAGIC_BYTES, MAX_CONTENT_TYPE_LEN as ZENV_MAX_CONTENT_TYPE_LEN,
    MAX_METADATA_LEN as ZENV_MAX_METADATA_LEN, MAX_SUBJECT_LEN as ZENV_MAX_SUBJECT_LEN,
    ZapEnvelope, ZapEnvelopeRef, ZapMessageKind,
};
use zap_ledger::{
    RECEIPT_REPLICATION_CONTENT_TYPE, RECEIPT_REPLICATION_REQUEST_SUBJECT,
    RECEIPT_REPLICATION_RESPONSE_SUBJECT, ReceiptReplicationRequest, ReceiptReplicationResponse,
    SignedActionReceipt,
};
use zap_memory::{JsonlMemoryStore, MemoryPut, MemoryStore};
use zap_net::{Peer, TransportKey, ZapEndpoint, ZapEndpointConfig};
use zap_policy::{PolicyInput, PolicyRule, PolicySet};
use zap_router::{RouteDecision, RouteMessage, RouteRule, RouteTable};
use zap_runtime::{ExecutionLimits, HostCallKind, HostCallRecord, WasmDriver, WasmExecutor};
use zap_schema::{MessageContract, MessageContractSet, MessageParts};
use zap_store::{DriverManifest, DriverRegistry};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZapNodeConfig {
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
    pub message_policy: MessagePolicyConfig,
    #[serde(default)]
    pub message_schema: MessageSchemaConfig,
    #[serde(default)]
    pub routes: Vec<RouteRule>,
}

impl ZapNodeConfig {
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

fn resolve_config_paths(mut config: ZapNodeConfig, config_path: &Path) -> ZapNodeConfig {
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
    if let Some(path) = config.receipts.path.take() {
        config.receipts.path = Some(resolve_relative_path(base_dir, &path));
    }
    if let Some(path) = config.registry.path.take() {
        config.registry.path = Some(resolve_relative_path(base_dir, &path));
    }
    if let Some(path) = config.poa.validator_set.take() {
        config.poa.validator_set = Some(resolve_relative_path(base_dir, &path));
    }
    if let Some(path) = config.memory.path.take() {
        config.memory.path = Some(resolve_relative_path(base_dir, &path));
    }
    if let Some(path) = config.capability_cache.path.take() {
        config.capability_cache.path = Some(resolve_relative_path(base_dir, &path));
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
    #[serde(default)]
    pub permissions: DriverPermissions,
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            max_memory_bytes: None,
            fuel: None,
            timeout_ms: None,
            max_output_bytes: None,
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

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ReceiptsConfig {
    #[serde(default)]
    pub path: Option<PathBuf>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RegistryConfig {
    #[serde(default)]
    pub path: Option<PathBuf>,
    #[serde(default)]
    pub require_signature: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MemoryConfig {
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
pub struct MessagePolicyConfig {
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

pub type MessagePolicyDecision = zap_policy::PolicyDecision;

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

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct SecurityConfig {
    #[serde(default = "default_max_clock_skew_micros")]
    pub max_clock_skew_micros: u64,
    #[serde(default = "default_replay_cache_capacity")]
    pub replay_cache_capacity: usize,
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            max_clock_skew_micros: default_max_clock_skew_micros(),
            replay_cache_capacity: default_replay_cache_capacity(),
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
    pub peer_count: usize,
    pub trusted_peer_count: usize,
    pub restricted_peer_count: usize,
    pub peer_send_enabled_count: usize,
    pub peer_receive_enabled_count: usize,
    pub peer_forward_enabled_count: usize,
    pub driver_count: usize,
    pub signed_driver_count: usize,
    pub receipt_log_enabled: bool,
    pub registry_enabled: bool,
    pub registry_entry_count: usize,
    pub registry_signature_required: bool,
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
    pub kind: ZapMessageKind,
    pub subject: String,
    /// Deprecated compatibility alias for legacy action-oriented consumers.
    pub action: String,
    pub output: Option<Vec<u8>>,
}

#[derive(Debug, Clone)]
struct InboundMessage {
    kind: ZapMessageKind,
    subject: String,
    content_type: Option<String>,
    body: Vec<u8>,
    metadata: Vec<u8>,
}

impl InboundMessage {
    fn from_universal(envelope: ZapEnvelopeRef<'_>) -> Self {
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
            kind: ZapMessageKind::Action,
            subject: envelope.action,
            content_type: None,
            body,
            metadata: Vec::new(),
        })
    }

    fn raw_data(payload: &[u8]) -> Self {
        Self {
            kind: ZapMessageKind::Data,
            subject: String::new(),
            content_type: None,
            body: payload.to_vec(),
            metadata: Vec::new(),
        }
    }
}

fn parse_inbound_message(payload: &[u8]) -> Result<InboundMessage> {
    if payload.starts_with(&ZENV_MAGIC_BYTES) {
        let envelope = ZapEnvelopeRef::parse(payload).context("invalid ZENV envelope")?;
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

pub struct ZapNode {
    endpoint: ZapEndpoint,
    keypair: Keypair,
    public_keys: HashMap<Uuid, PublicKey>,
    peer_trust: HashMap<Uuid, PeerTrustConfig>,
    drivers: HashMap<String, DriverRegistration>,
    runtime: WasmExecutor,
    limits: ExecutionLimits,
    require_signed: bool,
    replay_guard: Mutex<ReplayGuard>,
    security: SecurityConfig,
    poa_validators: Vec<(Uuid, PublicKey)>,
    poa_required_threshold: u16,
    poa_validator_set_path: Option<PathBuf>,
    poa_validator_set_authority: Option<PublicKey>,
    receipt_log_path: Option<PathBuf>,
    memory: MemoryConfig,
    route_table: RouteTable,
    message_policy: MessagePolicyConfig,
    message_contracts: MessageContractSet,
    peer_ids: Vec<Uuid>,
    capability_advertisement: CapabilityAdvertisement,
}

struct DriverRegistration {
    driver: WasmDriver,
    permissions: DriverPermissions,
}

impl ZapNode {
    pub async fn from_config(config: ZapNodeConfig) -> Result<Self> {
        config.validate()?;
        let keypair = load_keypair(&config.key_file)?;
        let node_id = keypair.node_id();
        let bind = config
            .bind
            .parse::<SocketAddr>()
            .with_context(|| format!("invalid bind address {}", config.bind))?;

        let mut endpoint_config = ZapEndpointConfig::new(bind, node_id);
        if let Some(max_datagram_size) = config.max_datagram_size {
            endpoint_config.max_datagram_size = max_datagram_size;
        }
        endpoint_config.inbound_nonce_cache_capacity = config.security.replay_cache_capacity;

        let mut public_keys = HashMap::new();
        let mut peer_trust = HashMap::new();
        let mut peer_ids = Vec::with_capacity(config.peers.len());
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

        let runtime = WasmExecutor::new()?;
        let registry = load_driver_registry_optional(&config.registry)?;
        let drivers = load_drivers(&runtime, &config.drivers, registry.as_ref())?;
        let route_table = RouteTable::new(config.routes.clone())?;
        let capability_advertisement = describe_capabilities(&config)?;
        let message_contracts = load_message_contract_set(&config.message_schema)?;
        let endpoint = ZapEndpoint::bind(endpoint_config).await?;

        Ok(Self {
            endpoint,
            keypair,
            public_keys,
            peer_trust,
            drivers,
            runtime,
            limits: config.runtime.to_limits(),
            require_signed: config.require_signed,
            replay_guard: Mutex::new(ReplayGuard::new(config.security.replay_cache_capacity)),
            security: config.security,
            poa_validators: poa_verifier.validators,
            poa_required_threshold: poa_verifier.required_threshold,
            poa_validator_set_path,
            poa_validator_set_authority,
            receipt_log_path: config.receipts.path,
            memory: config.memory,
            route_table,
            message_policy: config.message_policy,
            message_contracts,
            peer_ids,
            capability_advertisement,
        })
    }

    pub fn local_addr(&self) -> Result<SocketAddr> {
        Ok(self.endpoint.local_addr()?)
    }

    pub async fn run_forever(&self) -> Result<()> {
        info!(node_id = %self.endpoint.node_id(), addr = %self.local_addr()?, "ZAP node running");
        loop {
            match self.handle_once().await {
                Ok(event) => {
                    info!(
                        source = %event.source,
                        kind = %event.kind,
                        subject = %event.subject,
                        output_bytes = event.output.as_ref().map(|bytes| bytes.len()).unwrap_or(0),
                        "processed ZAP message"
                    );
                }
                Err(error) => warn!(%error, "failed to process inbound ZAP frame"),
            }
        }
    }

    pub async fn handle_once(&self) -> Result<NodeEvent> {
        let inbound = self.endpoint.recv().await?;
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
            .contains(zap_core::ZapFlags::REQUIRES_CONSENSUS)
        {
            self.verify_consensus(&inbound.frame)?;
        }
        self.validate_fresh_frame(&inbound.frame)
            .context("inbound frame failed anti-replay validation")?;

        let message = parse_inbound_message(&inbound.frame.payload)?;
        validate_inbound_message(&message)?;
        self.validate_message_contracts(&message)?;
        self.apply_message_policy(&inbound.frame, &message)?;
        let output = if message.kind == ZapMessageKind::Control
            && message.subject == POA_ATTESTATION_REQUEST_SUBJECT
        {
            self.respond_to_poa_attestation_request(inbound.peer.node_id, &message.body)
                .await?;
            None
        } else if message.kind == ZapMessageKind::Control
            && message.subject == CAPABILITY_QUERY_SUBJECT
        {
            self.respond_to_capability_query(inbound.peer.node_id, &message.body)
                .await?;
            None
        } else if message.kind == ZapMessageKind::Control
            && message.subject == RECEIPT_REPLICATION_REQUEST_SUBJECT
        {
            self.respond_to_receipt_replication_request(inbound.peer.node_id, &message.body)
                .await?;
            None
        } else if message.kind == ZapMessageKind::Control
            && message.subject == POA_VALIDATOR_SET_REQUEST_SUBJECT
        {
            self.respond_to_poa_validator_set_request(inbound.peer.node_id, &message.body)
                .await?;
            None
        } else {
            self.route_message(&inbound, &message).await?
        };
        self.write_receipt(
            &inbound.frame,
            message.kind,
            &message.subject,
            output.as_deref(),
        )?;

        Ok(NodeEvent {
            source: inbound.peer.node_id,
            kind: message.kind,
            subject: message.subject.clone(),
            action: message.subject,
            output,
        })
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
        let envelope = ZapEnvelope::new(
            ZapMessageKind::Control,
            POA_ATTESTATION_RESPONSE_SUBJECT,
            POA_ATTESTATION_CONTENT_TYPE,
            Bytes::from(serde_json::to_vec(&response)?),
        )?;
        let frame = ZapFrame::new(
            self.keypair.node_id(),
            requester_node,
            ZapFlags::ENCRYPTED,
            envelope.encode(),
        )?;
        let frame = sign_frame(&self.keypair, &frame)?;
        self.endpoint.send_frame(requester_node, &frame).await?;
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
        let envelope = ZapEnvelope::new(
            ZapMessageKind::Control,
            CAPABILITY_RESPONSE_SUBJECT,
            CAPABILITY_CONTENT_TYPE,
            Bytes::from(serde_json::to_vec(&response)?),
        )?;
        let frame = ZapFrame::new(
            self.keypair.node_id(),
            requester_node,
            ZapFlags::ENCRYPTED,
            envelope.encode(),
        )?;
        let frame = sign_frame(&self.keypair, &frame)?;
        self.endpoint.send_frame(requester_node, &frame).await?;
        Ok(())
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
        let mut receipts = match &self.receipt_log_path {
            Some(path) => load_verified_receipt_log(path)?,
            None => Vec::new(),
        };
        receipts.retain(|receipt| request.matches(receipt));
        let truncated = receipts.len() > limit;
        receipts.truncate(limit);
        let response = ReceiptReplicationResponse::new(self.keypair.node_id(), receipts, truncated);
        let envelope = ZapEnvelope::new(
            ZapMessageKind::Control,
            RECEIPT_REPLICATION_RESPONSE_SUBJECT,
            RECEIPT_REPLICATION_CONTENT_TYPE,
            Bytes::from(serde_json::to_vec(&response)?),
        )?;
        let frame = ZapFrame::new(
            self.keypair.node_id(),
            requester_node,
            ZapFlags::ENCRYPTED,
            envelope.encode(),
        )?;
        let frame = sign_frame(&self.keypair, &frame)?;
        self.endpoint.send_frame(requester_node, &frame).await?;
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
        let envelope = ZapEnvelope::new(
            ZapMessageKind::Control,
            POA_VALIDATOR_SET_RESPONSE_SUBJECT,
            POA_VALIDATOR_SET_CONTENT_TYPE,
            Bytes::from(serde_json::to_vec(&response)?),
        )?;
        let frame = ZapFrame::new(
            self.keypair.node_id(),
            requester_node,
            ZapFlags::ENCRYPTED,
            envelope.encode(),
        )?;
        let frame = sign_frame(&self.keypair, &frame)?;
        self.endpoint.send_frame(requester_node, &frame).await?;
        Ok(())
    }

    async fn route_message(
        &self,
        inbound: &zap_net::InboundZap,
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
        frame: &ZapFrame,
        message: &InboundMessage,
    ) -> Result<Option<Vec<u8>>> {
        let target = decision.target;
        if target.drop {
            info!(
                kind = %message.kind,
                subject = %message.subject,
                reason = %decision.reason,
                "dropped ZAP message by route"
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
            return self.dispatch_capability(capability, message, frame);
        }
        if let Some(action) = target.local_driver {
            return self.dispatch_local_driver(&action, message, frame);
        }
        Ok(None)
    }

    fn dispatch_capability(
        &self,
        capability: CapabilityId,
        message: &InboundMessage,
        frame: &ZapFrame,
    ) -> Result<Option<Vec<u8>>> {
        if let Some(action) = capability.driver_action() {
            return self.dispatch_local_driver(action, message, frame);
        }
        warn!(
            capability = %capability,
            "capability route has no local executor; message acknowledged only"
        );
        Ok(None)
    }

    fn dispatch_local_driver(
        &self,
        action: &str,
        message: &InboundMessage,
        frame: &ZapFrame,
    ) -> Result<Option<Vec<u8>>> {
        if message.kind != ZapMessageKind::Action {
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
                let mut limits = self.limits;
                limits.permissions = merge_permissions(limits.permissions, driver.permissions);
                let result = self
                    .runtime
                    .execute(&driver.driver, action, &message.body, limits)?;
                self.record_host_calls(action, message, frame, &result.host_calls)?;
                Ok(Some(result.output))
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
        frame: &ZapFrame,
        host_calls: &[HostCallRecord],
    ) -> Result<()> {
        for call in host_calls {
            match call.kind {
                HostCallKind::MemoryWrite => {
                    if !self.memory.allow_driver_write {
                        bail!(
                            "driver `{}` called zap.memory_write but memory.allow_driver_write=false",
                            action
                        );
                    }
                    let path = self.memory.path.as_ref().ok_or_else(|| {
                        anyhow!(
                            "driver `{}` called zap.memory_write but memory.path is not configured",
                            action
                        )
                    })?;
                    let mut store = JsonlMemoryStore::open(path);
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

    fn apply_message_policy(&self, frame: &ZapFrame, message: &InboundMessage) -> Result<()> {
        let policy = PolicySet::new(
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
            consensus_protected: frame.header.flags.contains(ZapFlags::REQUIRES_CONSENSUS),
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
        frame: &ZapFrame,
        message: &InboundMessage,
    ) -> Result<()> {
        self.ensure_peer_can_forward(peer)?;
        if frame.header.flags.contains(ZapFlags::REQUIRES_CONSENSUS) {
            bail!("route forwarding of consensus-protected frames is not supported in v1");
        }
        let forwarded = ZapFrame::new(
            self.keypair.node_id(),
            peer,
            ZapFlags::ENCRYPTED,
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

    fn validate_fresh_frame(&self, frame: &zap_core::ZapFrame) -> Result<()> {
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

    fn verify_consensus(&self, frame: &zap_core::ZapFrame) -> Result<()> {
        if self.poa_validators.is_empty() {
            bail!("frame requires Proof-of-Action, but no PoA validators are configured");
        }
        verify_poa_certificate(frame, &self.poa_validators, self.poa_required_threshold)
            .context("inbound frame failed Proof-of-Action validation")
    }

    fn write_receipt(
        &self,
        frame: &zap_core::ZapFrame,
        kind: ZapMessageKind,
        subject: &str,
        output: Option<&[u8]>,
    ) -> Result<()> {
        let Some(path) = &self.receipt_log_path else {
            return Ok(());
        };
        if let Some(parent) = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            fs::create_dir_all(parent).with_context(|| {
                format!("failed to create receipt directory {}", parent.display())
            })?;
        }
        let processed_at_micros = now_micros()?;
        let required_threshold = frame
            .header
            .flags
            .contains(zap_core::ZapFlags::REQUIRES_CONSENSUS)
            .then_some(self.poa_required_threshold);
        let receipt = SignedActionReceipt::new_message(
            &self.keypair,
            frame,
            kind.as_str(),
            subject,
            output,
            processed_at_micros,
            required_threshold,
        )?;
        let mut options = OpenOptions::new();
        options.create(true).append(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options
            .open(path)
            .with_context(|| format!("failed to open receipt log {}", path.display()))?;
        file.write_all(receipt.to_json_line()?.as_bytes())
            .with_context(|| format!("failed to write receipt log {}", path.display()))
    }
}

fn validate_config(config: &ZapNodeConfig) -> Result<ConfigValidationReport> {
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

    warn_key_file_permissions(&config.key_file, &mut warnings)?;
    validate_receipts(config)?;
    validate_memory(config)?;
    validate_capability_cache_config(config)?;
    validate_message_policy(config, &mut warnings)?;
    let message_contracts = load_message_contract_set(&config.message_schema)?;
    validate_runtime(config.runtime)?;
    let peer_trust_summary = validate_peers(config, bind, node_id, &mut warnings)?;
    let poa_summary = validate_poa(config, &mut warnings)?;
    let registry = load_driver_registry_optional(&config.registry)?;
    let registry_entry_count = registry
        .as_ref()
        .map(|registry| registry.entries.len())
        .unwrap_or(0);
    let signed_driver_count = validate_drivers(
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
        peer_count: config.peers.len(),
        trusted_peer_count: peer_trust_summary.trusted_peer_count,
        restricted_peer_count: peer_trust_summary.restricted_peer_count,
        peer_send_enabled_count: peer_trust_summary.peer_send_enabled_count,
        peer_receive_enabled_count: peer_trust_summary.peer_receive_enabled_count,
        peer_forward_enabled_count: peer_trust_summary.peer_forward_enabled_count,
        driver_count: config.drivers.len(),
        signed_driver_count,
        receipt_log_enabled: config.receipts.path.is_some(),
        registry_enabled: config.registry.path.is_some(),
        registry_entry_count,
        registry_signature_required: config.registry.require_signature,
        require_signed: config.require_signed,
        poa_validator_count: poa_summary.validator_count,
        poa_required_threshold: poa_summary.required_threshold,
        poa_validator_set_enabled: poa_summary.validator_set_enabled,
        poa_validator_set_epoch: poa_summary.validator_set_epoch,
        memory_enabled: config.memory.path.is_some(),
        route_count: config.routes.len(),
        capability_count,
        capability_grant_count: advertisement.grants.len(),
        capability_requirement_count: advertisement.requirements.len(),
        ungranted_capability_count,
        capability_cache_enabled: config.capability_cache.path.is_some(),
        message_policy_rule_count: config.message_policy.rules.len(),
        message_schema_contract_count: message_contracts.contracts.len(),
        message_schema_require_match: message_contracts.require_match,
        peer_grant_route_count,
        warnings,
    })
}

fn validate_runtime(runtime: RuntimeConfig) -> Result<()> {
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
    Ok(())
}

fn validate_receipts(config: &ZapNodeConfig) -> Result<()> {
    let Some(receipt_path) = &config.receipts.path else {
        return Ok(());
    };
    if receipt_path == &config.key_file {
        bail!(
            "receipts.path must not point at key_file {}",
            config.key_file.display()
        );
    }
    if let Some(registry_path) = &config.registry.path
        && receipt_path == registry_path
    {
        bail!(
            "receipts.path must not point at registry.path {}",
            registry_path.display()
        );
    }
    for driver in &config.drivers {
        if receipt_path == &driver.path {
            bail!(
                "receipts.path must not point at driver `{}` path {}",
                driver.action,
                driver.path.display()
            );
        }
        if let Some(manifest_path) = &driver.manifest
            && receipt_path == manifest_path
        {
            bail!(
                "receipts.path must not point at driver `{}` manifest {}",
                driver.action,
                manifest_path.display()
            );
        }
    }
    Ok(())
}

fn validate_memory(config: &ZapNodeConfig) -> Result<()> {
    if matches!(config.memory.max_record_bytes, Some(0)) {
        bail!("memory.max_record_bytes must be greater than zero");
    }
    if (config.memory.allow_driver_read || config.memory.allow_driver_write)
        && config.memory.path.is_none()
    {
        bail!("memory driver access requires memory.path");
    }
    if let Some(memory_path) = &config.memory.path {
        if memory_path == &config.key_file {
            bail!(
                "memory.path must not point at key_file {}",
                config.key_file.display()
            );
        }
        if let Some(receipt_path) = &config.receipts.path
            && memory_path == receipt_path
        {
            bail!(
                "memory.path must not point at receipts.path {}",
                receipt_path.display()
            );
        }
        if let Some(registry_path) = &config.registry.path
            && memory_path == registry_path
        {
            bail!(
                "memory.path must not point at registry.path {}",
                registry_path.display()
            );
        }
        for driver in &config.drivers {
            if memory_path == &driver.path {
                bail!(
                    "memory.path must not point at driver `{}` path {}",
                    driver.action,
                    driver.path.display()
                );
            }
            if let Some(manifest_path) = &driver.manifest
                && memory_path == manifest_path
            {
                bail!(
                    "memory.path must not point at driver `{}` manifest {}",
                    driver.action,
                    manifest_path.display()
                );
            }
        }
    }
    Ok(())
}

fn validate_capability_cache_config(config: &ZapNodeConfig) -> Result<()> {
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
    if let Some(receipt_path) = &config.receipts.path
        && cache_path == receipt_path
    {
        bail!(
            "capability_cache.path must not point at receipts.path {}",
            receipt_path.display()
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
    if let Some(memory_path) = &config.memory.path
        && cache_path == memory_path
    {
        bail!(
            "capability_cache.path must not point at memory.path {}",
            memory_path.display()
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

fn validate_message_policy(config: &ZapNodeConfig, warnings: &mut Vec<String>) -> Result<()> {
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
            kind.parse::<ZapMessageKind>()
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

fn validate_routes(config: &ZapNodeConfig, warnings: &mut Vec<String>) -> Result<usize> {
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
    config: &ZapNodeConfig,
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
    config: &ZapNodeConfig,
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
    config: &ZapNodeConfig,
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
    config: &ZapNodeConfig,
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
    config: &ZapNodeConfig,
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

pub fn describe_capabilities(config: &ZapNodeConfig) -> Result<CapabilityAdvertisement> {
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

    if config.memory.path.is_some() {
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
    drivers: &[DriverConfig],
    runtime: RuntimeConfig,
    memory: &MemoryConfig,
    registry: Option<&DriverRegistry>,
    warnings: &mut Vec<String>,
) -> Result<usize> {
    let mut actions = HashSet::new();
    let executor = WasmExecutor::new()?;
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
            .compile_and_validate(&wasm)
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
                "driver `{}` has no signed ZapStore manifest; provenance validation disabled",
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
            .compile_and_validate(&wasm)
            .with_context(|| format!("invalid driver ABI {}", driver.path.display()))?;
        compiled.insert(
            driver.action.clone(),
            DriverRegistration {
                driver: wasm_driver,
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

fn hash_frame(frame: &ZapFrame) -> String {
    format!("blake3:{}", blake3::hash(&frame.encode()).to_hex())
}

fn load_verified_receipt_log(path: &Path) -> Result<Vec<SignedActionReceipt>> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let input = fs::read_to_string(path)
        .with_context(|| format!("failed to read receipt log {}", path.display()))?;
    let mut receipts = Vec::new();
    for (index, line) in input.lines().enumerate() {
        let line_number = index + 1;
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let receipt = SignedActionReceipt::from_json_str(line).with_context(|| {
            format!(
                "failed to parse receipt at {} line {}",
                path.display(),
                line_number
            )
        })?;
        receipt.verify().with_context(|| {
            format!(
                "invalid receipt signature at {} line {}",
                path.display(),
                line_number
            )
        })?;
        receipts.push(receipt);
    }
    Ok(receipts)
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
    if permissions.memory_read && !(memory.path.is_some() && memory.allow_driver_read) {
        bail!(
            "driver `{}` requests memory_read permission, but memory.path and memory.allow_driver_read=true are required",
            driver.action
        );
    }
    if permissions.memory_write && !(memory.path.is_some() && memory.allow_driver_write) {
        bail!(
            "driver `{}` requests memory_write permission, but memory.path and memory.allow_driver_write=true are required",
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
    seen: HashSet<[u8; 16]>,
    order: VecDeque<[u8; 16]>,
}

impl ReplayGuard {
    fn new(capacity: usize) -> Self {
        Self {
            capacity,
            seen: HashSet::with_capacity(capacity.min(4096)),
            order: VecDeque::with_capacity(capacity.min(4096)),
        }
    }

    fn remember(&mut self, frame: &zap_core::ZapFrame) -> Result<()> {
        if self.capacity == 0 {
            return Ok(());
        }

        let fingerprint = frame_fingerprint(frame);
        if self.seen.contains(&fingerprint) {
            bail!(
                "replayed frame rejected: source_node={}, timestamp_micros={}, signature_hint={}",
                frame.header.source_node,
                frame.header.timestamp_micros,
                hex_hint(frame.header.zap_sign)
            );
        }

        self.seen.insert(fingerprint);
        self.order.push_back(fingerprint);
        while self.order.len() > self.capacity {
            if let Some(expired) = self.order.pop_front() {
                self.seen.remove(&expired);
            }
        }
        Ok(())
    }
}

fn frame_fingerprint(frame: &zap_core::ZapFrame) -> [u8; 16] {
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
    use zap_core::{ZapFlags, ZapFrame, now_micros};
    use zap_crypto::{
        POA_VALIDATOR_SET_SCHEMA_VERSION, PoaValidatorDescriptor, PoaValidatorSet, certify_frame,
        sign_frame, sign_poa_validator_set, verify_frame,
    };
    use zap_envelope::{ZapEnvelope, ZapMessageKind};
    use zap_ledger::SignedActionReceipt;
    use zap_memory::MemoryQuery;
    use zap_net::{Peer, ZapEndpoint, ZapEndpointConfig};
    use zap_router::{RouteMatch, RouteTarget};
    use zap_store::{DriverManifest, DriverRegistry, DriverRegistryStatus};

    fn public_key_string(keypair: &Keypair) -> String {
        base64::Engine::encode(&STANDARD_NO_PAD, keypair.verifying_key().to_bytes())
    }

    fn echo_driver_wat() -> &'static str {
        r#"
        (module
          (memory (export "memory") 1)
          (global $heap (mut i32) (i32.const 1024))
          (func (export "zap_alloc") (param $len i32) (result i32)
            global.get $heap
            global.get $heap
            local.get $len
            i32.add
            global.set $heap)
          (func (export "zap_dealloc") (param i32 i32))
          (func (export "zap_execute")
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
          (func (export "zap_alloc") (param i32) (result i32) i32.const 0)
          (func (export "zap_dealloc") (param i32 i32)))
        "#
    }

    fn memory_write_driver_wat() -> &'static str {
        r#"
        (module
          (import "zap" "memory_write" (func $memory_write (param i32 i32) (result i32)))
          (memory (export "memory") 1)
          (global $heap (mut i32) (i32.const 4096))
          (data (i32.const 1024) "machine-note")
          (func (export "zap_alloc") (param $len i32) (result i32)
            global.get $heap
            global.get $heap
            local.get $len
            i32.add
            global.set $heap)
          (func (export "zap_dealloc") (param i32 i32))
          (func (export "zap_execute")
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

    fn zenv_payload(kind: ZapMessageKind, subject: &str, body: &[u8]) -> Vec<u8> {
        ZapEnvelope::new(
            kind,
            subject,
            "application/octet-stream",
            Bytes::copy_from_slice(body),
        )
        .unwrap()
        .with_metadata(Bytes::from_static(b"fixture=zap-node"))
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
        assert_eq!(message.kind, ZapMessageKind::Data);
        assert_eq!(message.subject, "");
        assert_eq!(message.body, b"\x00raw payload");
    }

    #[test]
    fn parses_config() {
        let toml = r#"
            bind = "127.0.0.1:7000"
            key_file = ".zap/node.key"
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
        "#;

        let config = ZapNodeConfig::from_toml_str(toml).unwrap();
        assert_eq!(config.bind, "127.0.0.1:7000");
        assert_eq!(config.peers.len(), 1);
        assert_eq!(config.drivers[0].action, "echo");
        assert_eq!(config.runtime.fuel, Some(100_000));
    }

    struct NodeHarness {
        _temp: tempfile::TempDir,
        driver_path: PathBuf,
        node: ZapNode,
        sender_endpoint: ZapEndpoint,
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
        let temp = tempfile::tempdir().unwrap();
        let receiver_key = Keypair::generate();
        let sender_key = Keypair::generate();
        let receiver_key_path = temp.path().join("receiver.key");
        let driver_path = temp.path().join("echo.wat");
        std::fs::write(&receiver_key_path, receiver_key.to_key_file_toml().unwrap()).unwrap();
        std::fs::write(&driver_path, echo_driver_wat()).unwrap();

        let sender_endpoint = ZapEndpoint::bind(ZapEndpointConfig::new(
            "127.0.0.1:0".parse().unwrap(),
            sender_key.node_id(),
        ))
        .await
        .unwrap();
        let transport_key = [0x55_u8; 32];
        let config = ZapNodeConfig {
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
            runtime: RuntimeConfig::default(),
            security,
            trust: TrustConfig::default(),
            poa,
            receipts: ReceiptsConfig::default(),
            registry: RegistryConfig::default(),
            memory: MemoryConfig::default(),
            capability_policy: CapabilityPolicyConfig::default(),
            capability_cache: CapabilityCacheConfig::default(),
            message_policy,
            message_schema,
            routes: Vec::new(),
        };
        let node = ZapNode::from_config(config).await.unwrap();
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
    ) -> ZapNodeConfig {
        let key_path = temp.path().join("node.key");
        let driver_path = temp.path().join("echo.wat");
        std::fs::write(&key_path, local_key.to_key_file_toml().unwrap()).unwrap();
        std::fs::write(&driver_path, echo_driver_wat()).unwrap();
        ZapNodeConfig {
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
            security: SecurityConfig::default(),
            trust: TrustConfig::default(),
            poa: PoaConfig::default(),
            receipts: ReceiptsConfig::default(),
            registry: RegistryConfig::default(),
            memory: MemoryConfig::default(),
            capability_policy: CapabilityPolicyConfig::default(),
            capability_cache: CapabilityCacheConfig::default(),
            message_policy: MessagePolicyConfig::default(),
            message_schema: MessageSchemaConfig::default(),
            routes: Vec::new(),
        }
    }

    fn signed_echo_frame(harness: &NodeHarness, timestamp_micros: u64) -> ZapFrame {
        let envelope = ActionEnvelope::new("echo", "hello-node");
        let unsigned = ZapFrame::with_timestamp(
            harness.sender_key.node_id(),
            harness.receiver_key.node_id(),
            ZapFlags::ENCRYPTED,
            timestamp_micros,
            Bytes::from(envelope.to_payload_bytes().unwrap()),
        )
        .unwrap();
        sign_frame(&harness.sender_key, &unsigned).unwrap()
    }

    fn signed_zenv_frame(
        harness: &NodeHarness,
        kind: ZapMessageKind,
        subject: &str,
        body: &[u8],
        timestamp_micros: u64,
    ) -> ZapFrame {
        let unsigned = ZapFrame::with_timestamp(
            harness.sender_key.node_id(),
            harness.receiver_key.node_id(),
            ZapFlags::ENCRYPTED,
            timestamp_micros,
            Bytes::from(zenv_payload(kind, subject, body)),
        )
        .unwrap();
        sign_frame(&harness.sender_key, &unsigned).unwrap()
    }

    fn signed_consensus_echo_frame(harness: &NodeHarness, timestamp_micros: u64) -> ZapFrame {
        let envelope = ActionEnvelope::new("echo", "critical-node");
        let unsigned = ZapFrame::with_timestamp(
            harness.sender_key.node_id(),
            harness.receiver_key.node_id(),
            ZapFlags::ENCRYPTED | ZapFlags::REQUIRES_CONSENSUS,
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
        config.memory.path = Some(temp.path().join("memory.jsonl"));
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
        assert_eq!(report.message_policy_rule_count, 1);
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
        config.memory.path = Some(temp.path().join("memory.jsonl"));
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
        config.memory.path = Some(temp.path().join("memory.jsonl"));
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
        let receipt_path = config_dir.join("logs").join("receipts.jsonl");
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
path = "logs/receipts.jsonl"
"#,
                peer.node_id(),
                public_key_string(&peer),
            ),
        )
        .unwrap();

        let config = ZapNodeConfig::from_path(&config_path).unwrap();
        assert_eq!(config.key_file, key_path);
        assert_eq!(config.drivers[0].path, driver_path);
        assert_eq!(
            config.drivers[0].manifest.as_deref(),
            Some(manifest_path.as_path())
        );
        assert_eq!(
            config.receipts.path.as_deref(),
            Some(receipt_path.as_path())
        );
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
                .any(|warning| warning.contains("no signed ZapStore manifest"))
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
        config.receipts.path = Some(config.key_file.clone());

        let error = config.validate().unwrap_err();
        assert!(format!("{error:#}").contains("receipts.path must not point at key_file"));
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
        assert!(format!("{error:#}").contains("missing required export `zap_execute`"));
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
        assert_eq!(event.kind, ZapMessageKind::Action);
        assert_eq!(event.subject, "echo");
        assert_eq!(event.action, "echo");
        assert_eq!(event.output.as_deref(), Some(b"hello-node".as_slice()));
    }

    #[tokio::test]
    async fn node_handles_zenv_action_and_executes_wasm_driver() {
        let harness = node_harness(SecurityConfig::default()).await;
        let signed = signed_zenv_frame(
            &harness,
            ZapMessageKind::Action,
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
        assert_eq!(event.kind, ZapMessageKind::Action);
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

        let sender_endpoint = ZapEndpoint::bind(ZapEndpointConfig::new(
            "127.0.0.1:0".parse().unwrap(),
            sender_key.node_id(),
        ))
        .await
        .unwrap();
        let transport_key = [0x61_u8; 32];
        let mut runtime = RuntimeConfig::default();
        runtime.permissions.memory_write = true;
        let config = ZapNodeConfig {
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
            security: SecurityConfig::default(),
            trust: TrustConfig::default(),
            poa: PoaConfig::default(),
            receipts: ReceiptsConfig::default(),
            registry: RegistryConfig::default(),
            memory: MemoryConfig {
                path: Some(memory_path.clone()),
                max_record_bytes: None,
                allow_driver_read: false,
                allow_driver_write: true,
            },
            capability_policy: CapabilityPolicyConfig::default(),
            capability_cache: CapabilityCacheConfig::default(),
            message_policy: MessagePolicyConfig::default(),
            message_schema: MessageSchemaConfig::default(),
            routes: Vec::new(),
        };
        let node = ZapNode::from_config(config).await.unwrap();
        sender_endpoint
            .add_peer(Peer::new(
                receiver_key.node_id(),
                node.local_addr().unwrap(),
                transport_key,
            ))
            .await;

        let payload = ZapEnvelope::action("machine.note", Bytes::from_static(b"driver-output"))
            .unwrap()
            .encode();
        let frame = ZapFrame::with_timestamp(
            sender_key.node_id(),
            receiver_key.node_id(),
            ZapFlags::ENCRYPTED,
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

        let store = JsonlMemoryStore::open(&memory_path);
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
        let envelope = ZapEnvelope::new(
            ZapMessageKind::Control,
            CAPABILITY_QUERY_SUBJECT,
            CAPABILITY_CONTENT_TYPE,
            Bytes::from(serde_json::to_vec(&query).unwrap()),
        )
        .unwrap();
        let unsigned = ZapFrame::with_timestamp(
            harness.sender_key.node_id(),
            harness.receiver_key.node_id(),
            ZapFlags::ENCRYPTED,
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
        assert_eq!(event.kind, ZapMessageKind::Control);
        assert_eq!(event.subject, CAPABILITY_QUERY_SUBJECT);

        let response = timeout(Duration::from_secs(2), harness.sender_endpoint.recv())
            .await
            .unwrap()
            .unwrap();
        verify_frame(&harness.receiver_key.verifying_key(), &response.frame).unwrap();
        let response_envelope = ZapEnvelopeRef::parse(&response.frame.payload).unwrap();
        assert_eq!(response_envelope.kind(), ZapMessageKind::Control);
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
    async fn node_routes_action_to_configured_peer() {
        let temp = tempfile::tempdir().unwrap();
        let router_key = Keypair::generate();
        let sender_key = Keypair::generate();
        let target_key = Keypair::generate();
        let router_key_path = temp.path().join("router.key");
        std::fs::write(&router_key_path, router_key.to_key_file_toml().unwrap()).unwrap();

        let sender_endpoint = ZapEndpoint::bind(ZapEndpointConfig::new(
            "127.0.0.1:0".parse().unwrap(),
            sender_key.node_id(),
        ))
        .await
        .unwrap();
        let target_endpoint = ZapEndpoint::bind(ZapEndpointConfig::new(
            "127.0.0.1:0".parse().unwrap(),
            target_key.node_id(),
        ))
        .await
        .unwrap();
        let sender_transport_key = [0x31_u8; 32];
        let target_transport_key = [0x32_u8; 32];
        let config = ZapNodeConfig {
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
            security: SecurityConfig::default(),
            trust: TrustConfig::default(),
            poa: PoaConfig::default(),
            receipts: ReceiptsConfig::default(),
            registry: RegistryConfig::default(),
            memory: MemoryConfig::default(),
            capability_policy: CapabilityPolicyConfig::default(),
            capability_cache: CapabilityCacheConfig::default(),
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
        };
        let router = ZapNode::from_config(config).await.unwrap();
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

        let payload = zenv_payload(ZapMessageKind::Action, "echo", b"forward-me");
        let unsigned = ZapFrame::with_timestamp(
            sender_key.node_id(),
            router_key.node_id(),
            ZapFlags::ENCRYPTED,
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
        assert_eq!(event.kind, ZapMessageKind::Action);
        assert_eq!(event.output, None);

        let forwarded = timeout(Duration::from_secs(2), target_endpoint.recv())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(forwarded.peer.node_id, router_key.node_id());
        verify_frame(&router_key.verifying_key(), &forwarded.frame).unwrap();
        let envelope = ZapEnvelopeRef::parse(&forwarded.frame.payload).unwrap();
        assert_eq!(envelope.kind(), ZapMessageKind::Action);
        assert_eq!(envelope.subject(), "echo");
        assert_eq!(envelope.body(), b"forward-me");
    }

    #[tokio::test]
    async fn node_accepts_zenv_event_without_wasm_dispatch() {
        let mut harness = node_harness(SecurityConfig::default()).await;
        let receipt_path = harness._temp.path().join("receipts").join("events.jsonl");
        harness.node.receipt_log_path = Some(receipt_path.clone());
        let signed = signed_zenv_frame(
            &harness,
            ZapMessageKind::Event,
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
        assert_eq!(event.kind, ZapMessageKind::Event);
        assert_eq!(event.subject, "echo");
        assert_eq!(event.action, "echo");
        assert_eq!(event.output, None);

        let lines = std::fs::read_to_string(&receipt_path).unwrap();
        let receipt = SignedActionReceipt::from_json_str(lines.trim()).unwrap();
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
            ZapMessageKind::Action,
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
        let receipt_path = harness._temp.path().join("receipts").join("actions.jsonl");
        harness.node.receipt_log_path = Some(receipt_path.clone());
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

        let lines = std::fs::read_to_string(&receipt_path).unwrap();
        let receipt = SignedActionReceipt::from_json_str(lines.trim()).unwrap();
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
            },
        )
        .await;
        let signed = signed_zenv_frame(
            &harness,
            ZapMessageKind::Action,
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
            },
        )
        .await;
        let signed = signed_zenv_frame(
            &harness,
            ZapMessageKind::Action,
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

        let sender_endpoint = ZapEndpoint::bind(ZapEndpointConfig::new(
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
        let config = ZapNodeConfig {
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
            security: SecurityConfig::default(),
            trust: TrustConfig::default(),
            poa: PoaConfig::default(),
            receipts: ReceiptsConfig::default(),
            registry: RegistryConfig::default(),
            memory: MemoryConfig::default(),
            capability_policy: CapabilityPolicyConfig::default(),
            capability_cache: CapabilityCacheConfig::default(),
            message_policy: MessagePolicyConfig::default(),
            message_schema: MessageSchemaConfig::default(),
            routes: Vec::new(),
        };
        let node = ZapNode::from_config(config).await.unwrap();
        sender_endpoint
            .add_peer(Peer::new(
                receiver_key.node_id(),
                node.local_addr().unwrap(),
                transport_key,
            ))
            .await;

        let frame = ZapFrame::new(
            sender_key.node_id(),
            receiver_key.node_id(),
            ZapFlags::ENCRYPTED,
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
