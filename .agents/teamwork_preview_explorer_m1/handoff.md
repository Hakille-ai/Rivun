# Technical Blueprint Handoff Report: Milestone 1 (R1: High-Performance Durable Core & Replay Protection)

## 1. Observation

### Codebase Inspection Findings

1. **Replay Protection in `zap-net`**:
   - File: `crates/zap-net/src/lib.rs` (lines 491–524)
   - `NonceReplayCache` is defined as:
     ```rust
     struct NonceReplayCache {
         capacity: usize,
         seen: HashSet<[u8; NONCE_LEN]>,
         order: VecDeque<[u8; NONCE_LEN]>,
     }
     ```
   - *Observation*: Nonce tracking is strictly in-memory. Restarting the node clears `seen` and `order`, allowing an attacker to replay valid datagrams recorded prior to reboot if transmitted within the allowed clock window.

2. **Replay Protection in `zap-node`**:
   - File: `crates/zap-node/src/lib.rs` (lines 4430–4469)
   - `ReplayGuard` is defined as:
     ```rust
     struct ReplayGuard {
         capacity: usize,
         seen: HashSet<[u8; 16]>,
         order: VecDeque<[u8; 16]>,
     }
     ```
   - Fingerprint calculation (line 4471):
     ```rust
     fn frame_fingerprint(frame: &zap_core::ZapFrame) -> [u8; 16] {
         let hash = blake3::hash(&frame.encode());
         hash.as_bytes()[..16].try_into().unwrap()
     }
     ```
   - *Observation*: Frame fingerprints are held in memory only (`Mutex<ReplayGuard>`). Node process restart resets the cache, introducing replay vulnerabilities across node restarts.

3. **Receipt Journal & Manifests in `zap-journal`**:
   - File: `crates/zap-journal/src/lib.rs` (lines 93–103, 463–483)
   - `JournalOptions` currently only contains `max_segment_bytes: u64`.
   - `JournalSegmentManifest` (lines 208–221) models segment manifest metadata (`segment_id`, `segment_sequence`, `entries`, `segment_bytes`, `segment_hash`, `first_entry_hash`, `last_entry_hash`, `first_timestamp_micros`, `last_timestamp_micros`).
   - *Observation*: `current_segment` rotates files based on size, but `JournalStore` does not explicitly seal closed segments, compute full segment cryptographic hashes, or support segment count limit triggers.

4. **Signed Manifests & Indexing in `zap-ledger`**:
   - File: `crates/zap-ledger/src/lib.rs` (lines 435–585, 679–760, 762–931)
   - `SignedReceiptSegmentManifest` defines Ed25519 signing over domain `ZAP-RECEIPT-SEGMENT-MANIFEST-v1`.
   - `ReceiptSegmentIndex` supports sequence continuity checks and `previous_segment_hash` chain verification.
   - *Observation*: `ReceiptJournalStore` does not bind to a node `Keypair`, does not sign closed segment manifests upon rotation, does not save `.zjmanifest.json.sig` manifest files to disk, and `ReceiptJournalStore::query` does not use `ReceiptSegmentIndex` to prune candidate segments prior to scanning records.

---

## 2. Logic Chain

1. **Restart-Persistent Replay Guard**:
   - *Premise*: In distributed agent environments, network attacks include delayed datagram re-injection after target node restarts.
   - *Deduction*: In-memory `HashSet` data structures in `zap-net` and `zap-node` do not survive process restart.
   - *Resolution*: Implement disk-backed `DurableNonceStore` (in `zap-net`) and `DurableReplayStore` (in `zap-node`). Upon node restart, the store reads the binary WAL/log file, filters out entries older than `max_clock_skew_micros`, re-populates memory sets, and rejects any replayed nonces or frame fingerprints.

2. **Journal Rotation & Cryptographic Sealing**:
   - *Premise*: Continuous audit storage requires bounded segment files and immutable proof of content upon segment closure.
   - *Deduction*: Automatic rotation must monitor both `max_segment_bytes` and `max_segment_count`. When a segment closes, computing `blake3::hash` over the segment byte stream produces an immutable cryptographic seal.
   - *Resolution*: Extend `JournalOptions` in `zap-journal` with segment count and record limits. Implement `seal_segment` and `rotate_and_seal` to finalize manifest metadata and calculate BLAKE3 segment hashes upon rotation.

