//! Swarm Commit Certificate and compact binary wire trailer ('ZSC1').

use serde::{Deserialize, Serialize};

use super::{
    batch_verify::verify_threshold_signatures, mod_types::ConsensusError,
    validator_set::ValidatorSet,
};

pub const CONSENSUS_TRAILER_MAGIC: [u8; 4] = *b"ZSC1";
pub const CONSENSUS_TRAILER_VERSION: u16 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SwarmCommitCertificate {
    pub epoch: u64,
    pub view: u64,
    pub round: u64,
    pub block_height: u64,
    pub proposal_digest: [u8; 32],
    pub threshold: u16,
    pub total_validators: u16,
    pub signer_bitmask: Vec<u8>,
    #[serde(with = "crate::serde_helpers::signatures_vec")]
    pub signatures: Vec<[u8; 64]>,
}

impl SwarmCommitCertificate {
    pub fn verify_against_set(&self, validator_set: &ValidatorSet) -> Result<(), ConsensusError> {
        if self.epoch != validator_set.epoch {
            return Err(ConsensusError::EpochMismatch {
                cert_epoch: self.epoch,
                set_epoch: validator_set.epoch,
            });
        }
        if self.threshold < validator_set.threshold {
            return Err(ConsensusError::ThresholdMismatch {
                cert_threshold: self.threshold,
                required_threshold: validator_set.threshold,
            });
        }
        let signers = validator_set.resolve_bitmask_signers(&self.signer_bitmask)?;
        if signers.len() < validator_set.threshold as usize {
            return Err(ConsensusError::InsufficientSignatures {
                received: signers.len(),
                required: validator_set.threshold as usize,
            });
        }
        if signers.len() != self.signatures.len() {
            return Err(ConsensusError::SignatureCountMismatch {
                signers_in_mask: signers.len(),
                signatures_provided: self.signatures.len(),
            });
        }

        verify_threshold_signatures(
            self.epoch,
            self.view,
            self.round,
            &self.proposal_digest,
            &signers,
            &self.signatures,
        )
    }

    #[must_use]
    pub fn compute_hash(&self) -> [u8; 32] {
        let encoded = self.encode_trailer();
        *blake3::hash(&encoded).as_bytes()
    }

    #[must_use]
    pub fn encode_trailer(&self) -> Vec<u8> {
        let bitmask_len = self.signer_bitmask.len() as u16;
        let mut out =
            Vec::with_capacity(76 + self.signer_bitmask.len() + self.signatures.len() * 64);
        out.extend_from_slice(&CONSENSUS_TRAILER_MAGIC);
        out.extend_from_slice(&CONSENSUS_TRAILER_VERSION.to_be_bytes());
        out.extend_from_slice(&self.threshold.to_be_bytes());
        out.extend_from_slice(&self.total_validators.to_be_bytes());
        out.extend_from_slice(&self.epoch.to_be_bytes());
        out.extend_from_slice(&self.view.to_be_bytes());
        out.extend_from_slice(&self.round.to_be_bytes());
        out.extend_from_slice(&self.block_height.to_be_bytes());
        out.extend_from_slice(&self.proposal_digest);
        out.extend_from_slice(&bitmask_len.to_be_bytes());
        out.extend_from_slice(&self.signer_bitmask);
        for sig in &self.signatures {
            out.extend_from_slice(sig);
        }
        out
    }

    pub fn decode_trailer(bytes: &[u8]) -> Result<Self, ConsensusError> {
        if bytes.len() < 76 {
            return Err(ConsensusError::TrailerTruncated {
                expected: 76,
                actual: bytes.len(),
            });
        }
        if bytes[0..4] != CONSENSUS_TRAILER_MAGIC {
            return Err(ConsensusError::InvalidTrailerMagic);
        }
        let version = u16::from_be_bytes([bytes[4], bytes[5]]);
        if version != CONSENSUS_TRAILER_VERSION {
            return Err(ConsensusError::UnsupportedTrailerVersion(version));
        }
        let threshold = u16::from_be_bytes([bytes[6], bytes[7]]);
        let total_validators = u16::from_be_bytes([bytes[8], bytes[9]]);
        let epoch = u64::from_be_bytes(bytes[10..18].try_into().unwrap());
        let view = u64::from_be_bytes(bytes[18..26].try_into().unwrap());
        let round = u64::from_be_bytes(bytes[26..34].try_into().unwrap());
        let block_height = u64::from_be_bytes(bytes[34..42].try_into().unwrap());
        let mut proposal_digest = [0_u8; 32];
        proposal_digest.copy_from_slice(&bytes[42..74]);
        let bitmask_len = u16::from_be_bytes([bytes[74], bytes[75]]) as usize;

        let mask_end = 76 + bitmask_len;
        if bytes.len() < mask_end {
            return Err(ConsensusError::TrailerTruncated {
                expected: mask_end,
                actual: bytes.len(),
            });
        }
        let signer_bitmask = bytes[76..mask_end].to_vec();
        let sigs_bytes = &bytes[mask_end..];
        if !sigs_bytes.len().is_multiple_of(64) {
            return Err(ConsensusError::InvalidSignaturePayloadLength(
                sigs_bytes.len(),
            ));
        }
        let sig_count = sigs_bytes.len() / 64;
        let mut signatures = Vec::with_capacity(sig_count);
        for chunk in sigs_bytes.chunks_exact(64) {
            let mut sig = [0_u8; 64];
            sig.copy_from_slice(chunk);
            signatures.push(sig);
        }

        Ok(Self {
            epoch,
            view,
            round,
            block_height,
            proposal_digest,
            threshold,
            total_validators,
            signer_bitmask,
            signatures,
        })
    }
}
