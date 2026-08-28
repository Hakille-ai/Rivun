//! ConsensusActor: BFT 2-Phase consensus state machine and finalized block emission.

use anyhow::Result;
use std::{sync::Arc, time::Duration};
use tokio::{
    sync::{broadcast, mpsc},
    time::{Instant, interval_at},
};
use tracing::{debug, info};
use rivun_agent::SwarmCommitCertificateRef;
use rivun_net::consensus::{SwarmConsensusEngine, SwarmProposal, SwarmVote};

use super::{ConsensusFinalizedBlock, InboundConsensusPacket};
use crate::config::SwarmConfig;

pub struct ConsensusActor {
    config: SwarmConfig,
    inbound_rx: mpsc::Receiver<InboundConsensusPacket>,
    finalized_tx: mpsc::Sender<ConsensusFinalizedBlock>,
    shutdown_rx: broadcast::Receiver<()>,
    engine: Option<Arc<dyn SwarmConsensusEngine>>,
}

impl ConsensusActor {
    #[must_use]
    pub fn new(
        config: SwarmConfig,
        inbound_rx: mpsc::Receiver<InboundConsensusPacket>,
        finalized_tx: mpsc::Sender<ConsensusFinalizedBlock>,
        shutdown_rx: broadcast::Receiver<()>,
        engine: Option<Arc<dyn SwarmConsensusEngine>>,
    ) -> Self {
        Self {
            config,
            inbound_rx,
            finalized_tx,
            shutdown_rx,
            engine,
        }
    }

    pub async fn run(mut self) -> Result<()> {
        debug!("ConsensusActor started");
        let timeout_ms = self.config.max_round_timeout_ms.unwrap_or(3000);
        // `tokio::time::interval` ticks immediately on its first poll. A consensus
        // node must first give the current leader a full round before declaring a
        // timeout, otherwise every fresh actor skips round zero at startup.
        let mut round_timeout = interval_at(
            Instant::now() + Duration::from_millis(timeout_ms),
            Duration::from_millis(timeout_ms),
        );

        loop {
            tokio::select! {
                _ = self.shutdown_rx.recv() => {
                    debug!("ConsensusActor shutting down");
                    break;
                }
                _ = round_timeout.tick() => {
                    if let Some(engine) = &self.engine {
                        engine.advance_round();
                    }
                }
                Some(pkt) = self.inbound_rx.recv() => {
                    self.handle_inbound_packet(pkt).await;
                }
                else => break,
            }
        }
        Ok(())
    }

    async fn handle_inbound_packet(&self, pkt: InboundConsensusPacket) {
        if let Some(engine) = &self.engine {
            if let Ok(proposal) = serde_json::from_slice::<SwarmProposal>(&pkt.payload) {
                let _ = engine.handle_proposal(proposal);
            } else if let Ok(vote) = serde_json::from_slice::<SwarmVote>(&pkt.payload)
                && let Ok(Some(cert)) = engine.handle_vote(vote)
            {
                info!(
                    epoch = cert.epoch,
                    round = cert.round,
                    "Consensus commit certificate finalized"
                );
                let cert_hash = hex::encode(cert.compute_hash());
                let cert_ref = SwarmCommitCertificateRef {
                    certificate_hash: cert_hash,
                    epoch: cert.epoch,
                    view: cert.view,
                    round: cert.round,
                    block_height: cert.block_height,
                    proposal_digest: cert.proposal_digest,
                    threshold: cert.threshold,
                    total_validators: cert.total_validators,
                    signer_bitmask: cert.signer_bitmask.clone(),
                    signatures_count: cert.signatures.len(),
                };

                let finalized = ConsensusFinalizedBlock {
                    epoch: cert.epoch,
                    round: cert.round,
                    block_height: cert.block_height,
                    payload_digest: cert.proposal_digest,
                    certificate: cert_ref,
                };
                let _ = self.finalized_tx.send(finalized).await;
            }
        }
    }
}
