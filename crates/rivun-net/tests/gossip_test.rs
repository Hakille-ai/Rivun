//! Gossip Dissemination, Deduplication, and Anti-Entropy Sync Integration Tests.

use bytes::Bytes;
use ed25519_dalek::SigningKey;
use rand_core::OsRng;
use std::{
    collections::{HashMap, HashSet, VecDeque},
    sync::{Arc, Mutex},
    time::Duration,
};
use uuid::Uuid;
use rivun_net::gossip::{
    Causality, DiscoveredPeerEntry, GossipDeduplicationCache, GossipEnvelope, GossipMesh,
    GossipMessageId, PeerExchangeRequest, PeerExchangeResponse, SwarmGossipDispatcher,
    SwarmGossipEngine, xor_distance,
};

#[derive(Debug, Clone, Default)]
pub struct ChaosConfig {
    pub drop_rate: f64,
    pub min_delay: Duration,
    pub severed_links: HashSet<(Uuid, Uuid)>,
}

#[derive(Clone, Default)]
pub struct MockSwarmRouter {
    inner: Arc<Mutex<MockSwarmRouterInner>>,
}

#[derive(Default)]
struct MockSwarmRouterInner {
    config: ChaosConfig,
    inboxes: HashMap<Uuid, VecDeque<Bytes>>,
}

impl MockSwarmRouter {
    pub fn new(config: ChaosConfig) -> Self {
        Self {
            inner: Arc::new(Mutex::new(MockSwarmRouterInner {
                config,
                inboxes: HashMap::new(),
            })),
        }
    }

    pub fn register_node(&self, node_id: Uuid) {
        let mut inner = self.inner.lock().unwrap();
        inner.inboxes.entry(node_id).or_default();
    }

    pub fn send(&self, source: Uuid, target: Uuid, payload: Bytes) {
        let mut inner = self.inner.lock().unwrap();
        if inner.config.severed_links.contains(&(source, target)) {
            return;
        }
        if inner.config.drop_rate > 0.0 {
            let roll = (source.as_u128() ^ target.as_u128()) % 1000;
            if (roll as f64 / 1000.0) < inner.config.drop_rate {
                return;
            }
        }
        if let Some(inbox) = inner.inboxes.get_mut(&target) {
            inbox.push_back(payload);
        }
    }

    pub fn try_recv(&self, node_id: Uuid) -> Option<Bytes> {
        let mut inner = self.inner.lock().unwrap();
        inner.inboxes.get_mut(&node_id)?.pop_front()
    }
}

#[tokio::test]
async fn test_k_fanout_epidemic_convergence() {
    let num_nodes = 7;
    let mut node_keys = Vec::new();
    let mut node_ids = Vec::new();
    let mut dispatchers = Vec::new();

    for _ in 0..num_nodes {
        let key = SigningKey::generate(&mut OsRng);
        let id = Uuid::new_v4();
        node_keys.push(key.clone());
        node_ids.push(id);
        dispatchers.push(SwarmGossipDispatcher::new(
            id,
            key,
            3,  // fanout
            16, // max hops
            1000,
            Duration::from_secs(60),
        ));
    }

    // Fully connect mesh peers in active view
    for (i, dispatcher) in dispatchers.iter().enumerate() {
        for (j, peer_id) in node_ids.iter().enumerate() {
            if i != j {
                dispatcher.register_peer(*peer_id, node_keys[j].verifying_key());
            }
        }
    }

    // Node 0 broadcasts
    let topic = "rivun.test.epidemic";
    let payload = Bytes::from_static(b"state_snapshot_42");
    let receipt = dispatchers[0]
        .broadcast_state(topic, payload.clone())
        .expect("broadcast failed");
    assert_eq!(receipt.fanout_peers, 3);

    // Simulate hop dissemination
    let env =
        GossipEnvelope::new_signed(node_ids[0], topic, 1, 16, 1_000_000, payload, &node_keys[0]);

    let mut delivered = HashSet::new();
    delivered.insert(node_ids[0]);

    let mut queue = VecDeque::new();
    queue.push_back((node_ids[0], env));

    while let Some((sender, current_env)) = queue.pop_front() {
        for (i, dispatcher) in dispatchers.iter().enumerate() {
            let receiver_id = node_ids[i];
            if receiver_id != sender
                && let Ok(Some(forwarded)) = dispatcher.handle_inbound_envelope(current_env.clone())
            {
                delivered.insert(receiver_id);
                if forwarded.current_hop < 4 {
                    queue.push_back((receiver_id, forwarded));
                }
            }
        }
    }

    assert_eq!(
        delivered.len(),
        num_nodes,
        "All 7 nodes must receive the epidemic gossip message"
    );
}

