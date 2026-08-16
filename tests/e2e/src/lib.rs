//! ZAP Next-Gen End-to-End Test Harness Library

pub mod harness;
pub use harness::*;

pub fn e2e_harness_version() -> &'static str {
    "0.1.0"
}
