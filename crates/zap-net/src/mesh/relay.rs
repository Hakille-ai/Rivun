//! Dynamic 2-Hop Failover Relay Routing Envelope.

use bytes::Bytes;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::mod_types::MeshError;

pub const RELAY_ENVELOPE_MAGIC: [u8; 4] = *b"ZRLY";
pub const RELAY_ENVELOPE_VERSION: u8 = 1;
pub const MAX_RELAY_HOPS: u8 = 2;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ZapRelayEnvelope {
    pub magic: [u8; 4],
    pub version: u8,
    pub origin_node: Uuid,
    pub relay_node: Uuid,
    pub final_target: Uuid,
    pub hop_count: u8,
    pub inner_frame: Bytes,
}

impl ZapRelayEnvelope {
    #[must_use]
    pub fn new(origin_node: Uuid, relay_node: Uuid, final_target: Uuid, inner_frame: Bytes) -> Self {
        Self {
            magic: RELAY_ENVELOPE_MAGIC,
            version: RELAY_ENVELOPE_VERSION,
            origin_node,
            relay_node,
            final_target,
            hop_count: 1,
            inner_frame,
        }
    }

    pub fn forward(&self) -> Result<Self, MeshError> {
        if self.hop_count >= MAX_RELAY_HOPS {
            return Err(MeshError::RelayHopLimitExceeded {
                max: MAX_RELAY_HOPS,
            });
        }
        let mut forwarded = self.clone();
        forwarded.hop_count += 1;
        Ok(forwarded)
    }

    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(4 + 1 + 16 + 16 + 16 + 1 + 4 + self.inner_frame.len());
        out.extend_from_slice(&self.magic);
        out.push(self.version);
        out.extend_from_slice(self.origin_node.as_bytes());
        out.extend_from_slice(self.relay_node.as_bytes());
        out.extend_from_slice(self.final_target.as_bytes());
        out.push(self.hop_count);
        out.extend_from_slice(&(self.inner_frame.len() as u32).to_be_bytes());
        out.extend_from_slice(&self.inner_frame);
        out
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, MeshError> {
        if bytes.len() < 58 {
            return Err(MeshError::RelayDecodeError("buffer truncated".into()));
        }
        if bytes[0..4] != RELAY_ENVELOPE_MAGIC {
            return Err(MeshError::InvalidRelayMagic);
        }
        let version = bytes[4];
        if version != RELAY_ENVELOPE_VERSION {
            return Err(MeshError::UnsupportedRelayVersion(version));
        }
        let origin_node = Uuid::from_slice(&bytes[5..21])
            .map_err(|e| MeshError::RelayDecodeError(e.to_string()))?;
        let relay_node = Uuid::from_slice(&bytes[21..37])
            .map_err(|e| MeshError::RelayDecodeError(e.to_string()))?;
        let final_target = Uuid::from_slice(&bytes[37..53])
            .map_err(|e| MeshError::RelayDecodeError(e.to_string()))?;
        let hop_count = bytes[53];
        let frame_len = u32::from_be_bytes([bytes[54], bytes[55], bytes[56], bytes[57]]) as usize;
        if bytes.len() < 58 + frame_len {
            return Err(MeshError::RelayDecodeError("frame length mismatch".into()));
        }
        let inner_frame = Bytes::copy_from_slice(&bytes[58..58 + frame_len]);

        Ok(Self {
            magic: RELAY_ENVELOPE_MAGIC,
            version,
            origin_node,
            relay_node,
            final_target,
            hop_count,
            inner_frame,
        })
    }
}