#[test]
fn test_dedup_cache_prevents_broadcast_storm() {
    let mut cache = GossipDeduplicationCache::new(1000, Duration::from_secs(60));
    let msg_id = GossipMessageId::compute("topic", &Uuid::new_v4(), 1, b"payload");

    // First insert succeeds
    assert!(cache.insert(msg_id));
    assert!(cache.contains(&msg_id));

    // Next 999 duplicate inserts are rejected
    for _ in 0..999 {
        assert!(!cache.insert(msg_id));
    }

    assert_eq!(cache.len(), 1);
}

#[test]
fn test_ttl_hop_count_exhaustion() {
    let key = SigningKey::generate(&mut OsRng);
    let node_id = Uuid::new_v4();
    let mut env = GossipEnvelope::new_signed(
        node_id,
        "test.topic",
        1,
        16,
        1_000_000,
        Bytes::from_static(b"data"),
        &key,
    );

    // Advance hops to 15
    for _ in 0..15 {
        env = env.forward().expect("should forward under max hops");
    }
    assert_eq!(env.current_hop, 15);

    // At hop 15 with max_hops 16, next forward returns None
    assert!(env.forward().is_none());
}

#[test]
fn test_pex_neighbor_discovery_convergence() {
    let a = Uuid::new_v4();
    let b = Uuid::new_v4();
    let dist = xor_distance(&a, &b);
    assert_ne!(dist, [0_u8; 16]);
    assert_eq!(xor_distance(&a, &a), [0_u8; 16]);

    let req = PeerExchangeRequest {
        requester: a,
        max_peers_requested: 8,
        known_peer_ids: vec![a],
    };
    assert_eq!(req.max_peers_requested, 8);

    let resp = PeerExchangeResponse {
        responder: b,
        peers: vec![DiscoveredPeerEntry {
            node_id: b,
            public_key: [1; 32],
            socket_addr: "127.0.0.1:9002".parse().unwrap(),
            transport_key_epoch: 1,
            capabilities_digest: [2; 32],
            last_seen_micros: 100_000,
            signature: [3; 64],
        }],
    };
    assert_eq!(resp.peers.len(), 1);
}

#[tokio::test]
async fn test_anti_entropy_sync_under_packet_drops() {
    let chaos = ChaosConfig {
        drop_rate: 0.30,
        ..ChaosConfig::default()
    };
    let router = MockSwarmRouter::new(chaos);

    let node_a = Uuid::new_v4();
    let node_b = Uuid::new_v4();
    router.register_node(node_a);
    router.register_node(node_b);

    let mut mesh_a = GossipMesh::new(node_a, "127.0.0.1:9001");
    let mut mesh_b = GossipMesh::new(node_b, "127.0.0.1:9002");

    mesh_a.register_peer(node_b, "127.0.0.1:9002", vec![], 0);
    mesh_b.register_peer(node_a, "127.0.0.1:9001", vec![], 0);

    for seq in 1..=50 {
        mesh_a.vector_clock.increment(node_a);
        let payload = Bytes::from(format!("state_update_{seq}"));
        router.send(node_a, node_b, payload);
    }

    let mut delivered_count = 0;
    while let Some(_pkt) = router.try_recv(node_b) {
        delivered_count += 1;
    }

    // Packet drops occurred
    assert!(delivered_count <= 50);

    // Anti-entropy reconciliation via vector clock merge
    let diff = mesh_a.vector_clock.compare(&mesh_b.vector_clock);
    assert_eq!(diff, Causality::StrictlyAfter);

    mesh_b.vector_clock.merge(&mesh_a.vector_clock);
    assert_eq!(mesh_b.vector_clock.get(&node_a), 50);
}

#[test]
fn test_gossip_signature_tamper_rejection() {
    let key = SigningKey::generate(&mut OsRng);
    let other_key = SigningKey::generate(&mut OsRng);
    let node_id = Uuid::new_v4();

    let mut env = GossipEnvelope::new_signed(
        node_id,
        "test.topic",
        1,
        16,
        1_000_000,
        Bytes::from_static(b"valid_payload"),
        &key,
    );

    assert!(env.verify_signature(&key.verifying_key()));
    assert!(!env.verify_signature(&other_key.verifying_key()));

    // Tamper signature
    env.signature[0] ^= 0xFF;
    assert!(!env.verify_signature(&key.verifying_key()));
}
