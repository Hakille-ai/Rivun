//! Hardware-neutral machine connection primitives for ZAP.
//!
//! The crate models device profiles, capability mapping, health/state, commands,
//! and protocol adapters. Mock adapters remain deterministic for tests, while
//! stream-backed adapters can use real TCP sockets or caller-supplied serial
//! streams.

use serde::{Deserialize, Deserializer, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    fmt,
    io::{Read, Write},
    net::TcpStream,
    str::FromStr,
    time::Duration,
};
use thiserror::Error;
use zap_capability::{CapabilityId, CapabilitySet};

pub const DEVICE_PROFILE_SCHEMA_VERSION: u8 = 1;
pub const MACHINE_HEALTH_CAPABILITY_PREFIX: &str = "machine.health:";
pub const MACHINE_STATE_CAPABILITY_PREFIX: &str = "machine.state:";
const MAX_MACHINE_ID_BYTES: usize = 128;

#[derive(Debug, Error)]
pub enum ZapMachineError {
    #[error("machine id must not be empty")]
    EmptyMachineId,
    #[error("machine id `{0}` exceeds maximum length of 128 bytes")]
    MachineIdTooLong(String),
    #[error("machine id `{0}` contains invalid characters")]
    InvalidMachineId(String),
    #[error("device profile `{profile}` uses unsupported schema version {version}")]
    UnsupportedProfileVersion { profile: String, version: u8 },
    #[error("device profile `{0}` display name must not be empty")]
    EmptyDisplayName(String),
    #[error("device profile `{0}` must declare at least one capability")]
    EmptyCapabilities(String),
    #[error("device profile `{profile}` declares duplicate command `{command}`")]
    DuplicateCommand { profile: String, command: String },
    #[error("device profile `{profile}` has an invalid adapter transport/protocol shape")]
    InvalidAdapterShape { profile: String },
    #[error("command `{command}` is not declared by profile `{profile}`")]
    CommandNotDeclared { profile: String, command: String },
    #[error("payload for command `{command}` is {actual} bytes, above the limit of {max}")]
    PayloadTooLarge {
        command: String,
        max: u32,
        actual: usize,
    },
    #[error("payload for command `{command}` does not match schema `{schema}`")]
    PayloadSchemaMismatch {
        command: String,
        schema: CommandPayloadSchema,
    },
    #[error("adapter `{adapter}` does not match profile `{profile}` adapter `{expected}`")]
    AdapterMismatch {
        profile: String,
        expected: AdapterKind,
        adapter: AdapterKind,
    },
    #[error("{adapter} adapter is not open")]
    AdapterNotOpen { adapter: AdapterKind },
    #[error("device `{0}` is already attached to the machine bus")]
    DuplicateDevice(String),
    #[error("device `{0}` is not attached to the machine bus")]
    UnknownDevice(String),
    #[error("protocol command `{0}` is not scripted by this adapter")]
    UnknownProtocolCommand(String),
    #[error("machine `{0}` has no heartbeat command configured")]
    MissingHeartbeatCommand(String),
    #[error("I/O error: {0}")]
    Io(String),
    #[error("protocol error: {0}")]
    Protocol(String),
    #[error("capability error: {0}")]
    Capability(#[from] zap_capability::ZapCapabilityError),
}

pub type Result<T> = std::result::Result<T, ZapMachineError>;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(transparent)]
pub struct MachineId(String);

