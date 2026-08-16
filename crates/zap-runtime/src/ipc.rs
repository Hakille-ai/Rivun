//! Deterministic inter-driver IPC routing, zero-copy pipes, and channel topologies.

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};
use zap_driver_sdk::{
    BackpressurePolicy, IpcChannel, IpcChannelConfig, IpcMessage, IpcTopology, IPC_MSG_DOMAIN,
};
use thiserror::Error;

/// IPC runtime errors.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum RuntimeIpcError {
    #[error("channel {0} not found")]
    ChannelNotFound(u32),
    #[error("channel {0} buffer overflow")]
    BufferOverflow(u32),
    #[error("stage {0} is not connected")]
    StageNotConnected(u32),
    #[error("causal digest mismatch")]
    DigestMismatch,
    #[error("lock poisoned")]
    LockPoisoned,
}

/// A zero-copy IPC pipe connecting a source driver stage to a target driver stage.
#[derive(Debug, Clone)]
pub struct IpcPipe {
    pub source_stage: u32,
    pub target_stage: u32,
    pub channel: IpcChannel,
    causal_hasher: Arc<Mutex<blake3::Hasher>>,
}

impl IpcPipe {
    pub fn new(source_stage: u32, target_stage: u32, config: IpcChannelConfig) -> Self {
        let mut hasher = blake3::Hasher::new();
        hasher.update(IPC_MSG_DOMAIN);
        hasher.update(&source_stage.to_le_bytes());
        hasher.update(&target_stage.to_le_bytes());

        Self {
            source_stage,
            target_stage,
            channel: IpcChannel::new(config),
            causal_hasher: Arc::new(Mutex::new(hasher)),
        }
    }

    /// Send a message through the pipe and update causal transcript digest.
    pub fn send(&self, msg: IpcMessage) -> Result<Option<IpcMessage>, RuntimeIpcError> {
        let hash = msg.compute_hash();
        if let Ok(mut h) = self.causal_hasher.lock() {
            h.update(&hash);
        } else {
            return Err(RuntimeIpcError::LockPoisoned);
        }

        self.channel.send(msg).map_err(|_| RuntimeIpcError::BufferOverflow(self.channel.config().channel_id))
    }

    /// Receive the next message from the pipe.
    pub fn recv(&self) -> Result<Option<IpcMessage>, RuntimeIpcError> {
        self.channel.recv().map_err(|_| RuntimeIpcError::LockPoisoned)
    }

    /// Get current causal chain transcript hash.
    pub fn current_causal_digest(&self) -> String {
        self.causal_hasher
            .lock()
            .map(|h| h.finalize().to_hex().to_string())
            .unwrap_or_default()
    }
}

/// Dynamic IPC Router supporting arbitrary multi-stage driver topologies.
#[derive(Debug, Default)]
pub struct IpcRouter {
    pipes: HashMap<(u32, u32), IpcPipe>,
    channels: HashMap<u32, IpcChannel>,
}

impl IpcRouter {
    pub fn new() -> Self {
        Self {
            pipes: HashMap::new(),
            channels: HashMap::new(),
        }
    }