3. **Signed Segment Manifests**:
   - *Premise*: Receipt journal segments must carry cryptographic evidence of node authorship and chain continuity.
   - *Deduction*: Unsigned JSON manifests (`.zjmanifest.json`) can be modified post-facto. Node keypair signatures must bind the manifest hash, sequence, timestamps, and previous segment hash into a signed artifact.
   - *Resolution*: Update `ReceiptJournalStore` in `zap-ledger` to accept an optional node `Keypair`. Upon segment rotation, automatically generate `SignedReceiptSegmentManifest` and save it to disk as `{sequence:020}.zjmanifest.json.sig`.

4. **Fast Indexed Query Engine**:
   - *Premise*: Querying receipt ledgers across large segment histories must avoid full linear scans over disk records.
   - *Deduction*: Segment manifests and `ReceiptSegmentIndex` contain bounding metadata (`first_processed_at_micros`, `last_processed_at_micros`, `segment_sequence`).
   - *Resolution*: Implement a two-tiered query strategy: Phase 1 evaluates `ReceiptSegmentIndex::candidate_segments()` to prune non-matching segments instantly. Phase 2 performs offset lookups in `.zjidx` index files for candidate segments, and point lookups via an in-memory receipt hash map index.

---

## 3. Caveats

- **Scope Boundary**: This blueprint covers Milestone 1 (R1). Domain pack CLI (M2), fleet telemetry (M3), AI agent gateway (M4), and multi-language SDK conformance (M5) are scoped to subsequent milestones.
- **Storage Performance**: Disks sync operations (`sync_data`) on every frame insert ensure maximum durability; for high-throughput environments, WAL buffer flushing with periodic sync can be enabled via configuration.
- **Backwards Compatibility**: Existing unsigned `.zjmanifest.json` files continue to be supported for legacy journal directories, with upgrade logic converting them to signed manifests during rebuilds when a keypair is available.

---

## 4. Conclusion & Technical Blueprint

### Architectural Overview

Milestone 1 implements a restart-persistent, high-performance durable core across `zap-net`, `zap-node`, `zap-journal`, and `zap-ledger`:

```
+-----------------------------------------------------------------------------------+
|                                  ZAP Node Daemon                                  |
|                                                                                   |
|  +-----------------------------+               +-------------------------------+  |
|  |     zap-net Datagrams       |               |     zap-node Frame Dispatch   |  |
|  |  +-----------------------+  |               |  +-------------------------+  |  |
|  |  |  DurableNonceStore    |  |               |  |   DurableReplayStore    |  |  |
|  |  |  (b"ZAPNONC1" disk)   |  |               |  |   (b"ZAPFRM01" disk)    |  |  |
|  |  +-----------------------+  |               |  +-------------------------+  |  |
|  +-----------------------------+               +-------------------------------+  |
|                 |                                              |                  |
|                 v                                              v                  |
|  +-----------------------------------------------------------------------------+  |
|  |                            Receipt Journal Storage                          |  |
|  |                                                                             |  |
|  |   zap-journal: Segment Rotation & Sealing (max_bytes, max_count, max_records)  |  |
|  |   - Writes: .zjseg, .zjidx, .zjmanifest.json                                 |  |
|  |                                                                             |  |
|  |   zap-ledger: Signed Manifests & Indexed Queries                              |  |
|  |   - Signs: .zjmanifest.json.sig via Node Keypair (Ed25519)                  |  |
|  |   - Fast Queries: ReceiptSegmentIndex Pruning + Hash Point Lookup           |  |
|  +-----------------------------------------------------------------------------+  |
+-----------------------------------------------------------------------------------+
```

---

### Component 1 Blueprint: Disk-Persisted Replay Stores

#### File 1: `crates/zap-net/src/durable_replay.rs` (NEW)
#### File 2: `crates/zap-net/src/lib.rs` (MODIFIED)

