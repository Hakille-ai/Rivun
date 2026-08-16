//! Error types for ZAP driver SDK operations.

use std::fmt;
use thiserror::Error;

/// Core error returned by ZAP drivers and driver execution hosts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DriverError {
    message: String,
}

impl DriverError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    pub fn fuel_limit_exceeded(consumed: u64, limit: u64) -> Self {
        Self::new(format!(
            "fuel limit exceeded: consumed {consumed}, limit {limit}"
        ))
    }

    pub fn buffer_error(err: BufferError) -> Self {
        Self::new(err.to_string())
    }

    pub fn ipc_error(err: IpcError) -> Self {
        Self::new(err.to_string())
    }
}

impl fmt::Display for DriverError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for DriverError {}

impl From<BufferError> for DriverError {
    fn from(err: BufferError) -> Self {
        Self::buffer_error(err)
    }
}

impl From<IpcError> for DriverError {
    fn from(err: IpcError) -> Self {
        Self::ipc_error(err)
    }
}

/// Errors related to buffer allocation, slicing, and memory mapping.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum BufferError {
    #[error("buffer index out of bounds: offset {offset}, len {len}, bound {bound}")]
    OutOfBounds {
        offset: usize,
        len: usize,
        bound: usize,
    },

    #[error("buffer capacity exceeded: requested {requested}, capacity {capacity}")]
    CapacityExceeded { requested: usize, capacity: usize },

    #[error("invalid pointer: ptr {ptr}, len {len}")]
    InvalidPointer { ptr: u32, len: u32 },

    #[error("null pointer encountered")]
    NullPointer,

    #[error("buffer alignment error: address {addr:#x} not aligned to {align}")]
    UnalignedAddress { addr: usize, align: usize },
}

/// Errors related to inter-driver IPC messaging and pipes.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum IpcError {
    #[error("channel {0} is full")]
    ChannelFull(u32),

    #[error("channel {0} is closed")]
    ChannelClosed(u32),

    #[error("invalid sequence number: expected {expected}, got {actual}")]
    InvalidSequence { expected: u64, actual: u64 },

    #[error("channel {0} not found")]
    ChannelNotFound(u32),

    #[error("buffer overflow: payload size {size} exceeds max {max}")]
    BufferOverflow { size: usize, max: usize },

    #[error("causal transcript digest mismatch")]
    DigestMismatch,

    #[error("ipc error: {0}")]
    Custom(String),
}
