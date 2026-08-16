//! Concurrent Tokio actor daemon architecture for ZAP Node.

pub mod consensus;
pub mod execution;
pub mod gossip;
pub mod mesh;
pub mod udp_rx;

pub use consensus::ConsensusActor;
pub use execution::ExecutionActor;
pub use gossip::GossipActor;
pub use mesh::MeshActor;
pub use udp_rx::UdpRxActor;

use bytes::Bytes;
use std::collections::HashMap;
use tokio::sync::{broadcast, mpsc, watch};
use uuid::Uuid;
use zap_agent::SwarmCommitCertificateRef;
use zap_core::ZapFrame;
use zap_envelope::ZapEnvelope;

#[derive(Debug, Clone)]
pub struct InboundGossipPacket {
    pub peer: Uuid,
    pub topic: String,
    pub raw_envelope: Bytes,
    pub received_at_micros: u64,
}

#[derive(Debug, Clone)]
pub struct InboundConsensusPacket {
    pub peer: Uuid,
    pub epoch: u64,
    pub view: u64,
    pub round: u64,
    pub payload: Bytes,
}

#[derive(Debug, Clone)]
pub struct InboundMeshPacket {
    pub peer: Uuid,
    pub kind: MeshPacketKind,
    pub timestamp_micros: u64,
    pub echo_rtt_micros: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MeshPacketKind {
    HeartbeatProbe,
    HeartbeatAck,
    RelayEncapsulation,
}

#[derive(Debug, Clone)]
pub struct InboundExecutionPacket {
    pub peer: Uuid,
    pub frame: ZapFrame,
    pub message: ZapEnvelope,
}

#[derive(Debug, Clone)]
pub struct ConsensusFinalizedBlock {
    pub epoch: u64,
    pub round: u64,
    pub block_height: u64,
    pub payload_digest: [u8; 32],
    pub certificate: SwarmCommitCertificateRef,
}

#[derive(Debug, Clone)]
pub struct MeshHealthStatus {
    pub is_partitioned: bool,
    pub quorum_ratio: f64,
    pub reachable_validators: usize,
    pub total_validators: usize,
    pub peer_phi_scores: HashMap<Uuid, f64>,
    pub relay_paths: HashMap<Uuid, Uuid>,
}

impl Default for MeshHealthStatus {
    fn default() -> Self {
        Self {
            is_partitioned: false,
            quorum_ratio: 1.0,
            reachable_validators: 1,
            total_validators: 1,
            peer_phi_scores: HashMap::new(),
            relay_paths: HashMap::new(),
        }
    }
}

pub struct NodeActorChannels {
    pub udp_to_gossip_tx: mpsc::Sender<InboundGossipPacket>,
    pub udp_to_gossip_rx: mpsc::Receiver<InboundGossipPacket>,
    pub udp_to_consensus_tx: mpsc::Sender<InboundConsensusPacket>,
    pub udp_to_consensus_rx: mpsc::Receiver<InboundConsensusPacket>,
    pub udp_to_mesh_tx: mpsc::Sender<InboundMeshPacket>,
    pub udp_to_mesh_rx: mpsc::Receiver<InboundMeshPacket>,
    pub udp_to_execution_tx: mpsc::Sender<InboundExecutionPacket>,
    pub udp_to_execution_rx: mpsc::Receiver<InboundExecutionPacket>,
    pub consensus_to_execution_tx: mpsc::Sender<ConsensusFinalizedBlock>,
    pub consensus_to_execution_rx: mpsc::Receiver<ConsensusFinalizedBlock>,
    pub mesh_to_execution_watch_tx: watch::Sender<MeshHealthStatus>,
    pub mesh_to_execution_watch_rx: watch::Receiver<MeshHealthStatus>,
    pub shutdown_tx: broadcast::Sender<()>,
}

impl NodeActorChannels {
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        let (udp_to_gossip_tx, udp_to_gossip_rx) = mpsc::channel(capacity);
        let (udp_to_consensus_tx, udp_to_consensus_rx) = mpsc::channel(capacity);
        let (udp_to_mesh_tx, udp_to_mesh_rx) = mpsc::channel(capacity);
        let (udp_to_execution_tx, udp_to_execution_rx) = mpsc::channel(capacity);
        let (consensus_to_execution_tx, consensus_to_execution_rx) = mpsc::channel(capacity);
        let (mesh_to_execution_watch_tx, mesh_to_execution_watch_rx) =
            watch::channel(MeshHealthStatus::default());
        let (shutdown_tx, _) = broadcast::channel(16);

        Self {
            udp_to_gossip_tx,
            udp_to_gossip_rx,
            udp_to_consensus_tx,
            udp_to_consensus_rx,
            udp_to_mesh_tx,
            udp_to_mesh_rx,
            udp_to_execution_tx,
            udp_to_execution_rx,
            consensus_to_execution_tx,
            consensus_to_execution_rx,
            mesh_to_execution_watch_tx,
            mesh_to_execution_watch_rx,
            shutdown_tx,
        }
    }
}