    /// Build and register channels for a given topology.
    pub fn configure_topology(&mut self, topology: IpcTopology, capacity: usize) {
        match topology {
            IpcTopology::PointToPoint { source_stage, target_stage } => {
                let channel_id = (source_stage << 16) | target_stage;
                let pipe = IpcPipe::new(
                    source_stage,
                    target_stage,
                    IpcChannelConfig {
                        channel_id,
                        capacity,
                        backpressure: BackpressurePolicy::DropOldest,
                        max_payload_bytes: 1024 * 1024,
                    },
                );
                self.channels.insert(channel_id, pipe.channel.clone());
                self.pipes.insert((source_stage, target_stage), pipe);
            }
            IpcTopology::PipelineChain { stages } => {
                for window in stages.windows(2) {
                    let (src, dst) = (window[0], window[1]);
                    let channel_id = (src << 16) | dst;
                    let pipe = IpcPipe::new(
                        src,
                        dst,
                        IpcChannelConfig {
                            channel_id,
                            capacity,
                            backpressure: BackpressurePolicy::DropOldest,
                            max_payload_bytes: 1024 * 1024,
                        },
                    );
                    self.channels.insert(channel_id, pipe.channel.clone());
                    self.pipes.insert((src, dst), pipe);
                }
            }
            IpcTopology::FanOut { source_stage, target_stages } => {
                for target in target_stages {
                    let channel_id = (source_stage << 16) | target;
                    let pipe = IpcPipe::new(
                        source_stage,
                        target,
                        IpcChannelConfig {
                            channel_id,
                            capacity,
                            backpressure: BackpressurePolicy::DropOldest,
                            max_payload_bytes: 1024 * 1024,
                        },
                    );
                    self.channels.insert(channel_id, pipe.channel.clone());
                    self.pipes.insert((source_stage, target), pipe);
                }
            }
            IpcTopology::FanIn { source_stages, target_stage } => {
                for source in source_stages {
                    let channel_id = (source << 16) | target_stage;
                    let pipe = IpcPipe::new(
                        source,
                        target_stage,
                        IpcChannelConfig {
                            channel_id,
                            capacity,
                            backpressure: BackpressurePolicy::DropOldest,
                            max_payload_bytes: 1024 * 1024,
                        },
                    );
                    self.channels.insert(channel_id, pipe.channel.clone());
                    self.pipes.insert((source, target_stage), pipe);
                }
            }
        }
    }

    /// Forward a message from a source stage to a target stage.
    pub fn route_message(&self, src: u32, dst: u32, msg: IpcMessage) -> Result<(), RuntimeIpcError> {
        let pipe = self.pipes.get(&(src, dst)).ok_or(RuntimeIpcError::StageNotConnected(src))?;
        pipe.send(msg)?;
        Ok(())
    }

    /// Receive next message destined for a stage from a source stage.
    pub fn receive_message(&self, src: u32, dst: u32) -> Result<Option<IpcMessage>, RuntimeIpcError> {
        let pipe = self.pipes.get(&(src, dst)).ok_or(RuntimeIpcError::StageNotConnected(dst))?;
        pipe.recv()
    }

    /// Get pipe between two stages.
    pub fn get_pipe(&self, src: u32, dst: u32) -> Option<&IpcPipe> {
        self.pipes.get(&(src, dst))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ipc_pipe_causal_hashing() {
        let config = IpcChannelConfig {
            channel_id: 1,
            capacity: 8,
            backpressure: BackpressurePolicy::DropOldest,
            max_payload_bytes: 1024,
        };
        let pipe = IpcPipe::new(0, 1, config);
        let d0 = pipe.current_causal_digest();

        let msg = IpcMessage::new(1, 1, 1000, 0, b"frame_step_0");
        pipe.send(msg).unwrap();

        let d1 = pipe.current_causal_digest();
        assert_ne!(d0, d1);

        let rec = pipe.recv().unwrap().unwrap();
        assert_eq!(rec.payload, b"frame_step_0");
    }

    #[test]
    fn test_ipc_router_pipeline_chain() {
        let mut router = IpcRouter::new();
        let topology = IpcTopology::PipelineChain {
            stages: vec![0, 1, 2],
        };
        router.configure_topology(topology, 4);

        // Send from Stage 0 to Stage 1
        let msg0 = IpcMessage::new(1, 1, 100, 0, b"stage0_output");
        router.route_message(0, 1, msg0).unwrap();

        let rec1 = router.receive_message(0, 1).unwrap().unwrap();
        assert_eq!(rec1.payload, b"stage0_output");

        // Send from Stage 1 to Stage 2
        let msg1 = IpcMessage::new(2, 1, 200, 0, b"stage1_output");
        router.route_message(1, 2, msg1).unwrap();

        let rec2 = router.receive_message(1, 2).unwrap().unwrap();
        assert_eq!(rec2.payload, b"stage1_output");
    }
}
