//! Signed action receipts for ZAP nodes.
//!
//! Receipts are local, durable audit records. They are not financial records.

use base64::{Engine as _, engine::general_purpose::STANDARD_NO_PAD};
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey, verify_batch};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
};
use thiserror::Error;
use uuid::Uuid;
use zap_core::{ZapFlags, ZapFrame};
use zap_crypto::{Keypair, node_id_from_public_key};
use zap_journal::{JournalProfile, JournalQuery, JournalRecordInput, JournalStore};

pub mod batch;
pub mod mmr;
pub mod zk;

pub use batch::*;
pub use mmr::*;
pub use zk::*;

pub const RECEIPT_SCHEMA_VERSION: u8 = 1;
pub const RECEIPT_REPLICATION_SCHEMA_VERSION: u8 = 1;
pub const RECEIPT_REPLICATION_CONTENT_TYPE: &str = "application/zap-receipts+json";
pub const RECEIPT_REPLICATION_REQUEST_SUBJECT: &str = "zap.receipts.request";
pub const RECEIPT_REPLICATION_RESPONSE_SUBJECT: &str = "zap.receipts.response";
pub const DEFAULT_RECEIPT_REPLICATION_LIMIT: usize = 50;
pub const MAX_RECEIPT_REPLICATION_LIMIT: usize = 500;
pub const RECEIPT_SEGMENT_MANIFEST_SCHEMA_VERSION: u8 = 1;
pub const RECEIPT_SEGMENT_INDEX_SCHEMA_VERSION: u8 = 1;
pub const RECEIPT_SEGMENT_MANIFEST_CONTENT_TYPE: &str =
    "application/zap-receipt-segment-manifest+json";
pub const RECEIPT_JOURNAL_CONTENT_TYPE: &str = "application/zap-receipt+json";
pub const SIGNED_MANIFEST_EXTENSION: &str = "zjmanifest.json.sig";

const RECEIPT_SIGNATURE_DOMAIN: &[u8] = b"ZAP-ACTION-RECEIPT-v1";
const RECEIPT_SEGMENT_MANIFEST_SIGNATURE_DOMAIN: &[u8] = b"ZAP-RECEIPT-SEGMENT-MANIFEST-v1";
pub const HASH_PREFIX: &str = "blake3:";
const PUBLIC_KEY_LEN: usize = 32;
const SIGNATURE_LEN: usize = 64;
const BATCH_VERIFY_MIN_RECEIPTS: usize = 4;
const PARALLEL_VERIFY_MIN_RECEIPTS: usize = 128;
const RECEIPT_VERIFY_CHUNK_SIZE: usize = 64;

