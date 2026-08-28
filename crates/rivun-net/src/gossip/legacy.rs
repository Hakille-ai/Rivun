//! Legacy GossipMesh and QuorumProposal compatibility structures.

use super::{error::GossipError, vector_clock::VectorClock};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fmt,
    time::{Duration, Instant},
};
use uuid::Uuid;

/// Peer Liveness Status.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PeerHealth {
    Alive,
    Suspect,
    Dead,
}

/// Swarm Peer State tracked in Gossip Mesh.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SwarmPeer {
    pub node_id: Uuid,
    pub endpoint: String,
    pub capabilities: Vec<String>,
    pub health: PeerHealth,
    pub last_seen_micros: u64,
    pub vector_clock: VectorClock,
    pub load_factor: u8,
}

/// Quorum Proposal State.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct QuorumProposal {
    pub proposal_id: Uuid,
    pub proposer_id: Uuid,
    pub topic: String,
    pub terms_hash: String,
    pub required_threshold: usize,
    pub deadline_micros: u64,
    pub votes: HashMap<Uuid, String>, // voter_id -> signature
    pub finalized: bool,
}

/// P2P Gossip Mesh State Engine.
#[derive(Clone, Debug)]
pub struct GossipMesh {
    pub self_node_id: Uuid,
    pub self_endpoint: String,
    pub peers: HashMap<Uuid, SwarmPeer>,
    pub vector_clock: VectorClock,
    pub proposals: HashMap<Uuid, QuorumProposal>,
    pub suspect_timeout: Duration,
    pub dead_timeout: Duration,
    pub last_tick: Instant,
}

impl GossipMesh {
    #[must_use]
    pub fn new(self_node_id: Uuid, self_endpoint: impl Into<String>) -> Self {
        Self {
            self_node_id,
            self_endpoint: self_endpoint.into(),
            peers: HashMap::new(),
            vector_clock: VectorClock::new(),
            proposals: HashMap::new(),
            suspect_timeout: Duration::from_millis(3000),
            dead_timeout: Duration::from_millis(8000),
            last_tick: Instant::now(),
        }
    }

    pub fn register_peer(
        &mut self,
        node_id: Uuid,
        endpoint: impl Into<String>,
        capabilities: Vec<String>,
        now_micros: u64,
    ) {
        if node_id == self.self_node_id {
            return;
        }
        self.peers.insert(
            node_id,
            SwarmPeer {
                node_id,
                endpoint: endpoint.into(),
                capabilities,
                health: PeerHealth::Alive,
                last_seen_micros: now_micros,
                vector_clock: VectorClock::new(),
                load_factor: 0,
            },
        );
    }

    pub fn record_heartbeat(
        &mut self,
        from_node: Uuid,
        clock: &VectorClock,
        load_factor: u8,
        now_micros: u64,
    ) {
        self.vector_clock.merge(clock);
        if let Some(peer) = self.peers.get_mut(&from_node) {
            peer.health = PeerHealth::Alive;
            peer.last_seen_micros = now_micros;
            peer.vector_clock.merge(clock);
            peer.load_factor = load_factor;
        }
    }

    /// Check peer timeouts and detect network partitions.
    pub fn evaluate_health(&mut self, now_micros: u64) -> Result<(), GossipError> {
        let mut unreachable_count = 0;
        let total_nodes = self.peers.len() + 1;

        let suspect_threshold_micros = self.suspect_timeout.as_micros() as u64;
        let dead_threshold_micros = self.dead_timeout.as_micros() as u64;

        for peer in self.peers.values_mut() {
            let elapsed = now_micros.saturating_sub(peer.last_seen_micros);
            if elapsed > dead_threshold_micros {
                peer.health = PeerHealth::Dead;
                unreachable_count += 1;
            } else if elapsed > suspect_threshold_micros {
                peer.health = PeerHealth::Suspect;
                unreachable_count += 1;
            } else {
                peer.health = PeerHealth::Alive;
            }
        }

        // Byzantine partition threshold: more than 1/3 of total cluster unreachable
        if total_nodes >= 3 && unreachable_count * 3 >= total_nodes {
            return Err(GossipError::NetworkPartition {
                unreachable_count,
                total_nodes,
            });
        }

        Ok(())
    }

    /// Select optimal peer for routing a capability action, automatically failing over
    /// away from Suspect or Dead nodes to lowest-load Alive nodes.
    #[must_use]
    pub fn select_route_for_capability(&self, capability: &str) -> Option<&SwarmPeer> {
        self.peers
            .values()
            .filter(|p| {
                p.health == PeerHealth::Alive && p.capabilities.iter().any(|c| c == capability)
            })
            .min_by_key(|p| p.load_factor)
    }

