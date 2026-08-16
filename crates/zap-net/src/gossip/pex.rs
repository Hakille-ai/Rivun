//! Peer Exchange (PEX) discovery messages and XOR distance metric.

use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DiscoveredPeerEntry {
    pub node_id: Uuid,
    pub public_key: [u8; 32],
    pub socket_addr: SocketAddr,
    pub transport_key_epoch: u64,
    pub capabilities_digest: [u8; 32],
    pub last_seen_micros: u64,
    #[serde(with = "crate::serde_helpers::signature_bytes")]
    pub signature: [u8; 64],
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PeerExchangeRequest {
    pub requester: Uuid,
    pub max_peers_requested: u16,
    pub known_peer_ids: Vec<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PeerExchangeResponse {
    pub responder: Uuid,
    pub peers: Vec<DiscoveredPeerEntry>,
}

#[must_use]
pub fn xor_distance(a: &Uuid, b: &Uuid) -> [u8; 16] {
    let a_bytes = a.as_bytes();
    let b_bytes = b.as_bytes();
    let mut dist = [0_u8; 16];
    for i in 0..16 {
        dist[i] = a_bytes[i] ^ b_bytes[i];
    }
    dist
}
