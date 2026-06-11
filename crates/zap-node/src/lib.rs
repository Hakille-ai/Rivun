//! ZAP node daemon.
//!
//! The node combines static peer discovery, encrypted UDP transport, optional
//! Ed25519 frame verification, and WASM action dispatch.

use anyhow::{Context, Result, anyhow, bail};
use base64::{Engine as _, engine::general_purpose::STANDARD_NO_PAD};
use bytes::Bytes;
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet, VecDeque},
    fs,
    fs::OpenOptions,
    io::Write,
    net::SocketAddr,
    path::{Path, PathBuf},
    sync::Mutex,
};
use tracing::{info, warn};
use uuid::Uuid;
use zap_core::{ZapFlags, ZapFrame, now_micros};
use zap_crypto::{
    Keypair, POA_ATTESTATION_CONTENT_TYPE, POA_ATTESTATION_REQUEST_SUBJECT,
    POA_ATTESTATION_RESPONSE_SUBJECT, PublicKey, sign_frame, sign_poa_attestation_request,
    verify_frame, verify_poa_certificate,
};
use zap_envelope::{
    MAGIC_BYTES as ZENV_MAGIC_BYTES, MAX_CONTENT_TYPE_LEN as ZENV_MAX_CONTENT_TYPE_LEN,
    MAX_METADATA_LEN as ZENV_MAX_METADATA_LEN, MAX_SUBJECT_LEN as ZENV_MAX_SUBJECT_LEN,
    ZapEnvelope, ZapEnvelopeRef, ZapMessageKind,
};
use zap_ledger::SignedActionReceipt;
use zap_net::{Peer, TransportKey, ZapEndpoint, ZapEndpointConfig};
use zap_runtime::{DriverPermissions, ExecutionLimits, WasmDriver, WasmExecutor};
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
    pub poa: PoaConfig,
    #[serde(default)]
    pub receipts: ReceiptsConfig,
    #[serde(default)]
    pub registry: RegistryConfig,
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
}

