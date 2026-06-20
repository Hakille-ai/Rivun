//! Signed action receipts for ZAP nodes.
//!
//! Receipts are local, durable audit records. They are not financial records.

use base64::{Engine as _, engine::general_purpose::STANDARD_NO_PAD};
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;
use zap_core::{ZapFlags, ZapFrame};
use zap_crypto::{Keypair, node_id_from_public_key};

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

const RECEIPT_SIGNATURE_DOMAIN: &[u8] = b"ZAP-ACTION-RECEIPT-v1";
const RECEIPT_SEGMENT_MANIFEST_SIGNATURE_DOMAIN: &[u8] = b"ZAP-RECEIPT-SEGMENT-MANIFEST-v1";
const HASH_PREFIX: &str = "blake3:";
const PUBLIC_KEY_LEN: usize = 32;
const SIGNATURE_LEN: usize = 64;

#[derive(Debug, Error)]
pub enum ZapLedgerError {
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
    #[error("failed to serialize receipt signing payload: {0}")]
    Json(#[from] serde_json::Error),
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
        for receipt in &self.receipts {
            receipt.verify()?;
            if receipt.receipt.node_id != self.node_id {
                return Err(ZapLedgerError::ReceiptNodeMismatch {
                    receipt_node_id: receipt.receipt.node_id,
                    signer_node_id: self.node_id,
                });
            }
        }
        Ok(())
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
        for entry in &self.entries {
            if entry.node_id != self.node_id {
                return Err(ZapLedgerError::ReceiptSegmentNodeMismatch {
                    segment_node_id: self.node_id,
                    receipt_node_id: entry.node_id,
                });
            }
            validate_artifact_hash("manifest_hash", &entry.manifest_hash)?;
            validate_artifact_hash("segment_hash", &entry.segment_hash)?;
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
        if self.receipt.schema_version != RECEIPT_SCHEMA_VERSION {
            return Err(ZapLedgerError::UnsupportedSchemaVersion(
                self.receipt.schema_version,
            ));
        }
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

fn receipt_hash(receipt: &SignedActionReceipt) -> Result<String> {
    Ok(hash_bytes(&serde_json::to_vec(receipt)?))
}

fn validate_artifact_hash(field: &'static str, value: &str) -> Result<()> {
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

fn decode_fixed<const N: usize>(encoded: &str, kind: &'static str) -> Result<[u8; N]> {
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
}
