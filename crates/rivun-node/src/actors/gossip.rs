//! GossipActor: Epidemic gossip state dissemination, LRU deduplication, and peer sampling.

use anyhow::Result;
use std::{sync::Arc, time::Duration};
use tokio::sync::{broadcast, mpsc};
use tracing::debug;
use rivun_net::gossip::{GossipDeduplicationCache, GossipEnvelope, SwarmGossipEngine};

use super::InboundGossipPacket;
use crate::config::GossipConfig;

pub struct GossipActor {
    config: GossipConfig,
    inbound_rx: mpsc::Receiver<InboundGossipPacket>,
    shutdown_rx: broadcast::Receiver<()>,
    dedup_cache: GossipDeduplicationCache,
    engine: Option<Arc<dyn SwarmGossipEngine>>,
}

impl GossipActor {
    #[must_use]
    pub fn new(
        config: GossipConfig,
        inbound_rx: mpsc::Receiver<InboundGossipPacket>,
        shutdown_rx: broadcast::Receiver<()>,
        engine: Option<Arc<dyn SwarmGossipEngine>>,
    ) -> Self {
        let dedup_cache = GossipDeduplicationCache::new(
            config.dedup_cache_size,
            Duration::from_millis(config.anti_entropy_interval_ms * 12),
        );
        Self {
            config,
            inbound_rx,
            shutdown_rx,
            dedup_cache,
            engine,
        }
    }

    pub async fn run(mut self) -> Result<()> {
        debug!("GossipActor started");
        let pex_interval = Duration::from_millis(self.config.pex_interval_ms);
        let mut pex_ticker = tokio::time::interval(pex_interval);

        let anti_entropy_interval = Duration::from_millis(self.config.anti_entropy_interval_ms);
        let mut sync_ticker = tokio::time::interval(anti_entropy_interval);

        loop {
            tokio::select! {
                _ = self.shutdown_rx.recv() => {
                    debug!("GossipActor shutting down");
                    break;
                }
                _ = pex_ticker.tick() => {
                    self.perform_peer_exchange().await;
                }
                _ = sync_ticker.tick() => {
                    self.perform_anti_entropy_sync().await;
                }
                Some(pkt) = self.inbound_rx.recv() => {
                    self.handle_inbound_packet(pkt).await;
                }
                else => break,
            }
        }
        Ok(())
    }

    async fn handle_inbound_packet(&mut self, pkt: InboundGossipPacket) {
        if let Ok(envelope) = serde_json::from_slice::<GossipEnvelope>(&pkt.raw_envelope) {
            if !self.dedup_cache.insert(envelope.message_id) {
                debug!(?envelope.message_id, "Dropped duplicate gossip envelope");
                return;
            }
            if let Some(engine) = &self.engine {
                let _ = engine.handle_inbound_envelope(envelope);
            }
        }
    }

    async fn perform_peer_exchange(&self) {
        debug!("Performing periodic peer exchange (PEX)...");
    }

    async fn perform_anti_entropy_sync(&self) {
        debug!("Performing periodic anti-entropy sync...");
    }
}
