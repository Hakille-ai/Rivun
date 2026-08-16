//! 2-Phase BFT Swarm Consensus Engine with Dynamic Threshold Signatures.

pub mod batch_verify;
pub mod certificate;
pub mod engine;
pub mod equivocation;
pub mod mod_types;
pub mod proposal;
pub mod validator_set;
pub mod vote;

pub use batch_verify::verify_threshold_signatures;
pub use certificate::{CONSENSUS_TRAILER_MAGIC, CONSENSUS_TRAILER_VERSION, SwarmCommitCertificate};
pub use engine::{BftConsensusEngine, SwarmConsensusEngine};
pub use equivocation::EquivocationProof;
pub use mod_types::ConsensusError;
pub use proposal::{PROPOSAL_DOMAIN, SwarmProposal};
pub use validator_set::{ValidatorEntry, ValidatorSet};
pub use vote::{SwarmVote, VOTE_DOMAIN, VoteKind};

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::SigningKey;
    use rand_core::OsRng;
    use uuid::Uuid;

    #[test]
    fn test_proposal_and_vote_signing() {
        let key = SigningKey::generate(&mut OsRng);
        let node_id = Uuid::new_v4();
        let payload = [42_u8; 32];
        let root = [1_u8; 32];

        let proposal =
            SwarmProposal::new_signed(1, 0, 0, 1, node_id, payload, root, None, 1_000_000, &key);
        assert!(proposal.verify_signature(&key.verifying_key()));

        let vote = SwarmVote::new_signed(
            1,
            0,
            0,
            VoteKind::Prevote,
            payload,
            node_id,
            1_000_000,
            &key,
        );
        assert!(vote.verify_signature(&key.verifying_key()));
    }

    #[test]
    fn test_certificate_wire_trailer_roundtrip() {
        let cert = SwarmCommitCertificate {
            epoch: 1,
            view: 0,
            round: 0,
            block_height: 10,
            proposal_digest: [7_u8; 32],
            threshold: 3,
            total_validators: 4,
            signer_bitmask: vec![0b00000111],
            signatures: vec![[1; 64], [2; 64], [3; 64]],
        };

        let encoded = cert.encode_trailer();
        let decoded = SwarmCommitCertificate::decode_trailer(&encoded).expect("decode failed");
        assert_eq!(cert, decoded);
    }
}
