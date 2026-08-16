//! Sliding-window LRU message deduplication cache with TTL pruning.

use super::envelope::GossipMessageId;
use std::{
    collections::{HashSet, VecDeque},
    time::{Duration, Instant},
};

pub struct GossipDeduplicationCache {
    capacity: usize,
    ttl: Duration,
    seen: HashSet<GossipMessageId>,
    order: VecDeque<(GossipMessageId, Instant)>,
}

impl GossipDeduplicationCache {
    #[must_use]
    pub fn new(capacity: usize, ttl: Duration) -> Self {
        Self {
            capacity,
            ttl,
            seen: HashSet::with_capacity(capacity.min(65536)),
            order: VecDeque::with_capacity(capacity.min(65536)),
        }
    }

    #[must_use]
    pub fn contains(&self, id: &GossipMessageId) -> bool {
        self.seen.contains(id)
    }

    pub fn insert(&mut self, id: GossipMessageId) -> bool {
        self.prune_expired();
        if self.seen.contains(&id) {
            return false;
        }
        if self.order.len() >= self.capacity
            && let Some((old_id, _)) = self.order.pop_front()
        {
            self.seen.remove(&old_id);
        }
        self.seen.insert(id);
        self.order.push_back((id, Instant::now()));
        true
    }

    pub fn prune_expired(&mut self) {
        let now = Instant::now();
        while let Some((_, timestamp)) = self.order.front() {
            if now.duration_since(*timestamp) > self.ttl {
                if let Some((old_id, _)) = self.order.pop_front() {
                    self.seen.remove(&old_id);
                }
            } else {
                break;
            }
        }
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.seen.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.seen.is_empty()
    }
}