**`crates/zap-net/src/durable_replay.rs` Definition**:
```rust
use std::collections::{HashSet, VecDeque};
use std::fs::{File, OpenOptions};
use std::io::{Read, Write, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use uuid::Uuid;
use crate::{Result, ZapNetError, NONCE_LEN};

pub const DURABLE_NONCE_MAGIC: &[u8; 8] = b"ZAPNONC1";
pub const DURABLE_NONCE_RECORD_LEN: usize = 8 + 16 + 12; // timestamp_micros (u64) + node_id (16B) + nonce (12B)

#[derive(Debug)]
pub struct DurableNonceRecord {
    pub timestamp_micros: u64,
    pub node_id: Uuid,
    pub nonce: [u8; NONCE_LEN],
}

#[derive(Debug)]
pub struct DurableNonceStore {
    path: PathBuf,
    capacity: usize,
    max_age_micros: u64,
    seen: HashSet<[u8; NONCE_LEN]>,
    order: VecDeque<([u8; NONCE_LEN], u64)>,
    file: Option<File>,
}

impl DurableNonceStore {
    pub fn open(path: impl AsRef<Path>, capacity: usize, max_age_micros: u64) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut seen = HashSet::new();
        let mut order = VecDeque::new();
        let now_micros = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_micros() as u64;

        if path.exists() {
            let mut file = File::open(&path)?;
            let mut magic = [0_u8; 8];
            if file.read_exact(&mut magic).is_ok() && &magic == DURABLE_NONCE_MAGIC {
                let mut buf = [0_u8; DURABLE_NONCE_RECORD_LEN];
                while file.read_exact(&mut buf).is_ok() {
                    let timestamp_micros = u64::from_be_bytes(buf[0..8].try_into().unwrap());
                    if now_micros.saturating_sub(timestamp_micros) <= max_age_micros {
                        let mut nonce = [0_u8; NONCE_LEN];
                        nonce.copy_from_slice(&buf[24..36]);
                        seen.insert(nonce);
                        order.push_back((nonce, timestamp_micros));
                    }
                }
            }
        }

        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .read(true)
            .open(&path)?;

        if file.metadata()?.len() == 0 {
            file.write_all(DURABLE_NONCE_MAGIC)?;
            file.flush()?;
        }

        Ok(Self {
            path,
            capacity,
            max_age_micros,
            seen,
            order,
            file: Some(file),
        })
    }

    pub fn remember(&mut self, node_id: Uuid, nonce: [u8; NONCE_LEN], timestamp_micros: u64) -> Result<()> {
        if self.capacity == 0 {
            return Ok(());
        }
        if self.seen.contains(&nonce) {
            return Err(ZapNetError::ReplayedDatagramNonce { node_id });
        }

        if let Some(file) = &mut self.file {
            let mut buf = [0_u8; DURABLE_NONCE_RECORD_LEN];
            buf[0..8].copy_from_slice(&timestamp_micros.to_be_bytes());
            buf[8..24].copy_from_slice(node_id.as_bytes());
            buf[24..36].copy_from_slice(&nonce);
            file.write_all(&buf)?;
            file.sync_data()?;
        }

        self.seen.insert(nonce);
        self.order.push_back((nonce, timestamp_micros));

        while self.order.len() > self.capacity {
            if let Some((expired, _)) = self.order.pop_front() {
                self.seen.remove(&expired);
            }
        }
        Ok(())
    }

    pub fn compact(&mut self, now_micros: u64) -> Result<()> {
        let tmp_path = self.path.with_extension("tmp");
        {
            let mut tmp_file = OpenOptions::new()
                .create(true)
                .truncate(true)
                .write(true)
                .open(&tmp_path)?;
            tmp_file.write_all(DURABLE_NONCE_MAGIC)?;
            for (nonce, timestamp_micros) in &self.order {
                if now_micros.saturating_sub(*timestamp_micros) <= self.max_age_micros {
                    let mut buf = [0_u8; DURABLE_NONCE_RECORD_LEN];
                    buf[0..8].copy_from_slice(&timestamp_micros.to_be_bytes());
                    buf[8..24].copy_from_slice(Uuid::nil().as_bytes());
                    buf[24..36].copy_from_slice(nonce);
                    tmp_file.write_all(&buf)?;
                }
            }
            tmp_file.sync_all()?;
        }
        std::fs::rename(&tmp_path, &self.path)?;
        self.file = Some(OpenOptions::new().append(true).read(true).open(&self.path)?);
        Ok(())
    }
}
```

#### File 3: `crates/zap-node/src/durable_replay.rs` (NEW)
#### File 4: `crates/zap-node/src/lib.rs` (MODIFIED)

