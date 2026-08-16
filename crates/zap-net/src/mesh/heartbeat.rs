//! Heartbeat protocol messages and jittered exponential scheduler.

use rand_core::{OsRng, RngCore};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HeartbeatPing {
    pub sender: Uuid,
    pub sequence: u64,
    pub timestamp_micros: u64,
    pub load_factor: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HeartbeatAck {
    pub responder: Uuid,
    pub echo_sequence: u64,
    pub echo_timestamp_micros: u64,
    pub ack_timestamp_micros: u64,
    pub load_factor: u8,
}

#[derive(Debug, Clone)]
pub struct HeartbeatScheduler {
    base_interval: Duration,
    max_interval: Duration,
    jitter_max: Duration,
    backoff_multiplier: f64,
    consecutive_failures: u32,
}

impl HeartbeatScheduler {
    #[must_use]
    pub fn new(base_interval: Duration, jitter_max: Duration, max_interval: Duration) -> Self {
        Self {
            base_interval,
            max_interval,
            jitter_max,
            backoff_multiplier: 1.5,
            consecutive_failures: 0,
        }
    }

    pub fn record_success(&mut self) {
        self.consecutive_failures = 0;
    }

    pub fn record_failure(&mut self) {
        self.consecutive_failures = self.consecutive_failures.saturating_add(1);
    }

    #[must_use]
    pub fn next_interval(&self) -> Duration {
        let factor = self
            .backoff_multiplier
            .powi(self.consecutive_failures.min(10) as i32);
        let base_millis = (self.base_interval.as_millis() as f64 * factor) as u64;
        let clamped_base = base_millis.min(self.max_interval.as_millis() as u64);

        let jitter_range = self.jitter_max.as_millis() as u64;
        let jitter = if jitter_range > 0 {
            OsRng.next_u64() % jitter_range
        } else {
            0
        };

        Duration::from_millis(clamped_base + jitter)
    }
}
