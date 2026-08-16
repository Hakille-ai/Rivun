//! Anti-Entropy state digest synchronization and range reconciliation.

use super::envelope::GossipEnvelope;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StateDigest {
    pub topic: String,
    pub origin_node: Uuid,
    pub highest_sequence: u64,
    pub state_merkle_root: [u8; 32],
    pub timestamp_micros: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AntiEntropyDigestRequest {
    pub requester: Uuid,
    pub digests: Vec<StateDigest>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MissingRange {
    pub topic: String,
    pub origin_node: Uuid,
    pub start_seq: u64,
    pub end_seq: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AntiEntropyDigestResponse {
    pub responder: Uuid,
    pub missing_ranges: Vec<MissingRange>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AntiEntropyBatchResponse {
    pub responder: Uuid,
    pub envelopes: Vec<GossipEnvelope>,
}
