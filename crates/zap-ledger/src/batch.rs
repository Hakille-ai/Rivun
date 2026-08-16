//! Cryptographic batch sealing and swarm quorum multi-signatures for ZAP receipts.
//!
//! Binds sequence ranges, Merkle Mountain Range roots, state transitions, and
//! threshold multi-signatures into durable verifiable batch seals.

use base64::{Engine as _, engine::general_purpose::STANDARD_NO_PAD};
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use uuid::Uuid;
use zap_crypto::{Keypair, PoaValidatorSet, node_id_from_public_key};

use crate::{
    MerkleMountainRange, SignedActionReceipt, ZapLedgerError, decode_fixed,
    validate_artifact_hash, HASH_PREFIX,
};

pub const RECEIPT_BATCH_SEAL_SCHEMA_VERSION: u8 = 1;
pub const SIGNED_RECEIPT_BATCH_SCHEMA_VERSION: u8 = 1;
pub const BATCH_SEAL_ATTESTATION_REQUEST_SCHEMA_VERSION: u8 = 1;
pub const BATCH_SEAL_ATTESTATION_RESPONSE_SCHEMA_VERSION: u8 = 1;
pub const BATCH_SEAL_SIGNATURE_DOMAIN: &[u8] = b"ZAP-RECEIPT-BATCH-SEAL-v1";
pub const BATCH_SEAL_EXTENSION: &str = "zjseal.json";

const PUBLIC_KEY_LEN: usize = 32;
const SIGNATURE_LEN: usize = 64;

/// Represents a single validator's threshold attestation on a batch seal.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct BatchValidatorSignature {
    pub validator_node: Uuid,
    pub validator_public_key: String, // Base64 unpadded
    pub signature: String,            // Base64 unpadded Ed25519
}

/// The immutable cryptographic seal for a receipt batch.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReceiptBatchSeal {
    pub schema_version: u8,
    pub batch_id: Uuid,
    pub node_id: Uuid,
    pub segment_sequence: u64,
    pub start_sequence: u64,
    pub end_sequence: u64,
    pub receipt_count: u64,
    pub first_processed_at_micros: u64,
    pub last_processed_at_micros: u64,
    pub mmr_root: String,           // "blake3:<64-hex>"
    pub initial_state_hash: String, // "blake3:<64-hex>"
    pub final_state_hash: String,   // "blake3:<64-hex>"
    pub total_fuel_consumed: u64,
    pub quorum_threshold: u16,
    pub validator_signatures: Vec<BatchValidatorSignature>,
}

#[derive(Serialize)]
struct BatchSealSigningPayload<'a> {
    pub schema_version: u8,
    pub batch_id: Uuid,
    pub node_id: Uuid,
    pub segment_sequence: u64,
    pub start_sequence: u64,
    pub end_sequence: u64,
    pub receipt_count: u64,
    pub first_processed_at_micros: u64,
    pub last_processed_at_micros: u64,
    pub mmr_root: &'a str,
    pub initial_state_hash: &'a str,
    pub final_state_hash: &'a str,
    pub total_fuel_consumed: u64,
    pub quorum_threshold: u16,
}

impl ReceiptBatchSeal {
    /// Formats canonical signing transcript for threshold validator multi-signatures.
    pub fn signing_message(&self) -> Result<Vec<u8>, ZapLedgerError> {
        let payload = BatchSealSigningPayload {
            schema_version: self.schema_version,
            batch_id: self.batch_id,
            node_id: self.node_id,
            segment_sequence: self.segment_sequence,
            start_sequence: self.start_sequence,
            end_sequence: self.end_sequence,
            receipt_count: self.receipt_count,
            first_processed_at_micros: self.first_processed_at_micros,
            last_processed_at_micros: self.last_processed_at_micros,
            mmr_root: &self.mmr_root,
            initial_state_hash: &self.initial_state_hash,
            final_state_hash: &self.final_state_hash,
            total_fuel_consumed: self.total_fuel_consumed,
            quorum_threshold: self.quorum_threshold,
        };
        let encoded = serde_json::to_vec(&payload)?;
        let mut msg = Vec::with_capacity(BATCH_SEAL_SIGNATURE_DOMAIN.len() + 1 + encoded.len());
        msg.extend_from_slice(BATCH_SEAL_SIGNATURE_DOMAIN);
        msg.push(0);
        msg.extend_from_slice(&encoded);
        Ok(msg)
    }

