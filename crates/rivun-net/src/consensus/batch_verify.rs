//! Batch Ed25519 threshold signature verification.

use ed25519_dalek::{Signature, VerifyingKey};
use uuid::Uuid;

use super::{
    mod_types::ConsensusError,
    vote::{SwarmVote, VoteKind},
};

pub fn verify_threshold_signatures(
    epoch: u64,
    view: u64,
    round: u64,
    proposal_digest: &[u8; 32],
    signers: &[(Uuid, VerifyingKey)],
    signatures: &[[u8; 64]],
) -> Result<(), ConsensusError> {
    if signers.len() != signatures.len() {
        return Err(ConsensusError::SignatureCountMismatch {
            signers_in_mask: signers.len(),
            signatures_provided: signatures.len(),
        });
    }

    let mut messages: Vec<Vec<u8>> = Vec::with_capacity(signers.len());
    let mut dalek_signatures: Vec<Signature> = Vec::with_capacity(signatures.len());
    let mut verifying_keys: Vec<VerifyingKey> = Vec::with_capacity(signers.len());

    for ((node_id, vk), sig_bytes) in signers.iter().zip(signatures.iter()) {
        let msg_digest = SwarmVote::compute_digest(
            epoch,
            view,
            round,
            VoteKind::Precommit,
            proposal_digest,
            node_id,
            0,
        );
        messages.push(msg_digest.to_vec());
        dalek_signatures.push(Signature::from_bytes(sig_bytes));
        verifying_keys.push(*vk);
    }

    let message_refs: Vec<&[u8]> = messages.iter().map(Vec::as_slice).collect();
    ed25519_dalek::verify_batch(&message_refs, &dalek_signatures, &verifying_keys)
        .map_err(|_| ConsensusError::BatchVerificationFailed)
}
