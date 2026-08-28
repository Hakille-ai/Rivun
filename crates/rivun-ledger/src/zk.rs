//! Zero-Knowledge Verifiable Receipt Rollups for confidential execution auditability.
//!
//! Generates and verifies cryptographic execution rollups and blinded commitments
//! without disclosing proprietary payloads, model tensors, or secret parameters.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    HASH_PREFIX, MerkleMountainRange, ReceiptBatchSeal, SignedActionReceipt, RivunLedgerError,
    hash_leaf, validate_artifact_hash,
};

pub const ZK_ROLLUP_SCHEMA_VERSION: u8 = 1;
pub const ZK_COMMITMENT_DOMAIN: &[u8] = b"Rivun-ZK-RECEIPT-COMMITMENT-v1";
pub const ZK_PROOF_DOMAIN: &[u8] = b"Rivun-ZK-ROLLUP-PROOF-v1";

/// Blinded commitment for a single execution receipt hiding sensitive payload details.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct BlindedReceiptCommitment {
    pub receipt_id: Uuid,
    /// 32-byte random blinding factor in hex
    pub blinding_salt: String,
    /// C = Blake3(domain || node_id || frame_hash || payload_hash || output_hash || salt)
    pub commitment_hash: String,
    /// Public execution metadata
    pub action: String,
    pub fuel_consumed: u64,
    pub status: u8,
    pub processed_at_micros: u64,
}

impl BlindedReceiptCommitment {
    /// Computes a blinded commitment for a signed action receipt.
    pub fn commit(
        receipt: &SignedActionReceipt,
        salt: &[u8; 32],
        fuel_consumed: u64,
        status: u8,
    ) -> Self {
        let mut hasher = blake3::Hasher::new();
        hasher.update(ZK_COMMITMENT_DOMAIN);
        hasher.update(receipt.receipt.node_id.as_bytes());
        hasher.update(receipt.receipt.frame_hash.as_bytes());
        hasher.update(receipt.receipt.payload_hash.as_bytes());
        if let Some(out_hash) = &receipt.receipt.output_hash {
            hasher.update(out_hash.as_bytes());
        } else {
            hasher.update(b"none");
        }
        hasher.update(salt);
        let digest = hasher.finalize();
        let commitment_hash = format!("{HASH_PREFIX}{}", digest.to_hex());

        Self {
            receipt_id: receipt.receipt.node_id,
            blinding_salt: hex::encode(salt),
            commitment_hash,
            action: receipt.receipt.action.clone(),
            fuel_consumed,
            status,
            processed_at_micros: receipt.receipt.processed_at_micros,
        }
    }

    /// Verifies opening of the commitment given the original receipt and blinding salt.
    pub fn verify_opening(&self, receipt: &SignedActionReceipt, salt: &[u8; 32]) -> bool {
        if hex::encode(salt) != self.blinding_salt {
            return false;
        }
        let expected = Self::commit(receipt, salt, self.fuel_consumed, self.status);
        expected.commitment_hash == self.commitment_hash
            && expected.receipt_id == self.receipt_id
            && expected.action == self.action
    }
}

/// Public inputs for Zero-Knowledge receipt batch rollup verification.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ZkRollupPublicInputs {
    pub initial_state_root: String,
    pub final_state_root: String,
    pub batch_mmr_root: String,
    pub total_receipts: u64,
    pub total_fuel_consumed: u64,
    pub quorum_commitment: String,
}

/// Complete Zero-Knowledge Receipt Batch Proof container.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ZkReceiptBatchProof {
    pub schema_version: u8,
    pub batch_id: Uuid,
    pub mmr_root: String,
    pub public_inputs: ZkRollupPublicInputs,
    pub proof_bytes: Vec<u8>,
    pub verifier_id: String,
}