    /// Performs static structural and invariant validations on the batch seal.
    pub fn validate_static(&self) -> Result<(), ZapLedgerError> {
        if self.schema_version != RECEIPT_BATCH_SEAL_SCHEMA_VERSION {
            return Err(ZapLedgerError::UnsupportedSchemaVersion(
                self.schema_version,
            ));
        }
        if self.receipt_count == 0 {
            return Err(ZapLedgerError::EmptyReceiptSegment);
        }
        if self.end_sequence < self.start_sequence {
            return Err(ZapLedgerError::InvalidReceiptField {
                field: "end_sequence",
                reason: "must be greater than or equal to start_sequence",
            });
        }
        let expected_count = self.end_sequence - self.start_sequence + 1;
        if self.receipt_count != expected_count {
            return Err(ZapLedgerError::InvalidReceiptField {
                field: "receipt_count",
                reason: "does not match sequence range (end - start + 1)",
            });
        }
        if self.last_processed_at_micros < self.first_processed_at_micros {
            return Err(ZapLedgerError::ReceiptSegmentOutOfOrder {
                previous: self.first_processed_at_micros,
                current: self.last_processed_at_micros,
            });
        }
        validate_artifact_hash("mmr_root", &self.mmr_root)?;
        validate_artifact_hash("initial_state_hash", &self.initial_state_hash)?;
        validate_artifact_hash("final_state_hash", &self.final_state_hash)?;
        if self.quorum_threshold == 0 {
            return Err(ZapLedgerError::InvalidReceiptField {
                field: "quorum_threshold",
                reason: "must be greater than zero",
            });
        }
        if self.validator_signatures.len() < self.quorum_threshold as usize {
            return Err(ZapLedgerError::InvalidReceiptField {
                field: "validator_signatures",
                reason: "signature count is below required quorum threshold",
            });
        }
        Ok(())
    }

    /// Generates a validator signature over this batch seal.
    pub fn sign_with_validator(
        &self,
        keypair: &Keypair,
    ) -> Result<BatchValidatorSignature, ZapLedgerError> {
        let signing_msg = self.signing_message()?;
        let sig = keypair.sign_domain_message(BATCH_SEAL_SIGNATURE_DOMAIN, &signing_msg);
        let pub_key = STANDARD_NO_PAD.encode(keypair.verifying_key().to_bytes());
        Ok(BatchValidatorSignature {
            validator_node: keypair.node_id(),
            validator_public_key: pub_key,
            signature: STANDARD_NO_PAD.encode(sig),
        })
    }