impl Default for PoaConfig {
    fn default() -> Self {
        Self {
            required_threshold: default_poa_required_threshold(),
            validators: Vec::new(),
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
    pub driver_count: usize,
    pub signed_driver_count: usize,
    pub receipt_log_enabled: bool,
    pub registry_enabled: bool,
    pub registry_entry_count: usize,
    pub require_signed: bool,
    pub poa_validator_count: usize,
    pub poa_required_threshold: u16,
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
    drivers: HashMap<String, DriverRegistration>,
    runtime: WasmExecutor,
    limits: ExecutionLimits,
    require_signed: bool,
    replay_guard: Mutex<ReplayGuard>,
    security: SecurityConfig,
    poa_validators: Vec<(Uuid, PublicKey)>,
    poa_required_threshold: u16,
    receipt_log_path: Option<PathBuf>,
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
        for peer in &config.peers {
            let peer_addr = peer
                .addr
                .parse::<SocketAddr>()
                .with_context(|| format!("invalid peer address {}", peer.addr))?;
            let transport_key = TransportKey::from_hex(&peer.transport_key)?;
            endpoint_config.peers.push(Peer {
                node_id: peer.node_id,
                addr: peer_addr,
                transport_key,
            });
            public_keys.insert(peer.node_id, decode_public_key(&peer.public_key)?);
        }
        let poa_validators = load_poa_validators(&config.poa)?;

        let runtime = WasmExecutor::new()?;
        let registry = load_driver_registry_optional(config.registry.path.as_deref())?;
        let drivers = load_drivers(&runtime, &config.drivers, registry.as_ref())?;
        let endpoint = ZapEndpoint::bind(endpoint_config).await?;

        Ok(Self {
            endpoint,
            keypair,
            public_keys,
            drivers,
            runtime,
            limits: config.runtime.to_limits(),
            require_signed: config.require_signed,
            replay_guard: Mutex::new(ReplayGuard::new(config.security.replay_cache_capacity)),
            security: config.security,
            poa_validators,
            poa_required_threshold: config.poa.required_threshold,
            receipt_log_path: config.receipts.path,
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
        let output = if message.kind == ZapMessageKind::Control
            && message.subject == POA_ATTESTATION_REQUEST_SUBJECT
        {
            self.respond_to_poa_attestation_request(inbound.peer.node_id, &message.body)
                .await?;
            None
        } else {
            match message.kind {
                ZapMessageKind::Action => match self.drivers.get(&message.subject) {
                    Some(driver) => {
                        let mut limits = self.limits;
                        limits.permissions =
                            merge_permissions(limits.permissions, driver.permissions);
                        let result = self.runtime.execute(
                            &driver.driver,
                            &message.subject,
                            &message.body,
                            limits,
                        )?;
                        Some(result.output)
                    }
                    None => {
                        warn!(
                            action = %message.subject,
                            "no WASM driver registered; action acknowledged only"
                        );
                        None
                    }
                },
                _ => {
                    info!(
                        kind = %message.kind,
                        subject = %message.subject,
                        "accepted non-action ZAP message without WASM dispatch"
                    );
                    None
                }
            }
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
    validate_runtime(config.runtime)?;
    validate_peers(config, bind, node_id, &mut warnings)?;
    validate_poa(config, &mut warnings)?;
    let registry = load_driver_registry_optional(config.registry.path.as_deref())?;
    let registry_entry_count = registry
        .as_ref()
        .map(|registry| registry.entries.len())
        .unwrap_or(0);
    let signed_driver_count = validate_drivers(
        &config.drivers,
        config.runtime,
        registry.as_ref(),
        &mut warnings,
    )?;

    Ok(ConfigValidationReport {
        bind,
        node_id,
        peer_count: config.peers.len(),
        driver_count: config.drivers.len(),
        signed_driver_count,
        receipt_log_enabled: config.receipts.path.is_some(),
        registry_enabled: config.registry.path.is_some(),
        registry_entry_count,
        require_signed: config.require_signed,
        poa_validator_count: config.poa.validators.len(),
        poa_required_threshold: config.poa.required_threshold,
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

fn validate_peers(
    config: &ZapNodeConfig,
    bind: SocketAddr,
    local_node_id: Uuid,
    warnings: &mut Vec<String>,
) -> Result<()> {
    let mut peer_ids = HashSet::new();
    let mut peer_addrs = HashSet::new();
    let mut transport_keys: HashMap<[u8; 32], Uuid> = HashMap::new();
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
    }
    Ok(())
}

fn validate_poa(config: &ZapNodeConfig, warnings: &mut Vec<String>) -> Result<()> {
    if config.poa.required_threshold == 0 {
        bail!("poa.required_threshold must be greater than zero");
    }
    if config.poa.validators.is_empty() {
        warnings.push(
            "no PoA validators configured; frames marked REQUIRES_CONSENSUS will be rejected"
                .to_string(),
        );
        return Ok(());
    }
    if config.poa.required_threshold as usize > config.poa.validators.len() {
        bail!(
            "poa.required_threshold {} exceeds configured validator count {}",
            config.poa.required_threshold,
            config.poa.validators.len()
        );
    }

    let mut validator_ids = HashSet::new();
    for validator in &config.poa.validators {
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
    }
    Ok(())
}

fn load_poa_validators(config: &PoaConfig) -> Result<Vec<(Uuid, PublicKey)>> {
    config
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
        .collect()
}

fn validate_drivers(
    drivers: &[DriverConfig],
    runtime: RuntimeConfig,
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
        validate_effective_driver_permissions(driver, runtime, manifest_permissions)?;
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

fn load_driver_registry_optional(path: Option<&Path>) -> Result<Option<DriverRegistry>> {
    let Some(path) = path else {
        return Ok(None);
    };
    let input = fs::read_to_string(path)
        .with_context(|| format!("failed to read driver registry {}", path.display()))?;
    let registry = DriverRegistry::from_toml_str(&input)
        .with_context(|| format!("failed to parse driver registry {}", path.display()))?;
    registry
        .validate()
        .with_context(|| format!("invalid driver registry {}", path.display()))?;
    Ok(Some(registry))
}

fn validate_effective_driver_permissions(
    driver: &DriverConfig,
    runtime: RuntimeConfig,
    manifest_permissions: DriverPermissions,
) -> Result<()> {
    let permissions = merge_permissions(runtime.permissions, manifest_permissions);
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
    Ok(())
}

fn merge_permissions(a: DriverPermissions, b: DriverPermissions) -> DriverPermissions {
    DriverPermissions {
        network: a.network || b.network,
        filesystem: a.filesystem || b.filesystem,
        clock: a.clock || b.clock,
        environment: a.environment || b.environment,
    }
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
    use zap_crypto::{certify_frame, sign_frame};
    use zap_envelope::{ZapEnvelope, ZapMessageKind};
    use zap_ledger::SignedActionReceipt;
    use zap_net::{Peer, ZapEndpoint, ZapEndpointConfig};
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
            }],
            drivers: vec![DriverConfig {
                action: "echo".to_string(),
                path: driver_path.clone(),
                manifest: None,
            }],
            runtime: RuntimeConfig::default(),
            security,
            poa,
            receipts: ReceiptsConfig::default(),
            registry: RegistryConfig::default(),
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
            }],
            drivers: vec![DriverConfig {
                action: "echo".to_string(),
                path: driver_path,
                manifest: None,
            }],
            runtime: RuntimeConfig::default(),
            security: SecurityConfig::default(),
            poa: PoaConfig::default(),
            receipts: ReceiptsConfig::default(),
            registry: RegistryConfig::default(),
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
        };

        let error = config.validate().unwrap_err();
        assert!(format!("{error:#}").contains("poa.required_threshold 2 exceeds"));
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
