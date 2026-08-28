//! Inter-driver IPC channel endpoints, topologies, and message passing primitives.

use crate::{buffer::IpcBufferView, error::IpcError};
use serde::{Deserialize, Serialize};
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
};

/// IPC Message domain separator for cryptographic hashing.
pub const IPC_MSG_DOMAIN: &[u8] = b"Rivun-IPC-MSG-v1:";

bitflags::bitflags! {
    /// Flags attached to inter-driver IPC messages.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
    pub struct IpcFlags: u32 {
        const NONE = 0;
        const PRIORITY = 1 << 0;
        const STREAM_CHUNK = 1 << 1;
        const END_OF_STREAM = 1 << 2;
        const REQUIRES_ACK = 1 << 3;
    }
}

/// Backpressure policy for full IPC channels and ring buffers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum BackpressurePolicy {
    /// Drop the oldest message in the buffer to accommodate the new message.
    #[default]
    DropOldest,
    /// Drop the newly incoming message without modifying existing queue.
    DropNewest,
    /// Return an error when buffer is full.
    Error,
}

/// Alias for BackpressurePolicy for API consistency.
pub type BackpressureStrategy = BackpressurePolicy;

/// Deterministic zero-copy pipeline pipe connecting Stage A to Stage B.
#[derive(Debug)]
pub struct IpcPipe {
    pub name: String,
    pub channel_id: u32,
    causal_hasher: blake3::Hasher,
    sequence_counter: u64,
    total_bytes_transferred: u64,
}

impl IpcPipe {
    pub fn new(name: impl Into<String>, channel_id: u32) -> Self {
        let mut causal_hasher = blake3::Hasher::new();
        causal_hasher.update(b"Rivun-IPC-PIPE-v1:");
        let name_str = name.into();
        causal_hasher.update(name_str.as_bytes());
        Self {
            name: name_str,
            channel_id,
            causal_hasher,
            sequence_counter: 0,
            total_bytes_transferred: 0,
        }
    }

    pub fn transfer(&mut self, mut msg: IpcMessage) -> Result<IpcMessage, IpcError> {
        self.sequence_counter = self.sequence_counter.saturating_add(1);
        msg.channel_id = self.channel_id;
        msg.sequence = self.sequence_counter;
        let digest = msg.compute_hash();
        self.causal_hasher.update(&digest);
        self.total_bytes_transferred = self
            .total_bytes_transferred
            .saturating_add(msg.payload.len() as u64);
        Ok(msg)
    }

    pub fn current_causal_hash(&self) -> String {
        self.causal_hasher.finalize().to_hex().to_string()
    }

    pub fn sequence_counter(&self) -> u64 {
        self.sequence_counter
    }

    pub fn total_bytes_transferred(&self) -> u64 {
        self.total_bytes_transferred
    }
}

/// An IPC message passed between driver stages.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IpcMessage {
    pub channel_id: u32,
    pub sequence: u64,
    pub timestamp_micros: u64,
    pub flags: u32,
    pub payload: Vec<u8>,
}

impl IpcMessage {
    pub fn new(
        channel_id: u32,
        sequence: u64,
        timestamp_micros: u64,
        flags: u32,
        payload: impl Into<Vec<u8>>,
    ) -> Self {
        Self {
            channel_id,
            sequence,
            timestamp_micros,
            flags,
            payload: payload.into(),
        }
    }

    /// Compute cryptographic Blake3 hash of the message with domain separation.
    pub fn compute_hash(&self) -> [u8; 32] {
        let mut hasher = blake3::Hasher::new();
        hasher.update(IPC_MSG_DOMAIN);
        hasher.update(&self.channel_id.to_le_bytes());
        hasher.update(&self.sequence.to_le_bytes());
        hasher.update(&self.timestamp_micros.to_le_bytes());
        hasher.update(&self.flags.to_le_bytes());
        hasher.update(&(self.payload.len() as u64).to_le_bytes());
        hasher.update(&self.payload);
        *hasher.finalize().as_bytes()
    }

    /// Hex-encoded cryptographic digest.
    pub fn hex_digest(&self) -> String {
        hex::encode(self.compute_hash())
    }

    /// Borrow as a zero-copy `IpcBufferView`.
    pub fn as_buffer_view(&self) -> IpcBufferView<'_> {
        IpcBufferView::new(
            self.channel_id,
            self.sequence,
            self.timestamp_micros,
            self.flags,
            &self.payload,
        )
    }
}

/// Configuration for an IPC channel.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IpcChannelConfig {
    pub channel_id: u32,
    pub capacity: usize,
    pub backpressure: BackpressurePolicy,
    pub max_payload_bytes: usize,
}