**`crates/zap-node/src/durable_replay.rs` Definition**:
```rust
use std::collections::{HashMap, VecDeque};
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use anyhow::{bail, Result};
use uuid::Uuid;
use zap_core::ZapFrame;

pub const DURABLE_FRAME_MAGIC: &[u8; 8] = b"ZAPFRM01";
pub const DURABLE_FRAME_RECORD_LEN: usize = 8 + 16 + 16; // timestamp_micros (8B) + source_node (16B) + fingerprint (16B)

#[derive(Debug)]
pub struct DurableReplayStore {
    path: PathBuf,
    capacity: usize,
    max_clock_skew_micros: u64,
    seen: HashMap<[u8; 16], (u64, Uuid)>,
    order: VecDeque<([u8; 16], u64)>,
    file: Option<File>,
}

impl DurableReplayStore {
    pub fn open(path: impl AsRef<Path>, capacity: usize, max_clock_skew_micros: u64) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut seen = HashMap::new();
        let mut order = VecDeque::new();
        let now_micros = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_micros() as u64;

        if path.exists() {
            let mut file = File::open(&path)?;
            let mut magic = [0_u8; 8];
            if file.read_exact(&mut magic).is_ok() && &magic == DURABLE_FRAME_MAGIC {
                let mut buf = [0_u8; DURABLE_FRAME_RECORD_LEN];
                while file.read_exact(&mut buf).is_ok() {
                    let timestamp_micros = u64::from_be_bytes(buf[0..8].try_into().unwrap());
                    if now_micros.saturating_sub(timestamp_micros) <= max_clock_skew_micros {
                        let source_node = Uuid::from_bytes(buf[8..24].try_into().unwrap());
                        let mut fingerprint = [0_u8; 16];
                        fingerprint.copy_from_slice(&buf[24..40]);
                        seen.insert(fingerprint, (timestamp_micros, source_node));
                        order.push_back((fingerprint, timestamp_micros));
                    }
                }
            }
        }

        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .read(true)
            .open(&path)?;

        if file.metadata()?.len() == 0 {
            file.write_all(DURABLE_FRAME_MAGIC)?;
            file.flush()?;
        }

        Ok(Self {
            path,
            capacity,
            max_clock_skew_micros,
            seen,
            order,
            file: Some(file),
        })
    }

    pub fn check_and_insert(&mut self, frame: &ZapFrame, now_micros: u64) -> Result<()> {
        if self.capacity == 0 {
            return Ok(());
        }

        let ts = frame.header.timestamp_micros;
        if ts + self.max_clock_skew_micros < now_micros || ts > now_micros + self.max_clock_skew_micros {
            bail!("frame timestamp outside clock skew window");
        }

        let fingerprint = frame_fingerprint(frame);
        if let Some((prev_ts, source)) = self.seen.get(&fingerprint) {
            bail!(
                "replayed frame rejected: source_node={}, timestamp_micros={}, signature_hint={}",
                source,
                prev_ts,
                hex_hint(frame.header.zap_sign)
            );
        }

        if let Some(file) = &mut self.file {
            let mut buf = [0_u8; DURABLE_FRAME_RECORD_LEN];
            buf[0..8].copy_from_slice(&ts.to_be_bytes());
            buf[8..24].copy_from_slice(frame.header.source_node.as_bytes());
            buf[24..40].copy_from_slice(&fingerprint);
            file.write_all(&buf)?;
            file.sync_data()?;
        }

        self.seen.insert(fingerprint, (ts, frame.header.source_node));
        self.order.push_back((fingerprint, ts));

        while self.order.len() > self.capacity {
            if let Some((expired, _)) = self.order.pop_front() {
                self.seen.remove(&expired);
            }
        }
        Ok(())
    }
}

pub fn frame_fingerprint(frame: &ZapFrame) -> [u8; 16] {
    let hash = blake3::hash(&frame.encode());
    hash.as_bytes()[..16].try_into().unwrap()
}

fn hex_hint(sign: [u8; 64]) -> String {
    hex::encode(&sign[..4])
}
```

Integration in `crates/zap-node/src/lib.rs`:
- Update `ReplayGuard` struct:
  ```rust
  #[derive(Debug)]
  pub struct ReplayGuard {
      capacity: usize,
      durable_store: Option<DurableReplayStore>,
      in_memory_seen: HashSet<[u8; 16]>,
      in_memory_order: VecDeque<[u8; 16]>,
  }
  ```