    /// Create a quorum proposal across the swarm.
    pub fn create_proposal(
        &mut self,
        proposal_id: Uuid,
        topic: impl Into<String>,
        terms_hash: impl Into<String>,
        deadline_micros: u64,
    ) -> &QuorumProposal {
        let total_nodes = self.peers.len() + 1;
        let required_threshold = (total_nodes * 2 / 3) + 1;

        let proposal = QuorumProposal {
            proposal_id,
            proposer_id: self.self_node_id,
            topic: topic.into(),
            terms_hash: terms_hash.into(),
            required_threshold,
            deadline_micros,
            votes: HashMap::new(),
            finalized: false,
        };

        self.proposals.insert(proposal_id, proposal);
        self.proposals.get(&proposal_id).unwrap()
    }

    /// Record a signature vote on an active proposal.
    pub fn cast_vote(
        &mut self,
        proposal_id: Uuid,
        voter_id: Uuid,
        signature: impl Into<String>,
        now_micros: u64,
    ) -> Result<bool, GossipError> {
        let proposal = self
            .proposals
            .get_mut(&proposal_id)
            .ok_or(GossipError::ProposalClosed(proposal_id))?;

        if proposal.finalized || now_micros > proposal.deadline_micros {
            return Err(GossipError::ProposalClosed(proposal_id));
        }

        proposal.votes.insert(voter_id, signature.into());

        if proposal.votes.len() >= proposal.required_threshold {
            proposal.finalized = true;
            return Ok(true);
        }

        Ok(false)
    }

    /// Check if a proposal reached quorum.
    #[must_use]
    pub fn is_proposal_finalized(&self, proposal_id: &Uuid) -> bool {
        self.proposals
            .get(proposal_id)
            .map(|p| p.finalized)
            .unwrap_or(false)
    }
}

impl fmt::Display for GossipMesh {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "GossipMesh(self={}, peers={}, proposals={})",
            self.self_node_id,
            self.peers.len(),
            self.proposals.len()
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Causality;

    #[test]
    fn test_vector_clock_causality_and_merge() {
        let n1 = Uuid::new_v4();
        let n2 = Uuid::new_v4();

        let mut v1 = VectorClock::new();
        let mut v2 = VectorClock::new();

        v1.increment(n1);
        v2.increment(n2);
        assert_eq!(v1.compare(&v2), Causality::Concurrent);

        v1.merge(&v2);
        assert_eq!(v1.compare(&v2), Causality::StrictlyAfter);
        assert_eq!(v2.compare(&v1), Causality::StrictlyBefore);
    }

    #[test]
    fn test_gossip_health_and_failover_routing() {
        let self_id = Uuid::new_v4();
        let p1 = Uuid::new_v4();
        let p2 = Uuid::new_v4();

        let mut mesh = GossipMesh::new(self_id, "127.0.0.1:9000");
        mesh.register_peer(p1, "127.0.0.1:9001", vec!["compute.robotics".into()], 1000);
        mesh.register_peer(p2, "127.0.0.1:9002", vec!["compute.robotics".into()], 1000);

        // Initially both alive, choose lowest load
        let selected = mesh
            .select_route_for_capability("compute.robotics")
            .unwrap();
        assert!(selected.node_id == p1 || selected.node_id == p2);

        // Advance time: p1 missed heartbeats -> Dead
        let _ = mesh.evaluate_health(15_000_000);
        assert_eq!(mesh.peers.get(&p1).unwrap().health, PeerHealth::Dead);

        // P2 sends heartbeat -> Alive
        let mut clk = VectorClock::new();
        clk.increment(p2);
        mesh.record_heartbeat(p2, &clk, 10, 15_000_000);
        let _ = mesh.evaluate_health(15_000_000);

        // Routing automatically fails over exclusively to p2
        let route = mesh
            .select_route_for_capability("compute.robotics")
            .unwrap();
        assert_eq!(route.node_id, p2);
    }

    #[test]
    fn test_quorum_voting_threshold() {
        let self_id = Uuid::new_v4();
        let p1 = Uuid::new_v4();
        let p2 = Uuid::new_v4();
        let p3 = Uuid::new_v4();

        let mut mesh = GossipMesh::new(self_id, "127.0.0.1:9000");
        mesh.register_peer(p1, "127.0.0.1:9001", vec![], 1000);
        mesh.register_peer(p2, "127.0.0.1:9002", vec![], 1000);
        mesh.register_peer(p3, "127.0.0.1:9003", vec![], 1000);

        let prop_id = Uuid::new_v4();
        let prop = mesh.create_proposal(prop_id, "actuate_motor", "terms_hash_123", 10_000_000);
        // Total 4 nodes -> required threshold = (4 * 2 / 3) + 1 = 2 + 1 = 3
        assert_eq!(prop.required_threshold, 3);

        let fin1 = mesh.cast_vote(prop_id, self_id, "sig_self", 2000).unwrap();
        assert!(!fin1);
        let fin2 = mesh.cast_vote(prop_id, p1, "sig_p1", 2100).unwrap();
        assert!(!fin2);
        let fin3 = mesh.cast_vote(prop_id, p2, "sig_p2", 2200).unwrap();
        assert!(fin3);
        assert!(mesh.is_proposal_finalized(&prop_id));
    }
}
