//! Minimal helpers for authoring Rivun WASM action drivers.
//!
//! The runtime ABI is intentionally small:
//! - export `memory`
//! - export `rivun_alloc(len: i32) -> i32`
//! - export `rivun_dealloc(ptr: i32, len: i32)`
//! - export `rivun_execute(action_ptr, action_len, payload_ptr, payload_len) -> i64`
//!
//! `rivun_execute` returns `(result_ptr << 32) | result_len`.

pub mod async_driver;
pub mod buffer;
pub mod error;
pub mod ipc;

pub use async_driver::{
    AsyncStreamReader, AsyncStreamWriter, AsyncRivunDriver, BoxFuture, DriverContext,
    MemoryStreamReader, MemoryStreamWriter, SyncDriverAdapter, RivunDriverExt,
};
pub use buffer::{
    BufferSlice, BufferSliceMut, IpcBufferView, MemoryMapper, PinnedBuffer, ZeroCopyBuffer,
};
pub use error::{BufferError, DriverError, IpcError};
pub use ipc::{
    BackpressurePolicy, BackpressureStrategy, IPC_MSG_DOMAIN, IpcChannel, IpcChannelConfig,
    IpcFlags, IpcMessage, IpcPipe, IpcRingBuffer, IpcTopology,
};

pub const DRIVER_ABI_VERSION: u16 = 1;
pub const MEMORY_EXPORT: &str = "memory";
pub const ALLOC_EXPORT: &str = "rivun_alloc";
pub const DEALLOC_EXPORT: &str = "rivun_dealloc";
pub const EXECUTE_EXPORT: &str = "rivun_execute";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PackedResult {
    pub ptr: u32,
    pub len: u32,
}

impl PackedResult {
    pub const fn new(ptr: u32, len: u32) -> Self {
        Self { ptr, len }
    }

    pub const fn pack(self) -> i64 {
        ((self.ptr as u64) << 32 | self.len as u64) as i64
    }

    pub const fn unpack(value: i64) -> Self {
        let value = value as u64;
        Self {
            ptr: (value >> 32) as u32,
            len: (value & 0xFFFF_FFFF) as u32,
        }
    }
}

pub const fn pack_result(ptr: u32, len: u32) -> i64 {
    PackedResult::new(ptr, len).pack()
}

pub const fn unpack_result(value: i64) -> PackedResult {
    PackedResult::unpack(value)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DriverInput<'a> {
    pub action: &'a str,
    pub payload: &'a [u8],
}

pub trait RivunDriver {
    fn execute(&self, input: DriverInput<'_>) -> Result<Vec<u8>, DriverError>;
}

pub fn execute_driver(
    driver: &impl RivunDriver,
    action: &str,
    payload: &[u8],
) -> Result<Vec<u8>, DriverError> {
    driver.execute(DriverInput { action, payload })
}

#[cfg(test)]
mod tests {
    use super::*;

    struct EchoDriver;

    impl RivunDriver for EchoDriver {
        fn execute(&self, input: DriverInput<'_>) -> Result<Vec<u8>, DriverError> {
            if input.action != "echo" {
                return Err(DriverError::new("unsupported action"));
            }
            Ok(input.payload.to_vec())
        }
    }

    #[test]
    fn result_pack_round_trips() {
        let packed = pack_result(0x1020_3040, 0x5060_7080);
        assert_eq!(
            unpack_result(packed),
            PackedResult {
                ptr: 0x1020_3040,
                len: 0x5060_7080
            }
        );
    }

    #[test]
    fn driver_trait_executes() {
        let output = execute_driver(&EchoDriver, "echo", b"hello").unwrap();
        assert_eq!(output, b"hello");
    }

    #[test]
    fn driver_trait_reports_errors() {
        let error = execute_driver(&EchoDriver, "thermostat.setpoint", b"{}").unwrap_err();
        assert_eq!(error.message(), "unsupported action");
    }
}
