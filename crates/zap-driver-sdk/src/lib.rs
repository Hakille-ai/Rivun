//! Minimal helpers for authoring ZAP WASM action drivers.
//!
//! The runtime ABI is intentionally small:
//! - export `memory`
//! - export `zap_alloc(len: i32) -> i32`
//! - export `zap_dealloc(ptr: i32, len: i32)`
//! - export `zap_execute(action_ptr, action_len, payload_ptr, payload_len) -> i64`
//!
//! `zap_execute` returns `(result_ptr << 32) | result_len`.

use std::fmt;

pub const DRIVER_ABI_VERSION: u16 = 1;
pub const MEMORY_EXPORT: &str = "memory";
pub const ALLOC_EXPORT: &str = "zap_alloc";
pub const DEALLOC_EXPORT: &str = "zap_dealloc";
pub const EXECUTE_EXPORT: &str = "zap_execute";

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

pub trait ZapDriver {
    fn execute(&self, input: DriverInput<'_>) -> Result<Vec<u8>, DriverError>;
}

pub fn execute_driver(
    driver: &impl ZapDriver,
    action: &str,
    payload: &[u8],
) -> Result<Vec<u8>, DriverError> {
    driver.execute(DriverInput { action, payload })
}

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
}

impl fmt::Display for DriverError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for DriverError {}

#[cfg(test)]
mod tests {
    use super::*;

    struct EchoDriver;

    impl ZapDriver for EchoDriver {
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