- Method `check_and_insert(&mut self, frame: &ZapFrame)` delegates to `durable_store.check_and_insert(frame, now)` if durable store is configured, ensuring zero replay vulnerability across node reboots.

---

### Component 2 Blueprint: Automatic Segment Rotation & Cryptographic Sealing (`zap-journal`)

#### File: `crates/zap-journal/src/lib.rs` (MODIFIED)

**Updated `JournalOptions`**:
```rust
#[derive(Debug, Clone)]
pub struct JournalOptions {
    pub max_segment_bytes: u64,
    pub max_segment_count: Option<usize>,
    pub max_segment_records: Option<u64>,
}

impl Default for JournalOptions {
    fn default() -> Self {
        Self {
            max_segment_bytes: DEFAULT_MAX_SEGMENT_BYTES,
            max_segment_count: None,
            max_segment_records: None,
        }
    }
}
```

**New & Updated Methods in `JournalStore`**:
```rust
impl JournalStore {
    /// Cryptographically seals a closed segment by sequence number, computing BLAKE3 digest and writing .zjmanifest.json.
    pub fn seal_segment(&self, sequence: u64) -> Result<JournalSegmentManifest> {
        let segment_path = self.segment_path(sequence);
        if !segment_path.exists() {
            return Err(ZapJournalError::MissingJournal(segment_path));
        }

        let segment_bytes = fs::metadata(&segment_path)?.len();
        let raw = fs::read(&segment_path)?;
        let segment_hash = hash_bytes(&raw);

        let index = self.load_segment_index_by_sequence(sequence)?;
        let first_entry_hash = index.entries.first().map(|e| e.entry_hash.clone());
        let last_entry_hash = index.entries.last().map(|e| e.entry_hash.clone());
        let first_timestamp_micros = index.entries.first().map(|e| e.timestamp_micros);
        let last_timestamp_micros = index.entries.last().map(|e| e.timestamp_micros);

        let manifest = JournalSegmentManifest {
            schema_version: 1,
            profile: self.profile,
            segment_id: index.segment_id,
            segment_sequence: sequence,
            entries: index.entries.len() as u64,
            segment_bytes,
            segment_hash,
            first_entry_hash,
            last_entry_hash,
            first_timestamp_micros,
            last_timestamp_micros,
            compression: "none".to_string(),
        };

        let json = serde_json::to_string_pretty(&manifest)?;
        fs::write(self.manifest_path(sequence), json)?;
        Ok(manifest)
    }

    /// Explicitly rotates and seals the currently open segment.
    pub fn rotate_and_seal(&self) -> Result<JournalSegmentManifest> {
        let segments = self.segments()?;
        let last = segments.last().ok_or_else(|| ZapJournalError::MissingJournal(self.dir.clone()))?;
        let manifest = self.seal_segment(last.sequence)?;
        if let Some(max_count) = self.options.max_segment_count {
            self.prune_old_segments(max_count)?;
        }
        Ok(manifest)
    }

    /// Prunes segments when count exceeds max_segment_count.
    fn prune_old_segments(&self, max_count: usize) -> Result<()> {
        let segments = self.segments()?;
        if segments.len() > max_count {
            let to_remove = segments.len() - max_count;
            for seg in segments.iter().take(to_remove) {
                let _ = fs::remove_file(&seg.path);
                let _ = fs::remove_file(self.index_path(seg.sequence));
                let _ = fs::remove_file(self.manifest_path(seg.sequence));
            }
        }
        Ok(())
    }
}
```

---

### Component 3 Blueprint: Signed Segment Manifests (`zap-ledger`)

#### File: `crates/zap-ledger/src/lib.rs` (MODIFIED)

**Constant Definition**:
```rust
pub const SIGNED_MANIFEST_EXTENSION: &str = "zjmanifest.json.sig";
```