    /// Verifies that the batch seal meets the Swarm Quorum multi-signature threshold against a `PoaValidatorSet`.
    pub fn verify_quorum(&self, validator_set: &PoaValidatorSet) -> Result<bool, ZapLedgerError> {
        self.validate_static()?;
        validator_set
            .validate_static()
            .map_err(ZapLedgerError::from)?;

        let required_threshold = self.quorum_threshold.max(validator_set.required_threshold);
        if self.validator_signatures.len() < required_threshold as usize {
            return Ok(false);
        }

        let signing_message = self.signing_message()?;
        let mut seen_validators = HashSet::with_capacity(self.validator_signatures.len());
        let mut valid_signatures = 0_u16;

        for val_sig in &self.validator_signatures {
            // Check duplicate validator signatures
            if !seen_validators.insert(val_sig.validator_node) {
                return Err(ZapLedgerError::InvalidReceiptField {
                    field: "validator_signatures",
                    reason: "duplicate validator signature detected",
                });
            }

            // Verify validator is an authorized member of the active validator set
            let descriptor = validator_set
                .validators
                .iter()
                .find(|v| v.node_id == val_sig.validator_node)
                .ok_or(ZapLedgerError::InvalidReceiptField {
                    field: "validator_signatures",
                    reason: "validator not present in active PoaValidatorSet",
                })?;

            if descriptor.public_key != val_sig.validator_public_key {
                return Err(ZapLedgerError::InvalidReceiptField {
                    field: "validator_public_key",
                    reason: "validator public key mismatch with set descriptor",
                });
            }

            let public_key_bytes = decode_fixed::<PUBLIC_KEY_LEN>(
                &val_sig.validator_public_key,
                "validator_public_key",
            )?;
            let derived_node = node_id_from_public_key(&public_key_bytes);
            if derived_node != val_sig.validator_node {
                return Err(ZapLedgerError::SignerNodeMismatch {
                    declared: val_sig.validator_node,
                    derived: derived_node,
                });
            }

            let verifying_key = VerifyingKey::from_bytes(&public_key_bytes)?;
            let sig_bytes = decode_fixed::<SIGNATURE_LEN>(&val_sig.signature, "signature")?;
            let signature = Signature::from_bytes(&sig_bytes);

            verifying_key
                .verify(&signing_message, &signature)
                .map_err(|_| ZapLedgerError::InvalidSignature)?;

            valid_signatures = valid_signatures.saturating_add(1);
        }

        Ok(valid_signatures >= required_threshold)
    }

    /// Creates an attestation request to send to a swarm validator.
    pub fn create_attestation_request(
        &self,
        requester_node: Uuid,
    ) -> BatchSealAttestationRequest {
        BatchSealAttestationRequest {
            schema_version: BATCH_SEAL_ATTESTATION_REQUEST_SCHEMA_VERSION,
            requester_node,
            seal: self.clone(),
        }
    }
}

/// Signed batch container holding the seal, the receipts, and the node's signature.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct SignedReceiptBatch {
    pub schema_version: u8,
    pub seal: ReceiptBatchSeal,
    pub receipts: Vec<SignedActionReceipt>,
    pub node_signature: String,
}

impl SignedReceiptBatch {
    /// Creates and signs a receipt batch container.
    pub fn sign(
        seal: ReceiptBatchSeal,
        receipts: Vec<SignedActionReceipt>,
        keypair: &Keypair,
    ) -> Result<Self, ZapLedgerError> {
        if keypair.node_id() != seal.node_id {
            return Err(ZapLedgerError::ReceiptNodeMismatch {
                receipt_node_id: seal.node_id,
                signer_node_id: keypair.node_id(),
            });
        }
        seal.validate_static()?;
        if receipts.len() as u64 != seal.receipt_count {
            return Err(ZapLedgerError::InvalidReceiptField {
                field: "receipts",
                reason: "receipts count does not match seal receipt_count",
            });
        }

        let signing_msg = seal.signing_message()?;
        let sig = keypair.sign_domain_message(BATCH_SEAL_SIGNATURE_DOMAIN, &signing_msg);

        Ok(Self {
            schema_version: SIGNED_RECEIPT_BATCH_SCHEMA_VERSION,
            seal,
            receipts,
            node_signature: STANDARD_NO_PAD.encode(sig),
        })
    }

