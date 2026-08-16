//! BFT Swarm Vote data structures and signing.

use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const VOTE_DOMAIN: &[u8] = b"ZAP-SWARM-VOTE-v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VoteKind {
    Prevote = 1,
    Precommit = 2,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SwarmVote {
    pub epoch: u64,
    pub view: u64,
    pub round: u64,
    pub vote_kind: VoteKind,
    pub proposal_digest: [u8; 32],
    pub voter_node: Uuid,
    pub timestamp_micros: u64,
    #[serde(with = "crate::serde_helpers::signature_bytes")]
    pub signature: [u8; 64],
}

impl SwarmVote {
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub fn new_signed(
        epoch: u64,
        view: u64,
        round: u64,
        vote_kind: VoteKind,
        proposal_digest: [u8; 32],
        voter_node: Uuid,
        timestamp_micros: u64,
        signing_key: &SigningKey,
    ) -> Self {
        let digest = Self::compute_digest(
            epoch,
            view,
            round,
            vote_kind,
            &proposal_digest,
            &voter_node,
            timestamp_micros,
        );
        let signature = signing_key.sign(&digest).to_bytes();
        Self {
            epoch,
            view,
            round,
            vote_kind,
            proposal_digest,
            voter_node,
            timestamp_micros,
            signature,
        }
    }

    #[must_use]
    pub fn compute_digest(
        epoch: u64,
        view: u64,
        round: u64,
        vote_kind: VoteKind,
        proposal_digest: &[u8; 32],
        voter_node: &Uuid,
        timestamp_micros: u64,
    ) -> [u8; 32] {
        let mut hasher = blake3::Hasher::new_derive_key("ZAP-SWARM-VOTE-v1");
        hasher.update(&epoch.to_be_bytes());
        hasher.update(&view.to_be_bytes());
        hasher.update(&round.to_be_bytes());
        hasher.update(&[vote_kind as u8]);
        hasher.update(proposal_digest);
        hasher.update(voter_node.as_bytes());
        hasher.update(&timestamp_micros.to_be_bytes());
        *hasher.finalize().as_bytes()
    }

    #[must_use]
    pub fn verify_signature(&self, verifying_key: &VerifyingKey) -> bool {
        let digest = Self::compute_digest(
            self.epoch,
            self.view,
            self.round,
            self.vote_kind,
            &self.proposal_digest,
            &self.voter_node,
            self.timestamp_micros,
        );
        let sig = Signature::from_bytes(&self.signature);
        verifying_key.verify(&digest, &sig).is_ok()
    }
}