impl MachineId {
    pub fn new(input: impl Into<String>) -> Result<Self> {
        let input = input.into();
        validate_machine_id(&input)?;
        Ok(Self(input))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl AsRef<str> for MachineId {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl fmt::Display for MachineId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl FromStr for MachineId {
    type Err = ZapMachineError;

    fn from_str(input: &str) -> Result<Self> {
        Self::new(input)
    }
}

impl<'de> Deserialize<'de> for MachineId {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let input = String::deserialize(deserializer)?;
        Self::new(input).map_err(serde::de::Error::custom)
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AdapterKind {
    Mock,
    Serial,
    Tcp,
    ModbusLike,
}

impl fmt::Display for AdapterKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Mock => "mock",
            Self::Serial => "serial",
            Self::Tcp => "tcp",
            Self::ModbusLike => "modbus_like",
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TransportProfile {
    Mock { channel: String },
    Serial { port: String, baud_rate: u32 },
    Tcp { host: String, port: u16 },
    IndustrialBus { bus_id: String, node_id: u8 },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ProtocolProfile {
    Mock,
    SerialLine { delimiter: String },
    TcpFrames { max_frame_bytes: u32 },
    ModbusLike { unit_id: u8 },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HealthPolicy {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub heartbeat_command: Option<String>,
    pub stale_after_ms: u64,
}

impl Default for HealthPolicy {
    fn default() -> Self {
        Self {
            heartbeat_command: None,
            stale_after_ms: 30_000,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DeviceProfile {
    pub schema_version: u8,
    pub profile_id: MachineId,
    pub display_name: String,
    pub adapter: AdapterKind,
    pub transport: TransportProfile,
    pub protocol: ProtocolProfile,
    #[serde(default)]
    pub health: HealthPolicy,
    #[serde(default)]
    pub capabilities: Vec<DeviceCapability>,
}

impl DeviceProfile {
    pub fn new(
        profile_id: impl Into<String>,
        display_name: impl Into<String>,
        adapter: AdapterKind,
        transport: TransportProfile,
        protocol: ProtocolProfile,
    ) -> Result<Self> {
        let profile = Self {
            schema_version: DEVICE_PROFILE_SCHEMA_VERSION,
            profile_id: MachineId::new(profile_id)?,
            display_name: display_name.into(),
            adapter,
            transport,
            protocol,
            health: HealthPolicy::default(),
            capabilities: Vec::new(),
        };
        profile.validate_shape()?;
        Ok(profile)
    }

    pub fn with_health_policy(mut self, health: HealthPolicy) -> Self {
        self.health = health;
        self
    }

    pub fn with_capability(mut self, capability: DeviceCapability) -> Self {
        self.capabilities.push(capability);
        self
    }

    pub fn validate(&self) -> Result<()> {
        self.validate_shape()?;
        if self.capabilities.is_empty() {
            return Err(ZapMachineError::EmptyCapabilities(
                self.profile_id.to_string(),
            ));
        }

        let mut command_names = BTreeSet::new();
        for capability in &self.capabilities {
            if let Some(command) = &capability.command {
                validate_machine_id(&command.name)?;
                if !command_names.insert(command.name.clone()) {
                    return Err(ZapMachineError::DuplicateCommand {
                        profile: self.profile_id.to_string(),
                        command: command.name.clone(),
                    });
                }
            }
            if let Some(state_key) = &capability.state_key {
                validate_machine_id(state_key)?;
            }
        }

        Ok(())
    }

    pub fn command_spec(&self, command_name: &str) -> Option<&CommandSpec> {
        self.capabilities
            .iter()
            .filter_map(|capability| capability.command.as_ref())
            .find(|command| command.name == command_name)
    }

    fn validate_shape(&self) -> Result<()> {
        if self.schema_version != DEVICE_PROFILE_SCHEMA_VERSION {
            return Err(ZapMachineError::UnsupportedProfileVersion {
                profile: self.profile_id.to_string(),
                version: self.schema_version,
            });
        }
        if self.display_name.trim().is_empty() {
            return Err(ZapMachineError::EmptyDisplayName(
                self.profile_id.to_string(),
            ));
        }
        if !profile_shape_matches(self.adapter, &self.transport, &self.protocol) {
            return Err(ZapMachineError::InvalidAdapterShape {
                profile: self.profile_id.to_string(),
            });
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityKind {
    Sensor,
    Actuator,
    Command,
    Health,
    Diagnostic,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DeviceCapability {
    pub capability: CapabilityId,
    pub kind: CapabilityKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command: Option<CommandSpec>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state_key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

impl DeviceCapability {
    pub fn command(name: impl Into<String>) -> Result<Self> {
        let command = CommandSpec::new(name)?;
        Ok(Self {
            capability: CapabilityId::driver_execute(&command.name)?,
            kind: CapabilityKind::Command,
            command: Some(command),
            state_key: None,
            description: None,
        })
    }

    pub fn command_spec(command: CommandSpec) -> Result<Self> {
        Ok(Self {
            capability: CapabilityId::driver_execute(&command.name)?,
            kind: CapabilityKind::Command,
            command: Some(command),
            state_key: None,
            description: None,
        })
    }

    pub fn state(key: impl Into<String>) -> Result<Self> {
        let key = key.into();
        validate_machine_id(&key)?;
        Ok(Self {
            capability: CapabilityId::new(format!("{MACHINE_STATE_CAPABILITY_PREFIX}{key}"))?,
            kind: CapabilityKind::Sensor,
            command: None,
            state_key: Some(key),
            description: None,
        })
    }

    pub fn health(profile_id: impl Into<String>) -> Result<Self> {
        let profile_id = MachineId::new(profile_id)?;
        Ok(Self {
            capability: CapabilityId::new(format!(
                "{MACHINE_HEALTH_CAPABILITY_PREFIX}{profile_id}"
            ))?,
            kind: CapabilityKind::Health,
            command: None,
            state_key: None,
            description: None,
        })
    }

    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CommandSpec {
    pub name: String,
    pub max_payload_bytes: u32,
    pub idempotent: bool,
    #[serde(default)]
    pub payload_schema: CommandPayloadSchema,
}

impl CommandSpec {
    pub fn new(name: impl Into<String>) -> Result<Self> {
        let name = name.into();
        validate_machine_id(&name)?;
        Ok(Self {
            name,
            max_payload_bytes: 4096,
            idempotent: false,
            payload_schema: CommandPayloadSchema::Bytes,
        })
    }

    pub const fn with_max_payload_bytes(mut self, max_payload_bytes: u32) -> Self {
        self.max_payload_bytes = max_payload_bytes;
        self
    }

    pub const fn idempotent(mut self, idempotent: bool) -> Self {
        self.idempotent = idempotent;
        self
    }

    pub const fn with_payload_schema(mut self, payload_schema: CommandPayloadSchema) -> Self {
        self.payload_schema = payload_schema;
        self
    }

    fn validate_payload(&self, payload: &[u8]) -> Result<()> {
        if !self.payload_schema.matches(payload) {
            return Err(ZapMachineError::PayloadSchemaMismatch {
                command: self.name.clone(),
                schema: self.payload_schema,
            });
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CommandPayloadSchema {
    Empty,
    #[default]
    Bytes,
    Utf8Text,
    Json,
    U16Be,
}

impl CommandPayloadSchema {
    fn matches(self, payload: &[u8]) -> bool {
        match self {
            Self::Empty => payload.is_empty(),
            Self::Bytes => true,
            Self::Utf8Text => std::str::from_utf8(payload).is_ok(),
            Self::Json => serde_json::from_slice::<serde_json::Value>(payload).is_ok(),
            Self::U16Be => payload.len() == 2,
        }
    }
}

impl fmt::Display for CommandPayloadSchema {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Empty => "empty",
            Self::Bytes => "bytes",
            Self::Utf8Text => "utf8_text",
            Self::Json => "json",
            Self::U16Be => "u16_be",
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilityMapping {
    capabilities: CapabilitySet,
    commands: BTreeMap<String, CapabilityId>,
    states: BTreeMap<String, CapabilityId>,
    health: Vec<CapabilityId>,
}

impl CapabilityMapping {
    pub fn for_profile(profile: &DeviceProfile) -> Result<Self> {
        profile.validate()?;
        let mut capabilities = CapabilitySet::new();
        let mut commands = BTreeMap::new();
        let mut states = BTreeMap::new();
        let mut health = Vec::new();

        for capability in &profile.capabilities {
            capabilities.insert(capability.capability.clone());
            match capability.kind {
                CapabilityKind::Command => {
                    if let Some(command) = &capability.command {
                        commands.insert(command.name.clone(), capability.capability.clone());
                    }
                }
                CapabilityKind::Health => health.push(capability.capability.clone()),
                _ => {
                    if let Some(state_key) = &capability.state_key {
                        states.insert(state_key.clone(), capability.capability.clone());
                    }
                }
            }
        }

        Ok(Self {
            capabilities,
            commands,
            states,
            health,
        })
    }

    pub fn capability_set(&self) -> &CapabilitySet {
        &self.capabilities
    }

    pub fn command_capability(&self, command: &str) -> Option<&CapabilityId> {
        self.commands.get(command)
    }

    pub fn state_capability(&self, state_key: &str) -> Option<&CapabilityId> {
        self.states.get(state_key)
    }

    pub fn health_capabilities(&self) -> &[CapabilityId] {
        &self.health
    }

    pub fn require_command(&self, profile: &DeviceProfile, command: &str) -> Result<&CapabilityId> {
        self.command_capability(command)
            .ok_or_else(|| ZapMachineError::CommandNotDeclared {
                profile: profile.profile_id.to_string(),
                command: command.to_string(),
            })
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum HealthStatus {
    Unknown,
    Online,
    Degraded,
    Offline,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MachineHealth {
    pub status: HealthStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

impl MachineHealth {
    pub fn new(status: HealthStatus, message: Option<String>) -> Self {
        Self { status, message }
    }

    pub fn unknown() -> Self {
        Self::new(HealthStatus::Unknown, None)
    }

    pub fn online() -> Self {
        Self::new(HealthStatus::Online, None)
    }

    pub fn degraded(message: impl Into<String>) -> Self {
        Self::new(HealthStatus::Degraded, Some(message.into()))
    }

    pub fn offline(message: impl Into<String>) -> Self {
        Self::new(HealthStatus::Offline, Some(message.into()))
    }
}

impl Default for MachineHealth {
    fn default() -> Self {
        Self::unknown()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
pub enum MachineValue {
    Bool(bool),
    I64(i64),
    F64(f64),
    Text(String),
    Bytes(Vec<u8>),
}

impl From<bool> for MachineValue {
    fn from(value: bool) -> Self {
        Self::Bool(value)
    }
}

impl From<i64> for MachineValue {
    fn from(value: i64) -> Self {
        Self::I64(value)
    }
}

impl From<u16> for MachineValue {
    fn from(value: u16) -> Self {
        Self::I64(i64::from(value))
    }
}

impl From<f64> for MachineValue {
    fn from(value: f64) -> Self {
        Self::F64(value)
    }
}

impl From<String> for MachineValue {
    fn from(value: String) -> Self {
        Self::Text(value)
    }
}

impl From<&str> for MachineValue {
    fn from(value: &str) -> Self {
        Self::Text(value.to_string())
    }
}

impl From<Vec<u8>> for MachineValue {
    fn from(value: Vec<u8>) -> Self {
        Self::Bytes(value)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MachineState {
    pub health: MachineHealth,
    #[serde(default)]
    pub values: BTreeMap<String, MachineValue>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_command: Option<String>,
}

impl MachineState {
    pub fn with_health(health: MachineHealth) -> Self {
        Self {
            health,
            values: BTreeMap::new(),
            last_command: None,
        }
    }

    pub fn insert(&mut self, key: impl Into<String>, value: impl Into<MachineValue>) {
        self.values.insert(key.into(), value.into());
    }

    pub fn with_value(mut self, key: impl Into<String>, value: impl Into<MachineValue>) -> Self {
        self.insert(key, value);
        self
    }

    pub fn get(&self, key: &str) -> Option<&MachineValue> {
        self.values.get(key)
    }
}

impl Default for MachineState {
    fn default() -> Self {
        Self::with_health(MachineHealth::unknown())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MachineCommand {
    pub name: String,
    #[serde(default)]
    pub payload: Vec<u8>,
}

impl MachineCommand {
    pub fn new(name: impl Into<String>, payload: impl Into<Vec<u8>>) -> Result<Self> {
        let name = name.into();
        validate_machine_id(&name)?;
        Ok(Self {
            name,
            payload: payload.into(),
        })
    }

    pub fn empty(name: impl Into<String>) -> Result<Self> {
        Self::new(name, Vec::new())
    }

    pub fn payload_u16(name: impl Into<String>, value: u16) -> Result<Self> {
        Self::new(name, value.to_be_bytes().to_vec())
    }

    pub fn payload_text(name: impl Into<String>, value: impl AsRef<str>) -> Result<Self> {
        Self::new(name, value.as_ref().as_bytes().to_vec())
    }

    pub fn payload_json(name: impl Into<String>, value: &serde_json::Value) -> Result<Self> {
        let payload = serde_json::to_vec(value).map_err(|error| {
            ZapMachineError::Protocol(format!("failed to encode JSON payload: {error}"))
        })?;
        Self::new(name, payload)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CommandOutcome {
    pub command: String,
    pub accepted: bool,
    #[serde(default)]
    pub response: Vec<u8>,
    pub state: MachineState,
}

impl CommandOutcome {
    pub fn accepted(command: impl Into<String>, response: Vec<u8>, state: MachineState) -> Self {
        Self {
            command: command.into(),
            accepted: true,
            response,
            state,
        }
    }
}

pub trait ProtocolAdapter: Send {
    fn kind(&self) -> AdapterKind;
    fn open(&mut self, profile: &DeviceProfile) -> Result<MachineHealth>;
    fn health(&self) -> MachineHealth;
    fn read_state(&mut self) -> Result<MachineState>;
    fn execute(&mut self, command: MachineCommand) -> Result<CommandOutcome>;
    fn close(&mut self) -> Result<()>;
}

pub struct MachineConnection {
    device_id: MachineId,
    profile: DeviceProfile,
    mapping: CapabilityMapping,
    adapter: Box<dyn ProtocolAdapter>,
    last_heartbeat_micros: Option<u64>,
    heartbeat_timer: Option<HeartbeatTimer>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HeartbeatTimer {
    pub interval_micros: u64,
    pub next_due_micros: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_tick_micros: Option<u64>,
}

impl HeartbeatTimer {
    pub fn new(interval_micros: u64, start_micros: u64) -> Result<Self> {
        if interval_micros == 0 {
            return Err(ZapMachineError::Protocol(
                "heartbeat interval must be greater than zero".to_string(),
            ));
        }
        Ok(Self {
            interval_micros,
            next_due_micros: start_micros.saturating_add(interval_micros),
            last_tick_micros: None,
        })
    }

    pub fn is_due(&self, now_micros: u64) -> bool {
        now_micros >= self.next_due_micros
    }

    pub fn record_tick(&mut self, now_micros: u64) {
        self.last_tick_micros = Some(now_micros);
        self.next_due_micros = now_micros.saturating_add(self.interval_micros);
    }
}

impl MachineConnection {
    pub fn new(
        device_id: impl Into<String>,
        profile: DeviceProfile,
        adapter: Box<dyn ProtocolAdapter>,
    ) -> Result<Self> {
        let mapping = CapabilityMapping::for_profile(&profile)?;
        Ok(Self {
            device_id: MachineId::new(device_id)?,
            profile,
            mapping,
            adapter,
            last_heartbeat_micros: None,
            heartbeat_timer: None,
        })
    }

    pub fn device_id(&self) -> &MachineId {
        &self.device_id
    }

    pub fn profile(&self) -> &DeviceProfile {
        &self.profile
    }

    pub fn mapping(&self) -> &CapabilityMapping {
        &self.mapping
    }

    pub fn connect(&mut self) -> Result<MachineHealth> {
        let adapter = self.adapter.kind();
        let expected = self.profile.adapter;
        if adapter != expected {
            return Err(ZapMachineError::AdapterMismatch {
                profile: self.profile.profile_id.to_string(),
                expected,
                adapter,
            });
        }
        self.adapter.open(&self.profile)
    }

    pub async fn connect_async(&mut self) -> Result<MachineHealth> {
        self.connect()
    }

    pub fn health(&self) -> MachineHealth {
        self.adapter.health()
    }

    pub fn health_at(&self, now_micros: u64) -> MachineHealth {
        let health = self.adapter.health();
        if health.status == HealthStatus::Offline {
            return health;
        }
        let Some(last_heartbeat) = self.last_heartbeat_micros else {
            return health;
        };
        let stale_after_micros = self.profile.health.stale_after_ms.saturating_mul(1_000);
        if stale_after_micros > 0 && now_micros.saturating_sub(last_heartbeat) > stale_after_micros
        {
            MachineHealth::degraded(format!(
                "heartbeat stale for {} micros",
                now_micros.saturating_sub(last_heartbeat)
            ))
        } else {
            health
        }
    }

    pub fn heartbeat_at(&mut self, now_micros: u64) -> Result<CommandOutcome> {
        let command = self
            .profile
            .health
            .heartbeat_command
            .clone()
            .ok_or_else(|| ZapMachineError::MissingHeartbeatCommand(self.device_id.to_string()))?;
        let outcome = self.execute(MachineCommand::empty(command)?)?;
        self.last_heartbeat_micros = Some(now_micros);
        Ok(outcome)
    }

    pub async fn heartbeat_async_at(&mut self, now_micros: u64) -> Result<CommandOutcome> {
        self.heartbeat_at(now_micros)
    }

    pub fn last_heartbeat_micros(&self) -> Option<u64> {
        self.last_heartbeat_micros
    }

    pub fn enable_heartbeat_timer(
        &mut self,
        interval_micros: u64,
        start_micros: u64,
    ) -> Result<()> {
        if self.profile.health.heartbeat_command.is_none() {
            return Err(ZapMachineError::MissingHeartbeatCommand(
                self.device_id.to_string(),
            ));
        }
        self.heartbeat_timer = Some(HeartbeatTimer::new(interval_micros, start_micros)?);
        Ok(())
    }

    pub fn disable_heartbeat_timer(&mut self) {
        self.heartbeat_timer = None;
    }

    pub fn heartbeat_timer(&self) -> Option<&HeartbeatTimer> {
        self.heartbeat_timer.as_ref()
    }

    pub fn tick_heartbeat_timer_at(&mut self, now_micros: u64) -> Result<Option<MachineHealth>> {
        let Some(mut timer) = self.heartbeat_timer.take() else {
            return Ok(None);
        };
        if !timer.is_due(now_micros) {
            self.heartbeat_timer = Some(timer);
            return Ok(None);
        }
        if let Err(error) = self.heartbeat_at(now_micros) {
            self.heartbeat_timer = Some(timer);
            return Err(error);
        }
        timer.record_tick(now_micros);
        self.heartbeat_timer = Some(timer);
        Ok(Some(self.health_at(now_micros)))
    }

    pub async fn tick_heartbeat_timer_async_at(
        &mut self,
        now_micros: u64,
    ) -> Result<Option<MachineHealth>> {
        self.tick_heartbeat_timer_at(now_micros)
    }

    pub fn read_state(&mut self) -> Result<MachineState> {
        self.adapter.read_state()
    }

    pub async fn read_state_async(&mut self) -> Result<MachineState> {
        self.read_state()
    }

    pub fn execute(&mut self, command: MachineCommand) -> Result<CommandOutcome> {
        self.mapping.require_command(&self.profile, &command.name)?;
        let spec = self.profile.command_spec(&command.name).ok_or_else(|| {
            ZapMachineError::CommandNotDeclared {
                profile: self.profile.profile_id.to_string(),
                command: command.name.clone(),
            }
        })?;
        if command.payload.len() > spec.max_payload_bytes as usize {
            return Err(ZapMachineError::PayloadTooLarge {
                command: command.name,
                max: spec.max_payload_bytes,
                actual: command.payload.len(),
            });
        }
        spec.validate_payload(&command.payload)?;
        self.adapter.execute(command)
    }

    pub async fn execute_async(&mut self, command: MachineCommand) -> Result<CommandOutcome> {
        self.execute(command)
    }

    pub fn close(&mut self) -> Result<()> {
        self.adapter.close()
    }

    pub async fn close_async(&mut self) -> Result<()> {
        self.close()
    }
}

#[derive(Default)]
pub struct MachineBus {
    connections: BTreeMap<MachineId, MachineConnection>,
}

impl MachineBus {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn attach(&mut self, connection: MachineConnection) -> Result<()> {
        let device_id = connection.device_id.clone();
        if self.connections.contains_key(&device_id) {
            return Err(ZapMachineError::DuplicateDevice(device_id.to_string()));
        }
        self.connections.insert(device_id, connection);
        Ok(())
    }

    pub fn connect_all(&mut self) -> Result<BTreeMap<MachineId, MachineHealth>> {
        let mut health = BTreeMap::new();
        for (device_id, connection) in &mut self.connections {
            health.insert(device_id.clone(), connection.connect()?);
        }
        Ok(health)
    }

    pub async fn connect_all_async(&mut self) -> Result<BTreeMap<MachineId, MachineHealth>> {
        self.connect_all()
    }

    pub fn heartbeat_all_at(
        &mut self,
        now_micros: u64,
    ) -> Result<BTreeMap<MachineId, MachineHealth>> {
        let mut health = BTreeMap::new();
        for (device_id, connection) in &mut self.connections {
            if connection.profile.health.heartbeat_command.is_some() {
                connection.heartbeat_at(now_micros)?;
            }
            health.insert(device_id.clone(), connection.health_at(now_micros));
        }
        Ok(health)
    }

    pub async fn heartbeat_all_async_at(
        &mut self,
        now_micros: u64,
    ) -> Result<BTreeMap<MachineId, MachineHealth>> {
        self.heartbeat_all_at(now_micros)
    }

    pub fn enable_heartbeat_timers(
        &mut self,
        interval_micros: u64,
        start_micros: u64,
    ) -> Result<()> {
        for connection in self.connections.values_mut() {
            if connection.profile.health.heartbeat_command.is_some() {
                connection.enable_heartbeat_timer(interval_micros, start_micros)?;
            }
        }
        Ok(())
    }

    pub fn tick_heartbeat_timers_at(
        &mut self,
        now_micros: u64,
    ) -> Result<BTreeMap<MachineId, MachineHealth>> {
        let mut health = BTreeMap::new();
        for (device_id, connection) in &mut self.connections {
            if let Some(current) = connection.tick_heartbeat_timer_at(now_micros)? {
                health.insert(device_id.clone(), current);
            }
        }
        Ok(health)
    }

    pub async fn tick_heartbeat_timers_async_at(
        &mut self,
        now_micros: u64,
    ) -> Result<BTreeMap<MachineId, MachineHealth>> {
        self.tick_heartbeat_timers_at(now_micros)
    }

    pub fn execute(
        &mut self,
        device_id: &MachineId,
        command: MachineCommand,
    ) -> Result<CommandOutcome> {
        self.connections
            .get_mut(device_id)
            .ok_or_else(|| ZapMachineError::UnknownDevice(device_id.to_string()))?
            .execute(command)
    }

    pub fn read_state(&mut self, device_id: &MachineId) -> Result<MachineState> {
        self.connections
            .get_mut(device_id)
            .ok_or_else(|| ZapMachineError::UnknownDevice(device_id.to_string()))?
            .read_state()
    }

    pub async fn execute_async(
        &mut self,
        device_id: &MachineId,
        command: MachineCommand,
    ) -> Result<CommandOutcome> {
        self.execute(device_id, command)
    }

    pub async fn read_state_async(&mut self, device_id: &MachineId) -> Result<MachineState> {
        self.read_state(device_id)
    }

    pub fn health_snapshot(&self) -> BTreeMap<MachineId, MachineHealth> {
        self.connections
            .iter()
            .map(|(device_id, connection)| (device_id.clone(), connection.health()))
            .collect()
    }
}

#[derive(Debug, Clone)]
pub struct MockAdapter {
    opened: bool,
    health: MachineHealth,
    state: MachineState,
    responses: BTreeMap<String, Vec<u8>>,
    command_log: Vec<MachineCommand>,
}

impl MockAdapter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_response(
        mut self,
        command: impl Into<String>,
        response: impl Into<Vec<u8>>,
    ) -> Result<Self> {
        let command = command.into();
        validate_machine_id(&command)?;
        self.responses.insert(command, response.into());
        Ok(self)
    }

    pub fn set_value(&mut self, key: impl Into<String>, value: impl Into<MachineValue>) {
        self.state.insert(key, value);
    }

    pub fn command_log(&self) -> &[MachineCommand] {
        &self.command_log
    }
}

impl Default for MockAdapter {
    fn default() -> Self {
        Self {
            opened: false,
            health: MachineHealth::offline("not connected"),
            state: MachineState::default(),
            responses: BTreeMap::new(),
            command_log: Vec::new(),
        }
    }
}

impl ProtocolAdapter for MockAdapter {
    fn kind(&self) -> AdapterKind {
        AdapterKind::Mock
    }

    fn open(&mut self, _profile: &DeviceProfile) -> Result<MachineHealth> {
        self.opened = true;
        self.health = MachineHealth::online();
        self.state.health = self.health.clone();
        Ok(self.health.clone())
    }

    fn health(&self) -> MachineHealth {
        self.health.clone()
    }

    fn read_state(&mut self) -> Result<MachineState> {
        ensure_open(self.opened, AdapterKind::Mock)?;
        Ok(self.state.clone())
    }

    fn execute(&mut self, command: MachineCommand) -> Result<CommandOutcome> {
        ensure_open(self.opened, AdapterKind::Mock)?;
        self.command_log.push(command.clone());
        self.state.last_command = Some(command.name.clone());
        self.state.insert("last_payload", command.payload.clone());
        let response = self
            .responses
            .get(&command.name)
            .cloned()
            .unwrap_or_else(|| b"ok".to_vec());
        Ok(CommandOutcome::accepted(
            command.name,
            response,
            self.state.clone(),
        ))
    }

    fn close(&mut self) -> Result<()> {
        self.opened = false;
        self.health = MachineHealth::offline("closed");
        self.state.health = self.health.clone();
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct SerialAdapter {
    opened: bool,
    port: String,
    baud_rate: u32,
    delimiter: Vec<u8>,
    outbound_frames: Vec<Vec<u8>>,
    inbound_frames: VecDeque<Vec<u8>>,
    health: MachineHealth,
}

impl SerialAdapter {
    pub fn scripted<I, B>(port: impl Into<String>, baud_rate: u32, inbound_frames: I) -> Self
    where
        I: IntoIterator<Item = B>,
        B: Into<Vec<u8>>,
    {
        Self {
            opened: false,
            port: port.into(),
            baud_rate,
            delimiter: b"\n".to_vec(),
            outbound_frames: Vec::new(),
            inbound_frames: inbound_frames.into_iter().map(Into::into).collect(),
            health: MachineHealth::offline("not connected"),
        }
    }

    pub fn outbound_frames(&self) -> &[Vec<u8>] {
        &self.outbound_frames
    }
}

impl ProtocolAdapter for SerialAdapter {
    fn kind(&self) -> AdapterKind {
        AdapterKind::Serial
    }

    fn open(&mut self, profile: &DeviceProfile) -> Result<MachineHealth> {
        if let TransportProfile::Serial { port, baud_rate } = &profile.transport {
            self.port.clone_from(port);
            self.baud_rate = *baud_rate;
        }
        if let ProtocolProfile::SerialLine { delimiter } = &profile.protocol {
            self.delimiter = delimiter.as_bytes().to_vec();
        }
        self.opened = true;
        self.health = MachineHealth::online();
        Ok(self.health.clone())
    }

    fn health(&self) -> MachineHealth {
        self.health.clone()
    }

    fn read_state(&mut self) -> Result<MachineState> {
        ensure_open(self.opened, AdapterKind::Serial)?;
        Ok(MachineState::with_health(self.health.clone())
            .with_value("serial.port", self.port.clone())
            .with_value("serial.baud_rate", i64::from(self.baud_rate))
            .with_value("serial.outbound_frames", self.outbound_frames.len() as i64))
    }

    fn execute(&mut self, command: MachineCommand) -> Result<CommandOutcome> {
        ensure_open(self.opened, AdapterKind::Serial)?;
        let mut frame = Vec::new();
        frame.extend_from_slice(command.name.as_bytes());
        if !command.payload.is_empty() {
            frame.push(b' ');
            frame.extend_from_slice(&command.payload);
        }
        frame.extend_from_slice(&self.delimiter);
        self.outbound_frames.push(frame);
        let response = self
            .inbound_frames
            .pop_front()
            .unwrap_or_else(|| b"OK".to_vec());
        let mut state = self.read_state()?;
        state.last_command = Some(command.name.clone());
        Ok(CommandOutcome::accepted(command.name, response, state))
    }

    fn close(&mut self) -> Result<()> {
        self.opened = false;
        self.health = MachineHealth::offline("closed");
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct TcpAdapter {
    opened: bool,
    host: String,
    port: u16,
    max_frame_bytes: u32,
    outbound_frames: Vec<Vec<u8>>,
    inbound_frames: VecDeque<Vec<u8>>,
    health: MachineHealth,
}

impl TcpAdapter {
    pub fn scripted<I, B>(host: impl Into<String>, port: u16, inbound_frames: I) -> Self
    where
        I: IntoIterator<Item = B>,
        B: Into<Vec<u8>>,
    {
        Self {
            opened: false,
            host: host.into(),
            port,
            max_frame_bytes: 8192,
            outbound_frames: Vec::new(),
            inbound_frames: inbound_frames.into_iter().map(Into::into).collect(),
            health: MachineHealth::offline("not connected"),
        }
    }

    pub fn outbound_frames(&self) -> &[Vec<u8>] {
        &self.outbound_frames
    }
}

impl ProtocolAdapter for TcpAdapter {
    fn kind(&self) -> AdapterKind {
        AdapterKind::Tcp
    }

    fn open(&mut self, profile: &DeviceProfile) -> Result<MachineHealth> {
        if let TransportProfile::Tcp { host, port } = &profile.transport {
            self.host.clone_from(host);
            self.port = *port;
        }
        if let ProtocolProfile::TcpFrames { max_frame_bytes } = profile.protocol {
            self.max_frame_bytes = max_frame_bytes;
        }
        self.opened = true;
        self.health = MachineHealth::online();
        Ok(self.health.clone())
    }

    fn health(&self) -> MachineHealth {
        self.health.clone()
    }

    fn read_state(&mut self) -> Result<MachineState> {
        ensure_open(self.opened, AdapterKind::Tcp)?;
        Ok(MachineState::with_health(self.health.clone())
            .with_value("tcp.host", self.host.clone())
            .with_value("tcp.port", i64::from(self.port))
            .with_value("tcp.outbound_frames", self.outbound_frames.len() as i64))
    }

    fn execute(&mut self, command: MachineCommand) -> Result<CommandOutcome> {
        ensure_open(self.opened, AdapterKind::Tcp)?;
        let frame = encode_tcp_frame(&command, self.max_frame_bytes)?;
        self.outbound_frames.push(frame);
        let response = self
            .inbound_frames
            .pop_front()
            .unwrap_or_else(|| b"ACK".to_vec());
        let mut state = self.read_state()?;
        state.last_command = Some(command.name.clone());
        Ok(CommandOutcome::accepted(command.name, response, state))
    }

    fn close(&mut self) -> Result<()> {
        self.opened = false;
        self.health = MachineHealth::offline("closed");
        Ok(())
    }
}

pub struct StreamSerialAdapter<S>
where
    S: Read + Write + Send,
{
    opened: bool,
    stream: S,
    port: String,
    baud_rate: u32,
    delimiter: Vec<u8>,
    health: MachineHealth,
}

impl<S> StreamSerialAdapter<S>
where
    S: Read + Write + Send,
{
    pub fn new(stream: S) -> Self {
        Self {
            opened: false,
            stream,
            port: String::new(),
            baud_rate: 0,
            delimiter: b"\n".to_vec(),
            health: MachineHealth::offline("not connected"),
        }
    }

    pub fn into_inner(self) -> S {
        self.stream
    }
}

impl<S> ProtocolAdapter for StreamSerialAdapter<S>
where
    S: Read + Write + Send,
{
    fn kind(&self) -> AdapterKind {
        AdapterKind::Serial
    }

    fn open(&mut self, profile: &DeviceProfile) -> Result<MachineHealth> {
        if let TransportProfile::Serial { port, baud_rate } = &profile.transport {
            self.port.clone_from(port);
            self.baud_rate = *baud_rate;
        }
        if let ProtocolProfile::SerialLine { delimiter } = &profile.protocol {
            self.delimiter = delimiter.as_bytes().to_vec();
        }
        self.opened = true;
        self.health = MachineHealth::online();
        Ok(self.health.clone())
    }

    fn health(&self) -> MachineHealth {
        self.health.clone()
    }

    fn read_state(&mut self) -> Result<MachineState> {
        ensure_open(self.opened, AdapterKind::Serial)?;
        Ok(MachineState::with_health(self.health.clone())
            .with_value("serial.port", self.port.clone())
            .with_value("serial.baud_rate", i64::from(self.baud_rate)))
    }

    fn execute(&mut self, command: MachineCommand) -> Result<CommandOutcome> {
        ensure_open(self.opened, AdapterKind::Serial)?;
        let mut frame = Vec::new();
        frame.extend_from_slice(command.name.as_bytes());
        if !command.payload.is_empty() {
            frame.push(b' ');
            frame.extend_from_slice(&command.payload);
        }
        frame.extend_from_slice(&self.delimiter);
        self.stream.write_all(&frame).map_err(io_error)?;
        self.stream.flush().map_err(io_error)?;
        let response = read_until_delimiter(&mut self.stream, &self.delimiter)?;
        let mut state = self.read_state()?;
        state.last_command = Some(command.name.clone());
        Ok(CommandOutcome::accepted(command.name, response, state))
    }

    fn close(&mut self) -> Result<()> {
        self.opened = false;
        self.health = MachineHealth::offline("closed");
        Ok(())
    }
}

pub struct TcpStreamAdapter {
    opened: bool,
    stream: Option<TcpStream>,
    host: String,
    port: u16,
    max_frame_bytes: u32,
    read_timeout: Option<Duration>,
    write_timeout: Option<Duration>,
    health: MachineHealth,
}

impl TcpStreamAdapter {
    pub fn new() -> Self {
        Self {
            opened: false,
            stream: None,
            host: String::new(),
            port: 0,
            max_frame_bytes: 8192,
            read_timeout: Some(Duration::from_secs(2)),
            write_timeout: Some(Duration::from_secs(2)),
            health: MachineHealth::offline("not connected"),
        }
    }

    pub fn with_timeouts(
        mut self,
        read_timeout: Option<Duration>,
        write_timeout: Option<Duration>,
    ) -> Self {
        self.read_timeout = read_timeout;
        self.write_timeout = write_timeout;
        self
    }
}

impl Default for TcpStreamAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl ProtocolAdapter for TcpStreamAdapter {
    fn kind(&self) -> AdapterKind {
        AdapterKind::Tcp
    }

    fn open(&mut self, profile: &DeviceProfile) -> Result<MachineHealth> {
        if let TransportProfile::Tcp { host, port } = &profile.transport {
            self.host.clone_from(host);
            self.port = *port;
        }
        if let ProtocolProfile::TcpFrames { max_frame_bytes } = profile.protocol {
            self.max_frame_bytes = max_frame_bytes;
        }
        let stream = TcpStream::connect((self.host.as_str(), self.port)).map_err(io_error)?;
        stream.set_nodelay(true).map_err(io_error)?;
        stream
            .set_read_timeout(self.read_timeout)
            .map_err(io_error)?;
        stream
            .set_write_timeout(self.write_timeout)
            .map_err(io_error)?;
        self.stream = Some(stream);
        self.opened = true;
        self.health = MachineHealth::online();
        Ok(self.health.clone())
    }

    fn health(&self) -> MachineHealth {
        self.health.clone()
    }

    fn read_state(&mut self) -> Result<MachineState> {
        ensure_open(self.opened, AdapterKind::Tcp)?;
        Ok(MachineState::with_health(self.health.clone())
            .with_value("tcp.host", self.host.clone())
            .with_value("tcp.port", i64::from(self.port)))
    }

    fn execute(&mut self, command: MachineCommand) -> Result<CommandOutcome> {
        ensure_open(self.opened, AdapterKind::Tcp)?;
        let frame = encode_tcp_frame(&command, self.max_frame_bytes)?;
        let stream = self
            .stream
            .as_mut()
            .ok_or(ZapMachineError::AdapterNotOpen {
                adapter: AdapterKind::Tcp,
            })?;
        stream.write_all(&frame).map_err(io_error)?;
        stream.flush().map_err(io_error)?;
        let response = read_tcp_frame(stream, self.max_frame_bytes)?;
        let mut state = self.read_state()?;
        state.last_command = Some(command.name.clone());
        Ok(CommandOutcome::accepted(command.name, response, state))
    }

    fn close(&mut self) -> Result<()> {
        self.stream = None;
        self.opened = false;
        self.health = MachineHealth::offline("closed");
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ModbusOperation {
    ReadHolding { register: u16 },
    WriteSingle { register: u16, value: u16 },
    WritePayloadU16 { register: u16 },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ModbusFrame {
    pub unit_id: u8,
    pub function: u8,
    pub register: u16,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<u16>,
}

#[derive(Debug, Clone)]
pub struct ModbusLikeAdapter {
    opened: bool,
    unit_id: u8,
    registers: BTreeMap<u16, u16>,
    commands: BTreeMap<String, ModbusOperation>,
    transactions: Vec<ModbusFrame>,
    health: MachineHealth,
}

impl ModbusLikeAdapter {
    pub fn new(unit_id: u8) -> Self {
        Self {
            opened: false,
            unit_id,
            registers: BTreeMap::new(),
            commands: BTreeMap::new(),
            transactions: Vec::new(),
            health: MachineHealth::offline("not connected"),
        }
    }

    pub fn with_register(mut self, register: u16, value: u16) -> Self {
        self.registers.insert(register, value);
        self
    }

    pub fn map_command(
        mut self,
        command: impl Into<String>,
        operation: ModbusOperation,
    ) -> Result<Self> {
        let command = command.into();
        validate_machine_id(&command)?;
        self.commands.insert(command, operation);
        Ok(self)
    }

    pub fn register(&self, register: u16) -> Option<u16> {
        self.registers.get(&register).copied()
    }

    pub fn transactions(&self) -> &[ModbusFrame] {
        &self.transactions
    }
}

impl ProtocolAdapter for ModbusLikeAdapter {
    fn kind(&self) -> AdapterKind {
        AdapterKind::ModbusLike
    }

    fn open(&mut self, profile: &DeviceProfile) -> Result<MachineHealth> {
        if let ProtocolProfile::ModbusLike { unit_id } = profile.protocol {
            self.unit_id = unit_id;
        }
        self.opened = true;
        self.health = MachineHealth::online();
        Ok(self.health.clone())
    }

    fn health(&self) -> MachineHealth {
        self.health.clone()
    }

    fn read_state(&mut self) -> Result<MachineState> {
        ensure_open(self.opened, AdapterKind::ModbusLike)?;
        Ok(MachineState::with_health(self.health.clone())
            .with_value("modbus.unit_id", i64::from(self.unit_id))
            .with_value("modbus.transactions", self.transactions.len() as i64))
    }

    fn execute(&mut self, command: MachineCommand) -> Result<CommandOutcome> {
        ensure_open(self.opened, AdapterKind::ModbusLike)?;
        let operation = self
            .commands
            .get(&command.name)
            .cloned()
            .ok_or_else(|| ZapMachineError::UnknownProtocolCommand(command.name.clone()))?;
        let (frame, response, state_register) = match operation {
            ModbusOperation::ReadHolding { register } => {
                let value = self.registers.get(&register).copied().unwrap_or_default();
                (
                    ModbusFrame {
                        unit_id: self.unit_id,
                        function: 3,
                        register,
                        value: None,
                    },
                    value.to_be_bytes().to_vec(),
                    Some((register, value)),
                )
            }
            ModbusOperation::WriteSingle { register, value } => {
                self.registers.insert(register, value);
                (
                    ModbusFrame {
                        unit_id: self.unit_id,
                        function: 6,
                        register,
                        value: Some(value),
                    },
                    value.to_be_bytes().to_vec(),
                    Some((register, value)),
                )
            }
            ModbusOperation::WritePayloadU16 { register } => {
                let value = read_u16_payload(&command.payload)?;
                self.registers.insert(register, value);
                (
                    ModbusFrame {
                        unit_id: self.unit_id,
                        function: 6,
                        register,
                        value: Some(value),
                    },
                    value.to_be_bytes().to_vec(),
                    Some((register, value)),
                )
            }
        };
        self.transactions.push(frame);
        let mut state = self.read_state()?;
        state.last_command = Some(command.name.clone());
        if let Some((register, value)) = state_register {
            state.insert(format!("modbus.register.{register}"), value);
        }
        Ok(CommandOutcome::accepted(command.name, response, state))
    }

    fn close(&mut self) -> Result<()> {
        self.opened = false;
        self.health = MachineHealth::offline("closed");
        Ok(())
    }
}

fn profile_shape_matches(
    adapter: AdapterKind,
    transport: &TransportProfile,
    protocol: &ProtocolProfile,
) -> bool {
    matches!(
        (adapter, transport, protocol),
        (
            AdapterKind::Mock,
            TransportProfile::Mock { .. },
            ProtocolProfile::Mock
        ) | (
            AdapterKind::Serial,
            TransportProfile::Serial { .. },
            ProtocolProfile::SerialLine { .. }
        ) | (
            AdapterKind::Tcp,
            TransportProfile::Tcp { .. },
            ProtocolProfile::TcpFrames { .. }
        ) | (
            AdapterKind::ModbusLike,
            TransportProfile::IndustrialBus { .. }
                | TransportProfile::Serial { .. }
                | TransportProfile::Tcp { .. },
            ProtocolProfile::ModbusLike { .. }
        )
    )
}

fn validate_machine_id(input: &str) -> Result<()> {
    if input.trim().is_empty() {
        return Err(ZapMachineError::EmptyMachineId);
    }
    if input.len() > MAX_MACHINE_ID_BYTES {
        return Err(ZapMachineError::MachineIdTooLong(input.to_string()));
    }
    if !input
        .bytes()
        .all(|byte| matches!(byte, b'a'..=b'z' | b'0'..=b'9' | b'.' | b':' | b'_' | b'-'))
    {
        return Err(ZapMachineError::InvalidMachineId(input.to_string()));
    }
    Ok(())
}

fn ensure_open(opened: bool, adapter: AdapterKind) -> Result<()> {
    if opened {
        Ok(())
    } else {
        Err(ZapMachineError::AdapterNotOpen { adapter })
    }
}

fn encode_tcp_frame(command: &MachineCommand, max_frame_bytes: u32) -> Result<Vec<u8>> {
    let body_len = command
        .name
        .len()
        .checked_add(1)
        .and_then(|len| len.checked_add(command.payload.len()))
        .ok_or_else(|| ZapMachineError::Protocol("tcp frame length overflow".to_string()))?;
    if body_len > max_frame_bytes as usize {
        return Err(ZapMachineError::Protocol(format!(
            "tcp frame body is {body_len} bytes, above the limit of {max_frame_bytes}"
        )));
    }
    if body_len > u16::MAX as usize {
        return Err(ZapMachineError::Protocol(format!(
            "tcp frame body is {body_len} bytes, above u16 length prefix capacity"
        )));
    }

    let mut frame = Vec::with_capacity(body_len + 2);
    frame.extend_from_slice(&(body_len as u16).to_be_bytes());
    frame.extend_from_slice(command.name.as_bytes());
    frame.push(0);
    frame.extend_from_slice(&command.payload);
    Ok(frame)
}

fn read_tcp_frame(stream: &mut TcpStream, max_frame_bytes: u32) -> Result<Vec<u8>> {
    let mut len = [0_u8; 2];
    stream.read_exact(&mut len).map_err(io_error)?;
    let len = u16::from_be_bytes(len) as usize;
    if len > max_frame_bytes as usize {
        return Err(ZapMachineError::Protocol(format!(
            "tcp response body is {len} bytes, above the limit of {max_frame_bytes}"
        )));
    }
    let mut body = vec![0_u8; len];
    stream.read_exact(&mut body).map_err(io_error)?;
    Ok(body)
}

fn read_until_delimiter<S>(stream: &mut S, delimiter: &[u8]) -> Result<Vec<u8>>
where
    S: Read,
{
    if delimiter.is_empty() {
        return Err(ZapMachineError::Protocol(
            "serial delimiter must not be empty".to_string(),
        ));
    }
    let mut response = Vec::new();
    let mut byte = [0_u8; 1];
    loop {
        let read = stream.read(&mut byte).map_err(io_error)?;
        if read == 0 {
            if response.is_empty() {
                return Err(ZapMachineError::Protocol(
                    "serial stream closed before response".to_string(),
                ));
            }
            return Ok(response);
        }
        response.push(byte[0]);
        if response.ends_with(delimiter) {
            let end = response.len() - delimiter.len();
            response.truncate(end);
            return Ok(response);
        }
    }
}

fn io_error(error: std::io::Error) -> ZapMachineError {
    ZapMachineError::Io(error.to_string())
}

fn read_u16_payload(payload: &[u8]) -> Result<u16> {
    if payload.len() != 2 {
        return Err(ZapMachineError::Protocol(format!(
            "expected 2 payload bytes for u16 register write, got {}",
            payload.len()
        )));
    }
    Ok(u16::from_be_bytes([payload[0], payload[1]]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::{Cursor, Read, Write},
        net::TcpListener,
        thread,
    };

    fn mock_profile() -> DeviceProfile {
        DeviceProfile::new(
            "lab.thermostat",
            "Lab Thermostat",
            AdapterKind::Mock,
            TransportProfile::Mock {
                channel: "unit-test".to_string(),
            },
            ProtocolProfile::Mock,
        )
        .unwrap()
        .with_capability(DeviceCapability::health("lab.thermostat").unwrap())
        .with_capability(DeviceCapability::state("temperature.celsius").unwrap())
        .with_capability(
            DeviceCapability::command_spec(
                CommandSpec::new("thermostat.setpoint.write")
                    .unwrap()
                    .with_max_payload_bytes(8),
            )
            .unwrap(),
        )
    }

    fn serial_profile() -> DeviceProfile {
        DeviceProfile::new(
            "lab.scale",
            "Lab Scale",
            AdapterKind::Serial,
            TransportProfile::Serial {
                port: "COM-MOCK".to_string(),
                baud_rate: 115_200,
            },
            ProtocolProfile::SerialLine {
                delimiter: "\n".to_string(),
            },
        )
        .unwrap()
        .with_capability(DeviceCapability::health("lab.scale").unwrap())
        .with_capability(DeviceCapability::command("scale.tare").unwrap())
    }

    fn tcp_profile() -> DeviceProfile {
        DeviceProfile::new(
            "cell.robot",
            "Robot Cell Controller",
            AdapterKind::Tcp,
            TransportProfile::Tcp {
                host: "127.0.0.1".to_string(),
                port: 15020,
            },
            ProtocolProfile::TcpFrames {
                max_frame_bytes: 1024,
            },
        )
        .unwrap()
        .with_capability(DeviceCapability::health("cell.robot").unwrap())
        .with_capability(DeviceCapability::command("robot.home").unwrap())
    }

    fn modbus_profile() -> DeviceProfile {
        DeviceProfile::new(
            "line.plc",
            "Line PLC",
            AdapterKind::ModbusLike,
            TransportProfile::IndustrialBus {
                bus_id: "bench-bus".to_string(),
                node_id: 7,
            },
            ProtocolProfile::ModbusLike { unit_id: 7 },
        )
        .unwrap()
        .with_capability(DeviceCapability::health("line.plc").unwrap())
        .with_capability(DeviceCapability::state("holding.40001").unwrap())
        .with_capability(DeviceCapability::command("plc.speed.read").unwrap())
        .with_capability(DeviceCapability::command("plc.speed.write").unwrap())
    }

    #[derive(Debug)]
    struct MemoryDuplex {
        read: Cursor<Vec<u8>>,
        written: Vec<u8>,
    }

    impl MemoryDuplex {
        fn new(read: impl Into<Vec<u8>>) -> Self {
            Self {
                read: Cursor::new(read.into()),
                written: Vec::new(),
            }
        }
    }

    impl Read for MemoryDuplex {
        fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            self.read.read(buf)
        }
    }

    impl Write for MemoryDuplex {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.written.extend_from_slice(buf);
            Ok(buf.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn maps_profile_capabilities_to_commands_state_and_health() {
        let profile = mock_profile();
        let mapping = CapabilityMapping::for_profile(&profile).unwrap();

        assert_eq!(mapping.capability_set().capabilities.len(), 3);
        assert_eq!(
            mapping
                .command_capability("thermostat.setpoint.write")
                .unwrap()
                .as_str(),
            "driver.execute:thermostat.setpoint.write"
        );
        assert_eq!(
            mapping
                .state_capability("temperature.celsius")
                .unwrap()
                .as_str(),
            "machine.state:temperature.celsius"
        );
        assert_eq!(mapping.health_capabilities().len(), 1);
    }

    #[test]
    fn serde_rejects_invalid_machine_ids() {
        let error = serde_json::from_str::<MachineId>(r#""Lab Thermostat""#).unwrap_err();

        assert!(error.to_string().contains("invalid characters"));
    }

    #[test]
    fn bus_executes_mock_commands_without_hardware() {
        let profile = mock_profile();
        let mut adapter = MockAdapter::new()
            .with_response("thermostat.setpoint.write", b"accepted".to_vec())
            .unwrap();
        adapter.set_value("temperature.celsius", 21.5_f64);
        let connection =
            MachineConnection::new("lab.thermostat.1", profile, Box::new(adapter)).unwrap();
        let mut bus = MachineBus::new();
        bus.attach(connection).unwrap();

        let health = bus.connect_all().unwrap();
        assert_eq!(
            health.get(&MachineId::new("lab.thermostat.1").unwrap()),
            Some(&MachineHealth::online())
        );
        let outcome = bus
            .execute(
                &MachineId::new("lab.thermostat.1").unwrap(),
                MachineCommand::new("thermostat.setpoint.write", b"22.0".to_vec()).unwrap(),
            )
            .unwrap();

        assert!(outcome.accepted);
        assert_eq!(outcome.response, b"accepted");
        assert_eq!(
            outcome.state.get("last_payload"),
            Some(&MachineValue::Bytes(b"22.0".to_vec()))
        );
    }

    #[test]
    fn connection_rejects_undeclared_commands() {
        let profile = mock_profile();
        let mut connection =
            MachineConnection::new("lab.thermostat.2", profile, Box::new(MockAdapter::new()))
                .unwrap();
        connection.connect().unwrap();

        let error = connection
            .execute(MachineCommand::empty("thermostat.restart").unwrap())
            .unwrap_err();

        assert!(matches!(
            error,
            ZapMachineError::CommandNotDeclared { command, .. } if command == "thermostat.restart"
        ));
    }

    #[test]
    fn connection_enforces_declared_payload_limits() {
        let profile = mock_profile();
        let mut connection =
            MachineConnection::new("lab.thermostat.3", profile, Box::new(MockAdapter::new()))
                .unwrap();
        connection.connect().unwrap();

        let error = connection
            .execute(
                MachineCommand::new("thermostat.setpoint.write", b"too-large".to_vec()).unwrap(),
            )
            .unwrap_err();

        assert!(matches!(
            error,
            ZapMachineError::PayloadTooLarge {
                command,
                max: 8,
                actual: 9
            } if command == "thermostat.setpoint.write"
        ));
    }

    #[test]
    fn command_specs_validate_typed_payloads() {
        let mut profile = mock_profile();
        profile.capabilities.push(
            DeviceCapability::command_spec(
                CommandSpec::new("thermostat.mode.write")
                    .unwrap()
                    .with_payload_schema(CommandPayloadSchema::Json),
            )
            .unwrap(),
        );
        let mut connection = MachineConnection::new(
            "lab.thermostat.typed",
            profile,
            Box::new(MockAdapter::new()),
        )
        .unwrap();
        connection.connect().unwrap();

        connection
            .execute(
                MachineCommand::payload_json(
                    "thermostat.mode.write",
                    &serde_json::json!({"mode":"eco"}),
                )
                .unwrap(),
            )
            .unwrap();
        let error = connection
            .execute(MachineCommand::payload_text("thermostat.mode.write", "not-json").unwrap())
            .unwrap_err();

        assert!(matches!(
            error,
            ZapMachineError::PayloadSchemaMismatch { .. }
        ));
    }

    #[tokio::test]
    async fn async_connection_and_heartbeat_track_staleness() {
        let profile = mock_profile().with_health_policy(HealthPolicy {
            heartbeat_command: Some("thermostat.setpoint.write".to_string()),
            stale_after_ms: 10,
        });
        let mut connection = MachineConnection::new(
            "lab.thermostat.async",
            profile,
            Box::new(MockAdapter::new()),
        )
        .unwrap();

        connection.connect_async().await.unwrap();
        connection.heartbeat_async_at(1_000).await.unwrap();
        assert_eq!(connection.last_heartbeat_micros(), Some(1_000));
        assert_eq!(connection.health_at(5_000).status, HealthStatus::Online);
        assert_eq!(connection.health_at(20_000).status, HealthStatus::Degraded);
    }

    #[test]
    fn heartbeat_timer_ticks_only_when_due() {
        let profile = mock_profile().with_health_policy(HealthPolicy {
            heartbeat_command: Some("thermostat.setpoint.write".to_string()),
            stale_after_ms: 10,
        });
        let mut connection = MachineConnection::new(
            "lab.thermostat.timer",
            profile,
            Box::new(MockAdapter::new()),
        )
        .unwrap();
        connection.connect().unwrap();
        connection.enable_heartbeat_timer(5_000, 1_000).unwrap();

        assert!(connection.tick_heartbeat_timer_at(5_999).unwrap().is_none());
        assert_eq!(connection.last_heartbeat_micros(), None);
        assert!(connection.tick_heartbeat_timer_at(6_000).unwrap().is_some());
        assert_eq!(connection.last_heartbeat_micros(), Some(6_000));
        assert_eq!(
            connection.heartbeat_timer().unwrap().next_due_micros,
            11_000
        );
        assert!(
            connection
                .tick_heartbeat_timer_at(10_999)
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn bus_ticks_configured_heartbeat_timers() {
        let profile = mock_profile().with_health_policy(HealthPolicy {
            heartbeat_command: Some("thermostat.setpoint.write".to_string()),
            stale_after_ms: 10,
        });
        let connection =
            MachineConnection::new("lab.thermostat.bus", profile, Box::new(MockAdapter::new()))
                .unwrap();
        let mut bus = MachineBus::new();
        bus.attach(connection).unwrap();
        bus.connect_all().unwrap();
        bus.enable_heartbeat_timers(1_000, 0).unwrap();

        assert!(bus.tick_heartbeat_timers_at(999).unwrap().is_empty());
        let health = bus.tick_heartbeat_timers_at(1_000).unwrap();

        assert_eq!(
            health
                .get(&MachineId::new("lab.thermostat.bus").unwrap())
                .unwrap()
                .status,
            HealthStatus::Online
        );
    }

    #[test]
    fn serial_adapter_scripts_line_frames() {
        let profile = serial_profile();
        let mut adapter = SerialAdapter::scripted("placeholder", 9600, [b"TARED".to_vec()]);
        adapter.open(&profile).unwrap();

        let outcome = adapter
            .execute(MachineCommand::empty("scale.tare").unwrap())
            .unwrap();

        assert_eq!(outcome.response, b"TARED");
        assert_eq!(adapter.outbound_frames(), &[b"scale.tare\n".to_vec()]);
        assert_eq!(
            outcome.state.get("serial.baud_rate"),
            Some(&MachineValue::I64(115_200))
        );
    }

    #[test]
    fn stream_serial_adapter_uses_real_read_write_streams() {
        let stream = MemoryDuplex::new(b"TARED\n".to_vec());
        let mut adapter = StreamSerialAdapter::new(stream);
        adapter.open(&serial_profile()).unwrap();
        let outcome = adapter
            .execute(MachineCommand::empty("scale.tare").unwrap())
            .unwrap();
        let stream = adapter.into_inner();

        assert_eq!(outcome.response, b"TARED".to_vec());
        assert_eq!(stream.written, b"scale.tare\n".to_vec());
    }

    #[test]
    fn tcp_adapter_scripts_length_prefixed_frames() {
        let profile = tcp_profile();
        let mut adapter = TcpAdapter::scripted("0.0.0.0", 1, [b"HOMED".to_vec()]);
        adapter.open(&profile).unwrap();

        let outcome = adapter
            .execute(MachineCommand::empty("robot.home").unwrap())
            .unwrap();

        assert_eq!(outcome.response, b"HOMED");
        assert_eq!(
            adapter.outbound_frames(),
            &[vec![
                0, 11, b'r', b'o', b'b', b'o', b't', b'.', b'h', b'o', b'm', b'e', 0
            ]]
        );
    }

    #[test]
    fn tcp_stream_adapter_talks_to_loopback_socket() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let server = thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            let mut len = [0_u8; 2];
            socket.read_exact(&mut len).unwrap();
            let len = u16::from_be_bytes(len) as usize;
            let mut body = vec![0_u8; len];
            socket.read_exact(&mut body).unwrap();
            assert_eq!(body, b"robot.home\0");
            socket.write_all(&3_u16.to_be_bytes()).unwrap();
            socket.write_all(b"ACK").unwrap();
        });

        let profile = DeviceProfile::new(
            "cell.robot",
            "Cell Robot",
            AdapterKind::Tcp,
            TransportProfile::Tcp {
                host: addr.ip().to_string(),
                port: addr.port(),
            },
            ProtocolProfile::TcpFrames {
                max_frame_bytes: 64,
            },
        )
        .unwrap()
        .with_capability(DeviceCapability::health("cell.robot").unwrap())
        .with_capability(DeviceCapability::command("robot.home").unwrap());
        let mut adapter = TcpStreamAdapter::new();
        adapter.open(&profile).unwrap();
        let outcome = adapter
            .execute(MachineCommand::empty("robot.home").unwrap())
            .unwrap();
        server.join().unwrap();

        assert_eq!(outcome.response, b"ACK".to_vec());
    }

    #[test]
    fn modbus_like_adapter_reads_and_writes_registers() {
        let profile = modbus_profile();
        let mut adapter = ModbusLikeAdapter::new(1)
            .with_register(40_001, 120)
            .map_command(
                "plc.speed.read",
                ModbusOperation::ReadHolding { register: 40_001 },
            )
            .unwrap()
            .map_command(
                "plc.speed.write",
                ModbusOperation::WritePayloadU16 { register: 40_001 },
            )
            .unwrap();
        adapter.open(&profile).unwrap();

        let read = adapter
            .execute(MachineCommand::empty("plc.speed.read").unwrap())
            .unwrap();
        assert_eq!(read.response, 120_u16.to_be_bytes());

        let write = adapter
            .execute(MachineCommand::payload_u16("plc.speed.write", 240).unwrap())
            .unwrap();
        assert_eq!(write.response, 240_u16.to_be_bytes());
        assert_eq!(adapter.register(40_001), Some(240));
        assert_eq!(adapter.transactions().len(), 2);
        assert_eq!(
            write.state.get("modbus.register.40001"),
            Some(&MachineValue::I64(240))
        );
    }

    #[test]
    fn adapter_health_moves_offline_on_close() {
        let profile = mock_profile();
        let mut adapter = MockAdapter::new();

        adapter.open(&profile).unwrap();
        adapter.close().unwrap();

        assert_eq!(adapter.health().status, HealthStatus::Offline);
        assert!(matches!(
            adapter.read_state(),
            Err(ZapMachineError::AdapterNotOpen {
                adapter: AdapterKind::Mock
            })
        ));
    }

    #[test]
    fn rejects_profile_adapter_shape_mismatches() {
        let error = DeviceProfile::new(
            "bad.profile",
            "Bad Profile",
            AdapterKind::Serial,
            TransportProfile::Tcp {
                host: "127.0.0.1".to_string(),
                port: 502,
            },
            ProtocolProfile::SerialLine {
                delimiter: "\n".to_string(),
            },
        )
        .unwrap_err();

        assert!(matches!(error, ZapMachineError::InvalidAdapterShape { .. }));
    }
}
