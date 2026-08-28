//! Gossip Envelope Wire Protocol with Ed25519 Signatures and Hop Damping.

use bytes::Bytes;
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const GOSSIP_ENVELOPE_MAGIC: [u8; 4] = *b"ZGSP";
pub const GOSSIP_ENVELOPE_VERSION: u8 = 1;
pub const GOSSIP_SIGNING_DOMAIN: &[u8] = b"Rivun-GOSSIP-ENVELOPE-v1";
pub const DEFAULT_MAX_HOPS: u8 = 16;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct GossipMessageId(pub [u8; 32]);

impl GossipMessageId {
    #[must_use]
    pub fn compute(topic: &str, origin: &Uuid, seq: u64, payload: &[u8]) -> Self {
        let mut hasher = blake3::Hasher::new_derive_key("Rivun-GOSSIP-MSG-ID-v1");
        hasher.update(topic.as_bytes());
        hasher.update(origin.as_bytes());
        hasher.update(&seq.to_be_bytes());
        let payload_hash = blake3::hash(payload);
        hasher.update(payload_hash.as_bytes());
        Self(*hasher.finalize().as_bytes())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GossipEnvelope {
    pub magic: [u8; 4],
    pub version: u8,
    pub message_id: GossipMessageId,
    pub origin_node: Uuid,
    pub topic: String,
    pub sequence: u64,
    pub max_hops: u8,
    pub current_hop: u8,
    pub timestamp_micros: u64,
    pub payload: Bytes,
    #[serde(with = "crate::serde_helpers::signature_bytes")]
    pub signature: [u8; 64],
}

impl GossipEnvelope {
    #[must_use]
    pub fn new_signed(
        origin_node: Uuid,
        topic: impl Into<String>,
        sequence: u64,
        max_hops: u8,
        timestamp_micros: u64,
        payload: Bytes,
        signing_key: &SigningKey,
    ) -> Self {
        let topic = topic.into();
        let message_id = GossipMessageId::compute(&topic, &origin_node, sequence, &payload);
        let digest = Self::signing_digest(&message_id, timestamp_micros, max_hops);
        let signature = signing_key.sign(&digest).to_bytes();

        Self {
            magic: GOSSIP_ENVELOPE_MAGIC,
            version: GOSSIP_ENVELOPE_VERSION,
            message_id,
            origin_node,
            topic,
            sequence,
            max_hops,
            current_hop: 0,
            timestamp_micros,
            payload,
            signature,
        }
    }

    #[must_use]
    pub fn verify_signature(&self, verifying_key: &VerifyingKey) -> bool {
        if self.magic != GOSSIP_ENVELOPE_MAGIC || self.version != GOSSIP_ENVELOPE_VERSION {
            return false;
        }
        let expected_id =
            GossipMessageId::compute(&self.topic, &self.origin_node, self.sequence, &self.payload);
        if self.message_id != expected_id {
            return false;
        }
        let digest = Self::signing_digest(&self.message_id, self.timestamp_micros, self.max_hops);
        let sig = Signature::from_bytes(&self.signature);
        verifying_key.verify(&digest, &sig).is_ok()
    }

    #[must_use]
    pub fn forward(&self) -> Option<Self> {
        if self.current_hop + 1 >= self.max_hops {
            return None;
        }
        let mut forwarded = self.clone();
        forwarded.current_hop += 1;
        Some(forwarded)
    }

    #[must_use]
    fn signing_digest(
        message_id: &GossipMessageId,
        timestamp_micros: u64,
        max_hops: u8,
    ) -> [u8; 32] {
        let mut hasher = blake3::Hasher::new_derive_key("Rivun-GOSSIP-ENVELOPE-v1");
        hasher.update(&message_id.0);
        hasher.update(&timestamp_micros.to_be_bytes());
        hasher.update(&[max_hops]);
        *hasher.finalize().as_bytes()
    }
}
