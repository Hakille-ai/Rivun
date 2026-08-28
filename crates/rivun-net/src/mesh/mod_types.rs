//! Adaptive mesh and failure detector error definitions.

use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum MeshError {
    #[error("network partition detected: in minority partition")]
    MinorityPartition,
    #[error("node {0} is dead or unreachable")]
    PeerUnreachable(Uuid),
    #[error("no healthy relay route available to destination {0}")]
    NoRelayRoute(Uuid),
    #[error("relay hop limit exceeded: max {max}")]
    RelayHopLimitExceeded { max: u8 },
    #[error("untrusted relay forwarder {0}")]
    UntrustedRelay(Uuid),
    #[error("invalid relay magic")]
    InvalidRelayMagic,
    #[error("unsupported relay version {0}")]
    UnsupportedRelayVersion(u8),
    #[error("relay decode error: {0}")]
    RelayDecodeError(String),
}