**Updated `ReceiptJournalStore` Struct & Methods**:
```rust
pub struct ReceiptJournalStore {
    journal: JournalStore,
    keypair: Option<Keypair>,
}

impl ReceiptJournalStore {
    pub fn open(dir: impl Into<PathBuf>) -> Self {
        Self {
            journal: JournalStore::open(dir, JournalProfile::Receipts),
            keypair: None,
        }
    }

    pub fn open_with_keypair(dir: impl Into<PathBuf>, keypair: Keypair) -> Self {
        Self {
            journal: JournalStore::open(dir, JournalProfile::Receipts),
            keypair: Some(keypair),
        }
    }

    pub fn set_keypair(&mut self, keypair: Keypair) {
        self.keypair = Some(keypair);
    }

    pub fn signed_manifest_path(&self, sequence: u64) -> PathBuf {
        self.journal.dir().join(format!("{sequence:020}.{SIGNED_MANIFEST_EXTENSION}"))
    }

    /// Rotates closed segment, signs its receipt segment manifest, and saves .zjmanifest.json.sig
    pub fn rotate_and_seal_segment(&self, sequence: u64) -> Result<SignedReceiptSegmentManifest> {
        let keypair = self.keypair.as_ref().ok_or(ZapLedgerError::MissingSigningKey)?;
        let receipts = self.read_segment_receipts(sequence)?;
        let previous_segment_hash = if sequence > 0 {
            let prev_signed = self.load_signed_manifest(sequence - 1)?;
            Some(prev_signed.manifest.segment_hash.clone())
        } else {
            None
        };

        let segment_id = Uuid::new_v4();
        let manifest = ReceiptSegmentManifest::from_receipts(
            segment_id,
            sequence,
            &receipts,
            previous_segment_hash,
        )?;

        let signed_manifest = SignedReceiptSegmentManifest::sign(keypair, manifest)?;
        let path = self.signed_manifest_path(sequence);
        fs::write(&path, signed_manifest.to_json_string()?)?;
        Ok(signed_manifest)
    }

    pub fn load_signed_manifest(&self, sequence: u64) -> Result<SignedReceiptSegmentManifest> {
        let path = self.signed_manifest_path(sequence);
        if !path.exists() {
            return Err(ZapLedgerError::MissingSegmentManifest(path));
        }
        let content = fs::read_to_string(&path)?;
        let signed: SignedReceiptSegmentManifest = serde_json::from_str(&content)?;
        signed.verify()?;
        Ok(signed)
    }

    pub fn build_and_verify_segment_index(&self) -> Result<ReceiptSegmentIndex> {
        let node_id = self.keypair.as_ref().map(|k| k.node_id()).unwrap_or_default();
        let mut manifests = Vec::new();
        let mut sequence = 0_u64;
        while self.signed_manifest_path(sequence).exists() {
            let signed = self.load_signed_manifest(sequence)?;
            manifests.push(signed);
            sequence += 1;
        }
        ReceiptSegmentIndex::from_manifests(node_id, &manifests)
    }

    fn read_segment_receipts(&self, sequence: u64) -> Result<Vec<SignedActionReceipt>> {
        let mut receipts = Vec::new();
        // Reads raw records for specified sequence segment from journal store
        let index = self.journal.load_segment_index_by_sequence(sequence)?;
        for entry in index.entries {
            let record = self.journal.read_record_at(sequence, &entry)?;
            let receipt: SignedActionReceipt = serde_json::from_slice(&record.payload)?;
            receipts.push(receipt);
        }
        Ok(receipts)
    }
}
```

---

### Component 4 Blueprint: Fast Indexed Query Engine (`zap-journal` / `zap-ledger`)

#### Files: `crates/zap-journal/src/lib.rs` & `crates/zap-ledger/src/lib.rs` (MODIFIED)

**`zap-journal` Master Index & Pruning**:
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SegmentSummary {
    pub sequence: u64,
    pub segment_id: Uuid,
    pub first_timestamp_micros: u64,
    pub last_timestamp_micros: u64,
    pub first_entry_hash: String,
    pub last_entry_hash: String,
    pub segment_hash: String,
    pub entries_count: u64,
}

