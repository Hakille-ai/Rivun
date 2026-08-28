//! BFT Swarm Proposal data structures and verification.

use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const PROPOSAL_DOMAIN: &[u8] = b"Rivun-SWARM-PROPOSAL-v1";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SwarmProposal {
    pub epoch: u64,
    pub view: u64,
    pub round: u64,
    pub block_height: u64,
    pub proposer_node: Uuid,
    pub payload_digest: [u8; 32],
    pub state_merkle_root: [u8; 32],
    pub valid_round: Option<u64>,
    pub timestamp_micros: u64,
    #[serde(with = "crate::serde_helpers::signature_bytes")]
    pub signature: [u8; 64],
}

impl SwarmProposal {
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub fn new_signed(
        epoch: u64,
        view: u64,
        round: u64,
        block_height: u64,
        proposer_node: Uuid,
        payload_digest: [u8; 32],
        state_merkle_root: [u8; 32],
        valid_round: Option<u64>,
        timestamp_micros: u64,
        signing_key: &SigningKey,
    ) -> Self {
        let digest = Self::compute_digest(
            epoch,
            view,
            round,
            block_height,
            &proposer_node,
            &payload_digest,
            &state_merkle_root,
            valid_round,
            timestamp_micros,
        );
        let signature = signing_key.sign(&digest).to_bytes();
        Self {
            epoch,
            view,
            round,
            block_height,
            proposer_node,
            payload_digest,
            state_merkle_root,
            valid_round,
            timestamp_micros,
            signature,
        }
    }

    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub fn compute_digest(
        epoch: u64,
        view: u64,
        round: u64,
        block_height: u64,
        proposer_node: &Uuid,
        payload_digest: &[u8; 32],
        state_merkle_root: &[u8; 32],
        valid_round: Option<u64>,
        timestamp_micros: u64,
    ) -> [u8; 32] {
        let mut hasher = blake3::Hasher::new_derive_key("Rivun-SWARM-PROPOSAL-v1");
        hasher.update(&epoch.to_be_bytes());
        hasher.update(&view.to_be_bytes());
        hasher.update(&round.to_be_bytes());
        hasher.update(&block_height.to_be_bytes());
        hasher.update(proposer_node.as_bytes());
        hasher.update(payload_digest);
        hasher.update(state_merkle_root);
        hasher.update(&valid_round.unwrap_or(u64::MAX).to_be_bytes());
        hasher.update(&timestamp_micros.to_be_bytes());
        *hasher.finalize().as_bytes()
    }

    #[must_use]
    pub fn verify_signature(&self, verifying_key: &VerifyingKey) -> bool {
        let digest = Self::compute_digest(
            self.epoch,
            self.view,
            self.round,
            self.block_height,
            &self.proposer_node,
            &self.payload_digest,
            &self.state_merkle_root,
            self.valid_round,
            self.timestamp_micros,
        );
        let sig = Signature::from_bytes(&self.signature);
        verifying_key.verify(&digest, &sig).is_ok()
    }
}
