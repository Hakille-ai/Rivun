//! Binary append-only journal segments for ZAP local stores.
//!
//! `zap-journal` is intentionally generic: receipts, memory, and future audit
//! profiles own their schemas while this crate owns segment framing, hash
//! chaining, offset indexes, and crash-tail recovery.

use memmap2::Mmap;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    fs::{self, File, OpenOptions},
    io::{self, Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
};
use thiserror::Error;
use uuid::Uuid;

const SEGMENT_MAGIC: &[u8; 8] = b"ZJSEG001";
const RECORD_MAGIC: &[u8; 4] = b"ZJRC";
const RECORD_VERSION: u8 = 1;
const SEGMENT_HEADER_LEN: u64 = 8 + 1 + 8 + 16;
const HASH_LEN: usize = 32;
const DEFAULT_MAX_SEGMENT_BYTES: u64 = 64 * 1024 * 1024;
const NONE_STRING_LEN: u16 = u16::MAX;
const ENTRY_HASH_DOMAIN: &[u8] = b"ZAP-JOURNAL-ENTRY-v1";

pub const SEGMENT_EXTENSION: &str = "zjseg";
pub const INDEX_EXTENSION: &str = "zjidx";
pub const MANIFEST_EXTENSION: &str = "zjmanifest.json";

pub type Result<T> = std::result::Result<T, ZapJournalError>;