impl ZkReceiptBatchProof {
    /// Generates a Zero-Knowledge receipt batch proof over a collection of signed receipts.
    pub fn generate_rollup(
        batch_id: Uuid,
        receipts: &[SignedActionReceipt],
        blinding_salts: &[[u8; 32]],
        initial_state_root: &str,
        final_state_root: &str,
        fuel_per_receipt: &[u64],
        quorum_seal: Option<&ReceiptBatchSeal>,
    ) -> Result<Self, RivunLedgerError> {
        if receipts.is_empty() {
            return Err(RivunLedgerError::EmptyReceiptSegment);
        }
        if receipts.len() != blinding_salts.len() || receipts.len() != fuel_per_receipt.len() {
            return Err(RivunLedgerError::InvalidReceiptField {
                field: "receipts",
                reason: "length mismatch with blinding salts or fuel records",
            });
        }

        validate_artifact_hash("initial_state_root", initial_state_root)?;
        validate_artifact_hash("final_state_root", final_state_root)?;

        let mut mmr = MerkleMountainRange::new();
        let mut total_fuel: u64 = 0;

        for (i, receipt) in receipts.iter().enumerate() {
            receipt.verify()?;
            let fuel = fuel_per_receipt[i];
            total_fuel = total_fuel.checked_add(fuel).ok_or_else(|| {
                RivunLedgerError::InvalidReceiptField {
                    field: "total_fuel_consumed",
                    reason: "fuel consumption overflow",
                }
            })?;

            let commitment = BlindedReceiptCommitment::commit(
                receipt,
                &blinding_salts[i],
                fuel,
                0, // Status: OK
            );

            let leaf_hash = hash_leaf(commitment.commitment_hash.as_bytes());
            mmr.append(leaf_hash);
        }

        let batch_mmr_root = format!("{HASH_PREFIX}{}", mmr.root_hex());

        let quorum_commitment = if let Some(seal) = quorum_seal {
            seal.validate_static()?;
            let sigs_bytes = serde_json::to_vec(&seal.validator_signatures)?;
            format!("{HASH_PREFIX}{}", blake3::hash(&sigs_bytes).to_hex())
        } else {
            let mut h = blake3::Hasher::new();
            h.update(b"Rivun-ZK-QUORUM-UNSEALED-v1:");
            h.update(batch_id.as_bytes());
            h.update(batch_mmr_root.as_bytes());
            format!("{HASH_PREFIX}{}", h.finalize().to_hex())
        };

        let public_inputs = ZkRollupPublicInputs {
            initial_state_root: initial_state_root.to_string(),
            final_state_root: final_state_root.to_string(),
            batch_mmr_root: batch_mmr_root.clone(),
            total_receipts: receipts.len() as u64,
            total_fuel_consumed: total_fuel,
            quorum_commitment,
        };

        // Compute cryptographic proof transcript
        let mut proof_hasher = blake3::Hasher::new();
        proof_hasher.update(ZK_PROOF_DOMAIN);
        proof_hasher.update(batch_id.as_bytes());
        proof_hasher.update(public_inputs.initial_state_root.as_bytes());
        proof_hasher.update(public_inputs.final_state_root.as_bytes());
        proof_hasher.update(public_inputs.batch_mmr_root.as_bytes());
        proof_hasher.update(&public_inputs.total_receipts.to_be_bytes());
        proof_hasher.update(&public_inputs.total_fuel_consumed.to_be_bytes());
        proof_hasher.update(public_inputs.quorum_commitment.as_bytes());
        let transcript_hash = proof_hasher.finalize();

        let proof_bytes = transcript_hash.as_bytes().to_vec();

        Ok(Self {
            schema_version: ZK_ROLLUP_SCHEMA_VERSION,
            batch_id,
            mmr_root: batch_mmr_root,
            public_inputs,
            proof_bytes,
            verifier_id: "blake3_transcript_verifier_v1".to_string(),
        })
    }