impl Default for IpcChannelConfig {
    fn default() -> Self {
        Self {
            channel_id: 0,
            capacity: 256,
            backpressure: BackpressurePolicy::DropOldest,
            max_payload_bytes: 1024 * 1024,
        }
    }
}

/// Circular ring buffer for inter-driver IPC messages.
#[derive(Debug)]
pub struct IpcRingBuffer {
    capacity: usize,
    buffer: VecDeque<IpcMessage>,
    dropped_count: u64,
    total_pushed: u64,
}

impl IpcRingBuffer {
    pub fn new(capacity: usize) -> Self {
        let capacity = capacity.max(1);
        Self {
            capacity,
            buffer: VecDeque::with_capacity(capacity),
            dropped_count: 0,
            total_pushed: 0,
        }
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }

    pub fn len(&self) -> usize {
        self.buffer.len()
    }

    pub fn is_empty(&self) -> bool {
        self.buffer.is_empty()
    }

    pub fn is_full(&self) -> bool {
        self.buffer.len() >= self.capacity
    }

    pub fn dropped_count(&self) -> u64 {
        self.dropped_count
    }

    pub fn total_pushed(&self) -> u64 {
        self.total_pushed
    }

    /// Push message using the specified backpressure policy.
    /// Returns `Ok(Some(dropped_msg))` if a message was evicted under `DropOldest`.
    pub fn push(
        &mut self,
        msg: IpcMessage,
        policy: BackpressurePolicy,
    ) -> Result<Option<IpcMessage>, IpcError> {
        self.total_pushed = self.total_pushed.saturating_add(1);
        if self.buffer.len() < self.capacity {
            self.buffer.push_back(msg);
            Ok(None)
        } else {
            match policy {
                BackpressurePolicy::DropOldest => {
                    self.dropped_count = self.dropped_count.saturating_add(1);
                    let dropped = self.buffer.pop_front();
                    self.buffer.push_back(msg);
                    Ok(dropped)
                }
                BackpressurePolicy::DropNewest => {
                    self.dropped_count = self.dropped_count.saturating_add(1);
                    Ok(Some(msg))
                }
                BackpressurePolicy::Error => Err(IpcError::ChannelFull(msg.channel_id)),
            }
        }
    }

    /// Pop next available message from the ring buffer.
    pub fn pop(&mut self) -> Option<IpcMessage> {
        self.buffer.pop_front()
    }

    /// Peek at the head message without removing it.
    pub fn peek(&self) -> Option<&IpcMessage> {
        self.buffer.front()
    }

    /// Clear all messages from the buffer.
    pub fn clear(&mut self) {
        self.buffer.clear();
    }
}

/// Thread-safe shared IPC channel with synchronized ring buffer.
#[derive(Debug, Clone)]
pub struct IpcChannel {
    config: IpcChannelConfig,
    inner: Arc<Mutex<IpcRingBuffer>>,
}

impl IpcChannel {
    pub fn new(config: IpcChannelConfig) -> Self {
        let capacity = config.capacity;
        Self {
            config,
            inner: Arc::new(Mutex::new(IpcRingBuffer::new(capacity))),
        }
    }

    pub fn config(&self) -> &IpcChannelConfig {
        &self.config
    }

    pub fn send(&self, msg: IpcMessage) -> Result<Option<IpcMessage>, IpcError> {
        if msg.payload.len() > self.config.max_payload_bytes {
            return Err(IpcError::BufferOverflow {
                size: msg.payload.len(),
                max: self.config.max_payload_bytes,
            });
        }
        let mut guard = self
            .inner
            .lock()
            .map_err(|_| IpcError::Custom("lock poisoned".to_string()))?;
        guard.push(msg, self.config.backpressure)
    }

    pub fn recv(&self) -> Result<Option<IpcMessage>, IpcError> {
        let mut guard = self
            .inner
            .lock()
            .map_err(|_| IpcError::Custom("lock poisoned".to_string()))?;
        Ok(guard.pop())
    }

