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

const RECEIPT_SIGNATURE_DOMAIN: &[u8] = b"ZAP-ACTION-RECEIPT-v1";
const HASH_PREFIX: &str = "blake3:";
const PUBLIC_KEY_LEN: usize = 32;
const SIGNATURE_LEN: usize = 64;

#[derive(Debug, Error)]
pub enum ZapLedgerError {
    #[error("receipt schema version {0} is unsupported")]
    UnsupportedSchemaVersion(u8),
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
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SignedActionReceipt {
    pub receipt: ActionReceipt,
    pub signer_node_id: Uuid,
    pub signer_public_key: String,
    pub signature: String,
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
        let kind = kind.into();
        let subject = subject.into();
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
            output_hash: output.map(hash_bytes),
            frame_timestamp_micros: frame.header.timestamp_micros,
            processed_at_micros,
            flags: frame.header.flags.bits(),
            consensus_required: frame.header.flags.contains(ZapFlags::REQUIRES_CONSENSUS),
            poa: build_poa_receipt(frame, required_poa_threshold),
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

pub fn hash_bytes(bytes: &[u8]) -> String {
    format!("{HASH_PREFIX}{}", blake3::hash(bytes).to_hex())
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
}
