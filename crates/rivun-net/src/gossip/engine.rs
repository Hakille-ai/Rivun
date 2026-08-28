//! Swarm Gossip Engine and concrete dispatcher.

use super::{
    cache::GossipDeduplicationCache,
    envelope::{DEFAULT_MAX_HOPS, GossipEnvelope, GossipMessageId},
    error::GossipError,
};
use bytes::Bytes;
use ed25519_dalek::{SigningKey, VerifyingKey};
use std::{
    collections::HashMap,
    sync::{
        Mutex, RwLock,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};
use tokio::sync::mpsc;
use uuid::Uuid;
use rivun_core::now_micros;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GossipReceipt {
    pub message_id: GossipMessageId,
    pub topic: String,
    pub sequence: u64,
    pub fanout_peers: usize,
}

pub trait SwarmGossipEngine: Send + Sync {
    fn broadcast_state(&self, topic: &str, payload: Bytes) -> Result<GossipReceipt, GossipError>;
    fn handle_inbound_envelope(
        &self,
        envelope: GossipEnvelope,
    ) -> Result<Option<GossipEnvelope>, GossipError>;
    fn subscribe(&self, topic: &str) -> mpsc::Receiver<GossipEnvelope>;
    fn active_peer_count(&self) -> usize;
}

pub struct SwarmGossipDispatcher {
    self_node_id: Uuid,
    signing_key: SigningKey,
    sequence: AtomicU64,
    fanout: usize,
    max_hops: u8,
    dedup_cache: Mutex<GossipDeduplicationCache>,
    subscribers: RwLock<HashMap<String, Vec<mpsc::Sender<GossipEnvelope>>>>,
    active_peers: RwLock<HashMap<Uuid, VerifyingKey>>,
}

impl SwarmGossipDispatcher {
    #[must_use]
    pub fn new(
        self_node_id: Uuid,
        signing_key: SigningKey,
        fanout: usize,
        max_hops: u8,
        cache_capacity: usize,
        cache_ttl: Duration,
    ) -> Self {
        Self {
            self_node_id,
            signing_key,
            sequence: AtomicU64::new(1),
            fanout: fanout.max(1),
            max_hops: if max_hops == 0 {
                DEFAULT_MAX_HOPS
            } else {
                max_hops
            },
            dedup_cache: Mutex::new(GossipDeduplicationCache::new(cache_capacity, cache_ttl)),
            subscribers: RwLock::new(HashMap::new()),
            active_peers: RwLock::new(HashMap::new()),
        }
    }

    pub fn register_peer(&self, node_id: Uuid, verifying_key: VerifyingKey) {
        let mut peers = self.active_peers.write().unwrap();
        peers.insert(node_id, verifying_key);
    }

    pub fn remove_peer(&self, node_id: &Uuid) {
        let mut peers = self.active_peers.write().unwrap();
        peers.remove(node_id);
    }

    #[must_use]
    pub fn get_peer_key(&self, node_id: &Uuid) -> Option<VerifyingKey> {
        let peers = self.active_peers.read().unwrap();
        peers.get(node_id).copied()
    }
}

impl SwarmGossipEngine for SwarmGossipDispatcher {
    fn broadcast_state(&self, topic: &str, payload: Bytes) -> Result<GossipReceipt, GossipError> {
        let seq = self.sequence.fetch_add(1, Ordering::SeqCst);
        let timestamp = now_micros().unwrap_or(0);
        let envelope = GossipEnvelope::new_signed(
            self.self_node_id,
            topic,
            seq,
            self.max_hops,
            timestamp,
            payload,
            &self.signing_key,
        );

        let msg_id = envelope.message_id;
        {
            let mut cache = self.dedup_cache.lock().unwrap();
            cache.insert(msg_id);
        }

        let active_count = {
            let peers = self.active_peers.read().unwrap();
            peers.len()
        };
        let fanout_count = self.fanout.min(active_count);

        Ok(GossipReceipt {
            message_id: msg_id,
            topic: topic.to_string(),
            sequence: seq,
            fanout_peers: fanout_count,
        })
    }

    fn handle_inbound_envelope(
        &self,
        envelope: GossipEnvelope,
    ) -> Result<Option<GossipEnvelope>, GossipError> {
        if envelope.magic != super::envelope::GOSSIP_ENVELOPE_MAGIC {
            return Err(GossipError::InvalidMagic);
        }
        if envelope.version != super::envelope::GOSSIP_ENVELOPE_VERSION {
            return Err(GossipError::UnsupportedVersion(envelope.version));
        }

        // Verify signature if peer key is known
        if envelope.origin_node == self.self_node_id {
            if !envelope.verify_signature(&self.signing_key.verifying_key()) {
                return Err(GossipError::InvalidSignature(envelope.origin_node));
            }
        } else if let Some(vk) = self.get_peer_key(&envelope.origin_node)
            && !envelope.verify_signature(&vk)
        {
            return Err(GossipError::InvalidSignature(envelope.origin_node));
        }

        // Deduplication check
        {
            let mut cache = self.dedup_cache.lock().unwrap();
            if !cache.insert(envelope.message_id) {
                return Err(GossipError::DuplicateMessage(envelope.message_id));
            }
        }

        // Notify local subscribers
        {
            let subs = self.subscribers.read().unwrap();
            if let Some(senders) = subs.get(&envelope.topic) {
                for sender in senders {
                    let _ = sender.try_send(envelope.clone());
                }
            }
        }

        // Forward with hop increment
        Ok(envelope.forward())
    }

    fn subscribe(&self, topic: &str) -> mpsc::Receiver<GossipEnvelope> {
        let (tx, rx) = mpsc::channel(128);
        let mut subs = self.subscribers.write().unwrap();
        subs.entry(topic.to_string()).or_default().push(tx);
        rx
    }

    fn active_peer_count(&self) -> usize {
        let peers = self.active_peers.read().unwrap();
        peers.len()
    }
}
