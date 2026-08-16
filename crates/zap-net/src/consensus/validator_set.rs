//! Dynamic Validator Set tracking and signer bitmask resolution.

use ed25519_dalek::VerifyingKey;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::mod_types::ConsensusError;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ValidatorEntry {
    pub node_id: Uuid,
    pub public_key: [u8; 32],
    pub weight: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ValidatorSet {
    pub epoch: u64,
    pub validators: Vec<ValidatorEntry>,
    pub threshold: u16,
}

impl ValidatorSet {
    pub fn new(epoch: u64, validators: Vec<ValidatorEntry>) -> Result<Self, ConsensusError> {
        if validators.is_empty() {
            return Err(ConsensusError::EmptyValidatorSet);
        }
        let n = validators.len();
        let threshold = ((n * 2) / 3 + 1) as u16;
        Ok(Self {
            epoch,
            validators,
            threshold,
        })
    }

    #[must_use]
    pub fn proposer_for_round(&self, view: u64, round: u64) -> &ValidatorEntry {
        let idx = ((view + round) as usize) % self.validators.len();
        &self.validators[idx]
    }

    #[must_use]
    pub fn get_validator(&self, node_id: &Uuid) -> Option<&ValidatorEntry> {
        self.validators.iter().find(|v| v.node_id == *node_id)
    }

    pub fn resolve_bitmask_signers(&self, bitmask: &[u8]) -> Result<Vec<(Uuid, VerifyingKey)>, ConsensusError> {
        let mut signers = Vec::new();
        for (i, val) in self.validators.iter().enumerate() {
            let byte_idx = i / 8;
            let bit_idx = i % 8;
            if byte_idx < bitmask.len() && (bitmask[byte_idx] & (1 << bit_idx)) != 0 {
                let vk = VerifyingKey::from_bytes(&val.public_key)
                    .map_err(|_| ConsensusError::InvalidValidatorKey(val.node_id))?;
                signers.push((val.node_id, vk));
            }
        }
        Ok(signers)
    }

    #[must_use]
    pub fn create_bitmask(&self, signer_ids: &[Uuid]) -> Vec<u8> {
        let byte_len = (self.validators.len() + 7) / 8;
        let mut mask = vec![0_u8; byte_len];
        for id in signer_ids {
            if let Some(pos) = self.validators.iter().position(|v| v.node_id == *id) {
                let byte_idx = pos / 8;
                let bit_idx = pos % 8;
                mask[byte_idx] |= 1 << bit_idx;
            }
        }
        mask
    }
}