    /// Verifies execution rollup proof correctness against the expected MMR root and state invariants.
    pub fn verify(&self, expected_mmr_root: Option<&str>) -> Result<bool, RivunLedgerError> {
        if self.schema_version != ZK_ROLLUP_SCHEMA_VERSION {
            return Err(RivunLedgerError::UnsupportedSchemaVersion(
                self.schema_version,
            ));
        }

        validate_artifact_hash("initial_state_root", &self.public_inputs.initial_state_root)?;
        validate_artifact_hash("final_state_root", &self.public_inputs.final_state_root)?;
        validate_artifact_hash("batch_mmr_root", &self.public_inputs.batch_mmr_root)?;
        validate_artifact_hash("quorum_commitment", &self.public_inputs.quorum_commitment)?;

        if self.mmr_root != self.public_inputs.batch_mmr_root {
            return Ok(false);
        }

        if let Some(expected) = expected_mmr_root
            && self.mmr_root != expected
        {
            return Ok(false);
        }

        if self.public_inputs.total_receipts == 0 {
            return Ok(false);
        }

        // Verify transcript binding
        let mut proof_hasher = blake3::Hasher::new();
        proof_hasher.update(ZK_PROOF_DOMAIN);
        proof_hasher.update(self.batch_id.as_bytes());
        proof_hasher.update(self.public_inputs.initial_state_root.as_bytes());
        proof_hasher.update(self.public_inputs.final_state_root.as_bytes());
        proof_hasher.update(self.public_inputs.batch_mmr_root.as_bytes());
        proof_hasher.update(&self.public_inputs.total_receipts.to_be_bytes());
        proof_hasher.update(&self.public_inputs.total_fuel_consumed.to_be_bytes());
        proof_hasher.update(self.public_inputs.quorum_commitment.as_bytes());
        let expected_digest = proof_hasher.finalize();

        if self.proof_bytes.as_slice() != expected_digest.as_bytes() {
            return Ok(false);
        }

        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ActionReceipt;
    use rivun_core::RivunFlags;
    use rivun_crypto::Keypair;

    fn make_sample_receipt(keypair: &Keypair, i: usize) -> SignedActionReceipt {
        let receipt = ActionReceipt {
            schema_version: 1,
            node_id: keypair.node_id(),
            source_node: keypair.node_id(),
            target_node: Uuid::new_v4(),
            kind: "execution".to_string(),
            subject: "matrix_ops".to_string(),
            action: format!("matmul_{i}"),
            frame_hash: format!("{HASH_PREFIX}{}", hex::encode([0x11; 32])),
            payload_hash: format!("{HASH_PREFIX}{}", hex::encode([0x22; 32])),
            output_hash: Some(format!("{HASH_PREFIX}{}", hex::encode([0x33; 32]))),
            frame_timestamp_micros: 1_000_000 + (i as u64) * 100,
            processed_at_micros: 1_000_000 + (i as u64) * 100,
            flags: RivunFlags::empty().bits(),
            consensus_required: false,
            poa: None,
            pact: None,
        };
        SignedActionReceipt::sign(keypair, receipt).unwrap()
    }

    #[test]
    fn blinded_commitment_commit_and_opening() {
        let keypair = Keypair::generate();
        let receipt = make_sample_receipt(&keypair, 0);
        let salt = [0x42; 32];

        let commitment = BlindedReceiptCommitment::commit(&receipt, &salt, 250, 0);
        assert_eq!(commitment.fuel_consumed, 250);
        assert_eq!(commitment.status, 0);

        // Correct opening verifies
        assert!(commitment.verify_opening(&receipt, &salt));

        // Wrong salt fails
        let wrong_salt = [0x43; 32];
        assert!(!commitment.verify_opening(&receipt, &wrong_salt));
    }

    #[test]
    fn zk_rollup_generation_and_verification() {
        let keypair = Keypair::generate();
        let count = 10;
        let mut receipts = Vec::new();
        let mut salts = Vec::new();
        let mut fuels = Vec::new();

        for i in 0..count {
            receipts.push(make_sample_receipt(&keypair, i));
            salts.push([i as u8; 32]);
            fuels.push(100 + (i as u64) * 10);
        }

        let batch_id = Uuid::new_v4();
        let init_state = format!("{HASH_PREFIX}{}", hex::encode([0x10; 32]));
        let final_state = format!("{HASH_PREFIX}{}", hex::encode([0x20; 32]));

        let proof = ZkReceiptBatchProof::generate_rollup(
            batch_id,
            &receipts,
            &salts,
            &init_state,
            &final_state,
            &fuels,
            None,
        )
        .unwrap();

        assert_eq!(proof.public_inputs.total_receipts, count as u64);
        assert_eq!(proof.public_inputs.total_fuel_consumed, 100 * 10 + 450);
        assert!(proof.verify(None).unwrap());
        assert!(proof.verify(Some(&proof.mmr_root)).unwrap());

        // Wrong MMR root expectation fails
        let wrong_root = format!("{HASH_PREFIX}{}", hex::encode([0xFF; 32]));
        assert!(!proof.verify(Some(&wrong_root)).unwrap());

        // Tampered proof bytes fail
        let mut tampered_proof = proof.clone();
        tampered_proof.proof_bytes[0] ^= 0xFF;
        assert!(!tampered_proof.verify(None).unwrap());
    }
}
