//! MeshActor: Adaptive mesh health monitoring, heartbeat scheduling, and partition detection.

use anyhow::Result;
use std::{sync::Arc, time::Duration};
use tokio::sync::{broadcast, mpsc, watch};
use tracing::{debug, warn};
use rivun_net::mesh::{HeartbeatScheduler, MeshTopology, PartitionStatus};

use super::{InboundMeshPacket, MeshHealthStatus};
use crate::config::MeshConfig;

pub struct MeshActor {
    inbound_rx: mpsc::Receiver<InboundMeshPacket>,
    status_tx: watch::Sender<MeshHealthStatus>,
    shutdown_rx: broadcast::Receiver<()>,
    topology: Option<Arc<dyn MeshTopology>>,
    scheduler: HeartbeatScheduler,
}

impl MeshActor {
    #[must_use]
    pub fn new(
        config: MeshConfig,
        inbound_rx: mpsc::Receiver<InboundMeshPacket>,
        status_tx: watch::Sender<MeshHealthStatus>,
        shutdown_rx: broadcast::Receiver<()>,
        topology: Option<Arc<dyn MeshTopology>>,
    ) -> Self {
        let scheduler = HeartbeatScheduler::new(
            Duration::from_millis(config.heartbeat_interval_ms),
            Duration::from_millis(config.heartbeat_jitter_ms),
            Duration::from_millis(config.heartbeat_interval_ms * 10),
        );
        Self {
            inbound_rx,
            status_tx,
            shutdown_rx,
            topology,
            scheduler,
        }
    }

    pub async fn run(mut self) -> Result<()> {
        debug!("MeshActor started");
        let mut heartbeat_ticker = tokio::time::interval(self.scheduler.next_interval());

        loop {
            tokio::select! {
                _ = self.shutdown_rx.recv() => {
                    debug!("MeshActor shutting down");
                    break;
                }
                _ = heartbeat_ticker.tick() => {
                    self.send_heartbeats().await;
                    self.evaluate_partition_health();
                }
                Some(pkt) = self.inbound_rx.recv() => {
                    self.handle_inbound_packet(pkt).await;
                }
                else => break,
            }
        }
        Ok(())
    }

    async fn handle_inbound_packet(&mut self, pkt: InboundMeshPacket) {
        let now_micros = rivun_core::now_micros().unwrap_or(0);
        if let Some(topo) = &self.topology {
            let rtt = pkt.echo_rtt_micros.unwrap_or(1000);
            topo.record_heartbeat(pkt.peer, rtt, now_micros);
        }
        self.scheduler.record_success();
    }

    async fn send_heartbeats(&mut self) {
        debug!("Dispatching jittered heartbeat probes...");
    }

    fn evaluate_partition_health(&self) {
        if let Some(topo) = &self.topology {
            let now_micros = rivun_core::now_micros().unwrap_or(0);
            let status = topo.partition_status(4, now_micros);
            let is_partitioned = !status.is_operational();
            let (quorum_ratio, reachable_validators, total_validators) = match status {
                PartitionStatus::Normal {
                    reachable_ratio,
                    reachable_count,
                    total_validators,
                } => (reachable_ratio, reachable_count, total_validators),
                PartitionStatus::DegradedMinority {
                    reachable_ratio,
                    reachable_count,
                    total_validators,
                    ..
                } => {
                    warn!(
                        reachable_count,
                        total_validators, "Mesh degraded in minority partition"
                    );
                    (reachable_ratio, reachable_count, total_validators)
                }
                PartitionStatus::Isolated => (0.0, 1, 4),
            };

            let mesh_status = MeshHealthStatus {
                is_partitioned,
                quorum_ratio,
                reachable_validators,
                total_validators,
                peer_phi_scores: std::collections::HashMap::new(),
                relay_paths: std::collections::HashMap::new(),
            };
            let _ = self.status_tx.send(mesh_status);
        }
    }
}
