//! Capability discovery and permission contracts for ZAP.
//!
//! Capabilities describe what a node or driver can do. They do not grant
//! authority by themselves; node config, signed manifests, registries, and
//! policies remain the enforcement points.

use serde::{Deserialize, Deserializer, Serialize};
use std::{
    collections::{BTreeSet, HashSet},
    fmt,
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    str::FromStr,
    time::{SystemTime, UNIX_EPOCH},
};
use thiserror::Error;
use uuid::Uuid;

pub const CAPABILITY_QUERY_SUBJECT: &str = "zap.capability.query";
pub const CAPABILITY_RESPONSE_SUBJECT: &str = "zap.capability.response";
pub const CAPABILITY_ANNOUNCE_SUBJECT: &str = "zap.capability.announce";
pub const CAPABILITY_CONTENT_TYPE: &str = "application/zap-capability+json";

const DRIVER_EXECUTE_PREFIX: &str = "driver.execute:";
const HASH_PREFIX: &str = "blake3:";
pub const CAPABILITY_CACHE_SCHEMA_VERSION: u8 = 1;

#[derive(Debug, Error)]
pub enum ZapCapabilityError {
    #[error("capability id must not be empty")]
    EmptyCapabilityId,
    #[error("capability id `{0}` exceeds maximum length of 128 bytes")]
    CapabilityIdTooLong(String),
    #[error("capability id `{0}` contains invalid characters")]
    InvalidCapabilityId(String),
    #[error("capability cache contains no entries")]
    EmptyCapabilityCache,
    #[error("capability cache entry at line {line} has invalid schema version {version}")]
    UnsupportedCacheSchemaVersion { line: usize, version: u8 },
    #[error("capability cache entry {id} hash mismatch at line {line}")]
    CacheEntryHashMismatch { line: usize, id: Uuid },
    #[error("capability cache entry {id} chain mismatch at line {line}")]
    CacheEntryChainMismatch { line: usize, id: Uuid },
    #[error("capability cache entry id {id} is duplicated at line {line}")]
    DuplicateCacheEntryId { line: usize, id: Uuid },
    #[error(
        "capability cache entry {id} peer {peer_node_id} does not match advertisement node {advertisement_node_id}"
    )]
    CachePeerMismatch {
        id: Uuid,
        peer_node_id: Uuid,
        advertisement_node_id: Uuid,
    },
    #[error("capability cache entry {id} grant `{capability}` is not advertised")]
    CacheGrantNotAdvertised { id: Uuid, capability: CapabilityId },
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("system clock is before Unix epoch")]
    ClockBeforeUnixEpoch,
}

pub type Result<T> = std::result::Result<T, ZapCapabilityError>;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(transparent)]
pub struct CapabilityId(String);

impl CapabilityId {
    pub fn new(input: impl Into<String>) -> Result<Self> {
        let input = input.into();
        validate_capability_id(&input)?;
        Ok(Self(input))
    }