    /// Verifies the signed batch container, receipts, and optional swarm quorum.
    pub fn verify(&self, validator_set: Option<&PoaValidatorSet>) -> Result<bool, ZapLedgerError> {
        if self.schema_version != SIGNED_RECEIPT_BATCH_SCHEMA_VERSION {
            return Err(ZapLedgerError::UnsupportedSchemaVersion(
                self.schema_version,
            ));
        }
        self.seal.validate_static()?;
        if self.receipts.len() as u64 != self.seal.receipt_count {
            return Ok(false);
        }

        // Verify MMR root consistency from receipts
        let mut mmr = MerkleMountainRange::new();
        for r in &self.receipts {
            r.verify()?;
            let canon = r.signing_message()?;
            mmr.append_bytes(&canon);
        }
        let computed_mmr_root = format!("{HASH_PREFIX}{}", mmr.root_hex());
        if computed_mmr_root != self.seal.mmr_root {
            return Ok(false);
        }

        // Verify quorum if validator set is provided
        if let Some(vset) = validator_set
            && !self.seal.verify_quorum(vset)?
        {
            return Ok(false);
        }

        Ok(true)
    }
}

/// Request sent to Swarm Quorum validators to sign a batch seal.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct BatchSealAttestationRequest {
    pub schema_version: u8,
    pub requester_node: Uuid,
    pub seal: ReceiptBatchSeal,
}

impl BatchSealAttestationRequest {
    pub fn validate_static(&self) -> Result<(), ZapLedgerError> {
        if self.schema_version != BATCH_SEAL_ATTESTATION_REQUEST_SCHEMA_VERSION {
            return Err(ZapLedgerError::UnsupportedSchemaVersion(
                self.schema_version,
            ));
        }
        self.seal.validate_static()
    }
}

/// Response containing a validator's signature for a batch seal.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct BatchSealAttestationResponse {
    pub schema_version: u8,
    pub validator_node: Uuid,
    pub batch_id: Uuid,
    pub signature: BatchValidatorSignature,
}