#[derive(Debug, Error)]
pub enum ZapJournalError {
    #[error("journal io error: {0}")]
    Io(#[from] io::Error),
    #[error("journal json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("segment {path} has invalid magic")]
    InvalidSegmentMagic { path: PathBuf },
    #[error("segment {path} profile {actual:?} does not match expected {expected:?}")]
    SegmentProfileMismatch {
        path: PathBuf,
        expected: JournalProfile,
        actual: JournalProfile,
    },
    #[error("record at {path}:{offset} has invalid magic")]
    InvalidRecordMagic { path: PathBuf, offset: u64 },
    #[error("record at {path}:{offset} uses unsupported version {version}")]
    UnsupportedRecordVersion {
        path: PathBuf,
        offset: u64,
        version: u8,
    },
    #[error("record at {path}:{offset} is truncated")]
    TruncatedRecord { path: PathBuf, offset: u64 },
    #[error("record at {path}:{offset} has an invalid entry hash")]
    InvalidEntryHash { path: PathBuf, offset: u64 },
    #[error("record at {path}:{offset} breaks the journal hash chain")]
    HashChainMismatch { path: PathBuf, offset: u64 },
    #[error("journal field `{field}` exceeds {max} bytes")]
    FieldTooLarge { field: &'static str, max: usize },
    #[error("journal segment {path} has invalid file name")]
    InvalidSegmentName { path: PathBuf },
    #[error("journal directory {0} already exists")]
    OutputExists(PathBuf),
    #[error("journal directory {0} does not exist")]
    MissingJournal(PathBuf),
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum JournalProfile {
    Receipts = 1,
    Memory = 2,
}

impl JournalProfile {
    fn from_byte(value: u8) -> Self {
        match value {
            1 => Self::Receipts,
            2 => Self::Memory,
            _ => Self::Memory,
        }
    }

    fn as_byte(self) -> u8 {
        self as u8
    }
}

#[derive(Debug, Clone)]
pub struct JournalOptions {
    pub max_segment_bytes: u64,
}

impl Default for JournalOptions {
    fn default() -> Self {
        Self {
            max_segment_bytes: DEFAULT_MAX_SEGMENT_BYTES,
        }
    }
}

#[derive(Debug, Clone)]
pub struct JournalStore {
    dir: PathBuf,
    profile: JournalProfile,
    options: JournalOptions,
}

#[derive(Debug, Clone)]
pub struct JournalRecordInput {
    pub kind: String,
    pub schema_version: u16,
    pub timestamp_micros: u64,
    pub id: Option<Uuid>,
    pub namespace: Option<String>,
    pub subject: Option<String>,
    pub content_type: Option<String>,
    pub source_node: Option<Uuid>,
    pub target_node: Option<Uuid>,
    pub tombstone_for: Option<Uuid>,
    pub metadata: Value,
    pub payload: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct JournalRecord {
    pub segment_id: Uuid,
    pub segment_sequence: u64,
    pub offset: u64,
    pub encoded_len: u64,
    pub kind: String,
    pub schema_version: u16,
    pub timestamp_micros: u64,
    pub id: Option<Uuid>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub namespace: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subject: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_node: Option<Uuid>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_node: Option<Uuid>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tombstone_for: Option<Uuid>,
    pub previous_entry_hash: String,
    pub payload_hash: String,
    pub entry_hash: String,
    pub metadata: Value,
    #[serde(skip)]
    pub payload: Vec<u8>,
}

#[derive(Debug, Clone, Default)]
pub struct JournalQuery {
    pub kind: Option<String>,
    pub id: Option<Uuid>,
    pub namespace: Option<String>,
    pub subject: Option<String>,
    pub content_type: Option<String>,
    pub source_node: Option<Uuid>,
    pub target_node: Option<Uuid>,
    pub tombstone_for: Option<Uuid>,
    pub after_timestamp_micros: Option<u64>,
    pub until_timestamp_micros: Option<u64>,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct JournalSegmentIndex {
    pub schema_version: u8,
    pub profile: JournalProfile,
    pub segment_id: Uuid,
    pub segment_sequence: u64,
    pub entries: Vec<JournalIndexEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct JournalIndexEntry {
    pub offset: u64,
    pub encoded_len: u64,
    pub kind: String,
    pub schema_version: u16,
    pub timestamp_micros: u64,
    pub id: Option<Uuid>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub namespace: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subject: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_node: Option<Uuid>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_node: Option<Uuid>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tombstone_for: Option<Uuid>,
    pub previous_entry_hash: String,
    pub payload_hash: String,
    pub entry_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct JournalSegmentManifest {
    pub schema_version: u8,
    pub profile: JournalProfile,
    pub segment_id: Uuid,
    pub segment_sequence: u64,
    pub entries: u64,
    pub segment_bytes: u64,
    pub segment_hash: String,
    pub first_entry_hash: Option<String>,
    pub last_entry_hash: Option<String>,
    pub first_timestamp_micros: Option<u64>,
    pub last_timestamp_micros: Option<u64>,
    pub compression: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct JournalVerificationReport {
    pub dir: PathBuf,
    pub segments: usize,
    pub entries: usize,
    pub verified: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartialTail {
    pub segment: PathBuf,
    pub offset: u64,
}

#[derive(Debug, Clone)]
struct SegmentInfo {
    path: PathBuf,
    sequence: u64,
    id: Uuid,
}

impl JournalStore {
    pub fn open(dir: impl Into<PathBuf>, profile: JournalProfile) -> Self {
        Self {
            dir: dir.into(),
            profile,
            options: JournalOptions::default(),
        }
    }

    pub fn with_options(mut self, options: JournalOptions) -> Self {
        self.options = options;
        self
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    pub fn append(&self, input: JournalRecordInput, sync_data: bool) -> Result<JournalRecord> {
        fs::create_dir_all(&self.dir)?;
        self.rebuild_missing_indexes()?;
        let previous_hash = self.last_entry_hash()?;
        let segment = self.current_segment(input_estimate(&input))?;
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .read(true)
            .open(&segment.path)?;
        if file.metadata()?.len() == 0 {
            write_segment_header(&mut file, self.profile, segment.sequence, segment.id)?;
        }
        let offset = file.seek(SeekFrom::End(0))?;
        let encoded = encode_record(self.profile, &input, previous_hash.as_deref())?;
        file.write_all(&encoded)?;
        if sync_data {
            file.sync_data()?;
        }
        let record = decode_record_at(
            &segment.path,
            self.profile,
            segment.sequence,
            segment.id,
            offset,
            &encoded,
        )?;
        let index_entry = JournalIndexEntry::from(&record);
        append_index_entry(&self.index_path(segment.sequence), &index_entry)?;
        self.write_manifest_after_append(&segment, &record)?;
        Ok(record)
    }

    pub fn records(&self) -> Result<Vec<JournalRecord>> {
        let mut records = Vec::new();
        self.scan_records(false, &mut |record| {
            records.push(record);
            Ok(())
        })?;
        Ok(records)
    }

    pub fn query(&self, query: &JournalQuery) -> Result<Vec<JournalRecord>> {
        if !self.dir.exists() {
            return Ok(Vec::new());
        }
        self.rebuild_missing_indexes()?;
        let mut candidates = Vec::new();
        for segment in self.segments()? {
            let index = self.load_segment_index(&segment)?;
            for entry in index.entries {
                if entry_matches_query(&entry, query) {
                    candidates.push((segment.clone(), entry));
                }
            }
        }
        candidates.sort_by(|a, b| {
            a.1.timestamp_micros
                .cmp(&b.1.timestamp_micros)
                .then_with(|| a.0.sequence.cmp(&b.0.sequence))
                .then_with(|| a.1.offset.cmp(&b.1.offset))
        });
        let mut records = Vec::new();
        for (segment, entry) in candidates {
            let record = self.read_record(&segment, &entry)?;
            records.push(record);
            if let Some(limit) = query.limit
                && records.len() >= limit
            {
                break;
            }
        }
        Ok(records)
    }

    pub fn verify(&self) -> Result<JournalVerificationReport> {
        if !self.dir.exists() {
            return Err(ZapJournalError::MissingJournal(self.dir.clone()));
        }
        let mut entries = 0_usize;
        self.scan_records(false, &mut |_| {
            entries += 1;
            Ok(())
        })?;
        let segments = self.segments()?.len();
        Ok(JournalVerificationReport {
            dir: self.dir.clone(),
            segments,
            entries,
            verified: true,
        })
    }

    pub fn rebuild_indexes(&self) -> Result<JournalVerificationReport> {
        if !self.dir.exists() {
            return Err(ZapJournalError::MissingJournal(self.dir.clone()));
        }
        let mut entries = 0_usize;
        let segments = self.segments()?;
        for segment in &segments {
            let mut index = empty_index(self.profile, segment);
            scan_segment(&segment.path, self.profile, None, false, &mut |record| {
                entries += 1;
                index.entries.push((&record).into());
                Ok(())
            })?;
            write_index(&self.index_path(segment.sequence), &index)?;
            self.write_manifest(segment, &index)?;
        }
        Ok(JournalVerificationReport {
            dir: self.dir.clone(),
            segments: segments.len(),
            entries,
            verified: true,
        })
    }

    pub fn recover_partial_tail(&self) -> Result<Option<PartialTail>> {
        let segments = self.segments()?;
        for segment in segments {
            let mut partial = None;
            let result = scan_segment(&segment.path, self.profile, None, false, &mut |_| Ok(()));
            if let Err(ZapJournalError::TruncatedRecord { path, offset }) = result {
                partial = Some(PartialTail {
                    segment: path,
                    offset,
                });
            }
            if let Some(tail) = partial {
                let file = OpenOptions::new().write(true).open(&tail.segment)?;
                file.set_len(tail.offset)?;
                self.rebuild_indexes()?;
                return Ok(Some(tail));
            }
        }
        Ok(None)
    }

    pub fn remove_dir_if_allowed(out: &Path, force: bool) -> Result<()> {
        if out.exists() {
            if !force {
                return Err(ZapJournalError::OutputExists(out.to_path_buf()));
            }
            fs::remove_dir_all(out)?;
        }
        Ok(())
    }

    fn rebuild_missing_indexes(&self) -> Result<()> {
        if !self.dir.exists() {
            return Ok(());
        }
        let mut rebuild = false;
        for segment in self.segments()? {
            if self.segment_index_needs_rebuild(&segment)? {
                rebuild = true;
                break;
            }
        }
        if rebuild {
            self.rebuild_indexes()?;
        }
        Ok(())
    }

    fn segment_index_needs_rebuild(&self, segment: &SegmentInfo) -> Result<bool> {
        let index_path = self.index_path(segment.sequence);
        if !index_path.exists() || !self.manifest_path(segment.sequence).exists() {
            return Ok(true);
        }

        let segment_len = fs::metadata(&segment.path)?.len();
        let index_len = fs::metadata(&index_path)?.len();
        if index_len > 0 && !file_ends_with_newline(&index_path)? {
            return Ok(true);
        }

        let last_entry = match read_last_index_entry(&index_path) {
            Ok(entry) => entry,
            Err(ZapJournalError::Json(_)) => return Ok(true),
            Err(err) => return Err(err),
        };
        let Some(last_entry) = last_entry else {
            return Ok(segment_len > SEGMENT_HEADER_LEN);
        };
        let Some(indexed_end) = last_entry.offset.checked_add(last_entry.encoded_len) else {
            return Ok(true);
        };
        Ok(indexed_end != segment_len)
    }

    fn last_entry_hash(&self) -> Result<Option<String>> {
        let mut last = None;
        for segment in self.segments()? {
            if let Some(entry) = read_last_index_entry(&self.index_path(segment.sequence))? {
                last = Some(entry.entry_hash);
            }
        }
        Ok(last)
    }

    fn current_segment(&self, estimate: u64) -> Result<SegmentInfo> {
        let segments = self.segments()?;
        if let Some(last) = segments.last() {
            let len = fs::metadata(&last.path)?.len();
            if len > SEGMENT_HEADER_LEN
                && len.saturating_add(estimate) > self.options.max_segment_bytes
            {
                return Ok(SegmentInfo {
                    path: self.segment_path(last.sequence + 1),
                    sequence: last.sequence + 1,
                    id: Uuid::new_v4(),
                });
            }
            return Ok(last.clone());
        }
        Ok(SegmentInfo {
            path: self.segment_path(0),
            sequence: 0,
            id: Uuid::new_v4(),
        })
    }

    fn segments(&self) -> Result<Vec<SegmentInfo>> {
        if !self.dir.exists() {
            return Ok(Vec::new());
        }
        let mut segments = Vec::new();
        for entry in fs::read_dir(&self.dir)? {
            let path = entry?.path();
            if path.extension().and_then(|ext| ext.to_str()) != Some(SEGMENT_EXTENSION) {
                continue;
            }
            let sequence = segment_sequence_from_path(&path)?;
            let (profile, header_sequence, id) = read_segment_header(&path)?;
            if profile != self.profile {
                return Err(ZapJournalError::SegmentProfileMismatch {
                    path,
                    expected: self.profile,
                    actual: profile,
                });
            }
            if header_sequence != sequence {
                return Err(ZapJournalError::InvalidSegmentName { path });
            }
            segments.push(SegmentInfo { path, sequence, id });
        }
        segments.sort_by_key(|segment| segment.sequence);
        Ok(segments)
    }

    fn segment_path(&self, sequence: u64) -> PathBuf {
        self.dir.join(format!("{sequence:020}.{SEGMENT_EXTENSION}"))
    }

    fn index_path(&self, sequence: u64) -> PathBuf {
        self.dir.join(format!("{sequence:020}.{INDEX_EXTENSION}"))
    }

    fn manifest_path(&self, sequence: u64) -> PathBuf {
        self.dir
            .join(format!("{sequence:020}.{MANIFEST_EXTENSION}"))
    }

    fn load_segment_index(&self, segment: &SegmentInfo) -> Result<JournalSegmentIndex> {
        let input = fs::read_to_string(self.index_path(segment.sequence))?;
        let input = input.trim();
        if input.is_empty() {
            return Ok(empty_index(self.profile, segment));
        }
        if input.starts_with('{') {
            let index_res = serde_json::from_str(input);
            if let Ok(index) = index_res {
                return Ok(index);
            }
        }
        let entries = input
            .lines()
            .filter(|line| !line.trim().is_empty())
            .map(serde_json::from_str)
            .collect::<std::result::Result<Vec<JournalIndexEntry>, serde_json::Error>>()?;
        Ok(JournalSegmentIndex {
            schema_version: 1,
            profile: self.profile,
            segment_id: segment.id,
            segment_sequence: segment.sequence,
            entries,
        })
    }

    fn read_record(
        &self,
        segment: &SegmentInfo,
        entry: &JournalIndexEntry,
    ) -> Result<JournalRecord> {
        let mut file = File::open(&segment.path)?;
        file.seek(SeekFrom::Start(entry.offset))?;
        let mut len_buf = [0_u8; 4];
        file.read_exact(&mut len_buf)?;
        let len = u32::from_le_bytes(len_buf) as usize;
        let mut body = vec![0_u8; len];
        file.read_exact(&mut body)?;
        let mut encoded = Vec::with_capacity(4 + len);
        encoded.extend_from_slice(&len_buf);
        encoded.extend_from_slice(&body);
        let record = decode_record_at(
            &segment.path,
            self.profile,
            segment.sequence,
            segment.id,
            entry.offset,
            &encoded,
        )?;
        if record.entry_hash != entry.entry_hash {
            return Err(ZapJournalError::InvalidEntryHash {
                path: segment.path.clone(),
                offset: entry.offset,
            });
        }
        Ok(record)
    }

    fn scan_records<F>(&self, allow_partial_tail: bool, callback: &mut F) -> Result<()>
    where
        F: FnMut(JournalRecord) -> Result<()>,
    {
        let mut previous_hash = None;
        for segment in self.segments()? {
            scan_segment(
                &segment.path,
                self.profile,
                None,
                allow_partial_tail,
                &mut |record| {
                    if record.previous_entry_hash != hash_or_none(previous_hash.as_deref()) {
                        return Err(ZapJournalError::HashChainMismatch {
                            path: segment.path.clone(),
                            offset: record.offset,
                        });
                    }
                    previous_hash = Some(record.entry_hash.clone());
                    callback(record)
                },
            )?;
        }
        Ok(())
    }

    fn write_manifest(&self, segment: &SegmentInfo, index: &JournalSegmentIndex) -> Result<()> {
        let bytes = read_segment_bytes(&segment.path)?;
        let manifest = JournalSegmentManifest {
            schema_version: 1,
            profile: self.profile,
            segment_id: segment.id,
            segment_sequence: segment.sequence,
            entries: index.entries.len() as u64,
            segment_bytes: bytes.len() as u64,
            segment_hash: hash_bytes(&bytes),
            first_entry_hash: index.entries.first().map(|entry| entry.entry_hash.clone()),
            last_entry_hash: index.entries.last().map(|entry| entry.entry_hash.clone()),
            first_timestamp_micros: index.entries.first().map(|entry| entry.timestamp_micros),
            last_timestamp_micros: index.entries.last().map(|entry| entry.timestamp_micros),
            compression: "none".to_string(),
        };
        let output = serde_json::to_string_pretty(&manifest)?;
        fs::write(self.manifest_path(segment.sequence), output)?;
        Ok(())
    }

    fn write_manifest_after_append(
        &self,
        segment: &SegmentInfo,
        record: &JournalRecord,
    ) -> Result<()> {
        let path = self.manifest_path(segment.sequence);
        let previous = fs::read_to_string(&path)
            .ok()
            .and_then(|input| serde_json::from_str::<JournalSegmentManifest>(&input).ok());
        let manifest = JournalSegmentManifest {
            schema_version: 1,
            profile: self.profile,
            segment_id: segment.id,
            segment_sequence: segment.sequence,
            entries: previous.as_ref().map_or(1, |manifest| manifest.entries + 1),
            segment_bytes: fs::metadata(&segment.path)?.len(),
            segment_hash: hash_bytes(&read_segment_bytes(&segment.path)?),
            first_entry_hash: previous
                .as_ref()
                .and_then(|manifest| manifest.first_entry_hash.clone())
                .or_else(|| Some(record.entry_hash.clone())),
            last_entry_hash: Some(record.entry_hash.clone()),
            first_timestamp_micros: previous
                .as_ref()
                .and_then(|manifest| manifest.first_timestamp_micros)
                .or(Some(record.timestamp_micros)),
            last_timestamp_micros: Some(record.timestamp_micros),
            compression: "none".to_string(),
        };
        fs::write(path, serde_json::to_string_pretty(&manifest)?)?;
        Ok(())
    }
}

impl From<&JournalRecord> for JournalIndexEntry {
    fn from(record: &JournalRecord) -> Self {
        Self {
            offset: record.offset,
            encoded_len: record.encoded_len,
            kind: record.kind.clone(),
            schema_version: record.schema_version,
            timestamp_micros: record.timestamp_micros,
            id: record.id,
            namespace: record.namespace.clone(),
            subject: record.subject.clone(),
            content_type: record.content_type.clone(),
            source_node: record.source_node,
            target_node: record.target_node,
            tombstone_for: record.tombstone_for,
            previous_entry_hash: record.previous_entry_hash.clone(),
            payload_hash: record.payload_hash.clone(),
            entry_hash: record.entry_hash.clone(),
        }
    }
}

fn empty_index(profile: JournalProfile, segment: &SegmentInfo) -> JournalSegmentIndex {
    JournalSegmentIndex {
        schema_version: 1,
        profile,
        segment_id: segment.id,
        segment_sequence: segment.sequence,
        entries: Vec::new(),
    }
}

fn write_index(path: &Path, index: &JournalSegmentIndex) -> Result<()> {
    let mut output = String::new();
    for entry in &index.entries {
        output.push_str(&serde_json::to_string(entry)?);
        output.push('\n');
    }
    fs::write(path, output)?;
    Ok(())
}

fn append_index_entry(path: &Path, entry: &JournalIndexEntry) -> Result<()> {
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    file.write_all(serde_json::to_string(entry)?.as_bytes())?;
    file.write_all(b"\n")?;
    Ok(())
}

fn file_ends_with_newline(path: &Path) -> Result<bool> {
    let mut file = File::open(path)?;
    if file.metadata()?.len() == 0 {
        return Ok(true);
    }
    file.seek(SeekFrom::End(-1))?;
    let mut last = [0_u8; 1];
    file.read_exact(&mut last)?;
    Ok(last[0] == b'\n')
}

fn read_last_index_entry(path: &Path) -> Result<Option<JournalIndexEntry>> {
    if !path.exists() {
        return Ok(None);
    }
    let mut file = File::open(path)?;
    let len = file.metadata()?.len();
    if len == 0 {
        return Ok(None);
    }
    let read_len = len.min(8192) as usize;
    file.seek(SeekFrom::End(-(read_len as i64)))?;
    let mut buf = vec![0_u8; read_len];
    file.read_exact(&mut buf)?;
    let text = String::from_utf8_lossy(&buf);
    for line in text.lines().rev() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if line.ends_with('}') {
            return Ok(Some(serde_json::from_str(line)?));
        }
    }
    let input = fs::read_to_string(path)?;
    Ok(input
        .lines()
        .rev()
        .find(|line| !line.trim().is_empty())
        .map(serde_json::from_str)
        .transpose()?)
}

fn write_segment_header(
    file: &mut File,
    profile: JournalProfile,
    sequence: u64,
    segment_id: Uuid,
) -> Result<()> {
    file.write_all(SEGMENT_MAGIC)?;
    file.write_all(&[profile.as_byte()])?;
    file.write_all(&sequence.to_le_bytes())?;
    file.write_all(segment_id.as_bytes())?;
    Ok(())
}

fn read_segment_header(path: &Path) -> Result<(JournalProfile, u64, Uuid)> {
    let mut file = File::open(path)?;
    let mut magic = [0_u8; 8];
    file.read_exact(&mut magic)?;
    if &magic != SEGMENT_MAGIC {
        return Err(ZapJournalError::InvalidSegmentMagic {
            path: path.to_path_buf(),
        });
    }
    let mut profile = [0_u8; 1];
    file.read_exact(&mut profile)?;
    let mut sequence = [0_u8; 8];
    file.read_exact(&mut sequence)?;
    let mut id = [0_u8; 16];
    file.read_exact(&mut id)?;
    Ok((
        JournalProfile::from_byte(profile[0]),
        u64::from_le_bytes(sequence),
        Uuid::from_bytes(id),
    ))
}

fn scan_segment<F>(
    path: &Path,
    expected_profile: JournalProfile,
    _previous_hash: Option<&str>,
    allow_partial_tail: bool,
    callback: &mut F,
) -> Result<()>
where
    F: FnMut(JournalRecord) -> Result<()>,
{
    let (profile, sequence, id) = read_segment_header(path)?;
    if profile != expected_profile {
        return Err(ZapJournalError::SegmentProfileMismatch {
            path: path.to_path_buf(),
            expected: expected_profile,
            actual: profile,
        });
    }
    let bytes = read_segment_bytes(path)?;
    let mut offset = SEGMENT_HEADER_LEN as usize;
    while offset < bytes.len() {
        if bytes.len() - offset < 4 {
            if allow_partial_tail {
                return Ok(());
            }
            return Err(ZapJournalError::TruncatedRecord {
                path: path.to_path_buf(),
                offset: offset as u64,
            });
        }
        let len = u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap()) as usize;
        let end = offset + 4 + len;
        if end > bytes.len() {
            if allow_partial_tail {
                return Ok(());
            }
            return Err(ZapJournalError::TruncatedRecord {
                path: path.to_path_buf(),
                offset: offset as u64,
            });
        }
        let record = decode_record_at(
            path,
            expected_profile,
            sequence,
            id,
            offset as u64,
            &bytes[offset..end],
        )?;
        callback(record)?;
        offset = end;
    }
    Ok(())
}

fn read_segment_bytes(path: &Path) -> Result<Vec<u8>> {
    let file = File::open(path)?;
    let len = file.metadata()?.len();
    if len == 0 {
        return Ok(Vec::new());
    }
    // Mmap is used for segment scans so verification/query hydration is mostly
    // sequential memory access instead of repeated buffered reads.
    let mmap = unsafe { Mmap::map(&file)? };
    Ok(mmap.to_vec())
}

fn encode_record(
    profile: JournalProfile,
    input: &JournalRecordInput,
    previous_hash: Option<&str>,
) -> Result<Vec<u8>> {
    let mut body = Vec::new();
    body.extend_from_slice(RECORD_MAGIC);
    body.push(RECORD_VERSION);
    body.push(profile.as_byte());
    write_string(&mut body, "kind", Some(&input.kind))?;
    body.extend_from_slice(&input.schema_version.to_le_bytes());
    body.extend_from_slice(&input.timestamp_micros.to_le_bytes());
    write_uuid(&mut body, input.id);
    write_string(&mut body, "namespace", input.namespace.as_deref())?;
    write_string(&mut body, "subject", input.subject.as_deref())?;
    write_string(&mut body, "content_type", input.content_type.as_deref())?;
    write_uuid(&mut body, input.source_node);
    write_uuid(&mut body, input.target_node);
    write_uuid(&mut body, input.tombstone_for);
    let metadata = serde_json::to_vec(&input.metadata)?;
    if metadata.len() > u32::MAX as usize {
        return Err(ZapJournalError::FieldTooLarge {
            field: "metadata",
            max: u32::MAX as usize,
        });
    }
    body.extend_from_slice(&(metadata.len() as u32).to_le_bytes());
    body.extend_from_slice(&metadata);
    body.extend_from_slice(&(input.payload.len() as u64).to_le_bytes());
    body.extend_from_slice(&input.payload);
    body.extend_from_slice(&decode_hash_or_none(previous_hash));
    body.extend_from_slice(blake3::hash(&input.payload).as_bytes());
    let entry_hash = compute_entry_hash(&body);
    body.extend_from_slice(&entry_hash);
    let len = u32::try_from(body.len()).map_err(|_| ZapJournalError::FieldTooLarge {
        field: "record",
        max: u32::MAX as usize,
    })?;
    let mut encoded = Vec::with_capacity(4 + body.len());
    encoded.extend_from_slice(&len.to_le_bytes());
    encoded.extend_from_slice(&body);
    Ok(encoded)
}

fn decode_record_at(
    path: &Path,
    expected_profile: JournalProfile,
    segment_sequence: u64,
    segment_id: Uuid,
    offset: u64,
    encoded: &[u8],
) -> Result<JournalRecord> {
    if encoded.len() < 4 {
        return Err(ZapJournalError::TruncatedRecord {
            path: path.to_path_buf(),
            offset,
        });
    }
    let len = u32::from_le_bytes(encoded[..4].try_into().unwrap()) as usize;
    if encoded.len() != 4 + len || len < HASH_LEN {
        return Err(ZapJournalError::TruncatedRecord {
            path: path.to_path_buf(),
            offset,
        });
    }
    let body = &encoded[4..];
    let signed = &body[..body.len() - HASH_LEN];
    let entry_hash = &body[body.len() - HASH_LEN..];
    if compute_entry_hash(signed) != entry_hash {
        return Err(ZapJournalError::InvalidEntryHash {
            path: path.to_path_buf(),
            offset,
        });
    }

    let mut cursor = Cursor::new(signed);
    let magic = cursor.read_exact::<4>()?;
    if &magic != RECORD_MAGIC {
        return Err(ZapJournalError::InvalidRecordMagic {
            path: path.to_path_buf(),
            offset,
        });
    }
    let version = cursor.read_u8()?;
    if version != RECORD_VERSION {
        return Err(ZapJournalError::UnsupportedRecordVersion {
            path: path.to_path_buf(),
            offset,
            version,
        });
    }
    let profile = JournalProfile::from_byte(cursor.read_u8()?);
    if profile != expected_profile {
        return Err(ZapJournalError::SegmentProfileMismatch {
            path: path.to_path_buf(),
            expected: expected_profile,
            actual: profile,
        });
    }
    let kind = cursor.read_string()?.unwrap_or_default();
    let schema_version = cursor.read_u16()?;
    let timestamp_micros = cursor.read_u64()?;
    let id = cursor.read_uuid()?;
    let namespace = cursor.read_string()?;
    let subject = cursor.read_string()?;
    let content_type = cursor.read_string()?;
    let source_node = cursor.read_uuid()?;
    let target_node = cursor.read_uuid()?;
    let tombstone_for = cursor.read_uuid()?;
    let metadata_len = cursor.read_u32()? as usize;
    let metadata = serde_json::from_slice(cursor.read_slice(metadata_len)?)?;
    let payload_len = cursor.read_u64()? as usize;
    let payload = cursor.read_slice(payload_len)?.to_vec();
    let previous_entry_hash = hash_hex(cursor.read_slice(HASH_LEN)?);
    let payload_hash = hash_hex(cursor.read_slice(HASH_LEN)?);
    Ok(JournalRecord {
        segment_id,
        segment_sequence,
        offset,
        encoded_len: encoded.len() as u64,
        kind,
        schema_version,
        timestamp_micros,
        id,
        namespace,
        subject,
        content_type,
        source_node,
        target_node,
        tombstone_for,
        previous_entry_hash,
        payload_hash,
        entry_hash: hash_hex(entry_hash),
        metadata,
        payload,
    })
}

fn input_estimate(input: &JournalRecordInput) -> u64 {
    512 + input.payload.len() as u64
        + serde_json::to_vec(&input.metadata).map_or(0, |v| v.len()) as u64
}

fn write_string(out: &mut Vec<u8>, field: &'static str, value: Option<&str>) -> Result<()> {
    match value {
        Some(value) => {
            let bytes = value.as_bytes();
            if bytes.len() >= NONE_STRING_LEN as usize {
                return Err(ZapJournalError::FieldTooLarge {
                    field,
                    max: NONE_STRING_LEN as usize - 1,
                });
            }
            out.extend_from_slice(&(bytes.len() as u16).to_le_bytes());
            out.extend_from_slice(bytes);
        }
        None => out.extend_from_slice(&NONE_STRING_LEN.to_le_bytes()),
    }
    Ok(())
}

fn write_uuid(out: &mut Vec<u8>, value: Option<Uuid>) {
    match value {
        Some(value) => {
            out.push(1);
            out.extend_from_slice(value.as_bytes());
        }
        None => out.push(0),
    }
}

fn compute_entry_hash(body_without_entry_hash: &[u8]) -> [u8; HASH_LEN] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(ENTRY_HASH_DOMAIN);
    hasher.update(body_without_entry_hash);
    *hasher.finalize().as_bytes()
}

fn decode_hash_or_none(value: Option<&str>) -> [u8; HASH_LEN] {
    let Some(value) = value else {
        return [0_u8; HASH_LEN];
    };
    let Some(hex) = value.strip_prefix("blake3:") else {
        return [0_u8; HASH_LEN];
    };
    let mut out = [0_u8; HASH_LEN];
    for (index, chunk) in hex.as_bytes().chunks(2).take(HASH_LEN).enumerate() {
        if chunk.len() != 2 {
            return [0_u8; HASH_LEN];
        }
        let Ok(byte) = u8::from_str_radix(std::str::from_utf8(chunk).unwrap_or(""), 16) else {
            return [0_u8; HASH_LEN];
        };
        out[index] = byte;
    }
    out
}

fn hash_or_none(value: Option<&str>) -> String {
    value
        .unwrap_or("blake3:0000000000000000000000000000000000000000000000000000000000000000")
        .to_string()
}

fn hash_hex(bytes: &[u8]) -> String {
    format!("blake3:{}", to_hex(bytes))
}

fn hash_bytes(bytes: &[u8]) -> String {
    hash_hex(blake3::hash(bytes).as_bytes())
}

fn to_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
}

fn segment_sequence_from_path(path: &Path) -> Result<u64> {
    let stem = path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .ok_or_else(|| ZapJournalError::InvalidSegmentName {
            path: path.to_path_buf(),
        })?;
    stem.parse::<u64>()
        .map_err(|_| ZapJournalError::InvalidSegmentName {
            path: path.to_path_buf(),
        })
}

fn entry_matches_query(entry: &JournalIndexEntry, query: &JournalQuery) -> bool {
    if let Some(kind) = query.kind.as_deref()
        && entry.kind != kind
    {
        return false;
    }
    if let Some(id) = query.id
        && entry.id != Some(id)
    {
        return false;
    }
    if let Some(namespace) = query.namespace.as_deref()
        && entry.namespace.as_deref() != Some(namespace)
    {
        return false;
    }
    if let Some(subject) = query.subject.as_deref()
        && entry.subject.as_deref() != Some(subject)
    {
        return false;
    }
    if let Some(content_type) = query.content_type.as_deref()
        && entry.content_type.as_deref() != Some(content_type)
    {
        return false;
    }
    if let Some(source_node) = query.source_node
        && entry.source_node != Some(source_node)
    {
        return false;
    }
    if let Some(target_node) = query.target_node
        && entry.target_node != Some(target_node)
    {
        return false;
    }
    if let Some(tombstone_for) = query.tombstone_for
        && entry.tombstone_for != Some(tombstone_for)
    {
        return false;
    }
    if let Some(after) = query.after_timestamp_micros
        && entry.timestamp_micros <= after
    {
        return false;
    }
    if let Some(until) = query.until_timestamp_micros
        && entry.timestamp_micros > until
    {
        return false;
    }
    true
}

struct Cursor<'a> {
    bytes: &'a [u8],
    position: usize,
}

impl<'a> Cursor<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, position: 0 }
    }

    fn read_u8(&mut self) -> Result<u8> {
        Ok(self.read_slice(1)?[0])
    }

    fn read_u16(&mut self) -> Result<u16> {
        Ok(u16::from_le_bytes(self.read_exact()?))
    }

    fn read_u32(&mut self) -> Result<u32> {
        Ok(u32::from_le_bytes(self.read_exact()?))
    }

    fn read_u64(&mut self) -> Result<u64> {
        Ok(u64::from_le_bytes(self.read_exact()?))
    }

    fn read_uuid(&mut self) -> Result<Option<Uuid>> {
        let present = self.read_u8()?;
        if present == 0 {
            return Ok(None);
        }
        Ok(Some(Uuid::from_bytes(self.read_exact()?)))
    }

    fn read_string(&mut self) -> Result<Option<String>> {
        let len = self.read_u16()?;
        if len == NONE_STRING_LEN {
            return Ok(None);
        }
        let bytes = self.read_slice(len as usize)?;
        Ok(Some(String::from_utf8_lossy(bytes).into_owned()))
    }

    fn read_slice(&mut self, len: usize) -> Result<&'a [u8]> {
        let end = self.position.saturating_add(len);
        if end > self.bytes.len() {
            return Err(ZapJournalError::TruncatedRecord {
                path: PathBuf::new(),
                offset: self.position as u64,
            });
        }
        let slice = &self.bytes[self.position..end];
        self.position = end;
        Ok(slice)
    }

    fn read_exact<const N: usize>(&mut self) -> Result<[u8; N]> {
        let mut out = [0_u8; N];
        out.copy_from_slice(self.read_slice(N)?);
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(index: u64) -> JournalRecordInput {
        JournalRecordInput {
            kind: "receipt".to_string(),
            schema_version: 1,
            timestamp_micros: 1_000 + index,
            id: Some(Uuid::new_v4()),
            namespace: None,
            subject: Some(
                if index.is_multiple_of(2) {
                    "echo"
                } else {
                    "telemetry"
                }
                .to_string(),
            ),
            content_type: Some("application/json".to_string()),
            source_node: Some(Uuid::new_v4()),
            target_node: Some(Uuid::new_v4()),
            tombstone_for: None,
            metadata: serde_json::json!({ "index": index }),
            payload: format!("payload-{index}").into_bytes(),
        }
    }

    #[test]
    fn journal_appends_queries_and_verifies() {
        let temp = tempfile::tempdir().unwrap();
        let store = JournalStore::open(temp.path(), JournalProfile::Receipts);
        for index in 0..10 {
            store.append(input(index), false).unwrap();
        }
        let results = store
            .query(&JournalQuery {
                kind: Some("receipt".to_string()),
                subject: Some("echo".to_string()),
                limit: Some(3),
                ..JournalQuery::default()
            })
            .unwrap();
        assert_eq!(results.len(), 3);
        assert!(
            results
                .iter()
                .all(|record| record.subject.as_deref() == Some("echo"))
        );
        let report = store.verify().unwrap();
        assert_eq!(report.entries, 10);
        assert_eq!(report.segments, 1);
    }

    #[test]
    fn journal_rebuilds_missing_indexes() {
        let temp = tempfile::tempdir().unwrap();
        let store = JournalStore::open(temp.path(), JournalProfile::Memory);
        store.append(input(1), false).unwrap();
        fs::remove_file(
            temp.path()
                .join(format!("00000000000000000000.{INDEX_EXTENSION}")),
        )
        .unwrap();
        let results = store.query(&JournalQuery::default()).unwrap();
        assert_eq!(results.len(), 1);
        assert!(
            temp.path()
                .join(format!("00000000000000000000.{INDEX_EXTENSION}"))
                .exists()
        );
    }

    #[test]
    fn journal_rebuilds_stale_indexes() {
        let temp = tempfile::tempdir().unwrap();
        let store = JournalStore::open(temp.path(), JournalProfile::Memory);
        store.append(input(1), false).unwrap();
        store.append(input(2), false).unwrap();

        let index_path = temp
            .path()
            .join(format!("00000000000000000000.{INDEX_EXTENSION}"));
        let index = fs::read_to_string(&index_path).unwrap();
        let first_line = index.lines().next().unwrap();
        fs::write(&index_path, format!("{first_line}\n")).unwrap();

        let results = store.query(&JournalQuery::default()).unwrap();

        assert_eq!(results.len(), 2);
        assert_eq!(fs::read_to_string(index_path).unwrap().lines().count(), 2);
    }

    #[test]
    fn append_manifest_hashes_segment_bytes() {
        let temp = tempfile::tempdir().unwrap();
        let store = JournalStore::open(temp.path(), JournalProfile::Receipts);
        store.append(input(1), false).unwrap();

        let segment_path = temp
            .path()
            .join(format!("00000000000000000000.{SEGMENT_EXTENSION}"));
        let manifest_path = temp
            .path()
            .join(format!("00000000000000000000.{MANIFEST_EXTENSION}"));
        let manifest: JournalSegmentManifest =
            serde_json::from_str(&fs::read_to_string(manifest_path).unwrap()).unwrap();

        assert_eq!(
            manifest.segment_hash,
            hash_bytes(&fs::read(segment_path).unwrap())
        );
    }

    #[test]
    fn journal_detects_tampering() {
        let temp = tempfile::tempdir().unwrap();
        let store = JournalStore::open(temp.path(), JournalProfile::Receipts);
        store.append(input(1), false).unwrap();
        let path = temp
            .path()
            .join(format!("00000000000000000000.{SEGMENT_EXTENSION}"));
        let mut bytes = fs::read(&path).unwrap();
        let last = bytes.len() - 40;
        bytes[last] ^= 0x55;
        fs::write(&path, bytes).unwrap();
        assert!(matches!(
            store.verify(),
            Err(ZapJournalError::InvalidEntryHash { .. })
        ));
    }
}
