//! Monotonic Vector Clock for distributed causal ordering.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};
use uuid::Uuid;

/// Monotonic Vector Clock for distributed causal ordering.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct VectorClock {
    pub clocks: BTreeMap<Uuid, u64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Causality {
    StrictlyBefore,
    StrictlyAfter,
    Equal,
    Concurrent,
}

impl VectorClock {
    #[must_use]
    pub fn new() -> Self {
        Self {
            clocks: BTreeMap::new(),
        }
    }

    pub fn increment(&mut self, node_id: Uuid) -> u64 {
        let entry = self.clocks.entry(node_id).or_insert(0);
        *entry += 1;
        *entry
    }

    #[must_use]
    pub fn get(&self, node_id: &Uuid) -> u64 {
        self.clocks.get(node_id).copied().unwrap_or(0)
    }

    pub fn merge(&mut self, other: &VectorClock) {
        for (node_id, clock) in &other.clocks {
            let entry = self.clocks.entry(*node_id).or_insert(0);
            if *clock > *entry {
                *entry = *clock;
            }
        }
    }

    #[must_use]
    pub fn compare(&self, other: &VectorClock) -> Causality {
        let mut self_greater = false;
        let mut other_greater = false;

        let all_keys: HashSet<Uuid> = self
            .clocks
            .keys()
            .chain(other.clocks.keys())
            .copied()
            .collect();

        for key in all_keys {
            let s = self.get(&key);
            let o = other.get(&key);
            if s > o {
                self_greater = true;
            } else if o > s {
                other_greater = true;
            }
        }

        match (self_greater, other_greater) {
            (false, false) => Causality::Equal,
            (true, false) => Causality::StrictlyAfter,
            (false, true) => Causality::StrictlyBefore,
            (true, true) => Causality::Concurrent,
        }
    }
}