    pub fn driver_execute(action: impl AsRef<str>) -> Result<Self> {
        Self::new(format!("{DRIVER_EXECUTE_PREFIX}{}", action.as_ref()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn driver_action(&self) -> Option<&str> {
        self.0.strip_prefix(DRIVER_EXECUTE_PREFIX)
    }
}

impl fmt::Display for CapabilityId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl FromStr for CapabilityId {
    type Err = ZapCapabilityError;

    fn from_str(input: &str) -> Result<Self> {
        Self::new(input)
    }
}

impl<'de> Deserialize<'de> for CapabilityId {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let input = String::deserialize(deserializer)?;
        Self::new(input).map_err(serde::de::Error::custom)
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct DriverPermissions {
    pub network: bool,
    pub filesystem: bool,
    pub clock: bool,
    pub environment: bool,
}

impl DriverPermissions {
    pub const fn none() -> Self {
        Self {
            network: false,
            filesystem: false,
            clock: false,
            environment: false,
        }
    }

    pub const fn merge(self, other: Self) -> Self {
        Self {
            network: self.network || other.network,
            filesystem: self.filesystem || other.filesystem,
            clock: self.clock || other.clock,
            environment: self.environment || other.environment,
        }
    }
}

impl Default for DriverPermissions {
    fn default() -> Self {
        Self::none()
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct CapabilitySet {
    #[serde(default)]
    pub capabilities: BTreeSet<CapabilityId>,
}

impl CapabilitySet {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, capability: CapabilityId) -> bool {
        self.capabilities.insert(capability)
    }

    pub fn contains(&self, capability: &CapabilityId) -> bool {
        self.capabilities.contains(capability)
    }

    pub fn iter(&self) -> impl Iterator<Item = &CapabilityId> {
        self.capabilities.iter()
    }

    pub fn filtered(&self, requested: &[CapabilityId]) -> Self {
        if requested.is_empty() {
            return self.clone();
        }
        let capabilities = requested
            .iter()
            .filter(|capability| self.contains(capability))
            .cloned()
            .collect();
        Self { capabilities }
    }

    pub fn with_driver(action: impl AsRef<str>) -> Result<Self> {
        let mut set = Self::new();
        set.insert(CapabilityId::driver_execute(action)?);
        Ok(set)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CapabilityGrant {
    pub capability: CapabilityId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CapabilityRequirement {
    pub capability: CapabilityId,
    #[serde(default)]
    pub required: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CapabilityAdvertisement {
    pub schema_version: u8,
    pub node_id: Uuid,
    #[serde(default)]
    pub capabilities: CapabilitySet,
    #[serde(default)]
    pub grants: Vec<CapabilityGrant>,
    #[serde(default)]
    pub requirements: Vec<CapabilityRequirement>,
}

impl CapabilityAdvertisement {
    pub fn new(node_id: Uuid) -> Self {
        Self {
            schema_version: 1,
            node_id,
            capabilities: CapabilitySet::new(),
            grants: Vec::new(),
            requirements: Vec::new(),
        }
    }

    pub fn filtered(&self, requested: &[CapabilityId]) -> Self {
        let mut filtered = self.clone();
        filtered.capabilities = self.capabilities.filtered(requested);
        filtered.grants.retain(|grant| {
            requested.is_empty()
                || requested
                    .iter()
                    .any(|capability| capability == &grant.capability)
        });
        filtered.requirements.retain(|requirement| {
            requested.is_empty()
                || requested
                    .iter()
                    .any(|capability| capability == &requirement.capability)
        });
        filtered
    }

    pub fn grants_capability(&self, capability: &CapabilityId) -> bool {
        self.capabilities.contains(capability)
            && self
                .grants
                .iter()
                .any(|grant| &grant.capability == capability)
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct CapabilityQuery {
    #[serde(default)]
    pub requested: Vec<CapabilityId>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CapabilityResponse {
    pub advertisement: CapabilityAdvertisement,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CapabilityCacheEntry {
    pub schema_version: u8,
    pub id: Uuid,
    pub peer_node_id: Uuid,
    pub observed_at_micros: u64,
    pub advertisement: CapabilityAdvertisement,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub previous_entry_hash: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entry_hash: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CapabilityCacheVerificationReport {
    pub path: PathBuf,
    pub entries: usize,
    pub peers: usize,
    pub verified: bool,
}

#[derive(Debug, Clone)]
pub struct JsonlCapabilityCache {
    path: PathBuf,
}

impl JsonlCapabilityCache {
    pub fn open(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn put(
        &self,
        peer_node_id: Uuid,
        advertisement: CapabilityAdvertisement,
    ) -> Result<CapabilityCacheEntry> {
        let entry = CapabilityCacheEntry {
            schema_version: CAPABILITY_CACHE_SCHEMA_VERSION,
            id: Uuid::new_v4(),
            peer_node_id,
            observed_at_micros: now_micros()?,
            advertisement,
            previous_entry_hash: None,
            entry_hash: None,
        };
        validate_cache_entry(&entry)?;
        let sealed = seal_cache_entry(entry, self.last_verified_entry_hash()?)?;
        self.append_entry(&sealed)?;
        Ok(sealed)
    }

    pub fn entries(&self) -> Result<Vec<CapabilityCacheEntry>> {
        let entries = self.load_entries()?;
        verify_cache_entries(&self.path, &entries, false)?;
        Ok(entries)
    }

    pub fn latest_for_peer(&self, peer_node_id: Uuid) -> Result<Option<CapabilityCacheEntry>> {
        Ok(self
            .entries()?
            .into_iter()
            .rev()
            .find(|entry| entry.peer_node_id == peer_node_id))
    }

    pub fn verify(&self) -> Result<CapabilityCacheVerificationReport> {
        let entries = self.load_entries()?;
        verify_cache_entries(&self.path, &entries, true)
    }

    fn append_entry(&self, entry: &CapabilityCacheEntry) -> Result<()> {
        if let Some(parent) = self
            .path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            fs::create_dir_all(parent)?;
        }
        let mut encoded = serde_json::to_string(entry)?;
        encoded.push('\n');
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)?;
        file.write_all(encoded.as_bytes())?;
        Ok(())
    }

    fn load_entries(&self) -> Result<Vec<CapabilityCacheEntry>> {
        if !self.path.exists() {
            return Ok(Vec::new());
        }
        let input = fs::read_to_string(&self.path)?;
        input
            .lines()
            .filter(|line| !line.trim().is_empty())
            .map(|line| Ok(serde_json::from_str(line)?))
            .collect()
    }

    fn last_verified_entry_hash(&self) -> Result<Option<String>> {
        let entries = self.load_entries()?;
        verify_cache_entries(&self.path, &entries, false)?;
        match entries.last() {
            Some(entry) => Ok(Some(compute_cache_entry_hash(entry)?)),
            None => Ok(None),
        }
    }
}

pub fn capabilities_for_driver(
    action: impl AsRef<str>,
    permissions: DriverPermissions,
) -> Result<CapabilitySet> {
    let mut set = CapabilitySet::with_driver(action)?;
    if permissions.network {
        set.insert(CapabilityId::new("host.network")?);
    }
    if permissions.filesystem {
        set.insert(CapabilityId::new("host.filesystem")?);
    }
    if permissions.clock {
        set.insert(CapabilityId::new("host.clock")?);
    }
    if permissions.environment {
        set.insert(CapabilityId::new("host.environment")?);
    }
    Ok(set)
}

fn validate_capability_id(input: &str) -> Result<()> {
    if input.trim().is_empty() {
        return Err(ZapCapabilityError::EmptyCapabilityId);
    }
    if input.len() > 128 {
        return Err(ZapCapabilityError::CapabilityIdTooLong(input.to_string()));
    }
    if !input
        .bytes()
        .all(|byte| matches!(byte, b'a'..=b'z' | b'0'..=b'9' | b'.' | b':' | b'_' | b'-' | b'*'))
    {
        return Err(ZapCapabilityError::InvalidCapabilityId(input.to_string()));
    }
    Ok(())
}

pub fn hash_bytes(bytes: &[u8]) -> String {
    format!("{HASH_PREFIX}{}", blake3::hash(bytes).to_hex())
}

fn seal_cache_entry(
    mut entry: CapabilityCacheEntry,
    previous_entry_hash: Option<String>,
) -> Result<CapabilityCacheEntry> {
    entry.previous_entry_hash = previous_entry_hash;
    entry.entry_hash = None;
    let entry_hash = compute_cache_entry_hash(&entry)?;
    entry.entry_hash = Some(entry_hash);
    Ok(entry)
}

fn compute_cache_entry_hash(entry: &CapabilityCacheEntry) -> Result<String> {
    let mut transcript = entry.clone();
    transcript.entry_hash = None;
    Ok(hash_bytes(&serde_json::to_vec(&transcript)?))
}

fn verify_cache_entries(
    path: &Path,
    entries: &[CapabilityCacheEntry],
    require_non_empty: bool,
) -> Result<CapabilityCacheVerificationReport> {
    if require_non_empty && entries.is_empty() {
        return Err(ZapCapabilityError::EmptyCapabilityCache);
    }

    let mut seen_ids = HashSet::new();
    let mut peers = HashSet::new();
    let mut expected_previous_hash = None;
    for (index, entry) in entries.iter().enumerate() {
        let line = index + 1;
        if entry.schema_version != CAPABILITY_CACHE_SCHEMA_VERSION {
            return Err(ZapCapabilityError::UnsupportedCacheSchemaVersion {
                line,
                version: entry.schema_version,
            });
        }
        if !seen_ids.insert(entry.id) {
            return Err(ZapCapabilityError::DuplicateCacheEntryId { line, id: entry.id });
        }
        validate_cache_entry(entry)?;
        let computed_hash = compute_cache_entry_hash(entry)?;
        if let Some(recorded_hash) = entry.entry_hash.as_deref()
            && recorded_hash != computed_hash
        {
            return Err(ZapCapabilityError::CacheEntryHashMismatch { line, id: entry.id });
        }
        let entry_uses_chain = entry.entry_hash.is_some() || entry.previous_entry_hash.is_some();
        if entry_uses_chain
            && entry.previous_entry_hash.as_deref() != expected_previous_hash.as_deref()
        {
            return Err(ZapCapabilityError::CacheEntryChainMismatch { line, id: entry.id });
        }
        peers.insert(entry.peer_node_id);
        expected_previous_hash = Some(computed_hash);
    }

    Ok(CapabilityCacheVerificationReport {
        path: path.to_path_buf(),
        entries: entries.len(),
        peers: peers.len(),
        verified: true,
    })
}

fn validate_cache_entry(entry: &CapabilityCacheEntry) -> Result<()> {
    if entry.peer_node_id != entry.advertisement.node_id {
        return Err(ZapCapabilityError::CachePeerMismatch {
            id: entry.id,
            peer_node_id: entry.peer_node_id,
            advertisement_node_id: entry.advertisement.node_id,
        });
    }
    for grant in &entry.advertisement.grants {
        if !entry.advertisement.capabilities.contains(&grant.capability) {
            return Err(ZapCapabilityError::CacheGrantNotAdvertised {
                id: entry.id,
                capability: grant.capability.clone(),
            });
        }
    }
    Ok(())
}

fn now_micros() -> Result<u64> {
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| ZapCapabilityError::ClockBeforeUnixEpoch)?;
    Ok(duration.as_micros() as u64)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn advertisement(node_id: Uuid, capabilities: &[&str]) -> CapabilityAdvertisement {
        let mut advertisement = CapabilityAdvertisement::new(node_id);
        for capability in capabilities {
            advertisement
                .capabilities
                .insert(CapabilityId::new(*capability).unwrap());
        }
        advertisement
    }

    #[test]
    fn validates_capability_ids() {
        assert!(CapabilityId::new("driver.execute:echo").is_ok());
        assert!(CapabilityId::new("").is_err());
        assert!(CapabilityId::new("Driver.Execute").is_err());
    }

    #[test]
    fn maps_driver_permissions_to_capabilities() {
        let mut permissions = DriverPermissions::none();
        permissions.clock = true;
        let set = capabilities_for_driver("thermostat.setpoint", permissions).unwrap();
        assert!(set.contains(&CapabilityId::new("driver.execute:thermostat.setpoint").unwrap()));
        assert!(set.contains(&CapabilityId::new("host.clock").unwrap()));
    }

    #[test]
    fn filters_advertisement_by_query() {
        let node_id = Uuid::nil();
        let mut advertisement = CapabilityAdvertisement::new(node_id);
        advertisement
            .capabilities
            .insert(CapabilityId::new("driver.execute:echo").unwrap());
        advertisement
            .capabilities
            .insert(CapabilityId::new("host.clock").unwrap());

        let filtered = advertisement.filtered(&[CapabilityId::new("host.clock").unwrap()]);
        assert_eq!(filtered.capabilities.capabilities.len(), 1);
        assert!(
            filtered
                .capabilities
                .contains(&CapabilityId::new("host.clock").unwrap())
        );
    }

    #[test]
    fn checks_granted_capabilities() {
        let node_id = Uuid::nil();
        let capability = CapabilityId::new("driver.execute:echo").unwrap();
        let mut advertisement = CapabilityAdvertisement::new(node_id);
        advertisement.capabilities.insert(capability.clone());
        assert!(!advertisement.grants_capability(&capability));

        advertisement.grants.push(CapabilityGrant {
            capability: capability.clone(),
            reason: Some("approved".to_string()),
        });
        assert!(advertisement.grants_capability(&capability));
    }

    #[test]
    fn serde_rejects_invalid_capability_ids() {
        let error = serde_json::from_str::<CapabilityQuery>(r#"{"requested":["Bad"]}"#)
            .unwrap_err()
            .to_string();
        assert!(error.contains("invalid characters"));
    }

    #[test]
    fn capability_cache_stores_latest_and_verifies_chain() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("capabilities.jsonl");
        let cache = JsonlCapabilityCache::open(&path);
        let peer = Uuid::new_v4();
        let first = cache
            .put(peer, advertisement(peer, &["driver.execute:echo"]))
            .unwrap();
        let second = cache
            .put(
                peer,
                advertisement(peer, &["driver.execute:echo", "memory.local"]),
            )
            .unwrap();

        assert!(first.previous_entry_hash.is_none());
        assert_eq!(second.previous_entry_hash, first.entry_hash);
        let latest = cache.latest_for_peer(peer).unwrap().unwrap();
        assert_eq!(latest.id, second.id);
        assert!(
            latest
                .advertisement
                .capabilities
                .contains(&CapabilityId::new("memory.local").unwrap())
        );
        let report = cache.verify().unwrap();
        assert_eq!(report.entries, 2);
        assert_eq!(report.peers, 1);
    }

    #[test]
    fn capability_cache_detects_tampered_entry_hash() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("capabilities.jsonl");
        let cache = JsonlCapabilityCache::open(&path);
        let peer = Uuid::new_v4();
        let entry = cache
            .put(peer, advertisement(peer, &["driver.execute:echo"]))
            .unwrap();

        let mut line: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        line["entry_hash"] = serde_json::Value::String(hash_bytes(b"tampered"));
        fs::write(
            &path,
            format!("{}\n", serde_json::to_string(&line).unwrap()),
        )
        .unwrap();

        assert!(matches!(
            cache.verify(),
            Err(ZapCapabilityError::CacheEntryHashMismatch { line: 1, id }) if id == entry.id
        ));
    }

    #[test]
    fn capability_cache_detects_removed_middle_entry() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("capabilities.jsonl");
        let cache = JsonlCapabilityCache::open(&path);
        let peer = Uuid::new_v4();
        cache
            .put(peer, advertisement(peer, &["driver.execute:first"]))
            .unwrap();
        cache
            .put(peer, advertisement(peer, &["driver.execute:second"]))
            .unwrap();
        let third = cache
            .put(peer, advertisement(peer, &["driver.execute:third"]))
            .unwrap();

        let lines = fs::read_to_string(&path).unwrap();
        let mut lines = lines.lines().collect::<Vec<_>>();
        lines.remove(1);
        fs::write(&path, format!("{}\n", lines.join("\n"))).unwrap();

        assert!(matches!(
            cache.verify(),
            Err(ZapCapabilityError::CacheEntryChainMismatch { line: 2, id }) if id == third.id
        ));
    }

    #[test]
    fn capability_cache_rejects_grants_for_missing_capabilities() {
        let dir = tempfile::tempdir().unwrap();
        let cache = JsonlCapabilityCache::open(dir.path().join("capabilities.jsonl"));
        let peer = Uuid::new_v4();
        let mut advertisement = advertisement(peer, &["driver.execute:echo"]);
        advertisement.grants.push(CapabilityGrant {
            capability: CapabilityId::new("memory.local").unwrap(),
            reason: Some("not actually advertised".to_string()),
        });

        assert!(matches!(
            cache.put(peer, advertisement),
            Err(ZapCapabilityError::CacheGrantNotAdvertised { capability, .. })
                if capability == CapabilityId::new("memory.local").unwrap()
        ));
    }
}
