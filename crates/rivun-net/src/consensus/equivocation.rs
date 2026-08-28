//! Equivocation detection and slashing proof generation.

use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::vote::{SwarmVote, VoteKind};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EquivocationProof {
    pub offender_node: Uuid,
    pub epoch: u64,
    pub view: u64,
    pub round: u64,
    pub vote_kind: VoteKind,
    pub digest_a: [u8; 32],
    #[serde(with = "crate::serde_helpers::signature_bytes")]
    pub signature_a: [u8; 64],
    pub timestamp_a_micros: u64,
    pub digest_b: [u8; 32],
    #[serde(with = "crate::serde_helpers::signature_bytes")]
    pub signature_b: [u8; 64],
    pub timestamp_b_micros: u64,
}

impl EquivocationProof {
    #[must_use]
    pub fn verify(&self, verifying_key: &VerifyingKey) -> bool {
        if self.digest_a == self.digest_b {
            return false; // Not conflicting
        }

        let digest1 = SwarmVote::compute_digest(
            self.epoch,
            self.view,
            self.round,
            self.vote_kind,
            &self.digest_a,
            &self.offender_node,
            self.timestamp_a_micros,
        );
        let sig1 = Signature::from_bytes(&self.signature_a);
        if verifying_key.verify(&digest1, &sig1).is_err() {
            return false;
        }

        let digest2 = SwarmVote::compute_digest(
            self.epoch,
            self.view,
            self.round,
            self.vote_kind,
            &self.digest_b,
            &self.offender_node,
            self.timestamp_b_micros,
        );
        let sig2 = Signature::from_bytes(&self.signature_b);
        if verifying_key.verify(&digest2, &sig2).is_err() {
            return false;
        }

        true
    }

    #[must_use]
    pub fn from_votes(vote_a: &SwarmVote, vote_b: &SwarmVote) -> Option<Self> {
        if vote_a.voter_node == vote_b.voter_node
            && vote_a.epoch == vote_b.epoch
            && vote_a.view == vote_b.view
            && vote_a.round == vote_b.round
            && vote_a.vote_kind == vote_b.vote_kind
            && vote_a.proposal_digest != vote_b.proposal_digest
        {
            Some(Self {
                offender_node: vote_a.voter_node,
                epoch: vote_a.epoch,
                view: vote_a.view,
                round: vote_a.round,
                vote_kind: vote_a.vote_kind,
                digest_a: vote_a.proposal_digest,
                signature_a: vote_a.signature,
                timestamp_a_micros: vote_a.timestamp_micros,
                digest_b: vote_b.proposal_digest,
                signature_b: vote_b.signature,
                timestamp_b_micros: vote_b.timestamp_micros,
            })
        } else {
            None
        }
    }
}
