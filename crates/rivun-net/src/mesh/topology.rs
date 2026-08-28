//! Adaptive Mesh Topology Health Engine and Dynamic Relay Route Selector.

use std::{collections::HashMap, net::SocketAddr, sync::Mutex};
use uuid::Uuid;

use super::{
    mod_types::MeshError,
    partition::PartitionStatus,
    phi_detector::{PeerHealthState, PhiAccrualDetector},
};

#[derive(Debug, Clone)]
pub struct PeerMeshInfo {
    pub node_id: Uuid,
    pub addr: SocketAddr,
    pub rtt_micros: u64,
    pub loss_ratio: f64,
    pub queue_pressure: u8,
    pub phi_detector: PhiAccrualDetector,
    pub allow_forward: bool,
}

pub trait MeshTopology: Send + Sync {
    fn record_heartbeat(&self, peer_id: Uuid, rtt_micros: u64, now_micros: u64);
    fn peer_health(&self, peer_id: &Uuid, now_micros: u64) -> PeerHealthState;
    fn partition_status(&self, total_validators: usize, now_micros: u64) -> PartitionStatus;
    fn select_relay_route(&self, target_node: &Uuid) -> Result<Uuid, MeshError>;
}

pub struct SwarmMeshTopology {
    self_node_id: Uuid,
    peers: Mutex<HashMap<Uuid, PeerMeshInfo>>,
    phi_suspect_threshold: f64,
    phi_dead_threshold: f64,
    partition_quorum_ratio: f64,
}

impl SwarmMeshTopology {
    #[must_use]
    pub fn new(
        self_node_id: Uuid,
        phi_suspect_threshold: f64,
        phi_dead_threshold: f64,
        partition_quorum_ratio: f64,
    ) -> Self {
        Self {
            self_node_id,
            peers: Mutex::new(HashMap::new()),
            phi_suspect_threshold,
            phi_dead_threshold,
            partition_quorum_ratio,
        }
    }

    pub fn register_peer(&self, node_id: Uuid, addr: SocketAddr, allow_forward: bool) {
        let mut peers = self.peers.lock().unwrap();
        peers.insert(
            node_id,
            PeerMeshInfo {
                node_id,
                addr,
                rtt_micros: 1000,
                loss_ratio: 0.0,
                queue_pressure: 0,
                phi_detector: PhiAccrualDetector::new(
                    self_phi_suspect_or_default(self.phi_suspect_threshold),
                    self_phi_dead_or_default(self.phi_dead_threshold),
                ),
                allow_forward,
            },
        );
    }
}

fn self_phi_suspect_or_default(val: f64) -> f64 {
    if val <= 0.0 { 8.0 } else { val }
}

fn self_phi_dead_or_default(val: f64) -> f64 {
    if val <= 0.0 { 14.0 } else { val }
}

impl MeshTopology for SwarmMeshTopology {
    fn record_heartbeat(&self, peer_id: Uuid, rtt_micros: u64, now_micros: u64) {
        let mut peers = self.peers.lock().unwrap();
        if let Some(peer) = peers.get_mut(&peer_id) {
            peer.rtt_micros = rtt_micros;
            peer.phi_detector.record_heartbeat(now_micros);
        }
    }

    fn peer_health(&self, peer_id: &Uuid, now_micros: u64) -> PeerHealthState {
        let peers = self.peers.lock().unwrap();
        peers
            .get(peer_id)
            .map(|p| p.phi_detector.health(now_micros))
            .unwrap_or(PeerHealthState::Dead)
    }

    fn partition_status(&self, total_validators: usize, now_micros: u64) -> PartitionStatus {
        let peers = self.peers.lock().unwrap();
        let total = total_validators.max(1);

        // Include self as 1 reachable node
        let mut reachable_count = 1;
        for peer in peers.values() {
            if peer.phi_detector.health(now_micros) == PeerHealthState::Alive {
                reachable_count += 1;
            }
        }

        let ratio = reachable_count as f64 / total as f64;
        let required_quorum = ((total * 2) / 3) + 1;

        if reachable_count == 1 && total > 1 {
            PartitionStatus::Isolated
        } else if ratio >= self.partition_quorum_ratio && reachable_count >= required_quorum {
            PartitionStatus::Normal {
                reachable_ratio: ratio,
                reachable_count,
                total_validators: total,
            }
        } else {
            PartitionStatus::DegradedMinority {
                reachable_ratio: ratio,
                reachable_count,
                required_quorum,
                total_validators: total,
            }
        }
    }

    fn select_relay_route(&self, target_node: &Uuid) -> Result<Uuid, MeshError> {
        let peers = self.peers.lock().unwrap();
        let now_micros = rivun_core::now_micros().unwrap_or(0);

        // Filter peers that are alive, allow forwarding, and are not the target or self
        let mut candidates: Vec<_> = peers
            .values()
            .filter(|p| {
                p.node_id != *target_node
                    && p.node_id != self.self_node_id
                    && p.allow_forward
                    && p.phi_detector.health(now_micros) == PeerHealthState::Alive
            })
            .collect();

        if candidates.is_empty() {
            return Err(MeshError::NoRelayRoute(*target_node));
        }

        // Sort by lowest queue pressure then lowest RTT
        candidates.sort_by_key(|p| (p.queue_pressure, p.rtt_micros));
        Ok(candidates[0].node_id)
    }
}
