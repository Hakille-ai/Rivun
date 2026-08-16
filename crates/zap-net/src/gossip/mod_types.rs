//! Gossip protocol error definitions.

use thiserror::Error;
use uuid::Uuid;
use super::envelope::GossipMessageId;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum GossipError {
    #[error("peer {0} not found in gossip mesh")]
    PeerNotFound(Uuid),
    #[error("invalid gossip magic")]
    InvalidMagic,
    #[error("unsupported gossip version {0}")]
    UnsupportedVersion(u8),
    #[error("gossip hop limit exceeded: current {current}, max {max}")]
    HopLimitExceeded { current: u8, max: u8 },
    #[error("duplicate gossip message {0:?}")]
    DuplicateMessage(GossipMessageId),
    #[error("invalid gossip signature from {0}")]
    InvalidSignature(Uuid),
    #[error("quorum threshold not reached: got {received}/{required}")]
    QuorumNotReached { received: usize, required: usize },
    #[error("proposal {0} already expired or finalized")]
    ProposalClosed(Uuid),
    #[error("vector clock causality conflict for key {0}")]
    CausalityConflict(String),
    #[error("network partition detected: {unreachable_count}/{total_nodes} nodes unreachable")]
    NetworkPartition {
        unreachable_count: usize,
        total_nodes: usize,
    },
    #[error("channel error: {0}")]
    Channel(String),
}