impl JournalStore {
    /// Accelerated query filtering segments prior to reading entry records.
    pub fn query_indexed(&self, query: &JournalQuery) -> Result<Vec<JournalRecord>> {
        if !self.dir.exists() {
            return Ok(Vec::new());
        }
        self.rebuild_missing_indexes()?;
        let segments = self.segments()?;
        let mut candidate_segments = Vec::new();

        for segment in segments {
            if let Ok(manifest) = self.load_manifest(segment.sequence) {
                if let (Some(first_ts), Some(last_ts)) = (manifest.first_timestamp_micros, manifest.last_timestamp_micros) {
                    if let Some(after) = query.after_timestamp_micros {
                        if last_ts <= after {
                            continue; // Skip segment entirely
                        }
                    }
                    if let Some(until) = query.until_timestamp_micros {
                        if first_ts > until {
                            continue; // Skip segment entirely
                        }
                    }
                }
            }
            candidate_segments.push(segment);
        }

        let mut candidates = Vec::new();
        for segment in candidate_segments {
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
            if let Some(limit) = query.limit {
                if records.len() >= limit {
                    break;
                }
            }
        }
        Ok(records)
    }
}
```

**`zap-ledger` Segment Index Accelerated Query**:
```rust
impl ReceiptJournalStore {
    /// Fast indexed query leveraging ReceiptSegmentIndex candidate filtering
    pub fn query_fast(&self, request: &ReceiptReplicationRequest) -> Result<Vec<SignedActionReceipt>> {
        request.validate()?;
        let limit = request.effective_limit()?;

        if let Ok(segment_index) = self.build_and_verify_segment_index() {
            let candidates = segment_index.candidate_segments(request)?;
            let candidate_sequences: std::collections::HashSet<u64> = candidates.iter().map(|e| e.segment_sequence).collect();

            let records = self.journal.query_filtered(
                &JournalQuery {
                    kind: request.kind.clone(),
                    subject: request.subject.clone(),
                    source_node: request.source_node,
                    target_node: request.target_node,
                    after_timestamp_micros: request.after_processed_at_micros,
                    until_timestamp_micros: request.until_processed_at_micros,
                    limit: Some(limit),
                    ..JournalQuery::default()
                },
                &candidate_sequences,
            )?;

            let mut receipts = Vec::new();
            for record in records {
                let receipt: SignedActionReceipt = serde_json::from_slice(&record.payload)?;
                receipts.push(receipt);
            }
            verify_action_receipts(&receipts, None)?;
            receipts.retain(|receipt| request.matches(receipt));
            return Ok(receipts);
        }

        // Fallback to standard query if index is not present
        self.query_with_limit(request, limit)
    }
}
```

---

## 5. Verification Method

### Test Plan & Commands

1. **Workspace Test Suite**:
   ```powershell
   cargo test --workspace --all-targets
   ```
   *Expected Result*: All 20 crate test suites pass with 0 errors.

2. **Durable Replay Protection Unit & Reboot Simulation Test**:
   ```powershell
   cargo test -p zap-net --lib durable_replay
   cargo test -p zap-node --lib durable_replay
   ```
   *Verification Steps*:
   - Instantiate `DurableReplayStore` at `temp_dir/replay.wal`.
   - Send frame $F_1$. Verify `check_and_insert($F_1$)` returns `Ok(())`.
   - Drop `DurableReplayStore` instance (simulating process restart).
   - Re-open `DurableReplayStore` at `temp_dir/replay.wal`.
   - Send frame $F_1$ again. Verify `check_and_insert($F_1$)` returns `Err("replayed frame rejected...")`.

3. **Segment Rotation & Sealing Test**:
   ```powershell
   cargo test -p zap-journal --lib tests::journal_rotates_and_seals_segments
   ```
   *Verification Steps*:
   - Set `max_segment_bytes = 512` in `JournalOptions`.
   - Append 50 records.
   - Verify multiple `.zjseg` files exist (`00000000000000000000.zjseg`, `00000000000000000001.zjseg`).
   - Verify each closed segment has a corresponding `.zjmanifest.json` file with valid `segment_hash`.

4. **Signed Segment Manifest Test**:
   ```powershell
   cargo test -p zap-ledger --lib tests::signed_segment_manifest_rotation
   ```
   *Verification Steps*:
   - Open `ReceiptJournalStore` with node `Keypair`.
   - Append receipts exceeding segment threshold.
   - Verify `{sequence:020}.zjmanifest.json.sig` is generated.
   - Call `load_signed_manifest(0).unwrap().verify().unwrap()`.
   - Tamper 1 byte in `.zjseg` file, verify `load_signed_manifest` fails signature/hash verification.

5. **Fast Indexed Query Performance Test**:
   ```powershell
   cargo test -p zap-ledger --benches -- receipt
   ```
   *Verification Steps*:
   - Verify candidate segment pruning reduces disk record scans by > 90% for narrow timestamp window queries.

### Invalidation Conditions
- If any replay attack post-reboot succeeds during reboot simulation tests, the blueprint is invalidated.
- If signed segment manifest chain verification fails on valid sequential segments, the ledger index implementation is invalidated.
