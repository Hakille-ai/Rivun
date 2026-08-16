//! Adaptive Mesh Health Tracking, Failure Detection, and 2-Hop Relay Routing.

pub mod heartbeat;
pub mod mod_types;
pub mod partition;
pub mod phi_detector;
pub mod relay;
pub mod topology;

pub use heartbeat::{HeartbeatAck, HeartbeatPing, HeartbeatScheduler};
pub use mod_types::MeshError;
pub use partition::PartitionStatus;
pub use phi_detector::{PeerHealthState, PhiAccrualDetector};
pub use relay::{
    MAX_RELAY_HOPS, RELAY_ENVELOPE_MAGIC, RELAY_ENVELOPE_VERSION, ZapRelayEnvelope,
};
pub use topology::{MeshTopology, PeerMeshInfo, SwarmMeshTopology};

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn test_phi_accrual_detector() {
        let mut detector = PhiAccrualDetector::new(8.0, 14.0);
        let base_micros = 1_000_000_u64;

        // Feed regular 1,000ms intervals
        for i in 0..50 {
            detector.record_heartbeat(base_micros + (i * 1_000_000));
        }

        let last = base_micros + (49 * 1_000_000);
        // Immediate check: Alive
        assert_eq!(detector.health(last + 100_000), PeerHealthState::Alive);

        // 50ms after expected: Alive / Low phi
        let phi_short = detector.phi(last + 1_050_000);
        assert!(phi_short < 8.0);

        // 20s after: Dead
        let phi_long = detector.phi(last + 20_000_000);
        assert!(phi_long >= 14.0);
        assert_eq!(detector.health(last + 20_000_000), PeerHealthState::Dead);
    }

    #[test]
    fn test_relay_envelope_roundtrip_and_forwarding() {
        let n1 = Uuid::new_v4();
        let n2 = Uuid::new_v4();
        let n3 = Uuid::new_v4();
        let payload = bytes::Bytes::from_static(b"inner frame data");

        let relay = ZapRelayEnvelope::new(n1, n2, n3, payload.clone());
        assert_eq!(relay.hop_count, 1);

        let encoded = relay.encode();
        let decoded = ZapRelayEnvelope::decode(&encoded).expect("decode failed");
        assert_eq!(relay, decoded);

        let forwarded = relay.forward().expect("forward should succeed");
        assert_eq!(forwarded.hop_count, 2);

        // Third forward exceeds MAX_RELAY_HOPS (2)
        assert!(forwarded.forward().is_err());
    }
}