    pub fn len(&self) -> usize {
        self.inner.lock().map(|g| g.len()).unwrap_or(0)
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn dropped_count(&self) -> u64 {
        self.inner.lock().map(|g| g.dropped_count()).unwrap_or(0)
    }
}

/// Topology definitions for multi-stage driver pipelines.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum IpcTopology {
    /// Point-to-point 1:1 pipe between two stages.
    PointToPoint {
        source_stage: u32,
        target_stage: u32,
    },
    /// Sequential linear chain (e.g. 0 -> 1 -> 2 -> ...).
    PipelineChain { stages: Vec<u32> },
    /// 1-to-N fan-out broadcasting from one source to multiple targets.
    FanOut {
        source_stage: u32,
        target_stages: Vec<u32>,
    },
    /// N-to-1 fan-in merging from multiple sources to one target.
    FanIn {
        source_stages: Vec<u32>,
        target_stage: u32,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ipc_message_hashing() {
        let msg = IpcMessage::new(1, 42, 1_700_000_000_000, 0, b"telemetry_packet");
        let hash = msg.compute_hash();
        assert_ne!(hash, [0u8; 32]);
        let hex = msg.hex_digest();
        assert_eq!(hex.len(), 64);

        // Same message produces deterministic digest
        let msg2 = IpcMessage::new(1, 42, 1_700_000_000_000, 0, b"telemetry_packet");
        assert_eq!(msg.compute_hash(), msg2.compute_hash());

        // Altering payload alters digest
        let msg_tampered = IpcMessage::new(1, 42, 1_700_000_000_000, 0, b"tampered_payload");
        assert_ne!(msg.compute_hash(), msg_tampered.compute_hash());
    }

    #[test]
    fn test_ipc_ring_buffer_drop_oldest() {
        let mut ring = IpcRingBuffer::new(2);
        assert_eq!(ring.capacity(), 2);
        assert!(ring.is_empty());

        let m1 = IpcMessage::new(1, 1, 100, 0, b"m1");
        let m2 = IpcMessage::new(1, 2, 200, 0, b"m2");
        let m3 = IpcMessage::new(1, 3, 300, 0, b"m3");

        assert!(
            ring.push(m1.clone(), BackpressurePolicy::DropOldest)
                .unwrap()
                .is_none()
        );
        assert!(
            ring.push(m2.clone(), BackpressurePolicy::DropOldest)
                .unwrap()
                .is_none()
        );
        assert!(ring.is_full());

        // Pushing 3rd message drops m1
        let dropped = ring
            .push(m3.clone(), BackpressurePolicy::DropOldest)
            .unwrap();
        assert_eq!(dropped.unwrap().payload, b"m1");
        assert_eq!(ring.dropped_count(), 1);
        assert_eq!(ring.len(), 2);

        // Popping returns m2 then m3
        assert_eq!(ring.pop().unwrap().payload, b"m2");
        assert_eq!(ring.pop().unwrap().payload, b"m3");
        assert!(ring.pop().is_none());
    }

    #[test]
    fn test_ipc_ring_buffer_drop_newest() {
        let mut ring = IpcRingBuffer::new(2);
        let m1 = IpcMessage::new(1, 1, 100, 0, b"m1");
        let m2 = IpcMessage::new(1, 2, 200, 0, b"m2");
        let m3 = IpcMessage::new(1, 3, 300, 0, b"m3");

        ring.push(m1, BackpressurePolicy::DropNewest).unwrap();
        ring.push(m2, BackpressurePolicy::DropNewest).unwrap();

        let dropped = ring.push(m3, BackpressurePolicy::DropNewest).unwrap();
        assert_eq!(dropped.unwrap().payload, b"m3");
        assert_eq!(ring.dropped_count(), 1);

        assert_eq!(ring.pop().unwrap().payload, b"m1");
        assert_eq!(ring.pop().unwrap().payload, b"m2");
    }

    #[test]
    fn test_ipc_ring_buffer_error_policy() {
        let mut ring = IpcRingBuffer::new(1);
        let m1 = IpcMessage::new(7, 1, 100, 0, b"m1");
        let m2 = IpcMessage::new(7, 2, 200, 0, b"m2");

        ring.push(m1, BackpressurePolicy::Error).unwrap();
        let err = ring.push(m2, BackpressurePolicy::Error).unwrap_err();
        assert_eq!(err, IpcError::ChannelFull(7));
    }

    #[test]
    fn test_ipc_channel_thread_safe_send_recv() {
        let config = IpcChannelConfig {
            channel_id: 10,
            capacity: 4,
            backpressure: BackpressurePolicy::DropOldest,
            max_payload_bytes: 1024,
        };
        let channel = IpcChannel::new(config);

        channel
            .send(IpcMessage::new(10, 1, 100, 0, b"stage1_output"))
            .unwrap();
        channel
            .send(IpcMessage::new(10, 2, 200, 0, b"stage2_output"))
            .unwrap();

        assert_eq!(channel.len(), 2);
        let rec1 = channel.recv().unwrap().unwrap();
        assert_eq!(rec1.payload, b"stage1_output");
        let rec2 = channel.recv().unwrap().unwrap();
        assert_eq!(rec2.payload, b"stage2_output");
        assert!(channel.recv().unwrap().is_none());
    }
}
