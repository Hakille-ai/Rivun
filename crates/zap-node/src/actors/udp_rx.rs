//! UdpRxActor: Inbound UDP datagram reception and fast sub-microsecond packet classifier.

use anyhow::Result;
use bytes::Bytes;
use tokio::sync::{broadcast, mpsc};
use tracing::{debug, warn};
use uuid::Uuid;
use zap_core::{ZapFlags, ZapFrame};
use zap_envelope::ZapEnvelope;

use super::{
    InboundConsensusPacket, InboundExecutionPacket, InboundGossipPacket, InboundMeshPacket,
    MeshPacketKind,
};

pub struct UdpRxActor {
    gossip_tx: mpsc::Sender<InboundGossipPacket>,
    consensus_tx: mpsc::Sender<InboundConsensusPacket>,
    mesh_tx: mpsc::Sender<InboundMeshPacket>,
    execution_tx: mpsc::Sender<InboundExecutionPacket>,
    inbound_rx: mpsc::Receiver<(Uuid, ZapFrame)>,
    shutdown_rx: broadcast::Receiver<()>,
}

impl UdpRxActor {
    pub fn new(
        gossip_tx: mpsc::Sender<InboundGossipPacket>,
        consensus_tx: mpsc::Sender<InboundConsensusPacket>,
        mesh_tx: mpsc::Sender<InboundMeshPacket>,
        execution_tx: mpsc::Sender<InboundExecutionPacket>,
        inbound_rx: mpsc::Receiver<(Uuid, ZapFrame)>,
        shutdown_rx: broadcast::Receiver<()>,
    ) -> Self {
        Self {
            gossip_tx,
            consensus_tx,
            mesh_tx,
            execution_tx,
            inbound_rx,
            shutdown_rx,
        }
    }

    pub async fn run(mut self) -> Result<()> {
        debug!("UdpRxActor started");
        loop {
            tokio::select! {
                _ = self.shutdown_rx.recv() => {
                    debug!("UdpRxActor shutting down");
                    break;
                }
                Some((peer, frame)) = self.inbound_rx.recv() => {
                    self.classify_and_route(peer, frame).await;
                }
                else => break,
            }
        }
        Ok(())
    }

    async fn classify_and_route(&self, peer: Uuid, frame: ZapFrame) {
        let now_micros = zap_core::now_micros().unwrap_or(0);

        // Try decoding envelope if payload starts with ZENV
        if let Ok(env) = ZapEnvelope::decode(&frame.payload) {
            let subject = env.subject();
            if subject.starts_with("zap.gossip.") {
                let _ = self.gossip_tx.send(InboundGossipPacket {
                    peer,
                    topic: subject.to_string(),
                    raw_envelope: frame.payload.clone(),
                    received_at_micros: now_micros,
                }).await;
                return;
            } else if subject.starts_with("zap.consensus.") || frame.header.flags.contains(ZapFlags::REQUIRES_CONSENSUS) {
                let _ = self.consensus_tx.send(InboundConsensusPacket {
                    peer,
                    epoch: 1,
                    view: 0,
                    round: 0,
                    payload: frame.payload.clone(),
                }).await;
                return;
            } else if subject.starts_with("zap.p2p.heartbeat") {
                let kind = if subject.ends_with(".ack") {
                    MeshPacketKind::HeartbeatAck
                } else {
                    MeshPacketKind::HeartbeatProbe
                };
                let _ = self.mesh_tx.send(InboundMeshPacket {
                    peer,
                    kind,
                    timestamp_micros: now_micros,
                    echo_rtt_micros: None,
                }).await;
                return;
            }

            let _ = self.execution_tx.send(InboundExecutionPacket {
                peer,
                frame,
                message: env,
            }).await;
        } else {
            // Check raw frame flags
            if frame.header.flags.contains(ZapFlags::BROADCAST) {
                let _ = self.gossip_tx.send(InboundGossipPacket {
                    peer,
                    topic: "zap.broadcast".to_string(),
                    raw_envelope: frame.payload.clone(),
                    received_at_micros: now_micros,
                }).await;
            } else if frame.header.flags.contains(ZapFlags::REQUIRES_CONSENSUS) {
                let _ = self.consensus_tx.send(InboundConsensusPacket {
                    peer,
                    epoch: 1,
                    view: 0,
                    round: 0,
                    payload: frame.payload.clone(),
                }).await;
            } else {
                warn!(%peer, "Unclassified raw frame received");
            }
        }
    }
}
