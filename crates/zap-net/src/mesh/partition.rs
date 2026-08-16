//! Network partition status and split-brain mitigation metrics.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PartitionStatus {
    Normal {
        reachable_ratio: f64,
        reachable_count: usize,
        total_validators: usize,
    },
    DegradedMinority {
        reachable_ratio: f64,
        reachable_count: usize,
        required_quorum: usize,
        total_validators: usize,
    },
    Isolated,
}

impl PartitionStatus {
    #[must_use]
    pub fn is_operational(&self) -> bool {
        matches!(self, PartitionStatus::Normal { .. })
    }
}
