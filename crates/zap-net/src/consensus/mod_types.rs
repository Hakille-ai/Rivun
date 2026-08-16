//! Consensus protocol error definitions.

use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ConsensusError {
    #[error("quorum threshold not reached: received {received}, required {required}")]
    QuorumNotReached { received: usize, required: usize },
    #[error("epoch mismatch: cert epoch {cert_epoch}, validator set epoch {set_epoch}")]
    EpochMismatch { cert_epoch: u64, set_epoch: u64 },
    #[error("threshold mismatch: cert threshold {cert_threshold}, required {required_threshold}")]
    ThresholdMismatch { cert_threshold: u16, required_threshold: u16 },
    #[error("insufficient signatures: received {received}, required {required}")]
    InsufficientSignatures { received: usize, required: usize },
    #[error("signature count mismatch: {signers_in_mask} signers in bitmask vs {signatures_provided} signatures")]
    SignatureCountMismatch { signers_in_mask: usize, signatures_provided: usize },
    #[error("empty validator set")]
    EmptyValidatorSet,
    #[error("invalid validator key for {0}")]
    InvalidValidatorKey(Uuid),
    #[error("invalid trailer magic")]
    InvalidTrailerMagic,
    #[error("unsupported trailer version {0}")]
    UnsupportedTrailerVersion(u16),
    #[error("trailer truncated: expected {expected}, got {actual}")]
    TrailerTruncated { expected: usize, actual: usize },
    #[error("invalid signature payload length {0}")]
    InvalidSignaturePayloadLength(usize),
    #[error("batch verification failed")]
    BatchVerificationFailed,
    #[error("proposal {0} already expired or finalized")]
    ProposalClosed(Uuid),
    #[error("equivocation detected from validator {offender} in epoch {epoch}, round {round}")]
    EquivocationDetected { offender: Uuid, epoch: u64, round: u64 },
    #[error("invalid proposal signature from {0}")]
    InvalidProposalSignature(Uuid),
    #[error("invalid vote signature from {0}")]
    InvalidVoteSignature(Uuid),
    #[error("unauthorized proposer {proposer} for epoch {epoch}, round {round}")]
    UnauthorizedProposer { proposer: Uuid, epoch: u64, round: u64 },
}
