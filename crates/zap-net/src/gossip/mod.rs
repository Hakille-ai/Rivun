//! Decentralized Epidemic Gossip Subsystem.
//!
//! Provides message deduplication (LRU + TTL), hop-count damping, Peer Exchange (PEX),
//! anti-entropy state synchronization, vector clocks, and high-level gossip engine dispatcher.

pub mod cache;
pub mod engine;
pub mod envelope;
pub mod error;
pub mod legacy;
pub mod pex;
pub mod sync;
pub mod vector_clock;

pub use cache::GossipDeduplicationCache;
pub use engine::{GossipReceipt, SwarmGossipDispatcher, SwarmGossipEngine};
pub use envelope::{
    DEFAULT_MAX_HOPS, GOSSIP_ENVELOPE_MAGIC, GOSSIP_ENVELOPE_VERSION, GOSSIP_SIGNING_DOMAIN,
    GossipEnvelope, GossipMessageId,
};
pub use error::GossipError;
pub use legacy::{GossipMesh, PeerHealth, QuorumProposal, SwarmPeer};
pub use pex::{DiscoveredPeerEntry, PeerExchangeRequest, PeerExchangeResponse, xor_distance};
pub use sync::{
    AntiEntropyBatchResponse, AntiEntropyDigestRequest, AntiEntropyDigestResponse, MissingRange,
    StateDigest,
};
pub use vector_clock::{Causality, VectorClock};
