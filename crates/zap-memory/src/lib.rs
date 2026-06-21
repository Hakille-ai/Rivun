//! Auditable local memory stores for ZAP.

use base64::{Engine as _, engine::general_purpose::STANDARD_NO_PAD};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::{HashMap, HashSet},
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};
use thiserror::Error;
use uuid::Uuid;
use zap_journal::{JournalProfile, JournalQuery, JournalRecord, JournalRecordInput, JournalStore};

pub const MEMORY_SCHEMA_VERSION: u8 = 1;
pub const DEFAULT_MEMORY_CONTENT_TYPE: &str = "application/octet-stream";
pub const DEFAULT_MEMORY_MAX_RECORD_BYTES: usize = 1024 * 1024;

const HASH_PREFIX: &str = "blake3:";
const MEMORY_RECORD_KIND: &str = "memory.record";
const MEMORY_TOMBSTONE_KIND: &str = "memory.tombstone";

#[derive(Debug, Error)]
pub enum ZapMemoryError {
    #[error("memory namespace must not be empty")]
    EmptyNamespace,
    #[error("memory subject must not be empty")]
    EmptySubject,
    #[error("memory record {actual} bytes exceeds max {max}")]
    RecordTooLarge { max: usize, actual: usize },
    #[error("memory record {0} was not found")]
    NotFound(Uuid),
    #[error("memory store contains no records")]
    EmptyStore,
    #[error("memory entry at line {line} has invalid schema version {version}")]
    UnsupportedSchemaVersion { line: usize, version: u8 },
    #[error("memory record {id} body hash mismatch at line {line}")]
    BodyHashMismatch { line: usize, id: Uuid },
    #[error("memory entry {id} hash mismatch at line {line}")]
    EntryHashMismatch { line: usize, id: Uuid },
    #[error("memory entry {id} chain mismatch at line {line}")]
    EntryChainMismatch { line: usize, id: Uuid },
    #[error("memory entry id {id} is duplicated at line {line}")]
    DuplicateEntryId { line: usize, id: Uuid },
    #[error("memory tombstone {id} references missing record {record_id} at line {line}")]
    TombstoneTargetMissing {
        line: usize,
        id: Uuid,
        record_id: Uuid,
    },
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("base64 error: {0}")]
    Base64(#[from] base64::DecodeError),
    #[error("memory journal error: {0}")]
    Journal(#[from] zap_journal::ZapJournalError),
    #[error("system clock is before Unix epoch")]
    ClockBeforeUnixEpoch,
}

pub type Result<T> = std::result::Result<T, ZapMemoryError>;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MemoryRecord {
    pub schema_version: u8,
    pub id: Uuid,
    pub namespace: String,
    pub subject: String,
    pub content_type: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub previous_entry_hash: Option<String>,
    pub body_hash: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body_base64: Option<String>,
    #[serde(default, skip_serializing_if = "Value::is_null")]
    pub metadata: Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_node: Option<Uuid>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frame_hash: Option<String>,
    pub created_at_micros: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entry_hash: Option<String>,
}

impl MemoryRecord {
    pub fn body_bytes(&self) -> Result<Vec<u8>> {
        match &self.body_base64 {
            Some(body) => Ok(STANDARD_NO_PAD.decode(body)?),
            None => Ok(Vec::new()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MemoryTombstone {
    pub schema_version: u8,
    pub id: Uuid,
    pub record_id: Uuid,
    pub namespace: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub previous_entry_hash: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    pub created_at_micros: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entry_hash: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct MemoryNamespace {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct MemoryQuery {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub namespace: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subject: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_type: Option<String>,
    #[serde(default)]
    pub include_tombstoned: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MemoryVerificationReport {
    pub path: PathBuf,
    pub entries: usize,
    pub records: usize,
    pub tombstones: usize,
    pub verified: bool,
}

pub trait MemoryStore {
    fn put(&self, input: MemoryPut) -> Result<MemoryRecord>;
    fn get(&self, id: Uuid) -> Result<MemoryRecord>;
    fn query(&self, query: &MemoryQuery) -> Result<Vec<MemoryRecord>>;
    fn tombstone(&self, record_id: Uuid, reason: Option<String>) -> Result<MemoryTombstone>;
    fn verify(&self) -> Result<MemoryVerificationReport>;
}

#[derive(Debug, Clone)]
pub struct MemoryPut {
    pub namespace: String,
    pub subject: String,
    pub content_type: String,
    pub body: Vec<u8>,
    pub metadata: Value,
    pub source_node: Option<Uuid>,
    pub frame_hash: Option<String>,
}

#[derive(Debug, Clone)]
pub struct JsonlMemoryStore {
    path: PathBuf,
    max_record_bytes: usize,
}

#[derive(Debug, Clone)]
pub struct MemoryJournalStore {
    journal: JournalStore,
    max_record_bytes: usize,
}

impl MemoryJournalStore {
    pub fn open(dir: impl Into<PathBuf>) -> Self {
        Self {
            journal: JournalStore::open(dir, JournalProfile::Memory),
            max_record_bytes: DEFAULT_MEMORY_MAX_RECORD_BYTES,
        }
    }

    pub fn with_max_record_bytes(mut self, max_record_bytes: usize) -> Self {
        self.max_record_bytes = max_record_bytes.max(1);
        self
    }

    pub fn dir(&self) -> &Path {
        self.journal.dir()
    }

    pub fn import_jsonl(&self, input: &Path, force: bool) -> Result<usize> {
        JournalStore::remove_dir_if_allowed(self.dir(), force)?;
        fs::create_dir_all(self.dir())?;
        let entries = parse_entries(input)?;
        verify_entries(input, &entries, false)?;
        for entry in &entries {
            self.append_imported_entry(entry)?;
        }
        Ok(entries.len())
    }

    pub fn export_jsonl(&self, out: &Path, force: bool) -> Result<usize> {
        let entries = self.all_entries()?;
        write_rechained_entries(out, &entries, force)?;
        Ok(entries.len())
    }

    pub fn prune_to(
        &self,
        before_created_at_micros: u64,
        out: &Path,
        force: bool,
    ) -> Result<usize> {
        JournalStore::remove_dir_if_allowed(out, force)?;
        let entries = self.all_entries()?;
        let mut retained = Vec::new();
        let mut pruned = 0_usize;
        for entry in entries {
            if entry.created_at_micros() < before_created_at_micros {
                pruned += 1;
            } else {
                retained.push(entry);
            }
        }
        let retained_record_ids = retained
            .iter()
            .filter_map(|entry| match entry {
                MemoryEntry::Record(record) => Some(record.id),
                MemoryEntry::Tombstone(_) => None,
            })
            .collect::<HashSet<_>>();
        retained.retain(|entry| match entry {
            MemoryEntry::Record(_) => true,
            MemoryEntry::Tombstone(tombstone) => {
                let keep = retained_record_ids.contains(&tombstone.record_id);
                if !keep {
                    pruned += 1;
                }
                keep
            }
        });
        let compacted = MemoryJournalStore::open(out).with_max_record_bytes(self.max_record_bytes);
        for entry in &retained {
            compacted.append_imported_entry(entry)?;
        }
        Ok(pruned)
    }

    pub fn rebuild_indexes(&self) -> Result<MemoryVerificationReport> {
        self.journal.rebuild_indexes()?;
        self.verify()
    }

    pub fn recover_partial_tail(&self) -> Result<bool> {
        Ok(self.journal.recover_partial_tail()?.is_some())
    }

    fn append_imported_entry(&self, entry: &MemoryEntry) -> Result<()> {
        match entry {
            MemoryEntry::Record(record) => {
                let body = record.body_bytes()?;
                self.append_record(record.clone(), body, false)?;
            }
            MemoryEntry::Tombstone(tombstone) => {
                self.append_tombstone(tombstone.clone(), false)?;
            }
        }
        Ok(())
    }

    fn append_record(
        &self,
        mut record: MemoryRecord,
        body: Vec<u8>,
        sync_data: bool,
    ) -> Result<MemoryRecord> {
        let metadata_bytes = serde_json::to_vec(&record.metadata)?;
        let actual = body.len().saturating_add(metadata_bytes.len());
        if actual > self.max_record_bytes {
            return Err(ZapMemoryError::RecordTooLarge {
                max: self.max_record_bytes,
                actual,
            });
        }
        record.body_hash = hash_bytes(&body);
        record.body_base64 = Some(STANDARD_NO_PAD.encode(&body));
        record.previous_entry_hash = None;
        record.entry_hash = None;
        let journal_record = self.journal.append(
            JournalRecordInput {
                kind: MEMORY_RECORD_KIND.to_string(),
                schema_version: MEMORY_SCHEMA_VERSION as u16,
                timestamp_micros: record.created_at_micros,
                id: Some(record.id),
                namespace: Some(record.namespace.clone()),
                subject: Some(record.subject.clone()),
                content_type: Some(record.content_type.clone()),
                source_node: record.source_node,
                target_node: None,
                tombstone_for: None,
                metadata: serde_json::json!({
                    "metadata": record.metadata,
                    "frame_hash": record.frame_hash,
                }),
                payload: body,
            },
            sync_data,
        )?;
        memory_record_from_journal(&journal_record)
    }

    fn append_tombstone(
        &self,
        mut tombstone: MemoryTombstone,
        sync_data: bool,
    ) -> Result<MemoryTombstone> {
        tombstone.previous_entry_hash = None;
        tombstone.entry_hash = None;
        let journal_record = self.journal.append(
            JournalRecordInput {
                kind: MEMORY_TOMBSTONE_KIND.to_string(),
                schema_version: MEMORY_SCHEMA_VERSION as u16,
                timestamp_micros: tombstone.created_at_micros,
                id: Some(tombstone.id),
                namespace: Some(tombstone.namespace.clone()),
                subject: None,
                content_type: None,
                source_node: None,
                target_node: None,
                tombstone_for: Some(tombstone.record_id),
                metadata: serde_json::json!({
                    "reason": tombstone.reason,
                }),
                payload: Vec::new(),
            },
            sync_data,
        )?;
        memory_tombstone_from_journal(&journal_record)
    }

    fn all_entries(&self) -> Result<Vec<MemoryEntry>> {
        let mut entries = Vec::new();
        for record in self.journal.records()? {
            match record.kind.as_str() {
                MEMORY_RECORD_KIND => {
                    entries.push(MemoryEntry::Record(memory_record_from_journal(&record)?))
                }
                MEMORY_TOMBSTONE_KIND => {
                    entries.push(MemoryEntry::Tombstone(memory_tombstone_from_journal(
                        &record,
                    )?));
                }
                _ => {}
            }
        }
        Ok(entries)
    }
}

impl MemoryStore for MemoryJournalStore {
    fn put(&self, input: MemoryPut) -> Result<MemoryRecord> {
        validate_namespace(&input.namespace)?;
        validate_subject(&input.subject)?;
        let record = MemoryRecord {
            schema_version: MEMORY_SCHEMA_VERSION,
            id: Uuid::new_v4(),
            namespace: input.namespace,
            subject: input.subject,
            content_type: if input.content_type.trim().is_empty() {
                DEFAULT_MEMORY_CONTENT_TYPE.to_string()
            } else {
                input.content_type
            },
            previous_entry_hash: None,
            body_hash: hash_bytes(&input.body),
            body_base64: None,
            metadata: input.metadata,
            source_node: input.source_node,
            frame_hash: input.frame_hash,
            created_at_micros: now_micros()?,
            entry_hash: None,
        };
        self.append_record(record, input.body, false)
    }

    fn get(&self, id: Uuid) -> Result<MemoryRecord> {
        let tombstones = self.journal.query(&JournalQuery {
            kind: Some(MEMORY_TOMBSTONE_KIND.to_string()),
            tombstone_for: Some(id),
            limit: Some(1),
            ..JournalQuery::default()
        })?;
        if !tombstones.is_empty() {
            return Err(ZapMemoryError::NotFound(id));
        }
        let records = self.journal.query(&JournalQuery {
            kind: Some(MEMORY_RECORD_KIND.to_string()),
            id: Some(id),
            limit: Some(1),
            ..JournalQuery::default()
        })?;
        let record = records.first().ok_or(ZapMemoryError::NotFound(id))?;
        memory_record_from_journal(record)
    }

    fn query(&self, query: &MemoryQuery) -> Result<Vec<MemoryRecord>> {
        let tombstoned = if query.include_tombstoned {
            HashSet::new()
        } else {
            self.journal
                .query(&JournalQuery {
                    kind: Some(MEMORY_TOMBSTONE_KIND.to_string()),
                    ..JournalQuery::default()
                })?
                .into_iter()
                .filter_map(|record| record.tombstone_for)
                .collect::<HashSet<_>>()
        };
        let records = self.journal.query(&JournalQuery {
            kind: Some(MEMORY_RECORD_KIND.to_string()),
            namespace: query.namespace.clone(),
            subject: query.subject.clone(),
            content_type: query.content_type.clone(),
            limit: query.include_tombstoned.then_some(query.limit).flatten(),
            ..JournalQuery::default()
        })?;
        let mut out = Vec::new();
        for record in records {
            let memory = memory_record_from_journal(&record)?;
            if !tombstoned.contains(&memory.id) {
                out.push(memory);
                if let Some(limit) = query.limit
                    && out.len() >= limit
                {
                    break;
                }
            }
        }
        Ok(out)
    }

    fn tombstone(&self, record_id: Uuid, reason: Option<String>) -> Result<MemoryTombstone> {
        let record = self.get(record_id)?;
        self.append_tombstone(
            MemoryTombstone {
                schema_version: MEMORY_SCHEMA_VERSION,
                id: Uuid::new_v4(),
                record_id,
                namespace: record.namespace,
                previous_entry_hash: None,
                reason,
                created_at_micros: now_micros()?,
                entry_hash: None,
            },
            false,
        )
    }

    fn verify(&self) -> Result<MemoryVerificationReport> {
        self.journal.verify()?;
        let entries = self.all_entries()?;
        verify_journal_memory_entries(self.dir(), &entries, true)
    }
}

impl JsonlMemoryStore {
    pub fn open(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            max_record_bytes: DEFAULT_MEMORY_MAX_RECORD_BYTES,
        }
    }

    pub fn with_max_record_bytes(mut self, max_record_bytes: usize) -> Self {
        self.max_record_bytes = max_record_bytes.max(1);
        self
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn prune_to(
        &self,
        before_created_at_micros: u64,
        out: &Path,
        force: bool,
    ) -> Result<usize> {
        let entries = self.load_verified_entries(false)?;
        let mut retained = Vec::new();
        let mut pruned = 0_usize;
        for entry in entries {
            let created_at = entry.created_at_micros();
            if created_at < before_created_at_micros {
                pruned += 1;
            } else {
                retained.push(entry);
            }
        }
        let retained_record_ids = retained
            .iter()
            .filter_map(|entry| match entry {
                MemoryEntry::Record(record) => Some(record.id),
                MemoryEntry::Tombstone(_) => None,
            })
            .collect::<HashSet<_>>();
        retained.retain(|entry| match entry {
            MemoryEntry::Record(_) => true,
            MemoryEntry::Tombstone(tombstone) => {
                let keep = retained_record_ids.contains(&tombstone.record_id);
                if !keep {
                    pruned += 1;
                }
                keep
            }
        });
        write_rechained_entries(out, &retained, force)?;
        Ok(pruned)
    }

    fn append_entry(&self, entry: MemoryEntry) -> Result<MemoryEntry> {
        let previous_hash = self.last_verified_entry_hash()?;
        let sealed = seal_entry(entry, previous_hash)?;
        let mut encoded = serde_json::to_string(&sealed)?;
        encoded.push('\n');
        if encoded.len() > self.max_record_bytes {
            return Err(ZapMemoryError::RecordTooLarge {
                max: self.max_record_bytes,
                actual: encoded.len(),
            });
        }
        if let Some(parent) = self
            .path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            fs::create_dir_all(parent)?;
        }
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)?;
        file.write_all(encoded.as_bytes())?;
        Ok(sealed)
    }

    fn load_entries(&self) -> Result<Vec<MemoryEntry>> {
        if !self.path.exists() {
            return Ok(Vec::new());
        }
        parse_entries(&self.path)
    }

    fn load_verified_entries(&self, require_non_empty: bool) -> Result<Vec<MemoryEntry>> {
        let entries = self.load_entries()?;
        verify_entries(&self.path, &entries, require_non_empty)?;
        Ok(entries)
    }

    fn last_verified_entry_hash(&self) -> Result<Option<String>> {
        let entries = self.load_verified_entries(false)?;
        match entries.last() {
            Some(entry) => Ok(Some(compute_entry_hash(entry)?)),
            None => Ok(None),
        }
    }
}

impl MemoryStore for JsonlMemoryStore {
    fn put(&self, input: MemoryPut) -> Result<MemoryRecord> {
        validate_namespace(&input.namespace)?;
        validate_subject(&input.subject)?;
        let record = MemoryRecord {
            schema_version: MEMORY_SCHEMA_VERSION,
            id: Uuid::new_v4(),
            namespace: input.namespace,
            subject: input.subject,
            content_type: if input.content_type.trim().is_empty() {
                DEFAULT_MEMORY_CONTENT_TYPE.to_string()
            } else {
                input.content_type
            },
            previous_entry_hash: None,
            body_hash: hash_bytes(&input.body),
            body_base64: Some(STANDARD_NO_PAD.encode(&input.body)),
            metadata: input.metadata,
            source_node: input.source_node,
            frame_hash: input.frame_hash,
            created_at_micros: now_micros()?,
            entry_hash: None,
        };
        match self.append_entry(MemoryEntry::Record(record))? {
            MemoryEntry::Record(record) => Ok(record),
            MemoryEntry::Tombstone(_) => unreachable!("sealed memory record changed variant"),
        }
    }

    fn get(&self, id: Uuid) -> Result<MemoryRecord> {
        let mut records = HashMap::new();
        let mut tombstoned = HashSet::new();
        for entry in self.load_verified_entries(false)? {
            match entry {
                MemoryEntry::Record(record) => {
                    records.insert(record.id, record);
                }
                MemoryEntry::Tombstone(tombstone) => {
                    tombstoned.insert(tombstone.record_id);
                }
            }
        }
        if tombstoned.contains(&id) {
            return Err(ZapMemoryError::NotFound(id));
        }
        records.remove(&id).ok_or(ZapMemoryError::NotFound(id))
    }

    fn query(&self, query: &MemoryQuery) -> Result<Vec<MemoryRecord>> {
        let entries = self.load_verified_entries(false)?;
        let mut tombstoned = HashSet::new();
        for entry in &entries {
            if let MemoryEntry::Tombstone(tombstone) = entry {
                tombstoned.insert(tombstone.record_id);
            }
        }

        let mut records = Vec::new();
        for entry in entries {
            let MemoryEntry::Record(record) = entry else {
                continue;
            };
            if !query.include_tombstoned && tombstoned.contains(&record.id) {
                continue;
            }
            if let Some(namespace) = &query.namespace
                && &record.namespace != namespace
            {
                continue;
            }
            if let Some(subject) = &query.subject
                && &record.subject != subject
            {
                continue;
            }
            if let Some(content_type) = &query.content_type
                && &record.content_type != content_type
            {
                continue;
            }
            records.push(record);
            if let Some(limit) = query.limit
                && records.len() >= limit
            {
                break;
            }
        }
        Ok(records)
    }

    fn tombstone(&self, record_id: Uuid, reason: Option<String>) -> Result<MemoryTombstone> {
        let record = self.get(record_id)?;
        let tombstone = MemoryTombstone {
            schema_version: MEMORY_SCHEMA_VERSION,
            id: Uuid::new_v4(),
            record_id,
            namespace: record.namespace,
            previous_entry_hash: None,
            reason,
            created_at_micros: now_micros()?,
            entry_hash: None,
        };
        match self.append_entry(MemoryEntry::Tombstone(tombstone))? {
            MemoryEntry::Record(_) => unreachable!("sealed memory tombstone changed variant"),
            MemoryEntry::Tombstone(tombstone) => Ok(tombstone),
        }
    }

    fn verify(&self) -> Result<MemoryVerificationReport> {
        let entries = self.load_entries()?;
        verify_entries(&self.path, &entries, true)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "entry", rename_all = "snake_case")]
enum MemoryEntry {
    Record(MemoryRecord),
    Tombstone(MemoryTombstone),
}

impl MemoryEntry {
    fn created_at_micros(&self) -> u64 {
        match self {
            Self::Record(record) => record.created_at_micros,
            Self::Tombstone(tombstone) => tombstone.created_at_micros,
        }
    }

    fn id(&self) -> Uuid {
        match self {
            Self::Record(record) => record.id,
            Self::Tombstone(tombstone) => tombstone.id,
        }
    }

    fn previous_entry_hash(&self) -> Option<&str> {
        match self {
            Self::Record(record) => record.previous_entry_hash.as_deref(),
            Self::Tombstone(tombstone) => tombstone.previous_entry_hash.as_deref(),
        }
    }

    fn entry_hash(&self) -> Option<&str> {
        match self {
            Self::Record(record) => record.entry_hash.as_deref(),
            Self::Tombstone(tombstone) => tombstone.entry_hash.as_deref(),
        }
    }

    fn set_previous_entry_hash(&mut self, previous_entry_hash: Option<String>) {
        match self {
            Self::Record(record) => record.previous_entry_hash = previous_entry_hash,
            Self::Tombstone(tombstone) => tombstone.previous_entry_hash = previous_entry_hash,
        }
    }

    fn set_entry_hash(&mut self, entry_hash: Option<String>) {
        match self {
            Self::Record(record) => record.entry_hash = entry_hash,
            Self::Tombstone(tombstone) => tombstone.entry_hash = entry_hash,
        }
    }
}

pub fn hash_bytes(bytes: &[u8]) -> String {
    format!("{HASH_PREFIX}{}", blake3::hash(bytes).to_hex())
}

fn seal_entry(mut entry: MemoryEntry, previous_entry_hash: Option<String>) -> Result<MemoryEntry> {
    entry.set_previous_entry_hash(previous_entry_hash);
    entry.set_entry_hash(None);
    let entry_hash = compute_entry_hash(&entry)?;
    entry.set_entry_hash(Some(entry_hash));
    Ok(entry)
}

fn compute_entry_hash(entry: &MemoryEntry) -> Result<String> {
    let mut transcript = entry.clone();
    transcript.set_entry_hash(None);
    Ok(hash_bytes(&serde_json::to_vec(&transcript)?))
}

fn memory_record_from_journal(record: &JournalRecord) -> Result<MemoryRecord> {
    let id = record.id.ok_or(ZapMemoryError::NotFound(Uuid::nil()))?;
    let metadata = record
        .metadata
        .get("metadata")
        .cloned()
        .unwrap_or(Value::Null);
    let frame_hash = record
        .metadata
        .get("frame_hash")
        .and_then(Value::as_str)
        .map(ToString::to_string);
    Ok(MemoryRecord {
        schema_version: record.schema_version as u8,
        id,
        namespace: record.namespace.clone().unwrap_or_default(),
        subject: record.subject.clone().unwrap_or_default(),
        content_type: record
            .content_type
            .clone()
            .unwrap_or_else(|| DEFAULT_MEMORY_CONTENT_TYPE.to_string()),
        previous_entry_hash: non_zero_hash(&record.previous_entry_hash),
        body_hash: hash_bytes(&record.payload),
        body_base64: Some(STANDARD_NO_PAD.encode(&record.payload)),
        metadata,
        source_node: record.source_node,
        frame_hash,
        created_at_micros: record.timestamp_micros,
        entry_hash: Some(record.entry_hash.clone()),
    })
}

fn memory_tombstone_from_journal(record: &JournalRecord) -> Result<MemoryTombstone> {
    let id = record.id.ok_or(ZapMemoryError::NotFound(Uuid::nil()))?;
    let record_id = record
        .tombstone_for
        .ok_or(ZapMemoryError::NotFound(Uuid::nil()))?;
    let reason = record
        .metadata
        .get("reason")
        .and_then(Value::as_str)
        .map(ToString::to_string);
    Ok(MemoryTombstone {
        schema_version: record.schema_version as u8,
        id,
        record_id,
        namespace: record.namespace.clone().unwrap_or_default(),
        previous_entry_hash: non_zero_hash(&record.previous_entry_hash),
        reason,
        created_at_micros: record.timestamp_micros,
        entry_hash: Some(record.entry_hash.clone()),
    })
}

fn non_zero_hash(hash: &str) -> Option<String> {
    let zero = format!("{HASH_PREFIX}{}", "0".repeat(64));
    (hash != zero).then(|| hash.to_string())
}

fn verify_entries(
    path: &Path,
    entries: &[MemoryEntry],
    require_non_empty: bool,
) -> Result<MemoryVerificationReport> {
    if require_non_empty && entries.is_empty() {
        return Err(ZapMemoryError::EmptyStore);
    }

    let mut records = 0_usize;
    let mut tombstones = 0_usize;
    let mut seen_entry_ids = HashSet::new();
    let mut live_record_ids = HashSet::new();
    let mut expected_previous_hash = None;

    for (index, entry) in entries.iter().enumerate() {
        let line = index + 1;
        let entry_id = entry.id();
        if !seen_entry_ids.insert(entry_id) {
            return Err(ZapMemoryError::DuplicateEntryId { line, id: entry_id });
        }

        let computed_entry_hash = compute_entry_hash(entry)?;
        if let Some(recorded_entry_hash) = entry.entry_hash()
            && recorded_entry_hash != computed_entry_hash
        {
            return Err(ZapMemoryError::EntryHashMismatch { line, id: entry_id });
        }
        let entry_uses_chain =
            entry.entry_hash().is_some() || entry.previous_entry_hash().is_some();
        if entry_uses_chain && entry.previous_entry_hash() != expected_previous_hash.as_deref() {
            return Err(ZapMemoryError::EntryChainMismatch { line, id: entry_id });
        }

        match entry {
            MemoryEntry::Record(record) => {
                records += 1;
                if record.schema_version != MEMORY_SCHEMA_VERSION {
                    return Err(ZapMemoryError::UnsupportedSchemaVersion {
                        line,
                        version: record.schema_version,
                    });
                }
                let body = record.body_bytes()?;
                if record.body_hash != hash_bytes(&body) {
                    return Err(ZapMemoryError::BodyHashMismatch {
                        line,
                        id: record.id,
                    });
                }
                live_record_ids.insert(record.id);
            }
            MemoryEntry::Tombstone(tombstone) => {
                tombstones += 1;
                if tombstone.schema_version != MEMORY_SCHEMA_VERSION {
                    return Err(ZapMemoryError::UnsupportedSchemaVersion {
                        line,
                        version: tombstone.schema_version,
                    });
                }
                if !live_record_ids.contains(&tombstone.record_id) {
                    return Err(ZapMemoryError::TombstoneTargetMissing {
                        line,
                        id: tombstone.id,
                        record_id: tombstone.record_id,
                    });
                }
            }
        }

        expected_previous_hash = Some(computed_entry_hash);
    }

    Ok(MemoryVerificationReport {
        path: path.to_path_buf(),
        entries: entries.len(),
        records,
        tombstones,
        verified: true,
    })
}

fn verify_journal_memory_entries(
    path: &Path,
    entries: &[MemoryEntry],
    require_non_empty: bool,
) -> Result<MemoryVerificationReport> {
    if require_non_empty && entries.is_empty() {
        return Err(ZapMemoryError::EmptyStore);
    }

    let mut records = 0_usize;
    let mut tombstones = 0_usize;
    let mut seen_entry_ids = HashSet::new();
    let mut live_record_ids = HashSet::new();

    for (index, entry) in entries.iter().enumerate() {
        let line = index + 1;
        let entry_id = entry.id();
        if !seen_entry_ids.insert(entry_id) {
            return Err(ZapMemoryError::DuplicateEntryId { line, id: entry_id });
        }
        match entry {
            MemoryEntry::Record(record) => {
                records += 1;
                if record.schema_version != MEMORY_SCHEMA_VERSION {
                    return Err(ZapMemoryError::UnsupportedSchemaVersion {
                        line,
                        version: record.schema_version,
                    });
                }
                let body = record.body_bytes()?;
                if record.body_hash != hash_bytes(&body) {
                    return Err(ZapMemoryError::BodyHashMismatch {
                        line,
                        id: record.id,
                    });
                }
                live_record_ids.insert(record.id);
            }
            MemoryEntry::Tombstone(tombstone) => {
                tombstones += 1;
                if tombstone.schema_version != MEMORY_SCHEMA_VERSION {
                    return Err(ZapMemoryError::UnsupportedSchemaVersion {
                        line,
                        version: tombstone.schema_version,
                    });
                }
                if !live_record_ids.contains(&tombstone.record_id) {
                    return Err(ZapMemoryError::TombstoneTargetMissing {
                        line,
                        id: tombstone.id,
                        record_id: tombstone.record_id,
                    });
                }
            }
        }
    }

    Ok(MemoryVerificationReport {
        path: path.to_path_buf(),
        entries: entries.len(),
        records,
        tombstones,
        verified: true,
    })
}

fn parse_entries(path: &Path) -> Result<Vec<MemoryEntry>> {
    let input = fs::read_to_string(path)?;
    input
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| Ok(serde_json::from_str(line)?))
        .collect()
}

fn write_rechained_entries(path: &Path, entries: &[MemoryEntry], force: bool) -> Result<()> {
    if path.exists() && !force {
        return Err(std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            format!("{} already exists", path.display()),
        )
        .into());
    }
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)?;
    }
    let mut output = String::new();
    let mut previous_hash = None;
    for entry in entries {
        let sealed = seal_entry(entry.clone(), previous_hash)?;
        previous_hash = Some(compute_entry_hash(&sealed)?);
        output.push_str(&serde_json::to_string(&sealed)?);
        output.push('\n');
    }
    fs::write(path, output)?;
    Ok(())
}

fn validate_namespace(namespace: &str) -> Result<()> {
    if namespace.trim().is_empty() {
        return Err(ZapMemoryError::EmptyNamespace);
    }
    Ok(())
}

fn validate_subject(subject: &str) -> Result<()> {
    if subject.trim().is_empty() {
        return Err(ZapMemoryError::EmptySubject);
    }
    Ok(())
}

fn now_micros() -> Result<u64> {
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| ZapMemoryError::ClockBeforeUnixEpoch)?;
    Ok(duration.as_micros() as u64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn input(subject: &str, body: &[u8]) -> MemoryPut {
        MemoryPut {
            namespace: "default".to_string(),
            subject: subject.to_string(),
            content_type: "text/plain".to_string(),
            body: body.to_vec(),
            metadata: Value::Null,
            source_node: None,
            frame_hash: None,
        }
    }

    #[test]
    fn stores_queries_and_verifies_records() {
        let dir = tempfile::tempdir().unwrap();
        let store = JsonlMemoryStore::open(dir.path().join("memory.jsonl"));
        let record = store.put(input("note", b"hello")).unwrap();

        let loaded = store.get(record.id).unwrap();
        assert_eq!(loaded.body_bytes().unwrap(), b"hello");

        let matches = store
            .query(&MemoryQuery {
                subject: Some("note".to_string()),
                ..MemoryQuery::default()
            })
            .unwrap();
        assert_eq!(matches.len(), 1);

        let report = store.verify().unwrap();
        assert_eq!(report.records, 1);
        assert!(report.verified);
    }

    #[test]
    fn journal_store_round_trips_jsonl_and_prunes() {
        let dir = tempfile::tempdir().unwrap();
        let store = MemoryJournalStore::open(dir.path().join("memory"));
        let first = store.put(input("note", b"first")).unwrap();
        std::thread::sleep(Duration::from_millis(1));
        let second = store.put(input("note", b"second")).unwrap();
        store
            .tombstone(first.id, Some("expired".to_string()))
            .unwrap();

        assert!(matches!(
            store.get(first.id),
            Err(ZapMemoryError::NotFound(_))
        ));
        assert_eq!(
            store.get(second.id).unwrap().body_bytes().unwrap(),
            b"second"
        );
        assert_eq!(
            store
                .query(&MemoryQuery {
                    include_tombstoned: true,
                    ..MemoryQuery::default()
                })
                .unwrap()
                .len(),
            2
        );
        assert_eq!(store.verify().unwrap().records, 2);

        let jsonl = dir.path().join("memory.jsonl");
        assert_eq!(store.export_jsonl(&jsonl, false).unwrap(), 3);
        let imported = MemoryJournalStore::open(dir.path().join("imported"));
        assert_eq!(imported.import_jsonl(&jsonl, false).unwrap(), 3);
        assert_eq!(imported.verify().unwrap().entries, 3);

        let pruned = dir.path().join("pruned");
        let pruned_count = imported
            .prune_to(second.created_at_micros, &pruned, false)
            .unwrap();
        assert_eq!(pruned_count, 2);
        let pruned_store = MemoryJournalStore::open(pruned);
        assert_eq!(
            pruned_store.query(&MemoryQuery::default()).unwrap().len(),
            1
        );
    }

    #[test]
    fn tombstone_hides_record_by_default() {
        let dir = tempfile::tempdir().unwrap();
        let store = JsonlMemoryStore::open(dir.path().join("memory.jsonl"));
        let record = store.put(input("note", b"hello")).unwrap();
        store
            .tombstone(record.id, Some("done".to_string()))
            .unwrap();

        assert!(matches!(
            store.get(record.id),
            Err(ZapMemoryError::NotFound(_))
        ));
        assert_eq!(store.query(&MemoryQuery::default()).unwrap().len(), 0);
        assert_eq!(
            store
                .query(&MemoryQuery {
                    include_tombstoned: true,
                    ..MemoryQuery::default()
                })
                .unwrap()
                .len(),
            1
        );
    }

    #[test]
    fn entries_are_sealed_with_a_hash_chain() {
        let dir = tempfile::tempdir().unwrap();
        let store = JsonlMemoryStore::open(dir.path().join("memory.jsonl"));
        let first = store.put(input("note", b"first")).unwrap();
        let second = store.put(input("note", b"second")).unwrap();

        assert!(first.previous_entry_hash.is_none());
        assert!(first.entry_hash.is_some());
        assert_eq!(second.previous_entry_hash, first.entry_hash);
        assert!(second.entry_hash.is_some());
        store.verify().unwrap();
    }

    #[test]
    fn verify_detects_tampered_entry_hash() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("memory.jsonl");
        let store = JsonlMemoryStore::open(&path);
        let record = store.put(input("note", b"hello")).unwrap();

        let mut line: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        line["entry_hash"] = Value::String(hash_bytes(b"tampered"));
        fs::write(
            &path,
            format!("{}\n", serde_json::to_string(&line).unwrap()),
        )
        .unwrap();

        assert!(matches!(
            store.verify(),
            Err(ZapMemoryError::EntryHashMismatch { line: 1, id }) if id == record.id
        ));
    }

    #[test]
    fn verify_detects_removed_middle_entry() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("memory.jsonl");
        let store = JsonlMemoryStore::open(&path);
        let _first = store.put(input("note", b"first")).unwrap();
        store.put(input("note", b"second")).unwrap();
        let third = store.put(input("note", b"third")).unwrap();

        let lines = fs::read_to_string(&path).unwrap();
        let mut lines = lines.lines().collect::<Vec<_>>();
        lines.remove(1);
        fs::write(&path, format!("{}\n", lines.join("\n"))).unwrap();

        assert!(matches!(
            store.verify(),
            Err(ZapMemoryError::EntryChainMismatch { line: 2, id }) if id == third.id
        ));
    }

    #[test]
    fn prune_rechains_retained_entries() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("memory.jsonl");
        let pruned_path = dir.path().join("memory-pruned.jsonl");
        let store = JsonlMemoryStore::open(&path);
        let first = store.put(input("note", b"first")).unwrap();
        std::thread::sleep(Duration::from_millis(1));
        let second = store.put(input("note", b"second")).unwrap();
        let third = store.put(input("note", b"third")).unwrap();

        let pruned = store
            .prune_to(second.created_at_micros, &pruned_path, false)
            .unwrap();
        assert_eq!(pruned, 1);

        let pruned_store = JsonlMemoryStore::open(&pruned_path);
        pruned_store.verify().unwrap();
        assert!(matches!(
            pruned_store.get(first.id),
            Err(ZapMemoryError::NotFound(_))
        ));
        let retained_second = pruned_store.get(second.id).unwrap();
        let retained_third = pruned_store.get(third.id).unwrap();
        assert!(retained_second.previous_entry_hash.is_none());
        assert_eq!(
            retained_third.previous_entry_hash,
            retained_second.entry_hash
        );
    }

    #[test]
    fn prune_drops_orphaned_tombstones() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("memory.jsonl");
        let pruned_path = dir.path().join("memory-pruned.jsonl");
        let store = JsonlMemoryStore::open(&path);
        let record = store.put(input("note", b"done")).unwrap();
        std::thread::sleep(Duration::from_millis(1));
        let tombstone = store
            .tombstone(record.id, Some("expired".to_string()))
            .unwrap();

        let pruned = store
            .prune_to(tombstone.created_at_micros, &pruned_path, false)
            .unwrap();

        assert_eq!(pruned, 2);
        let pruned_store = JsonlMemoryStore::open(&pruned_path);
        assert!(matches!(
            pruned_store.verify(),
            Err(ZapMemoryError::EmptyStore)
        ));
        assert_eq!(fs::read_to_string(&pruned_path).unwrap(), "");
    }
}