impl BatchSealAttestationResponse {
    pub fn create(
        request: &BatchSealAttestationRequest,
        validator_keypair: &Keypair,
    ) -> Result<Self, ZapLedgerError> {
        request.validate_static()?;
        let sig = request.seal.sign_with_validator(validator_keypair)?;
        Ok(Self {
            schema_version: BATCH_SEAL_ATTESTATION_RESPONSE_SCHEMA_VERSION,
            validator_node: validator_keypair.node_id(),
            batch_id: request.seal.batch_id,
            signature: sig,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ActionReceipt;
    use zap_core::ZapFlags;
    use zap_crypto::{PoaValidatorDescriptor, POA_VALIDATOR_SET_SCHEMA_VERSION};

    fn make_test_receipt(node: &Keypair, action: &str, seq: u64) -> SignedActionReceipt {
        let receipt = ActionReceipt {
            schema_version: 1,
            node_id: node.node_id(),
            source_node: node.node_id(),
            target_node: Uuid::new_v4(),
            kind: "execution".to_string(),
            subject: "test".to_string(),
            action: action.to_string(),
            frame_hash: format!("{HASH_PREFIX}{}", hex::encode([0x11; 32])),
            payload_hash: format!("{HASH_PREFIX}{}", hex::encode([0x22; 32])),
            output_hash: None,
            frame_timestamp_micros: 1_000_000 + seq * 1000,
            processed_at_micros: 1_000_000 + seq * 1000,
            flags: ZapFlags::empty().bits(),
            consensus_required: false,
            poa: None,
            pact: None,
        };
        SignedActionReceipt::sign(node, receipt).unwrap()
    }

    #[test]
    fn batch_seal_quorum_verification() {
        let node = Keypair::generate();
        let val1 = Keypair::generate();
        let val2 = Keypair::generate();
        let val3 = Keypair::generate();

        let vset = PoaValidatorSet {
            schema_version: POA_VALIDATOR_SET_SCHEMA_VERSION,
            set_id: Uuid::new_v4(),
            epoch: 1,
            required_threshold: 2,
            validators: vec![
                PoaValidatorDescriptor {
                    node_id: val1.node_id(),
                    public_key: STANDARD_NO_PAD.encode(val1.verifying_key().to_bytes()),
                },
                PoaValidatorDescriptor {
                    node_id: val2.node_id(),
                    public_key: STANDARD_NO_PAD.encode(val2.verifying_key().to_bytes()),
                },
                PoaValidatorDescriptor {
                    node_id: val3.node_id(),
                    public_key: STANDARD_NO_PAD.encode(val3.verifying_key().to_bytes()),
                },
            ],
            valid_from_micros: None,
            expires_at_micros: None,
            labels: vec![],
        };

        let mut seal = ReceiptBatchSeal {
            schema_version: RECEIPT_BATCH_SEAL_SCHEMA_VERSION,
            batch_id: Uuid::new_v4(),
            node_id: node.node_id(),
            segment_sequence: 0,
            start_sequence: 0,
            end_sequence: 9,
            receipt_count: 10,
            first_processed_at_micros: 1_000_000,
            last_processed_at_micros: 2_000_000,
            mmr_root: format!("{HASH_PREFIX}{}", hex::encode([0xAA; 32])),
            initial_state_hash: format!("{HASH_PREFIX}{}", hex::encode([0xBB; 32])),
            final_state_hash: format!("{HASH_PREFIX}{}", hex::encode([0xCC; 32])),
            total_fuel_consumed: 50_000,
            quorum_threshold: 2,
            validator_signatures: Vec::new(),
        };

        // Sign with val1 and val2
        let sig1 = seal.sign_with_validator(&val1).unwrap();
        let sig2 = seal.sign_with_validator(&val2).unwrap();
        seal.validator_signatures = vec![sig1, sig2];

        // Quorum verification succeeds
        assert!(seal.verify_quorum(&vset).unwrap());

        // Duplicate signature is rejected
        let mut dup_seal = seal.clone();
        dup_seal.validator_signatures = vec![
            seal.validator_signatures[0].clone(),
            seal.validator_signatures[0].clone(),
        ];
        assert!(dup_seal.verify_quorum(&vset).is_err());

        // Non-member signature is rejected
        let non_member = Keypair::generate();
        let non_member_sig = seal.sign_with_validator(&non_member).unwrap();
        let mut non_member_seal = seal.clone();
        non_member_seal.validator_signatures = vec![seal.validator_signatures[0].clone(), non_member_sig];
        assert!(non_member_seal.verify_quorum(&vset).is_err());
    }

    #[test]
    fn signed_receipt_batch_lifecycle() {
        let node = Keypair::generate();
        let val1 = Keypair::generate();

        let mut receipts = Vec::new();
        let mut mmr = MerkleMountainRange::new();
        for i in 0..5 {
            let r = make_test_receipt(&node, "matmul", i);
            mmr.append_bytes(&r.signing_message().unwrap());
            receipts.push(r);
        }

        let mut seal = ReceiptBatchSeal {
            schema_version: RECEIPT_BATCH_SEAL_SCHEMA_VERSION,
            batch_id: Uuid::new_v4(),
            node_id: node.node_id(),
            segment_sequence: 0,
            start_sequence: 0,
            end_sequence: 4,
            receipt_count: 5,
            first_processed_at_micros: receipts[0].receipt.processed_at_micros,
            last_processed_at_micros: receipts[4].receipt.processed_at_micros,
            mmr_root: format!("{HASH_PREFIX}{}", mmr.root_hex()),
            initial_state_hash: format!("{HASH_PREFIX}{}", hex::encode([0x10; 32])),
            final_state_hash: format!("{HASH_PREFIX}{}", hex::encode([0x20; 32])),
            total_fuel_consumed: 1200,
            quorum_threshold: 1,
            validator_signatures: Vec::new(),
        };
        let val_sig = seal.sign_with_validator(&val1).unwrap();
        seal.validator_signatures.push(val_sig);

        let signed_batch = SignedReceiptBatch::sign(seal, receipts, &node).unwrap();
        assert!(signed_batch.verify(None).unwrap());
    }
}
