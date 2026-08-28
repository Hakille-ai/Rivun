//! ExecutionActor: Action routing, WASM driver dispatch, and receipt journaling.

use anyhow::Result;
use tokio::sync::{broadcast, mpsc, watch};
use tracing::{debug, info, warn};

use super::{ConsensusFinalizedBlock, InboundExecutionPacket, MeshHealthStatus};

pub struct ExecutionActor {
    inbound_rx: mpsc::Receiver<InboundExecutionPacket>,
    consensus_rx: mpsc::Receiver<ConsensusFinalizedBlock>,
    mesh_health_rx: watch::Receiver<MeshHealthStatus>,
    shutdown_rx: broadcast::Receiver<()>,
}

impl ExecutionActor {
    #[must_use]
    pub fn new(
        inbound_rx: mpsc::Receiver<InboundExecutionPacket>,
        consensus_rx: mpsc::Receiver<ConsensusFinalizedBlock>,
        mesh_health_rx: watch::Receiver<MeshHealthStatus>,
        shutdown_rx: broadcast::Receiver<()>,
    ) -> Self {
        Self {
            inbound_rx,
            consensus_rx,
            mesh_health_rx,
            shutdown_rx,
        }
    }

    pub async fn run(mut self) -> Result<()> {
        debug!("ExecutionActor started");
        loop {
            tokio::select! {
                _ = self.shutdown_rx.recv() => {
                    debug!("ExecutionActor shutting down");
                    break;
                }
                Some(block) = self.consensus_rx.recv() => {
                    self.handle_finalized_block(block).await;
                }
                Some(pkt) = self.inbound_rx.recv() => {
                    self.handle_inbound_packet(pkt).await;
                }
                else => break,
            }
        }
        Ok(())
    }

    async fn handle_finalized_block(&self, block: ConsensusFinalizedBlock) {
        info!(
            epoch = block.epoch,
            round = block.round,
            height = block.block_height,
            "Applying finalized consensus block to ledger"
        );
    }

    async fn handle_inbound_packet(&self, pkt: InboundExecutionPacket) {
        let health = self.mesh_health_rx.borrow();
        if health.is_partitioned {
            warn!(peer = %pkt.peer, "Rejecting state-mutating execution packet while partitioned");
            return;
        }

        debug!(peer = %pkt.peer, subject = %pkt.message.subject(), "Routing execution action");
    }
}