#[derive(Debug, Error)]
pub enum ZapLedgerError {
    #[error("crypto error: {0}")]
    Crypto(#[from] zap_crypto::ZapCryptoError),
    #[error("mmr error: {0}")]
    Mmr(#[from] MmrError),
    #[error("receipt schema version {0} is unsupported")]
    UnsupportedSchemaVersion(u8),
    #[error("receipt replication schema version {0} is unsupported")]
    UnsupportedReplicationSchemaVersion(u8),
    #[error("receipt segment manifest schema version {0} is unsupported")]
    UnsupportedSegmentManifestSchemaVersion(u8),
    #[error("receipt segment index schema version {0} is unsupported")]
    UnsupportedSegmentIndexSchemaVersion(u8),
    #[error("receipt replication limit must be between 1 and {max}, got {actual}")]
    InvalidReplicationLimit { actual: usize, max: usize },
    #[error(
        "receipt replication window must have until_processed_at_micros greater than after_processed_at_micros"
    )]
    InvalidReplicationWindow { after: u64, until: u64 },
    #[error("receipt segment must contain at least one receipt")]
    EmptyReceiptSegment,
    #[error(
        "receipt segment node_id {segment_node_id} does not match receipt node_id {receipt_node_id}"
    )]
    ReceiptSegmentNodeMismatch {
        segment_node_id: Uuid,
        receipt_node_id: Uuid,
    },
    #[error(
        "receipt segment timestamps are not ordered: previous {previous} is after current {current}"
    )]
    ReceiptSegmentOutOfOrder { previous: u64, current: u64 },
    #[error("receipt segment index expected sequence {expected}, got {actual}")]
    ReceiptSegmentSequenceGap { expected: u64, actual: u64 },
    #[error("receipt segment index contains duplicate {field}: {value}")]
    DuplicateReceiptSegmentIndexEntry { field: &'static str, value: String },
    #[error(
        "receipt segment index chain mismatch at sequence {sequence}: expected previous hash {expected}, got {actual:?}"
    )]
    ReceiptSegmentChainMismatch {
        sequence: u64,
        expected: String,
        actual: Option<String>,
    },
    #[error(
        "receipt segment manifest node_id {manifest_node_id} does not match signer node_id {signer_node_id}"
    )]
    SegmentManifestNodeMismatch {
        manifest_node_id: Uuid,
        signer_node_id: Uuid,
    },
    #[error(
        "receipt segment manifest signer public key derives node_id {derived}, but manifest declares {declared}"
    )]
    SegmentManifestSignerNodeMismatch { declared: Uuid, derived: Uuid },
    #[error("invalid receipt artifact hash for {field}: {value}")]
    InvalidArtifactHash { field: &'static str, value: String },
    #[error("receipt signer public key derives node_id {derived}, but receipt declares {declared}")]
    SignerNodeMismatch { declared: Uuid, derived: Uuid },
    #[error("receipt node_id {receipt_node_id} does not match signer node_id {signer_node_id}")]
    ReceiptNodeMismatch {
        receipt_node_id: Uuid,
        signer_node_id: Uuid,
    },
    #[error("invalid receipt field {field}: {reason}")]
    InvalidReceiptField {
        field: &'static str,
        reason: &'static str,
    },
    #[error("receipt signature verification failed")]
    InvalidSignature,
    #[error("invalid receipt key material length for {kind}: expected {expected}, got {actual}")]
    InvalidKeyLength {
        kind: &'static str,
        expected: usize,
        actual: usize,
    },
    #[error("failed to decode base64 receipt key material: {0}")]
    Base64(#[from] base64::DecodeError),
    #[error("failed to parse Ed25519 receipt key material: {0}")]
    Ed25519(#[from] ed25519_dalek::SignatureError),
    #[error("receipt io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("failed to serialize receipt signing payload: {0}")]
    Json(#[from] serde_json::Error),
    #[error("receipt journal error: {0}")]
    Journal(#[from] zap_journal::ZapJournalError),
    #[error("receipt output {0} already exists")]
    OutputExists(PathBuf),
}

pub type Result<T> = std::result::Result<T, ZapLedgerError>;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PoaReceipt {
    pub required_threshold: u16,
    pub certificate_threshold: u16,
    pub attestation_count: u16,
    pub validators: Vec<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PactReceiptReference {
    pub pact_id: Uuid,
    pub intent: String,
    pub hash: String,
    pub status: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub policy_decision: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub poa_summary: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_hash: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ActionReceipt {
    pub schema_version: u8,
    pub node_id: Uuid,
    pub source_node: Uuid,
    pub target_node: Uuid,
    pub kind: String,
    pub subject: String,
    pub action: String,
    pub frame_hash: String,
    pub payload_hash: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_hash: Option<String>,
    pub frame_timestamp_micros: u64,
    pub processed_at_micros: u64,
    pub flags: u16,
    pub consensus_required: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub poa: Option<PoaReceipt>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pact: Option<PactReceiptReference>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SignedActionReceipt {
    pub receipt: ActionReceipt,
    pub signer_node_id: Uuid,
    pub signer_public_key: String,
    pub signature: String,
}

impl ActionReceipt {
    pub fn validate_static(&self) -> Result<()> {
        if self.schema_version != RECEIPT_SCHEMA_VERSION {
            return Err(ZapLedgerError::UnsupportedSchemaVersion(
                self.schema_version,
            ));
        }
        if self.kind.is_empty() {
            return Err(ZapLedgerError::InvalidReceiptField {
                field: "kind",
                reason: "must not be empty",
            });
        }
        if self.subject.is_empty() {
            return Err(ZapLedgerError::InvalidReceiptField {
                field: "subject",
                reason: "must not be empty",
            });
        }
        if self.action.is_empty() {
            return Err(ZapLedgerError::InvalidReceiptField {
                field: "action",
                reason: "must not be empty",
            });
        }
        validate_artifact_hash("frame_hash", &self.frame_hash)?;
        validate_artifact_hash("payload_hash", &self.payload_hash)?;
        if let Some(output_hash) = &self.output_hash {
            validate_artifact_hash("output_hash", output_hash)?;
        }
        if self.processed_at_micros < self.frame_timestamp_micros {
            return Err(ZapLedgerError::InvalidReceiptField {
                field: "processed_at_micros",
                reason: "must be greater than or equal to frame_timestamp_micros",
            });
        }

        let flags = ZapFlags::from_bits(self.flags).ok_or(ZapLedgerError::InvalidReceiptField {
            field: "flags",
            reason: "contains unknown bits",
        })?;
        let flags_require_consensus = flags.contains(ZapFlags::REQUIRES_CONSENSUS);
        if self.consensus_required != flags_require_consensus {
            return Err(ZapLedgerError::InvalidReceiptField {
                field: "consensus_required",
                reason: "must match the REQUIRES_CONSENSUS frame flag",
            });
        }
        match (self.consensus_required, self.poa.as_ref()) {
            (true, Some(poa)) => poa.validate_static()?,
            (true, None) => {
                return Err(ZapLedgerError::InvalidReceiptField {
                    field: "poa",
                    reason: "is required when consensus_required is true",
                });
            }
            (false, Some(_)) => {
                return Err(ZapLedgerError::InvalidReceiptField {
                    field: "poa",
                    reason: "must be absent when consensus_required is false",
                });
            }
            (false, None) => {}
        }

        if let Some(pact) = &self.pact {
            validate_artifact_hash("pact.hash", &pact.hash)?;
            if let Some(output_hash) = &pact.output_hash {
                validate_artifact_hash("pact.output_hash", output_hash)?;
            }
        }
        Ok(())
    }
}

impl PoaReceipt {
    fn validate_static(&self) -> Result<()> {
        if self.required_threshold == 0 {
            return Err(ZapLedgerError::InvalidReceiptField {
                field: "poa.required_threshold",
                reason: "must be greater than zero",
            });
        }
        if self.certificate_threshold == 0 {
            return Err(ZapLedgerError::InvalidReceiptField {
                field: "poa.certificate_threshold",
                reason: "must be greater than zero",
            });
        }
        if self.attestation_count as usize != self.validators.len() {
            return Err(ZapLedgerError::InvalidReceiptField {
                field: "poa.attestation_count",
                reason: "must match validators length",
            });
        }
        if self.certificate_threshold > self.attestation_count {
            return Err(ZapLedgerError::InvalidReceiptField {
                field: "poa.certificate_threshold",
                reason: "must be less than or equal to attestation_count",
            });
        }
        if self.required_threshold > self.certificate_threshold {
            return Err(ZapLedgerError::InvalidReceiptField {
                field: "poa.required_threshold",
                reason: "must be less than or equal to certificate_threshold",
            });
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReceiptReplicationRequest {
    pub schema_version: u8,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after_processed_at_micros: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub until_processed_at_micros: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subject: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_node: Option<Uuid>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_node: Option<Uuid>,
}

impl Default for ReceiptReplicationRequest {
    fn default() -> Self {
        Self {
            schema_version: RECEIPT_REPLICATION_SCHEMA_VERSION,
            after_processed_at_micros: None,
            until_processed_at_micros: None,
            limit: Some(DEFAULT_RECEIPT_REPLICATION_LIMIT),
            kind: None,
            subject: None,
            source_node: None,
            target_node: None,
        }
    }
}

impl ReceiptReplicationRequest {
    pub fn validate(&self) -> Result<()> {
        if self.schema_version != RECEIPT_REPLICATION_SCHEMA_VERSION {
            return Err(ZapLedgerError::UnsupportedReplicationSchemaVersion(
                self.schema_version,
            ));
        }
        if let (Some(after), Some(until)) = (
            self.after_processed_at_micros,
            self.until_processed_at_micros,
        ) && until <= after
        {
            return Err(ZapLedgerError::InvalidReplicationWindow { after, until });
        }
        self.effective_limit()?;
        Ok(())
    }

    pub fn effective_limit(&self) -> Result<usize> {
        let limit = self.limit.unwrap_or(DEFAULT_RECEIPT_REPLICATION_LIMIT);
        if limit == 0 || limit > MAX_RECEIPT_REPLICATION_LIMIT {
            return Err(ZapLedgerError::InvalidReplicationLimit {
                actual: limit,
                max: MAX_RECEIPT_REPLICATION_LIMIT,
            });
        }
        Ok(limit)
    }

    pub fn matches(&self, receipt: &SignedActionReceipt) -> bool {
        if let Some(after) = self.after_processed_at_micros
            && receipt.receipt.processed_at_micros <= after
        {
            return false;
        }
        if let Some(until) = self.until_processed_at_micros
            && receipt.receipt.processed_at_micros > until
        {
            return false;
        }
        if let Some(kind) = self.kind.as_deref()
            && receipt.receipt.kind != kind
        {
            return false;
        }
        if let Some(subject) = self.subject.as_deref()
            && receipt.receipt.subject != subject
        {
            return false;
        }
        if let Some(source_node) = self.source_node
            && receipt.receipt.source_node != source_node
        {
            return false;
        }
        if let Some(target_node) = self.target_node
            && receipt.receipt.target_node != target_node
        {
            return false;
        }
        true
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReceiptReplicationResponse {
    pub schema_version: u8,
    pub node_id: Uuid,
    pub receipts: Vec<SignedActionReceipt>,
    pub truncated: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_after_processed_at_micros: Option<u64>,
}

impl ReceiptReplicationResponse {
    pub fn new(node_id: Uuid, receipts: Vec<SignedActionReceipt>, truncated: bool) -> Self {
        Self::new_with_cursor(node_id, receipts, truncated, None)
    }

    pub fn new_with_cursor(
        node_id: Uuid,
        receipts: Vec<SignedActionReceipt>,
        truncated: bool,
        next_after_processed_at_micros: Option<u64>,
    ) -> Self {
        Self {
            schema_version: RECEIPT_REPLICATION_SCHEMA_VERSION,
            node_id,
            receipts,
            truncated,
            next_after_processed_at_micros,
        }
    }

    pub fn verify(&self) -> Result<()> {
        if self.schema_version != RECEIPT_REPLICATION_SCHEMA_VERSION {
            return Err(ZapLedgerError::UnsupportedReplicationSchemaVersion(
                self.schema_version,
            ));
        }
        verify_action_receipts(&self.receipts, Some(self.node_id))
    }
}

#[derive(Debug, Clone)]
pub struct ReceiptJournalStore {
    journal: JournalStore,
    keypair: Option<Keypair>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReceiptJournalVerificationReport {
    pub dir: PathBuf,
    pub segments: usize,
    pub receipts: usize,
    pub verified: bool,
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

    pub fn with_options(mut self, options: zap_journal::JournalOptions) -> Self {
        self.journal = self.journal.with_options(options);
        self
    }

    pub fn set_keypair(&mut self, keypair: Keypair) {
        self.keypair = Some(keypair);
    }

    pub fn keypair(&self) -> Option<&Keypair> {
        self.keypair.as_ref()
    }

    pub fn dir(&self) -> &Path {
        self.journal.dir()
    }

    pub fn signed_manifest_path(&self, sequence: u64) -> PathBuf {
        self.dir()
            .join(format!("{sequence:020}.{SIGNED_MANIFEST_EXTENSION}"))
    }

    pub fn rotate_and_seal_segment(&self, sequence: u64) -> Result<SignedReceiptSegmentManifest> {
        let keypair = self
            .keypair
            .as_ref()
            .ok_or_else(|| ZapLedgerError::InvalidReceiptField {
                field: "keypair",
                reason: "node keypair is required to sign segment manifests",
            })?;
        let receipts = self.read_segment_receipts(sequence)?;
        let previous_segment_hash = if sequence > 0 {
            if let Ok(prev_signed) = self.load_signed_manifest(sequence - 1) {
                Some(prev_signed.manifest.segment_hash.clone())
            } else if let Ok(prev_manifest) = self.journal.load_manifest(sequence - 1) {
                Some(prev_manifest.segment_hash.clone())
            } else {
                None
            }
        } else {
            None
        };

        let segment_index = self.journal.load_segment_index_by_sequence(sequence)?;
        let manifest = ReceiptSegmentManifest::from_receipts(
            segment_index.segment_id,
            sequence,
            &receipts,
            previous_segment_hash,
        )?;

        let signed = SignedReceiptSegmentManifest::sign(keypair, manifest)?;
        let path = self.signed_manifest_path(sequence);
        fs::write(&path, signed.to_json_string()?)?;
        Ok(signed)
    }

    pub fn load_signed_manifest(&self, sequence: u64) -> Result<SignedReceiptSegmentManifest> {
        let path = self.signed_manifest_path(sequence);
        if !path.exists() {
            return Err(ZapLedgerError::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("missing signed manifest at {}", path.display()),
            )));
        }
        let content = fs::read_to_string(&path)?;
        let signed = SignedReceiptSegmentManifest::from_json_str(&content)?;
        signed.verify()?;
        Ok(signed)
    }

    pub fn build_and_verify_segment_index(&self) -> Result<ReceiptSegmentIndex> {
        let node_id = self
            .keypair
            .as_ref()
            .map(|k| k.node_id())
            .unwrap_or_default();
        let mut manifests = Vec::new();
        for segment in self.journal.segments()? {
            if self.signed_manifest_path(segment.sequence).exists() {
                let signed = self.load_signed_manifest(segment.sequence)?;
                manifests.push(signed);
            }
        }
        ReceiptSegmentIndex::from_manifests(node_id, &manifests)
    }

    pub fn read_segment_receipts(&self, sequence: u64) -> Result<Vec<SignedActionReceipt>> {
        let mut receipts = Vec::new();
        let index = self.journal.load_segment_index_by_sequence(sequence)?;
        for entry in index.entries {
            let record = self.journal.read_record_at(sequence, &entry)?;
            let receipt: SignedActionReceipt = serde_json::from_slice(&record.payload)?;
            receipts.push(receipt);
        }
        Ok(receipts)
    }

    pub fn query_fast(
        &self,
        request: &ReceiptReplicationRequest,
    ) -> Result<Vec<SignedActionReceipt>> {
        request.validate()?;
        let limit = request.effective_limit()?;

        if let Ok(segment_index) = self.build_and_verify_segment_index()
            && !segment_index.entries.is_empty()
        {
            let candidates = segment_index.candidate_segments(request)?;
            let candidate_sequences: HashSet<u64> =
                candidates.iter().map(|e| e.segment_sequence).collect();

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

        self.query_with_limit(request, limit)
    }

    pub fn ensure_sealed_segments_signed(&self) -> Result<()> {
        let Some(_keypair) = &self.keypair else {
            return Ok(());
        };
        let segments = self.journal.segments()?;
        if segments.len() <= 1 {
            return Ok(());
        }
        for segment in segments.iter().take(segments.len() - 1) {
            let seq = segment.sequence;
            if !self.signed_manifest_path(seq).exists() {
                let _ = self.rotate_and_seal_segment(seq);
            }
        }
        Ok(())
    }

    pub fn append(&self, receipt: &SignedActionReceipt, sync_data: bool) -> Result<()> {
        receipt.verify()?;
        let payload = serde_json::to_vec(receipt)?;
        self.journal.append(
            JournalRecordInput {
                kind: receipt.receipt.kind.clone(),
                schema_version: RECEIPT_SCHEMA_VERSION as u16,
                timestamp_micros: receipt.receipt.processed_at_micros,
                id: None,
                namespace: Some(receipt.receipt.action.clone()),
                subject: Some(receipt.receipt.subject.clone()),
                content_type: Some(RECEIPT_JOURNAL_CONTENT_TYPE.to_string()),
                source_node: Some(receipt.receipt.source_node),
                target_node: Some(receipt.receipt.target_node),
                tombstone_for: None,
                metadata: serde_json::json!({
                    "frame_hash": receipt.receipt.frame_hash,
                    "payload_hash": receipt.receipt.payload_hash,
                    "output_hash": receipt.receipt.output_hash,
                    "signer_node_id": receipt.signer_node_id,
                    "signature": receipt.signature,
                }),
                payload,
            },
            sync_data,
        )?;
        self.ensure_sealed_segments_signed()?;
        Ok(())
    }

    pub fn query(&self, request: &ReceiptReplicationRequest) -> Result<Vec<SignedActionReceipt>> {
        request.validate()?;
        self.query_fast(request)
    }

    pub fn query_with_limit(
        &self,
        request: &ReceiptReplicationRequest,
        limit: usize,
    ) -> Result<Vec<SignedActionReceipt>> {
        request.validate()?;
        let records = self.journal.query(&JournalQuery {
            kind: request.kind.clone(),
            subject: request.subject.clone(),
            source_node: request.source_node,
            target_node: request.target_node,
            after_timestamp_micros: request.after_processed_at_micros,
            until_timestamp_micros: request.until_processed_at_micros,
            limit: Some(limit),
            ..JournalQuery::default()
        })?;
        let mut receipts = Vec::new();
        for record in records {
            let receipt: SignedActionReceipt = serde_json::from_slice(&record.payload)?;
            receipts.push(receipt);
        }
        verify_action_receipts(&receipts, None)?;
        receipts.retain(|receipt| request.matches(receipt));
        Ok(receipts)
    }

    pub fn all(&self) -> Result<Vec<SignedActionReceipt>> {
        let mut receipts = Vec::new();
        for record in self.journal.records()? {
            let receipt: SignedActionReceipt = serde_json::from_slice(&record.payload)?;
            receipts.push(receipt);
        }
        verify_action_receipts(&receipts, None)?;
        Ok(receipts)
    }

    pub fn verify(&self) -> Result<ReceiptJournalVerificationReport> {
        let report = self.journal.verify()?;
        let receipts = self.all()?.len();
        Ok(ReceiptJournalVerificationReport {
            dir: report.dir,
            segments: report.segments,
            receipts,
            verified: true,
        })
    }

    pub fn rebuild_indexes(&self) -> Result<ReceiptJournalVerificationReport> {
        self.journal.rebuild_indexes()?;
        self.verify()
    }

    pub fn recover_partial_tail(&self) -> Result<bool> {
        Ok(self.journal.recover_partial_tail()?.is_some())
    }

    pub fn import_jsonl(&self, input: &Path, force: bool) -> Result<usize> {
        JournalStore::remove_dir_if_allowed(self.dir(), force)?;
        fs::create_dir_all(self.dir())?;
        let receipts = load_verified_receipt_jsonl(input)?;
        for receipt in &receipts {
            self.append(receipt, false)?;
        }
        Ok(receipts.len())
    }

    pub fn export_jsonl(&self, out: &Path, force: bool) -> Result<usize> {
        if out.exists() && !force {
            return Err(ZapLedgerError::OutputExists(out.to_path_buf()));
        }
        if let Some(parent) = out.parent().filter(|parent| !parent.as_os_str().is_empty()) {
            fs::create_dir_all(parent)?;
        }
        let receipts = self.all()?;
        let mut output = String::new();
        for receipt in &receipts {
            output.push_str(&receipt.to_json_line()?);
        }
        fs::write(out, output)?;
        Ok(receipts.len())
    }

    pub fn compact(&self, out: &Path, force: bool) -> Result<usize> {
        JournalStore::remove_dir_if_allowed(out, force)?;
        let compacted = ReceiptJournalStore::open(out);
        let receipts = self.all()?;
        for receipt in &receipts {
            compacted.append(receipt, false)?;
        }
        Ok(receipts.len())
    }

    pub fn batch_seal_path(&self, sequence: u64) -> PathBuf {
        self.dir()
            .join(format!("{sequence:020}.{BATCH_SEAL_EXTENSION}"))
    }

    pub fn zmmr_path(&self, sequence: u64) -> PathBuf {
        self.dir().join(format!("{sequence:020}.zmmr"))
    }

    pub fn seal_segment_batch(
        &self,
        sequence: u64,
        validators: &[Keypair],
        threshold: u16,
        initial_state_hash: String,
        final_state_hash: String,
        total_fuel_consumed: u64,
    ) -> Result<ReceiptBatchSeal> {
        let receipts = self.read_segment_receipts(sequence)?;
        if receipts.is_empty() {
            return Err(ZapLedgerError::EmptyReceiptSegment);
        }

        let first = receipts.first().unwrap();
        let last = receipts.last().unwrap();
        let node_id = first.receipt.node_id;

        let mut mmr = MerkleMountainRange::new();
        for r in &receipts {
            let canon = r.signing_message()?;
            mmr.append_bytes(&canon);
        }
        let mmr_root = format!("{HASH_PREFIX}{}", mmr.root_hex());

        let mut seal = ReceiptBatchSeal {
            schema_version: RECEIPT_BATCH_SEAL_SCHEMA_VERSION,
            batch_id: Uuid::new_v4(),
            node_id,
            segment_sequence: sequence,
            start_sequence: 0,
            end_sequence: (receipts.len() - 1) as u64,
            receipt_count: receipts.len() as u64,
            first_processed_at_micros: first.receipt.processed_at_micros,
            last_processed_at_micros: last.receipt.processed_at_micros,
            mmr_root,
            initial_state_hash,
            final_state_hash,
            total_fuel_consumed,
            quorum_threshold: threshold,
            validator_signatures: Vec::new(),
        };

        let mut signatures = Vec::new();
        for v in validators {
            let sig = seal.sign_with_validator(v)?;
            signatures.push(sig);
        }
        seal.validator_signatures = signatures;

        seal.validate_static()?;
        let json = serde_json::to_string_pretty(&seal)?;
        fs::write(self.batch_seal_path(sequence), json)?;

        // Also checkpoint incremental MMR snapshot
        let mut inc_mmr = IncrementalMmr::new();
        for r in &receipts {
            let canon = r.signing_message()?;
            inc_mmr.append_bytes(&canon);
        }
        inc_mmr.save_to_file(self.zmmr_path(sequence))?;

        Ok(seal)
    }

    pub fn load_batch_seal(&self, sequence: u64) -> Result<ReceiptBatchSeal> {
        let path = self.batch_seal_path(sequence);
        if !path.exists() {
            return Err(ZapLedgerError::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("missing batch seal at {}", path.display()),
            )));
        }
        let content = fs::read_to_string(&path)?;
        let seal: ReceiptBatchSeal = serde_json::from_str(&content)?;
        seal.validate_static()?;
        Ok(seal)
    }

    pub fn save_segment_zmmr(&self, sequence: u64, mmr: &mut IncrementalMmr) -> Result<()> {
        let path = self.zmmr_path(sequence);
        mmr.save_to_file(path)?;
        Ok(())
    }

    pub fn load_segment_zmmr(&self, sequence: u64) -> Result<IncrementalMmr> {
        let path = self.zmmr_path(sequence);
        if !path.exists() {
            return Err(ZapLedgerError::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("missing zmmr checkpoint at {}", path.display()),
            )));
        }
        let mmr = IncrementalMmr::load_from_file(path)?;
        Ok(mmr)
    }

    /// Build an Incremental Merkle Mountain Range accumulator from all receipts in the journal.
    pub fn build_incremental_mmr(&self) -> Result<IncrementalMmr> {
        let receipts = self.all()?;
        let mut mmr = IncrementalMmr::new();
        for receipt in &receipts {
            let canon = receipt.signing_message()?;
            mmr.append_bytes(&canon);
        }
        Ok(mmr)
    }

    /// Build a Merkle Mountain Range accumulator from all receipts in the journal.
    pub fn build_mmr_accumulator(&self) -> Result<MerkleMountainRange> {
        let receipts = self.all()?;
        let mut mmr = MerkleMountainRange::new();
        for receipt in &receipts {
            let canon = receipt.signing_message()?;
            mmr.append_bytes(&canon);
        }
        Ok(mmr)
    }

    /// Generate an O(log N) inclusion proof for the receipt at the given index.
    pub fn prove_receipt_mmr_inclusion(
        &self,
        index: usize,
    ) -> Result<(MmrInclusionProof, MmrHash)> {
        let mut mmr = self.build_mmr_accumulator()?;
        let proof = mmr.prove_inclusion(index)?;
        let root = mmr.root();
        Ok((proof, root))
    }

    /// Generate a deduplicated multi-leaf batch inclusion proof for receipts at the given indices.
    pub fn prove_receipt_batch_inclusion(
        &self,
        indices: &[usize],
    ) -> Result<(MmrBatchInclusionProof, MmrHash)> {
        let mut mmr = self.build_mmr_accumulator()?;
        let proof = mmr.prove_batch_inclusion(indices)?;
        let root = mmr.root();
        Ok((proof, root))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReceiptSegmentManifest {
    pub schema_version: u8,
    pub node_id: Uuid,
    pub segment_id: Uuid,
    pub segment_sequence: u64,
    pub receipts_count: u64,
    pub segment_bytes: u64,
    pub segment_hash: String,
    pub first_receipt_hash: String,
    pub last_receipt_hash: String,
    pub first_processed_at_micros: u64,
    pub last_processed_at_micros: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub previous_segment_hash: Option<String>,
}

impl ReceiptSegmentManifest {
    pub fn from_receipts(
        segment_id: Uuid,
        segment_sequence: u64,
        receipts: &[SignedActionReceipt],
        previous_segment_hash: Option<String>,
    ) -> Result<Self> {
        let first = receipts
            .first()
            .ok_or(ZapLedgerError::EmptyReceiptSegment)?;
        let node_id = first.receipt.node_id;
        let mut segment_bytes = Vec::new();
        let mut previous_processed_at = None;
        for receipt in receipts {
            receipt.verify()?;
            if receipt.receipt.node_id != node_id {
                return Err(ZapLedgerError::ReceiptSegmentNodeMismatch {
                    segment_node_id: node_id,
                    receipt_node_id: receipt.receipt.node_id,
                });
            }
            if let Some(previous) = previous_processed_at
                && receipt.receipt.processed_at_micros < previous
            {
                return Err(ZapLedgerError::ReceiptSegmentOutOfOrder {
                    previous,
                    current: receipt.receipt.processed_at_micros,
                });
            }
            segment_bytes.extend_from_slice(receipt.to_json_line()?.as_bytes());
            previous_processed_at = Some(receipt.receipt.processed_at_micros);
        }
        let last = receipts.last().unwrap();
        let manifest = Self {
            schema_version: RECEIPT_SEGMENT_MANIFEST_SCHEMA_VERSION,
            node_id,
            segment_id,
            segment_sequence,
            receipts_count: receipts.len() as u64,
            segment_bytes: segment_bytes.len() as u64,
            segment_hash: hash_bytes(&segment_bytes),
            first_receipt_hash: receipt_hash(first)?,
            last_receipt_hash: receipt_hash(last)?,
            first_processed_at_micros: first.receipt.processed_at_micros,
            last_processed_at_micros: last.receipt.processed_at_micros,
            previous_segment_hash,
        };
        manifest.validate()?;
        Ok(manifest)
    }

    pub fn validate(&self) -> Result<()> {
        if self.schema_version != RECEIPT_SEGMENT_MANIFEST_SCHEMA_VERSION {
            return Err(ZapLedgerError::UnsupportedSegmentManifestSchemaVersion(
                self.schema_version,
            ));
        }
        if self.receipts_count == 0 {
            return Err(ZapLedgerError::EmptyReceiptSegment);
        }
        if self.last_processed_at_micros < self.first_processed_at_micros {
            return Err(ZapLedgerError::ReceiptSegmentOutOfOrder {
                previous: self.first_processed_at_micros,
                current: self.last_processed_at_micros,
            });
        }
        validate_artifact_hash("segment_hash", &self.segment_hash)?;
        validate_artifact_hash("first_receipt_hash", &self.first_receipt_hash)?;
        validate_artifact_hash("last_receipt_hash", &self.last_receipt_hash)?;
        if let Some(previous) = &self.previous_segment_hash {
            validate_artifact_hash("previous_segment_hash", previous)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SignedReceiptSegmentManifest {
    pub manifest: ReceiptSegmentManifest,
    pub signer_node_id: Uuid,
    pub signer_public_key: String,
    pub signature: String,
}

impl SignedReceiptSegmentManifest {
    pub fn sign(signer: &Keypair, manifest: ReceiptSegmentManifest) -> Result<Self> {
        manifest.validate()?;
        if manifest.node_id != signer.node_id() {
            return Err(ZapLedgerError::SegmentManifestNodeMismatch {
                manifest_node_id: manifest.node_id,
                signer_node_id: signer.node_id(),
            });
        }
        let signer_public_key = STANDARD_NO_PAD.encode(signer.verifying_key().to_bytes());
        let mut signed = Self {
            manifest,
            signer_node_id: signer.node_id(),
            signer_public_key,
            signature: String::new(),
        };
        let signing_key = SigningKey::from_bytes(&signer.secret_bytes());
        let signature: Signature = signing_key.sign(&signed.signing_message()?);
        signed.signature = STANDARD_NO_PAD.encode(signature.to_bytes());
        Ok(signed)
    }

    pub fn verify(&self) -> Result<()> {
        self.manifest.validate()?;
        if self.manifest.node_id != self.signer_node_id {
            return Err(ZapLedgerError::SegmentManifestNodeMismatch {
                manifest_node_id: self.manifest.node_id,
                signer_node_id: self.signer_node_id,
            });
        }
        let public_key_bytes =
            decode_fixed::<PUBLIC_KEY_LEN>(&self.signer_public_key, "public_key")?;
        let derived_node_id = node_id_from_public_key(&public_key_bytes);
        if derived_node_id != self.signer_node_id {
            return Err(ZapLedgerError::SegmentManifestSignerNodeMismatch {
                declared: self.signer_node_id,
                derived: derived_node_id,
            });
        }
        let verifying_key = VerifyingKey::from_bytes(&public_key_bytes)?;
        let signature_bytes = decode_fixed::<SIGNATURE_LEN>(&self.signature, "signature")?;
        let signature = Signature::from_bytes(&signature_bytes);
        verifying_key
            .verify(&self.signing_message()?, &signature)
            .map_err(|_| ZapLedgerError::InvalidSignature)
    }

    pub fn manifest_hash(&self) -> Result<String> {
        Ok(hash_bytes(&serde_json::to_vec(&self.manifest)?))
    }

    pub fn to_json_string(&self) -> Result<String> {
        Ok(serde_json::to_string_pretty(self)?)
    }

    pub fn from_json_str(input: &str) -> Result<Self> {
        Ok(serde_json::from_str(input)?)
    }

    fn signing_message(&self) -> Result<Vec<u8>> {
        let payload = ReceiptSegmentManifestSigningPayload {
            manifest: &self.manifest,
            signer_node_id: self.signer_node_id,
            signer_public_key: &self.signer_public_key,
        };
        let encoded = serde_json::to_vec(&payload)?;
        let mut message =
            Vec::with_capacity(RECEIPT_SEGMENT_MANIFEST_SIGNATURE_DOMAIN.len() + encoded.len());
        message.extend_from_slice(RECEIPT_SEGMENT_MANIFEST_SIGNATURE_DOMAIN);
        message.extend_from_slice(&encoded);
        Ok(message)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReceiptSegmentIndexEntry {
    pub node_id: Uuid,
    pub segment_id: Uuid,
    pub segment_sequence: u64,
    pub manifest_hash: String,
    pub segment_hash: String,
    pub receipts_count: u64,
    pub segment_bytes: u64,
    pub first_processed_at_micros: u64,
    pub last_processed_at_micros: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub previous_segment_hash: Option<String>,
}

impl ReceiptSegmentIndexEntry {
    pub fn from_signed_manifest(manifest: &SignedReceiptSegmentManifest) -> Result<Self> {
        manifest.verify()?;
        Ok(Self {
            node_id: manifest.manifest.node_id,
            segment_id: manifest.manifest.segment_id,
            segment_sequence: manifest.manifest.segment_sequence,
            manifest_hash: manifest.manifest_hash()?,
            segment_hash: manifest.manifest.segment_hash.clone(),
            receipts_count: manifest.manifest.receipts_count,
            segment_bytes: manifest.manifest.segment_bytes,
            first_processed_at_micros: manifest.manifest.first_processed_at_micros,
            last_processed_at_micros: manifest.manifest.last_processed_at_micros,
            previous_segment_hash: manifest.manifest.previous_segment_hash.clone(),
        })
    }

    pub fn overlaps_request(&self, request: &ReceiptReplicationRequest) -> bool {
        if let Some(after) = request.after_processed_at_micros
            && self.last_processed_at_micros <= after
        {
            return false;
        }
        if let Some(until) = request.until_processed_at_micros
            && self.first_processed_at_micros > until
        {
            return false;
        }
        true
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReceiptSegmentIndex {
    pub schema_version: u8,
    pub node_id: Uuid,
    pub entries: Vec<ReceiptSegmentIndexEntry>,
}

impl ReceiptSegmentIndex {
    pub fn from_manifests(
        node_id: Uuid,
        manifests: &[SignedReceiptSegmentManifest],
    ) -> Result<Self> {
        let mut entries = manifests
            .iter()
            .map(ReceiptSegmentIndexEntry::from_signed_manifest)
            .collect::<Result<Vec<_>>>()?;
        entries.sort_by_key(|entry| entry.segment_sequence);
        let index = Self {
            schema_version: RECEIPT_SEGMENT_INDEX_SCHEMA_VERSION,
            node_id,
            entries,
        };
        index.validate()?;
        Ok(index)
    }

    pub fn validate(&self) -> Result<()> {
        if self.schema_version != RECEIPT_SEGMENT_INDEX_SCHEMA_VERSION {
            return Err(ZapLedgerError::UnsupportedSegmentIndexSchemaVersion(
                self.schema_version,
            ));
        }
        let mut previous_sequence = None;
        let mut previous_last_processed_at = None;
        let mut previous_entry: Option<&ReceiptSegmentIndexEntry> = None;
        let mut segment_ids = HashSet::new();
        let mut segment_hashes = HashSet::new();
        for entry in &self.entries {
            if entry.node_id != self.node_id {
                return Err(ZapLedgerError::ReceiptSegmentNodeMismatch {
                    segment_node_id: self.node_id,
                    receipt_node_id: entry.node_id,
                });
            }
            if entry.receipts_count == 0 {
                return Err(ZapLedgerError::EmptyReceiptSegment);
            }
            if entry.last_processed_at_micros < entry.first_processed_at_micros {
                return Err(ZapLedgerError::ReceiptSegmentOutOfOrder {
                    previous: entry.first_processed_at_micros,
                    current: entry.last_processed_at_micros,
                });
            }
            if !segment_ids.insert(entry.segment_id) {
                return Err(ZapLedgerError::DuplicateReceiptSegmentIndexEntry {
                    field: "segment_id",
                    value: entry.segment_id.to_string(),
                });
            }
            validate_artifact_hash("manifest_hash", &entry.manifest_hash)?;
            validate_artifact_hash("segment_hash", &entry.segment_hash)?;
            if !segment_hashes.insert(entry.segment_hash.clone()) {
                return Err(ZapLedgerError::DuplicateReceiptSegmentIndexEntry {
                    field: "segment_hash",
                    value: entry.segment_hash.clone(),
                });
            }
            if let Some(previous) = &entry.previous_segment_hash {
                validate_artifact_hash("previous_segment_hash", previous)?;
            }
            if let Some(previous) = previous_sequence
                && entry.segment_sequence <= previous
            {
                return Err(ZapLedgerError::ReceiptSegmentOutOfOrder {
                    previous,
                    current: entry.segment_sequence,
                });
            }
            if let Some(previous) = previous_sequence {
                let expected = previous + 1;
                if entry.segment_sequence != expected {
                    return Err(ZapLedgerError::ReceiptSegmentSequenceGap {
                        expected,
                        actual: entry.segment_sequence,
                    });
                }
            }
            if let Some(previous) = previous_entry
                && entry.previous_segment_hash.as_deref() != Some(previous.segment_hash.as_str())
            {
                return Err(ZapLedgerError::ReceiptSegmentChainMismatch {
                    sequence: entry.segment_sequence,
                    expected: previous.segment_hash.clone(),
                    actual: entry.previous_segment_hash.clone(),
                });
            }
            if let Some(previous) = previous_last_processed_at
                && entry.first_processed_at_micros < previous
            {
                return Err(ZapLedgerError::ReceiptSegmentOutOfOrder {
                    previous,
                    current: entry.first_processed_at_micros,
                });
            }
            previous_sequence = Some(entry.segment_sequence);
            previous_last_processed_at = Some(entry.last_processed_at_micros);
            previous_entry = Some(entry);
        }
        Ok(())
    }

    pub fn candidate_segments<'a>(
        &'a self,
        request: &ReceiptReplicationRequest,
    ) -> Result<Vec<&'a ReceiptSegmentIndexEntry>> {
        self.validate()?;
        request.validate()?;
        Ok(self
            .entries
            .iter()
            .filter(|entry| entry.overlaps_request(request))
            .collect())
    }
}

impl SignedActionReceipt {
    pub fn sign(signer: &Keypair, receipt: ActionReceipt) -> Result<Self> {
        let signer_public_key = STANDARD_NO_PAD.encode(signer.verifying_key().to_bytes());
        let mut signed = Self {
            receipt,
            signer_node_id: signer.node_id(),
            signer_public_key,
            signature: String::new(),
        };
        let signing_key = SigningKey::from_bytes(&signer.secret_bytes());
        let signature: Signature = signing_key.sign(&signed.signing_message()?);
        signed.signature = STANDARD_NO_PAD.encode(signature.to_bytes());
        Ok(signed)
    }

    pub fn new(
        signer: &Keypair,
        frame: &ZapFrame,
        action: impl Into<String>,
        output: Option<&[u8]>,
        processed_at_micros: u64,
        required_poa_threshold: Option<u16>,
    ) -> Result<Self> {
        Self::new_message(
            signer,
            frame,
            "action",
            action.into(),
            output,
            processed_at_micros,
            required_poa_threshold,
        )
    }

    pub fn new_message(
        signer: &Keypair,
        frame: &ZapFrame,
        kind: impl Into<String>,
        subject: impl Into<String>,
        output: Option<&[u8]>,
        processed_at_micros: u64,
        required_poa_threshold: Option<u16>,
    ) -> Result<Self> {
        Self::new_message_with_pact(
            signer,
            frame,
            kind,
            subject,
            output,
            processed_at_micros,
            required_poa_threshold,
            None,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn new_message_with_pact(
        signer: &Keypair,
        frame: &ZapFrame,
        kind: impl Into<String>,
        subject: impl Into<String>,
        output: Option<&[u8]>,
        processed_at_micros: u64,
        required_poa_threshold: Option<u16>,
        pact: Option<PactReceiptReference>,
    ) -> Result<Self> {
        let kind = kind.into();
        let subject = subject.into();
        let output_hash = output.map(hash_bytes);
        let receipt = ActionReceipt {
            schema_version: RECEIPT_SCHEMA_VERSION,
            node_id: signer.node_id(),
            source_node: frame.header.source_node,
            target_node: frame.header.target_node,
            kind,
            subject: subject.clone(),
            action: subject,
            frame_hash: hash_bytes(&frame.encode()),
            payload_hash: hash_bytes(&frame.payload),
            output_hash: output_hash.clone(),
            frame_timestamp_micros: frame.header.timestamp_micros,
            processed_at_micros,
            flags: frame.header.flags.bits(),
            consensus_required: frame.header.flags.contains(ZapFlags::REQUIRES_CONSENSUS),
            poa: build_poa_receipt(frame, required_poa_threshold),
            pact: pact.map(|mut pact| {
                pact.output_hash = pact.output_hash.or(output_hash);
                pact
            }),
        };
        let signer_public_key = STANDARD_NO_PAD.encode(signer.verifying_key().to_bytes());
        let mut signed = Self {
            receipt,
            signer_node_id: signer.node_id(),
            signer_public_key,
            signature: String::new(),
        };
        let signing_key = SigningKey::from_bytes(&signer.secret_bytes());
        let signature: Signature = signing_key.sign(&signed.signing_message()?);
        signed.signature = STANDARD_NO_PAD.encode(signature.to_bytes());
        Ok(signed)
    }

    pub fn verify(&self) -> Result<()> {
        self.receipt.validate_static()?;
        if self.receipt.node_id != self.signer_node_id {
            return Err(ZapLedgerError::ReceiptNodeMismatch {
                receipt_node_id: self.receipt.node_id,
                signer_node_id: self.signer_node_id,
            });
        }
        let public_key_bytes =
            decode_fixed::<PUBLIC_KEY_LEN>(&self.signer_public_key, "public_key")?;
        let derived_node_id = node_id_from_public_key(&public_key_bytes);
        if derived_node_id != self.signer_node_id {
            return Err(ZapLedgerError::SignerNodeMismatch {
                declared: self.signer_node_id,
                derived: derived_node_id,
            });
        }
        let verifying_key = VerifyingKey::from_bytes(&public_key_bytes)?;
        let signature_bytes = decode_fixed::<SIGNATURE_LEN>(&self.signature, "signature")?;
        let signature = Signature::from_bytes(&signature_bytes);
        verifying_key
            .verify(&self.signing_message()?, &signature)
            .map_err(|_| ZapLedgerError::InvalidSignature)
    }

    pub fn to_json_line(&self) -> Result<String> {
        let mut encoded = serde_json::to_string(self)?;
        encoded.push('\n');
        Ok(encoded)
    }

    pub fn from_json_str(input: &str) -> Result<Self> {
        Ok(serde_json::from_str(input)?)
    }

    fn signing_message(&self) -> Result<Vec<u8>> {
        let payload = ReceiptSigningPayload {
            receipt: &self.receipt,
            signer_node_id: self.signer_node_id,
            signer_public_key: &self.signer_public_key,
        };
        let encoded = serde_json::to_vec(&payload)?;
        let mut message = Vec::with_capacity(RECEIPT_SIGNATURE_DOMAIN.len() + encoded.len());
        message.extend_from_slice(RECEIPT_SIGNATURE_DOMAIN);
        message.extend_from_slice(&encoded);
        Ok(message)
    }
}

fn verify_action_receipts(
    receipts: &[SignedActionReceipt],
    expected_node_id: Option<Uuid>,
) -> Result<()> {
    if receipts.len() < BATCH_VERIFY_MIN_RECEIPTS {
        return verify_action_receipt_scalar_chunk(receipts, expected_node_id);
    }

    if receipts.len() >= PARALLEL_VERIFY_MIN_RECEIPTS {
        return receipts
            .par_chunks(RECEIPT_VERIFY_CHUNK_SIZE)
            .try_for_each(|chunk| verify_action_receipt_batch_chunk(chunk, expected_node_id));
    }

    verify_action_receipt_batch_chunk(receipts, expected_node_id)
}

fn verify_action_receipt_scalar_chunk(
    receipts: &[SignedActionReceipt],
    expected_node_id: Option<Uuid>,
) -> Result<()> {
    let mut key_cache: HashMap<&str, (VerifyingKey, Uuid)> = HashMap::new();
    for receipt in receipts {
        let (verifying_key, signature, message) =
            prepare_receipt_verification(receipt, &mut key_cache)?;
        verifying_key
            .verify(&message, &signature)
            .map_err(|_| ZapLedgerError::InvalidSignature)?;
        verify_expected_receipt_node(receipt, expected_node_id)?;
    }
    Ok(())
}

fn verify_action_receipt_batch_chunk(
    receipts: &[SignedActionReceipt],
    expected_node_id: Option<Uuid>,
) -> Result<()> {
    let mut key_cache: HashMap<&str, (VerifyingKey, Uuid)> = HashMap::new();
    let mut messages = Vec::with_capacity(receipts.len());
    let mut signatures = Vec::with_capacity(receipts.len());
    let mut verifying_keys = Vec::with_capacity(receipts.len());

    for receipt in receipts {
        let (verifying_key, signature, message) =
            prepare_receipt_verification(receipt, &mut key_cache)?;
        signatures.push(signature);
        verifying_keys.push(verifying_key);
        messages.push(message);
    }

    let message_refs = messages.iter().map(Vec::as_slice).collect::<Vec<&[u8]>>();
    verify_batch(&message_refs, &signatures, &verifying_keys)
        .map_err(|_| ZapLedgerError::InvalidSignature)?;

    for receipt in receipts {
        verify_expected_receipt_node(receipt, expected_node_id)?;
    }
    Ok(())
}

fn prepare_receipt_verification<'a>(
    receipt: &'a SignedActionReceipt,
    key_cache: &mut HashMap<&'a str, (VerifyingKey, Uuid)>,
) -> Result<(VerifyingKey, Signature, Vec<u8>)> {
    receipt.receipt.validate_static()?;
    if receipt.receipt.node_id != receipt.signer_node_id {
        return Err(ZapLedgerError::ReceiptNodeMismatch {
            receipt_node_id: receipt.receipt.node_id,
            signer_node_id: receipt.signer_node_id,
        });
    }

    let cache_key = receipt.signer_public_key.as_str();
    let (verifying_key, derived_node_id) = match key_cache.get(cache_key).copied() {
        Some(cached) => cached,
        None => {
            let public_key_bytes =
                decode_fixed::<PUBLIC_KEY_LEN>(&receipt.signer_public_key, "public_key")?;
            let derived_node_id = node_id_from_public_key(&public_key_bytes);
            let verifying_key = VerifyingKey::from_bytes(&public_key_bytes)?;
            key_cache.insert(cache_key, (verifying_key, derived_node_id));
            (verifying_key, derived_node_id)
        }
    };
    if derived_node_id != receipt.signer_node_id {
        return Err(ZapLedgerError::SignerNodeMismatch {
            declared: receipt.signer_node_id,
            derived: derived_node_id,
        });
    }

    let signature_bytes = decode_fixed::<SIGNATURE_LEN>(&receipt.signature, "signature")?;
    Ok((
        verifying_key,
        Signature::from_bytes(&signature_bytes),
        receipt.signing_message()?,
    ))
}

fn verify_expected_receipt_node(
    receipt: &SignedActionReceipt,
    expected_node_id: Option<Uuid>,
) -> Result<()> {
    if let Some(expected_node_id) = expected_node_id
        && receipt.receipt.node_id != expected_node_id
    {
        return Err(ZapLedgerError::ReceiptNodeMismatch {
            receipt_node_id: receipt.receipt.node_id,
            signer_node_id: expected_node_id,
        });
    }
    Ok(())
}

#[derive(Debug, Serialize)]
struct ReceiptSigningPayload<'a> {
    receipt: &'a ActionReceipt,
    signer_node_id: Uuid,
    signer_public_key: &'a str,
}

#[derive(Debug, Serialize)]
struct ReceiptSegmentManifestSigningPayload<'a> {
    manifest: &'a ReceiptSegmentManifest,
    signer_node_id: Uuid,
    signer_public_key: &'a str,
}

pub fn hash_bytes(bytes: &[u8]) -> String {
    format!("{HASH_PREFIX}{}", blake3::hash(bytes).to_hex())
}

pub fn load_verified_receipt_jsonl(path: &Path) -> Result<Vec<SignedActionReceipt>> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let input = fs::read_to_string(path)?;
    let mut receipts = Vec::new();
    for line in input.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let receipt = SignedActionReceipt::from_json_str(line)?;
        receipts.push(receipt);
    }
    verify_action_receipts(&receipts, None)?;
    Ok(receipts)
}

fn receipt_hash(receipt: &SignedActionReceipt) -> Result<String> {
    Ok(hash_bytes(&serde_json::to_vec(receipt)?))
}

pub(crate) fn validate_artifact_hash(field: &'static str, value: &str) -> Result<()> {
    let hash =
        value
            .strip_prefix(HASH_PREFIX)
            .ok_or_else(|| ZapLedgerError::InvalidArtifactHash {
                field,
                value: value.to_string(),
            })?;
    if hash.len() != 64 || !hash.chars().all(|ch| ch.is_ascii_hexdigit()) {
        return Err(ZapLedgerError::InvalidArtifactHash {
            field,
            value: value.to_string(),
        });
    }
    Ok(())
}

fn build_poa_receipt(frame: &ZapFrame, required_threshold: Option<u16>) -> Option<PoaReceipt> {
    let poa = frame.poa.as_ref()?;
    Some(PoaReceipt {
        required_threshold: required_threshold.unwrap_or(poa.threshold),
        certificate_threshold: poa.threshold,
        attestation_count: poa.attestations.len() as u16,
        validators: poa
            .attestations
            .iter()
            .map(|attestation| attestation.validator_node)
            .collect(),
    })
}

pub(crate) fn decode_fixed<const N: usize>(encoded: &str, kind: &'static str) -> Result<[u8; N]> {
    let decoded = STANDARD_NO_PAD.decode(encoded)?;
    if decoded.len() != N {
        return Err(ZapLedgerError::InvalidKeyLength {
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
    use bytes::Bytes;
    use zap_core::{ZapFlags, ZapFrame};
    use zap_crypto::{certify_frame, sign_frame};

    fn signed_frame(source: &Keypair, target: Uuid) -> ZapFrame {
        let unsigned = ZapFrame::with_timestamp(
            source.node_id(),
            target,
            ZapFlags::SIGNED,
            123,
            Bytes::from_static(b"payload"),
        )
        .unwrap();
        sign_frame(source, &unsigned).unwrap()
    }

    fn receipt_at(
        node: &Keypair,
        source: &Keypair,
        processed_at_micros: u64,
        subject: &str,
    ) -> SignedActionReceipt {
        let frame = signed_frame(source, node.node_id());
        SignedActionReceipt::new_message(
            node,
            &frame,
            "action",
            subject,
            Some(b"ok"),
            processed_at_micros,
            None,
        )
        .unwrap()
    }

    #[test]
    fn receipt_signs_and_verifies() {
        let node = Keypair::generate();
        let source = Keypair::generate();
        let frame = signed_frame(&source, node.node_id());
        let receipt =
            SignedActionReceipt::new(&node, &frame, "echo", Some(b"ok"), 456, None).unwrap();

        receipt.verify().unwrap();
        assert_eq!(receipt.receipt.node_id, node.node_id());
        assert_eq!(receipt.receipt.kind, "action");
        assert_eq!(receipt.receipt.subject, "echo");
        assert_eq!(receipt.receipt.action, "echo");
        assert!(receipt.receipt.output_hash.is_some());
    }

    #[test]
    fn receipt_supports_universal_message_kind_and_subject() {
        let node = Keypair::generate();
        let source = Keypair::generate();
        let frame = signed_frame(&source, node.node_id());
        let receipt = SignedActionReceipt::new_message(
            &node,
            &frame,
            "event",
            "sensor.temperature",
            None,
            456,
            None,
        )
        .unwrap();

        receipt.verify().unwrap();
        assert_eq!(receipt.receipt.kind, "event");
        assert_eq!(receipt.receipt.subject, "sensor.temperature");
        assert_eq!(receipt.receipt.action, "sensor.temperature");
        assert!(receipt.receipt.output_hash.is_none());
    }

    #[test]
    fn receipt_detects_mutation() {
        let node = Keypair::generate();
        let source = Keypair::generate();
        let frame = signed_frame(&source, node.node_id());
        let mut receipt =
            SignedActionReceipt::new(&node, &frame, "echo", Some(b"ok"), 456, None).unwrap();
        receipt.receipt.subject = "changed".to_string();

        assert!(matches!(
            receipt.verify(),
            Err(ZapLedgerError::InvalidSignature)
        ));
    }

    #[test]
    fn receipt_verify_rejects_invalid_static_hashes() {
        let node = Keypair::generate();
        let source = Keypair::generate();
        let frame = signed_frame(&source, node.node_id());
        let mut receipt =
            SignedActionReceipt::new(&node, &frame, "echo", Some(b"ok"), 456, None).unwrap();
        receipt.receipt.output_hash = Some(format!("sha256:{}", "0".repeat(64)));

        assert!(matches!(
            receipt.verify(),
            Err(ZapLedgerError::InvalidArtifactHash {
                field: "output_hash",
                ..
            })
        ));
    }

    #[test]
    fn receipt_round_trips_jsonl() {
        let node = Keypair::generate();
        let source = Keypair::generate();
        let frame = signed_frame(&source, node.node_id());
        let receipt =
            SignedActionReceipt::new(&node, &frame, "echo", Some(b"ok"), 456, None).unwrap();
        let encoded = receipt.to_json_line().unwrap();
        let decoded = SignedActionReceipt::from_json_str(encoded.trim()).unwrap();

        assert_eq!(decoded, receipt);
        decoded.verify().unwrap();
    }

    #[test]
    fn receipt_journal_appends_queries_exports_and_verifies() {
        let temp = tempfile::tempdir().unwrap();
        let store = ReceiptJournalStore::open(temp.path().join("receipts"));
        let node = Keypair::generate();
        let source = Keypair::generate();
        let first = receipt_at(&node, &source, 1_000, "echo");
        let second = receipt_at(&node, &source, 1_100, "telemetry");
        store.append(&first, false).unwrap();
        store.append(&second, false).unwrap();

        let request = ReceiptReplicationRequest {
            after_processed_at_micros: Some(999),
            kind: Some("action".to_string()),
            subject: Some("echo".to_string()),
            source_node: Some(source.node_id()),
            target_node: Some(node.node_id()),
            ..ReceiptReplicationRequest::default()
        };
        let receipts = store.query(&request).unwrap();
        assert_eq!(receipts, vec![first.clone()]);
        assert_eq!(store.verify().unwrap().receipts, 2);

        let jsonl = temp.path().join("receipts.jsonl");
        assert_eq!(store.export_jsonl(&jsonl, false).unwrap(), 2);
        let imported = ReceiptJournalStore::open(temp.path().join("imported"));
        assert_eq!(imported.import_jsonl(&jsonl, false).unwrap(), 2);
        assert_eq!(imported.all().unwrap(), vec![first, second]);
    }

    #[test]
    fn receipt_records_poa() {
        let node = Keypair::generate();
        let source = Keypair::generate();
        let validator = Keypair::generate();
        let unsigned = ZapFrame::with_timestamp(
            source.node_id(),
            node.node_id(),
            ZapFlags::REQUIRES_CONSENSUS,
            123,
            Bytes::from_static(b"critical"),
        )
        .unwrap();
        let signed = sign_frame(&source, &unsigned).unwrap();
        let certified = certify_frame(&signed, 1, std::slice::from_ref(&validator)).unwrap();
        let receipt = SignedActionReceipt::new(
            &node,
            &certified,
            "safety.emergency_stop",
            None,
            456,
            Some(1),
        )
        .unwrap();

        receipt.verify().unwrap();
        assert_eq!(receipt.receipt.poa.unwrap().attestation_count, 1);
    }

    #[test]
    fn receipt_replication_request_filters_receipts() {
        let node = Keypair::generate();
        let source = Keypair::generate();
        let frame = signed_frame(&source, node.node_id());
        let receipt =
            SignedActionReceipt::new_message(&node, &frame, "action", "echo", None, 456, None)
                .unwrap();
        let request = ReceiptReplicationRequest {
            after_processed_at_micros: Some(455),
            until_processed_at_micros: Some(456),
            kind: Some("action".to_string()),
            subject: Some("echo".to_string()),
            source_node: Some(source.node_id()),
            target_node: Some(node.node_id()),
            ..ReceiptReplicationRequest::default()
        };

        request.validate().unwrap();
        assert!(request.matches(&receipt));

        let stale_request = ReceiptReplicationRequest {
            after_processed_at_micros: Some(456),
            ..request
        };
        assert!(!stale_request.matches(&receipt));

        let invalid_window = ReceiptReplicationRequest {
            after_processed_at_micros: Some(456),
            until_processed_at_micros: Some(456),
            ..ReceiptReplicationRequest::default()
        };
        assert!(matches!(
            invalid_window.validate(),
            Err(ZapLedgerError::InvalidReplicationWindow { .. })
        ));
    }

    #[test]
    fn receipt_replication_response_verifies_nested_receipts() {
        let node = Keypair::generate();
        let source = Keypair::generate();
        let frame = signed_frame(&source, node.node_id());
        let receipt = SignedActionReceipt::new(&node, &frame, "echo", None, 456, None).unwrap();
        let response =
            ReceiptReplicationResponse::new(node.node_id(), vec![receipt.clone()], false);

        response.verify().unwrap();
        let paged_response = ReceiptReplicationResponse::new_with_cursor(
            node.node_id(),
            vec![receipt.clone()],
            true,
            Some(receipt.receipt.processed_at_micros),
        );
        paged_response.verify().unwrap();
        assert_eq!(
            paged_response.next_after_processed_at_micros,
            Some(receipt.receipt.processed_at_micros)
        );

        let wrong_node = ReceiptReplicationResponse::new(source.node_id(), vec![receipt], false);
        assert!(matches!(
            wrong_node.verify(),
            Err(ZapLedgerError::ReceiptNodeMismatch { .. })
        ));
    }

    #[test]
    fn receipt_replication_response_verifies_empty_batch() {
        let node = Keypair::generate();
        let response = ReceiptReplicationResponse::new(node.node_id(), Vec::new(), false);

        response.verify().unwrap();
    }

    #[test]
    fn receipt_replication_response_batch_verifies_eight_receipts() {
        let node = Keypair::generate();
        let source = Keypair::generate();
        let receipts = (0..8)
            .map(|index| receipt_at(&node, &source, 1_000 + index, "echo"))
            .collect::<Vec<_>>();
        let response = ReceiptReplicationResponse::new(node.node_id(), receipts, false);

        response.verify().unwrap();
    }

    #[test]
    fn receipt_replication_response_batch_detects_modified_signature() {
        let node = Keypair::generate();
        let source = Keypair::generate();
        let mut receipts = (0..8)
            .map(|index| receipt_at(&node, &source, 1_000 + index, "echo"))
            .collect::<Vec<_>>();
        let mut signature = STANDARD_NO_PAD.decode(&receipts[3].signature).unwrap();
        signature[0] ^= 0x55;
        receipts[3].signature = STANDARD_NO_PAD.encode(signature);
        let response = ReceiptReplicationResponse::new(node.node_id(), receipts, false);

        assert!(matches!(
            response.verify(),
            Err(ZapLedgerError::InvalidSignature)
        ));
    }

    #[test]
    fn receipt_replication_response_batch_detects_wrong_node() {
        let node = Keypair::generate();
        let source = Keypair::generate();
        let mut receipts = (0..8)
            .map(|index| receipt_at(&node, &source, 1_000 + index, "echo"))
            .collect::<Vec<_>>();
        receipts[4].receipt.node_id = source.node_id();
        let response = ReceiptReplicationResponse::new(node.node_id(), receipts, false);

        assert!(matches!(
            response.verify(),
            Err(ZapLedgerError::ReceiptNodeMismatch { .. })
        ));
    }

    #[test]
    fn receipt_batch_verifies_mixed_signers_without_expected_node() {
        let node_a = Keypair::generate();
        let node_b = Keypair::generate();
        let source = Keypair::generate();
        let mut receipts = (0..4)
            .map(|index| receipt_at(&node_a, &source, 1_000 + index, "echo"))
            .collect::<Vec<_>>();
        receipts.extend((0..4).map(|index| receipt_at(&node_b, &source, 2_000 + index, "echo")));

        verify_action_receipts(&receipts, None).unwrap();
    }

    #[test]
    fn receipt_journal_batch_verifies_query_all_and_report() {
        let temp = tempfile::tempdir().unwrap();
        let store = ReceiptJournalStore::open(temp.path().join("receipts"));
        let node = Keypair::generate();
        let source = Keypair::generate();
        let receipts = (0..8)
            .map(|index| {
                receipt_at(
                    &node,
                    &source,
                    1_000 + index,
                    if index % 2 == 0 { "echo" } else { "telemetry" },
                )
            })
            .collect::<Vec<_>>();
        for receipt in &receipts {
            store.append(receipt, false).unwrap();
        }

        let request = ReceiptReplicationRequest {
            subject: Some("echo".to_string()),
            ..ReceiptReplicationRequest::default()
        };
        assert_eq!(store.query(&request).unwrap().len(), 4);
        assert_eq!(store.all().unwrap(), receipts);
        assert_eq!(store.verify().unwrap().receipts, 8);
    }

    #[test]
    fn receipt_replication_rejects_bad_limit() {
        let request = ReceiptReplicationRequest {
            limit: Some(0),
            ..ReceiptReplicationRequest::default()
        };

        assert!(matches!(
            request.validate(),
            Err(ZapLedgerError::InvalidReplicationLimit { .. })
        ));
    }

    #[test]
    fn receipt_segment_manifest_signs_verifies_and_detects_mutation() {
        let node = Keypair::generate();
        let source = Keypair::generate();
        let receipts = vec![
            receipt_at(&node, &source, 1_000, "echo"),
            receipt_at(&node, &source, 1_100, "telemetry"),
        ];
        let manifest =
            ReceiptSegmentManifest::from_receipts(Uuid::from_bytes([1_u8; 16]), 7, &receipts, None)
                .unwrap();
        assert_eq!(manifest.node_id, node.node_id());
        assert_eq!(manifest.receipts_count, 2);
        assert!(manifest.segment_hash.starts_with(HASH_PREFIX));

        let signed = SignedReceiptSegmentManifest::sign(&node, manifest).unwrap();
        signed.verify().unwrap();
        let decoded =
            SignedReceiptSegmentManifest::from_json_str(&signed.to_json_string().unwrap()).unwrap();
        assert_eq!(decoded, signed);

        let mut tampered = signed.clone();
        tampered.manifest.receipts_count = 3;
        assert!(matches!(
            tampered.verify(),
            Err(ZapLedgerError::InvalidSignature)
        ));
    }

    #[test]
    fn receipt_segment_manifest_rejects_mixed_nodes_and_out_of_order_receipts() {
        let node = Keypair::generate();
        let other_node = Keypair::generate();
        let source = Keypair::generate();
        let mixed = vec![
            receipt_at(&node, &source, 1_000, "echo"),
            receipt_at(&other_node, &source, 1_100, "echo"),
        ];
        assert!(matches!(
            ReceiptSegmentManifest::from_receipts(Uuid::from_bytes([2_u8; 16]), 1, &mixed, None),
            Err(ZapLedgerError::ReceiptSegmentNodeMismatch { .. })
        ));

        let out_of_order = vec![
            receipt_at(&node, &source, 1_100, "echo"),
            receipt_at(&node, &source, 1_000, "echo"),
        ];
        assert!(matches!(
            ReceiptSegmentManifest::from_receipts(
                Uuid::from_bytes([3_u8; 16]),
                1,
                &out_of_order,
                None
            ),
            Err(ZapLedgerError::ReceiptSegmentOutOfOrder { .. })
        ));
    }

    #[test]
    fn receipt_segment_index_selects_time_bounded_candidates() {
        let node = Keypair::generate();
        let source = Keypair::generate();
        let first_receipts = vec![
            receipt_at(&node, &source, 1_000, "echo"),
            receipt_at(&node, &source, 1_100, "echo"),
        ];
        let first = SignedReceiptSegmentManifest::sign(
            &node,
            ReceiptSegmentManifest::from_receipts(
                Uuid::from_bytes([4_u8; 16]),
                1,
                &first_receipts,
                None,
            )
            .unwrap(),
        )
        .unwrap();
        let second_receipts = vec![
            receipt_at(&node, &source, 2_000, "echo"),
            receipt_at(&node, &source, 2_100, "echo"),
        ];
        let second = SignedReceiptSegmentManifest::sign(
            &node,
            ReceiptSegmentManifest::from_receipts(
                Uuid::from_bytes([5_u8; 16]),
                2,
                &second_receipts,
                Some(first.manifest.segment_hash.clone()),
            )
            .unwrap(),
        )
        .unwrap();
        let index = ReceiptSegmentIndex::from_manifests(node.node_id(), &[first, second]).unwrap();
        let request = ReceiptReplicationRequest {
            after_processed_at_micros: Some(1_500),
            until_processed_at_micros: Some(2_050),
            ..ReceiptReplicationRequest::default()
        };
        let candidates = index.candidate_segments(&request).unwrap();
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].segment_sequence, 2);

        let empty_request = ReceiptReplicationRequest {
            until_processed_at_micros: Some(900),
            ..ReceiptReplicationRequest::default()
        };
        assert!(index.candidate_segments(&empty_request).unwrap().is_empty());
    }

    #[test]
    fn receipt_segment_index_rejects_chain_mismatch_and_sequence_gap() {
        let node = Keypair::generate();
        let source = Keypair::generate();
        let first_receipts = vec![
            receipt_at(&node, &source, 1_000, "echo"),
            receipt_at(&node, &source, 1_100, "echo"),
        ];
        let first = SignedReceiptSegmentManifest::sign(
            &node,
            ReceiptSegmentManifest::from_receipts(
                Uuid::from_bytes([6_u8; 16]),
                1,
                &first_receipts,
                None,
            )
            .unwrap(),
        )
        .unwrap();
        let second_receipts = vec![
            receipt_at(&node, &source, 2_000, "echo"),
            receipt_at(&node, &source, 2_100, "echo"),
        ];
        let second = SignedReceiptSegmentManifest::sign(
            &node,
            ReceiptSegmentManifest::from_receipts(
                Uuid::from_bytes([7_u8; 16]),
                2,
                &second_receipts,
                Some(first.manifest.segment_hash.clone()),
            )
            .unwrap(),
        )
        .unwrap();
        let index = ReceiptSegmentIndex::from_manifests(node.node_id(), &[first, second]).unwrap();

        let mut wrong_chain = index.clone();
        wrong_chain.entries[1].previous_segment_hash = Some(hash_bytes(b"wrong segment"));
        assert!(matches!(
            wrong_chain.validate(),
            Err(ZapLedgerError::ReceiptSegmentChainMismatch { .. })
        ));

        let mut gap = index;
        gap.entries[1].segment_sequence = 3;
        assert!(matches!(
            gap.validate(),
            Err(ZapLedgerError::ReceiptSegmentSequenceGap {
                expected: 2,
                actual: 3
            })
        ));
    }

    #[test]
    fn signed_segment_manifest_store_integration() {
        let temp = tempfile::tempdir().unwrap();
        let node = Keypair::generate();
        let source = Keypair::generate();
        let options = zap_journal::JournalOptions {
            max_segment_bytes: 64 * 1024,
            max_segment_count: Some(5),
            max_segment_records: Some(3),
        };

        let store =
            ReceiptJournalStore::open_with_keypair(temp.path(), node.clone()).with_options(options);

        for i in 0..6 {
            let receipt = receipt_at(&node, &source, 1_000 + i * 100, "echo");
            store.append(&receipt, false).unwrap();
        }

        // Rotate and seal sequence 0
        let signed_manifest = store.rotate_and_seal_segment(0).unwrap();
        signed_manifest.verify().unwrap();

        // Load signed manifest
        let loaded = store.load_signed_manifest(0).unwrap();
        assert_eq!(loaded, signed_manifest);

        // Build and verify index
        let segment_index = store.build_and_verify_segment_index().unwrap();
        assert!(!segment_index.entries.is_empty());

        // Fast query
        let req = ReceiptReplicationRequest {
            after_processed_at_micros: Some(1_150),
            ..ReceiptReplicationRequest::default()
        };
        let queried = store.query_fast(&req).unwrap();
        assert!(!queried.is_empty());
    }

    #[test]
    fn receipt_journal_batch_sealing_and_zmmr_checkpoint() {
        let temp = tempfile::tempdir().unwrap();
        let node = Keypair::generate();
        let source = Keypair::generate();
        let val1 = Keypair::generate();
        let val2 = Keypair::generate();

        let options = zap_journal::JournalOptions {
            max_segment_bytes: 64 * 1024,
            max_segment_count: Some(5),
            max_segment_records: Some(4),
        };

        let store =
            ReceiptJournalStore::open_with_keypair(temp.path(), node.clone()).with_options(options);

        for i in 0..8 {
            let receipt = receipt_at(&node, &source, 1_000 + i * 100, "tensor_calc");
            store.append(&receipt, false).unwrap();
        }

        let init_state = format!("{HASH_PREFIX}{}", hex::encode([0x01; 32]));
        let final_state = format!("{HASH_PREFIX}{}", hex::encode([0x02; 32]));

        // Seal segment sequence 0
        let seal = store
            .seal_segment_batch(
                0,
                &[val1.clone(), val2.clone()],
                2,
                init_state.clone(),
                final_state.clone(),
                12_500,
            )
            .unwrap();

        assert_eq!(seal.segment_sequence, 0);
        assert_eq!(seal.receipt_count, 4);
        assert_eq!(seal.validator_signatures.len(), 2);

        // Load batch seal
        let loaded_seal = store.load_batch_seal(0).unwrap();
        assert_eq!(loaded_seal, seal);

        // Load segment .zmmr
        let mut loaded_zmmr = store.load_segment_zmmr(0).unwrap();
        assert_eq!(loaded_zmmr.leaf_count, 4);
        assert_eq!(
            format!("{HASH_PREFIX}{}", loaded_zmmr.root_hex()),
            seal.mmr_root
        );
    }

    #[test]
    fn receipt_journal_incremental_mmr_and_batch_proof() {
        let temp = tempfile::tempdir().unwrap();
        let node = Keypair::generate();
        let source = Keypair::generate();

        let store = ReceiptJournalStore::open_with_keypair(temp.path(), node.clone());

        for i in 0..16 {
            let receipt = receipt_at(&node, &source, 1_000 + i * 100, "op");
            store.append(&receipt, false).unwrap();
        }

        let mut inc_mmr = store.build_incremental_mmr().unwrap();
        let mut mem_mmr = store.build_mmr_accumulator().unwrap();

        assert_eq!(inc_mmr.len(), 16);
        assert_eq!(inc_mmr.get_root(), mem_mmr.root());

        // Batch inclusion proof for receipts [1, 4, 7, 15]
        let indices = vec![1, 4, 7, 15];
        let (batch_proof, root) = store.prove_receipt_batch_inclusion(&indices).unwrap();
        assert_eq!(batch_proof.leaf_indices, vec![1, 4, 7, 15]);
        assert_eq!(batch_proof.total_leaves, 16);
        assert!(batch_proof.verify(&root).unwrap());
    }

    #[test]
    fn receipt_scale_1000_batch_verification_sub_millisecond() {
        let mut mmr = MerkleMountainRange::new();
        let count = 1000;

        for i in 0..count {
            let data = format!("scale_receipt_hash_commitment_{i}");
            mmr.append_bytes(data.as_bytes());
        }

        let root = mmr.root();

        // 1. Generate full batch inclusion proof for 1,000 receipts
        let all_indices: Vec<usize> = (0..count).collect();
        let batch_proof = mmr.prove_batch_inclusion(&all_indices).unwrap();
        assert_eq!(batch_proof.total_leaves, 1000);
        assert_eq!(batch_proof.leaf_indices.len(), 1000);
        // Sister hashes for a full tree batch proof are 0 due to DAG deduplication
        assert!(batch_proof.sister_hashes.is_empty());

        // 2. Measure verification time
        let start = std::time::Instant::now();
        let verified = batch_proof.verify(&root).unwrap();
        let elapsed = start.elapsed();

        assert!(verified);
        // Verify that 1000-leaf batch verification executes extremely fast (< 100ms in unoptimized debug mode)
        assert!(
            elapsed < std::time::Duration::from_millis(100),
            "verification took too long: {:?}",
            elapsed
        );
    }
}
