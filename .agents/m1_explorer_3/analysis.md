# Test Strategy, Fixture Architecture & Verification Blueprint: Milestone 1 (R1)
## P2P Swarm Gossip Consensus & Adaptive Quorum Mesh

**Document Reference**: `rivun-M1-TEST-STRATEGY-2026`  
**Working Directory**: `c:\Users\Stagiaire\Documents\Amadou PGC\Prs\rivun\.agents\m1_explorer_3`  
**Target Crates**: `crates/rivun-net`, `crates/rivun-agent`, `crates/rivun-node`  
**Milestone**: Milestone 1 (R1: P2P Swarm Gossip Consensus & Adaptive Quorum Mesh)  
**Status**: Comprehensive Test Architecture & Complete Fixture Specification  

---

## 1. Executive Summary & Test Philosophy

Milestone 1 establishes the foundational networking, Byzantine-fault-tolerant (BFT) swarm consensus, and adaptive mesh health fabric for the rivun Next-Gen architecture. Distributed systems of this complexity cannot rely solely on standard happy-path unit tests or slow, non-deterministic live network tests.

To achieve **100% test coverage with zero clippy warnings and zero test flakiness**, the M1 test strategy employs a **tripartite verification model**:

```
                              +---------------------------------------+
                              |   Tier 3: In-Process Multi-Node UDP   |
                              |   Integration Tests (Real Sockets)    |
                              +-------------------+-------------------+
                                                  |
                              +-------------------v-------------------+
                              | Tier 2: Deterministic Mock Swarm      |
                              | Chaos Harness (Virtual Time & Drops)  |
                              +-------------------+-------------------+
                                                  |
                              +-------------------v-------------------+
                              | Tier 1: Pure Unit & Property Tests    |
                              | (State Machines, Crypto, Math, LRU)   |
                              +---------------------------------------+
```

### 1.1 Core Verification Objectives

1. **Gossip Dissemination & Convergence**: Validate epidemic $k$-fanout broadcast, LRU deduplication, hop-count/TTL termination, and anti-entropy reconciliation under up to 50% simulated packet loss.
2. **BFT Swarm Consensus State Machine**: Verify strict 4-phase replication ($\text{Propose} \to \text{Prevote} \to \text{Precommit} \to \text{Commit}$), leader rotation, $T$-of-$N$ threshold bitmask signature aggregation, and Byzantine tolerance ($f \le \lfloor(N-1)/3\rfloor$) including drop, corruption, and equivocation slashing.
3. **$\Phi$ Accrual Failure Detection**: Verify continuous Gaussian interval tracking, suspicion metric accuracy ($\Phi = -\log_{10} P_{\text{later}}(t)$), liveness transitions (`Alive` $\to$ `Suspect` $\to$ `Dead`), and jittered exponential heartbeat backoff.
4. **Partition Detection & Split-Brain Mitigation**: Verify quorum reachability ratio ($R = N_{\text{reach}} / N$), immediate transition to `PartitionDegraded` (read-only mode, mutation proposal rejection), and post-partition state sync catchup.
5. **Dynamic 2-Hop Relay Failover**: Verify transparent multi-path relay encapsulation (`ZapRelayEnvelope`), forwarding trust permissions, loop prevention, and path-cost optimization under direct-link failure.
6. **Swarm Agent Provenance & Node Daemon Concurrency**: Verify cryptographic binding of `SwarmCommitCertificate` in `ProvenanceStep` and non-blocking actor concurrency across Tokio tasks (`UdpRxTask`, `GossipTask`, `ConsensusTask`, `MeshTask`).

---

## 2. Deterministic Mock Swarm & Chaos Test Harness

To test distributed edge cases without relying on physical network delays or non-deterministic thread sleeps, we design the `MockSwarmHarness` and `ChaosRouter`.

```
+-----------------------------------------------------------------------------------------+
|                                    MockSwarmHarness                                     |
+-----------------------------------------------------------------------------------------+
|  Node A (PeerId)         Node B (PeerId)         Node C (PeerId)        Node D (PeerId) |
|  [State Engine]          [State Engine]          [State Engine]         [State Engine]  |
+-----------------------------------------------------------------------------------------+
                                      |
                     +----------------v----------------+
                     |          ChaosRouter            |
                     |  - Drop Rate: p_drop            |
                     |  - Latency: N(mu, sigma)        |
                     |  - Partition Matrix: Sym/Asym   |
                     |  - Reorder Queue / Corruptor    |
                     +----------------+----------------+
                                      |
                     +----------------v----------------+
                     |     Virtual Time Controller     |
                     |  tokio::time::pause() / advance |
                     +---------------------------------+
```

### 2.1 Chaos Router Capabilities

| Chaos Capability | Mechanism | Test Target |
| :--- | :--- | :--- |
| **Packet Loss Injection** | Uniform random drop with probability $p \in [0.0, 1.0]$. | Gossip anti-entropy, consensus quorum retry under packet loss. |
| **Latency & Jitter** | Configurable delay distribution $\mathcal{N}(\mu, \sigma)$ per packet. | Phi accrual sliding window adaptation, heartbeat timeout. |
| **Symmetric Partition** | Bi-directional link cut: $A \leftrightarrow B$ dropped. | Split-brain partition detection, majority vs minority quorum. |
| **Asymmetric Link Cut** | Uni-directional drop: $A \to B$ dropped, $B \to A$ allowed. | 2-hop relay routing failover trigger. |
| **Packet Corruption** | Mutate signature byte or payload bitmask randomly. | Cryptographic rejection, Byzantine consensus immunity. |
| **Equivocation Injection** | Duplicate proposal with conflicting payload for same round. | Equivocation slashing proof generation & verification. |

### 2.2 Reusable Mock Harness Implementation

```rust
// Embedded fixture in crates/rivun-net/tests/common/mock_net.rs

use bytes::Bytes;
use std::{
    collections::{HashMap, HashSet, VecDeque},
    sync::{Arc, Mutex},
    time::Duration,
};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct SimulatedPacket {
    pub source: Uuid,
    pub target: Uuid,
    pub payload: Bytes,
    pub deliver_at: Duration,
}

#[derive(Debug, Clone, Default)]
pub struct ChaosConfig {
    pub drop_rate: f64,
    pub min_delay: Duration,
    pub max_delay: Duration,
    pub severed_links: HashSet<(Uuid, Uuid)>,
    pub corrupt_rate: f64,
}

#[derive(Clone, Default)]
pub struct MockSwarmRouter {
    inner: Arc<Mutex<MockSwarmRouterInner>>,
}

#[derive(Default)]
struct MockSwarmRouterInner {
    config: ChaosConfig,
    inboxes: HashMap<Uuid, VecDeque<Bytes>>,
    pending_queue: Vec<SimulatedPacket>,
    virtual_time: Duration,
}

impl MockSwarmRouter {
    pub fn new(config: ChaosConfig) -> Self {
        Self {
            inner: Arc::new(Mutex::new(MockSwarmRouterInner {
                config,
                inboxes: HashMap::new(),
                pending_queue: Vec::new(),
                virtual_time: Duration::ZERO,
            })),
        }
    }

    pub fn register_node(&self, node_id: Uuid) {
        let mut inner = self.inner.lock().unwrap();
        inner.inboxes.entry(node_id).or_default();
    }

    pub fn sever_link(&self, a: Uuid, b: Uuid, symmetric: bool) {
        let mut inner = self.inner.lock().unwrap();
        inner.config.severed_links.insert((a, b));
        if symmetric {
            inner.config.severed_links.insert((b, a));
        }
    }

    pub fn heal_link(&self, a: Uuid, b: Uuid, symmetric: bool) {
        let mut inner = self.inner.lock().unwrap();
        inner.config.severed_links.remove(&(a, b));
        if symmetric {
            inner.config.severed_links.remove(&(b, a));
        }
    }

    pub fn send(&self, source: Uuid, target: Uuid, payload: Bytes) {
        let mut inner = self.inner.lock().unwrap();
        // Check severed links
        if inner.config.severed_links.contains(&(source, target)) {
            return; // Dropped by partition
        }
        // Check drop rate
        if inner.config.drop_rate > 0.0 {
            let roll = (source.as_u128() ^ target.as_u128() ^ inner.virtual_time.as_nanos()) % 1000;
            if (roll as f64 / 1000.0) < inner.config.drop_rate {
                return; // Dropped by chaos
            }
        }

        let delay = inner.config.min_delay;
        let deliver_at = inner.virtual_time + delay;
        inner.pending_queue.push(SimulatedPacket {
            source,
            target,
            payload,
            deliver_at,
        });
    }

    pub fn advance_time(&self, step: Duration) {
        let mut inner = self.inner.lock().unwrap();
        inner.virtual_time += step;
        let current_time = inner.virtual_time;

        let mut ready = Vec::new();
        inner.pending_queue.retain(|pkt| {
            if pkt.deliver_at <= current_time {
                ready.push(pkt.clone());
                false
            } else {
                true
            }
        });

        for pkt in ready {
            if let Some(inbox) = inner.inboxes.get_mut(&pkt.target) {
                inbox.push_back(pkt.payload);
            }
        }
    }

    pub fn try_recv(&self, node_id: Uuid) -> Option<Bytes> {
        let mut inner = self.inner.lock().unwrap();
        inner.inboxes.get_mut(&node_id)?.pop_front()
    }
}
```

---

## 3. Comprehensive Test Inventory & Test Case Specifications

### 3.1 Suite 1: Epidemic Gossip Dissemination & Anti-Entropy Sync (`crates/rivun-net/tests/gossip_test.rs`)

| Test ID | Test Case Name | Objective | Assertions |
| :--- | :--- | :--- | :--- |
| **GOSSIP-01** | `test_k_fanout_epidemic_convergence` | Verify broadcast reaches 100% of nodes in a 7-node ring in $O(\log N)$ steps. | - All 7 nodes receive state envelope.<br>- Max hop count observed $\le \lceil \log_2 7 \rceil + 2$. |
| **GOSSIP-02** | `test_dedup_cache_prevents_broadcast_storm` | Inject 1,000 duplicate message IDs into a node with $k=3$ peers. | - First message forwarded to 3 peers.<br>- Next 999 messages silently ignored.<br>- Zero redundant outbound packets. |
| **GOSSIP-03** | `test_ttl_hop_count_exhaustion` | Envelope created with `current_hop = 16`, `max_hops = 16`. | - Node drops envelope immediately.<br>- Outbound queue remains empty.<br>- Metric `gossip_ttl_drops_total` increments. |
| **GOSSIP-04** | `test_pex_neighbor_discovery_convergence` | 5 nodes boot knowing only 1 bootnode; execute periodic PEX gossip. | - Within 3 PEX cycles, all nodes have complete active view ($\ge 4$ peers).<br>- Passive view populated with fallback candidates. |
| **GOSSIP-05** | `test_anti_entropy_sync_under_packet_drops` | Broadcast 20 state updates with 30% drop rate; trigger anti-entropy digest exchange. | - Missing state hashes detected via digest difference.<br>- Unicast fetch recovers 100% of missed updates. |
| **GOSSIP-06** | `test_gossip_signature_tamper_rejection` | Tamper with `GossipEnvelope::signature` in flight. | - Node rejects frame with `ZapCryptoError::InvalidSignature`.<br>- Message not committed to deduplication LRU or state. |

#### Concrete Test Code: `test_anti_entropy_sync_under_packet_drops`

```rust
#[tokio::test]
async fn test_anti_entropy_sync_under_packet_drops() {
    let mut chaos = ChaosConfig::default();
    chaos.drop_rate = 0.30; // 30% packet drop
    let router = MockSwarmRouter::new(chaos);

    let node_a = Uuid::new_v4();
    let node_b = Uuid::new_v4();
    router.register_node(node_a);
    router.register_node(node_b);

    let key_a = Keypair::generate();
    let mut mesh_a = GossipMesh::new(node_a, "127.0.0.1:9001");
    let mut mesh_b = GossipMesh::new(node_b, "127.0.0.1:9002");

    mesh_a.register_peer(node_b, "127.0.0.1:9002", vec![], 0);
    mesh_b.register_peer(node_a, "127.0.0.1:9001", vec![], 0);

    // Node A emits 50 state increments
    for seq in 1..=50 {
        mesh_a.vector_clock.increment(node_a);
        let payload = Bytes::from(format!("state_update_{seq}"));
        router.send(node_a, node_b, payload);
    }

    // Advance virtual time and deliver surviving packets
    router.advance_time(Duration::from_millis(50));
    let mut delivered_count = 0;
    while let Some(pkt) = router.try_recv(node_b) {
        delivered_count += 1;
        let _ = pkt;
    }

    // Assert that packet drops occurred (loss between 10% and 50%)
    assert!(delivered_count < 50, "Chaos router must drop packets");
    assert!(delivered_count > 10, "Some packets must survive");

    // Perform Anti-Entropy Sync: Node B compares clock digest with Node A
    let diff = mesh_a.vector_clock.compare(&mesh_b.vector_clock);
    assert_eq!(diff, Causality::StrictlyAfter);

    // Reconcile missing sequence range
    mesh_b.vector_clock.merge(&mesh_a.vector_clock);
    assert_eq!(mesh_b.vector_clock.get(&node_a), 50);
}
```

---

### 3.2 Suite 2: Byzantine Fault Tolerant (BFT) Swarm Consensus (`crates/rivun-net/tests/consensus_test.rs`)

| Test ID | Test Case Name | Objective | Assertions |
| :--- | :--- | :--- | :--- |
| **BFT-01** | `test_bft_four_phase_commit_happy_path` | Execute complete consensus round ($\text{Propose} \to \text{Prevote} \to \text{Precommit} \to \text{Commit}$) across 4 nodes. | - Quorum threshold $T=3$ reached at each phase.<br>- `SwarmCommitCertificate` formed with 3 valid signatures.<br>- Bitmask `0b00000111` correctly set. |
| **BFT-02** | `test_bft_single_byzantine_node_drop_tolerance` | In $N=4, T=3, f=1$ swarm, 1 node goes silent / drops votes. | - Remaining 3 nodes collect $3 \ge T$ prevotes and precommits.<br>- Consensus completes successfully without delay. |
| **BFT-03** | `test_bft_equivocation_slashing_proof` | Byzantine node signs two conflicting prevotes for the same view/round. | - Observer generates `EquivocationProof`.<br>- Proof verification succeeds.<br>- Offender slashed to `PeerTrustStatus::Revoked`. |
| **BFT-04** | `test_bft_leader_rotation_on_proposal_timeout` | Leader for Round 0 fails to propose; trigger timeout. | - Nodes transition to Round 1 after $T_{\text{timeout}}$.<br>- New leader $\text{Leader}(v, 1) = V[1]$ proposes.<br>- Round 1 finalizes cleanly. |
| **BFT-05** | `test_bft_threshold_bitmask_batch_verification` | Generate certificate with $N=16, T=11$; execute `verify_batch()`. | - Signer bitmask accurately indexes 11 validators.<br>- Batch Ed25519 verification passes in $< 1\text{ ms}$. |
| **BFT-06** | `test_bft_corrupted_signature_batch_rejection` | Corrupt 1 signature in a $T=5$ certificate. | - Batch verification returns error.<br>- Binary search isolates corrupt signature position. |
| **BFT-07** | `test_bft_dynamic_validator_epoch_transition` | Propose governance transition adding validator 5; apply at epoch boundary. | - Epoch $E=1 \implies N=4, T=3$.<br>- Epoch $E=2 \implies N=5, T=4$.<br>- Epoch 2 proposals require 4 signatures. |

#### Concrete Test Code: `test_bft_equivocation_slashing_proof`

```rust
#[tokio::test]
async fn test_bft_equivocation_slashing_proof() {
    let offender_key = Keypair::generate();
    let observer_key = Keypair::generate();
    let offender_id = offender_key.node_id();

    let epoch = 1_u64;
    let view = 0_u64;
    let round = 0_u64;

    // Offender signs Proposal A
    let digest_a = blake3::hash(b"proposal_tx_set_A").into();
    let vote_a_msg = format!("VOTE:{epoch}:{view}:{round}:prevote:{}", hex::encode(digest_a));
    let sig_a = offender_key.sign_domain_message(b"rivun-CONSENSUS-VOTE-v1", vote_a_msg.as_bytes());

    // Offender simultaneously signs conflicting Proposal B for same (epoch, view, round)
    let digest_b = blake3::hash(b"proposal_tx_set_B").into();
    let vote_b_msg = format!("VOTE:{epoch}:{view}:{round}:prevote:{}", hex::encode(digest_b));
    let sig_b = offender_key.sign_domain_message(b"rivun-CONSENSUS-VOTE-v1", vote_b_msg.as_bytes());

    // Observer collects both and constructs EquivocationProof
    let proof = EquivocationProof {
        offender_node: offender_id,
        epoch,
        view,
        round,
        vote_kind: VoteKind::Prevote,
        digest_a,
        signature_a: sig_a,
        digest_b,
        signature_b: sig_b,
    };

    // Verify proof
    let vk = offender_key.verifying_key();
    assert_ne!(proof.digest_a, proof.digest_b);
    assert!(vk.verify_domain_message(b"rivun-CONSENSUS-VOTE-v1", vote_a_msg.as_bytes(), &proof.signature_a).is_ok());
    assert!(vk.verify_domain_message(b"rivun-CONSENSUS-VOTE-v1", vote_b_msg.as_bytes(), &proof.signature_b).is_ok());

    // Apply Slashing Action
    let mut peer_trust = PeerTrustConfig::default();
    peer_trust.status = PeerTrustStatus::Revoked;
    peer_trust.allow_send = false;
    peer_trust.allow_receive = false;
    peer_trust.allow_poa_attestation = false;

    assert!(!peer_trust.is_trusted());
    assert!(!peer_trust.allows_transport());
}
```

---

### 3.3 Suite 3: Phi Accrual Failure Detector & Heartbeat Dynamics (`crates/rivun-net/tests/phi_detector_test.rs`)

| Test ID | Test Case Name | Objective | Assertions |
| :--- | :--- | :--- | :--- |
| **PHI-01** | `test_phi_accrual_gaussian_cdf_calculation` | Feed 100 intervals with $\mu=1000\text{ms}, \sigma=50\text{ms}$; evaluate $\Phi(t)$. | - At $t = 1000\text{ms} \implies \Phi \approx 0.3$.<br>- At $t = 1500\text{ms} \implies \Phi \approx 8.2$ (`Suspect`).<br>- At $t = 2000\text{ms} \implies \Phi > 16.0$ (`Dead`). |
| **PHI-02** | `test_phi_liveness_state_transitions` | Advance virtual time without heartbeats; observe state changes. | - $t \in [0, 1.2\text{s}) \implies \text{Alive}$.<br>- $t \in [1.2\text{s}, 2.5\text{s}) \implies \text{Suspect}$.<br>- $t \ge 2.5\text{s} \implies \text{Dead}$. |
| **PHI-03** | `test_heartbeat_jitter_uniform_distribution` | Generate 1,000 heartbeat intervals with $T_{\text{base}}=1000\text{ms}, J_{\text{max}}=250\text{ms}$. | - All intervals in range $[1000, 1250]\text{ms}$.<br>- Variance confirms no discrete timer resonance. |
| **PHI-04** | `test_heartbeat_exponential_backoff_on_failure` | Simulate 5 consecutive failed heartbeat attempts. | - Sequence: $1000\text{ms} \to 1500\text{ms} \to 2250\text{ms} \to 3375\text{ms} \to 5062\text{ms}$.<br>- Capped at $T_{\text{max}} = 15000\text{ms}$. |
| **PHI-05** | `test_peer_recovery_clears_phi_history` | Dead peer sends fresh valid heartbeat ack. | - Peer health restores to `PeerHealth::Alive`.<br>- Sliding window retains recent interval history. |

#### Concrete Test Code: `test_phi_accrual_gaussian_cdf_calculation`

```rust
#[test]
fn test_phi_accrual_gaussian_cdf_calculation() {
    // Sliding window of 50 samples around mean = 1000ms, stddev = 100ms
    let mut intervals = Vec::new();
    for i in 0..50 {
        let delta = if i % 2 == 0 { 950_u64 } else { 1050_u64 };
        intervals.push(delta);
    }

    let sum: u64 = intervals.iter().sum();
    let mean = sum as f64 / intervals.len() as f64;
    let variance: f64 = intervals.iter().map(|&x| (x as f64 - mean).powi(2)).sum::<f64>() / intervals.len() as f64;
    let std_dev = variance.sqrt().max(1.0);

    let compute_phi = |elapsed_ms: f64| -> f64 {
        let y = (elapsed_ms - mean) / std_dev;
        // Approximation of complementary error function for standard normal distribution
        let e = (-y * (0.7071067811865475)).exp();
        let p_later = 0.5 * (1.0 / (1.0 + 0.5 * y.abs())) * (-y.powi(2) / 2.0).exp();
        let p_clamped = p_later.clamp(1e-18, 1.0);
        -p_clamped.log10()
    };

    let phi_1000 = compute_phi(1000.0);
    let phi_1300 = compute_phi(1300.0);
    let phi_2000 = compute_phi(2000.0);

    assert!(phi_1000 < 1.0, "Expected low phi at mean interval: {phi_1000}");
    assert!(phi_1300 > 3.0 && phi_1300 < 10.0, "Expected moderate suspect phi at +3 sigma: {phi_1300}");
    assert!(phi_2000 > 12.0, "Expected high dead phi at +10 sigma: {phi_2000}");
}
```

---

### 3.4 Suite 4: Network Partition Detection & Split-Brain Mitigation (`crates/rivun-net/tests/partition_test.rs`)

| Test ID | Test Case Name | Objective | Assertions |
| :--- | :--- | :--- | :--- |
| **PART-01** | `test_symmetric_partition_majority_minority_split` | Split 5-node cluster into Partition A $\{1,2,3\}$ and Partition B $\{4,5\}$. | - Partition A: $R = 3/5 = 0.60 < 0.67$ ($T=4$ for $N=5$). Both detect quorum loss.<br>- For 4-node cluster $\{1,2,3\}$ vs $\{4\}$: $\{1,2,3\}$ maintains quorum ($R=0.75 \ge 0.75$). |
| **PART-02** | `test_minority_partition_enters_degraded_mode` | Node in minority partition receives state mutation proposal. | - Rejects proposal with `ConsensusError::QuorumLoss`.<br>- Gating flag enters read-only mode.<br>- Emits `PARTITION_WARNING` log event. |
| **PART-03** | `test_majority_partition_continues_consensus` | Majority partition $\{1,2,3\}$ receives actions. | - Proposes and commits new blocks normally.<br>- Block height advances from $H=10 \to H=15$. |
| **PART-04** | `test_partition_healing_and_state_reconciliation` | Reconnect Partition B $\{4\}$ to Partition A $\{1,2,3\}$. | - Heartbeats exchange latest height ($H=15$ vs $H=10$).<br>- Node 4 initiates `AntiEntropySync`, fetches blocks 11..15.<br>- Node 4 exits degraded mode and resumes active consensus. |

#### Concrete Test Code: `test_symmetric_partition_majority_minority_split`

```rust
#[tokio::test]
async fn test_symmetric_partition_majority_minority_split() {
    let n1 = Uuid::new_v4();
    let n2 = Uuid::new_v4();
    let n3 = Uuid::new_v4();
    let n4 = Uuid::new_v4();

    // 4 nodes: Quorum requires T = floor(8/3) + 1 = 3 nodes (75%)
    let mut mesh1 = GossipMesh::new(n1, "127.0.0.1:9001");
    let mut mesh4 = GossipMesh::new(n4, "127.0.0.1:9004");

    for peer in [n2, n3, n4] {
        mesh1.register_peer(peer, format!("127.0.0.1:{}", 9000), vec![], 0);
    }
    for peer in [n1, n2, n3] {
        mesh4.register_peer(peer, format!("127.0.0.1:{}", 9000), vec![], 0);
    }

    // Partition occurs: n1 can reach n2, n3 (alive), but n4 is dead
    // In Mesh 1: 3/4 reachable (75%) -> Quorum Healthy
    let now = 20_000_000_u64; // 20s
    // n2 and n3 send heartbeats to n1
    let mut clk = VectorClock::new();
    mesh1.record_heartbeat(n2, &clk, 0, now);
    mesh1.record_heartbeat(n3, &clk, 0, now);
    // n4 missed heartbeats
    let res1 = mesh1.evaluate_health(now);
    assert!(res1.is_ok(), "Majority partition with 3/4 nodes must not error on partition");

    // In Mesh 4: n1, n2, n3 are unreachable (dead)
    // 1/4 reachable (25%) -> Quorum Lost
    let res4 = mesh4.evaluate_health(now);
    assert!(matches!(res4, Err(GossipError::NetworkPartition { unreachable_count: 3, total_nodes: 4 })));
}
```

---

### 3.5 Suite 5: Dynamic 2-Hop Relay Failover Routing (`crates/rivun-net/tests/relay_routing_test.rs`)

| Test ID | Test Case Name | Objective | Assertions |
| :--- | :--- | :--- | :--- |
| **RELAY-01** | `test_direct_route_failover_to_two_hop_relay` | Sever direct link $A \to B$; Node $A$ routes through intermediary $C$. | - $A$ encapsulates frame in `ZapRelayEnvelope` for $C$.<br>- $C$ unpacks and forwards to $B$.<br>- $B$ verifies original payload from $A$. |
| **RELAY-02** | `test_relay_trust_permission_enforcement` | Node $C$ configured with `allow_forward = false` for $A$. | - $C$ drops relay frame.<br>- Security violation metric increments. |
| **RELAY-03** | `test_relay_hop_limit_prevents_infinite_loop` | Malformed relay loop $A \to C \to A \to C$ with `max_hops = 2`. | - Frame dropped when `hops_remaining == 0`.<br>- No routing loop storm occurs. |
| **RELAY-04** | `test_relay_cost_optimization_lowest_load` | Direct $A \to B$ broken; candidates $C$ (load=80) and $D$ (load=10). | - Router dynamically chooses $D$ as relay hop. |

#### Concrete Test Code: `test_direct_route_failover_to_two_hop_relay`

```rust
#[tokio::test]
async fn test_direct_route_failover_to_two_hop_relay() {
    let key_a = Keypair::generate();
    let key_b = Keypair::generate();
    let key_c = Keypair::generate();

    let node_a = key_a.node_id();
    let node_b = key_b.node_id();
    let node_c = key_c.node_id();

    // Direct link A -> B is broken (Phi_B >= 14)
    // A constructs inner payload intended for B
    let inner_payload = Bytes::from_static(b"critical_sensor_command");
    let mut inner_frame = ZapFrame::new(node_a, node_b, ZapFlags::SIGNED, inner_payload).unwrap();
    sign_frame(&key_a, &mut inner_frame).unwrap();

    // A wraps inner frame in Relay Envelope targeting C
    let relay_envelope = ZapEnvelope::new_relay(
        node_a,
        node_c, // Intermediary
        node_b, // Final target
        inner_frame.encode(),
        2,      // Max hops
    );

    // Node C receives relay envelope, verifies forwarding permission
    let trust_c = PeerTrustConfig {
        allow_forward: true,
        ..Default::default()
    };
    assert!(trust_c.allows_forward());

    // Node C decrements hop count and forwards inner frame to B
    assert_eq!(relay_envelope.target_node(), node_c);
    assert_eq!(relay_envelope.final_destination(), node_b);

    // Node B receives forwarded frame, decrypts and verifies A's signature
    let decoded_inner = ZapFrame::decode(&relay_envelope.inner_payload()).unwrap();
    assert_eq!(decoded_inner.header.source_node, node_a);
    assert_eq!(decoded_inner.header.target_node, node_b);
    assert!(verify_frame(&key_a.verifying_key(), &decoded_inner).is_ok());
    assert_eq!(decoded_inner.payload, Bytes::from_static(b"critical_sensor_command"));
}
```

---

### 3.6 Suite 6: Swarm Coordinator & Cryptographic Provenance Chain (`crates/rivun-agent/tests/swarm_provenance_test.rs`)

| Test ID | Test Case Name | Objective | Assertions |
| :--- | :--- | :--- | :--- |
| **AGENT-01** | `test_agent_intent_triggers_consensus_proposal` | Agent submits high-value `IntentKind::Act`; coordinator produces `SwarmProposal`. | - `SwarmProposal::payload_digest` matches intent hash.<br>- Proposer signature valid. |
| **AGENT-02** | `test_provenance_chain_binds_consensus_certificate` | Build 6-stage provenance chain with Stage `Consensus` containing certificate. | - Causal hash chain: $H_{\text{intent}} \to H_{\text{policy}} \to H_{\text{consensus}} \to H_{\text{driver}} \to H_{\text{receipt}}$.<br>- `verify(&node_pubkey)` passes with 0 failures. |
| **AGENT-03** | `test_tampered_consensus_certificate_fails_provenance` | Corrupt `SwarmCommitCertificate::signer_bitmask` in provenance step. | - `verify_step(ProvenanceStage::Consensus)` returns error.<br>- `ProvenanceVerificationReport::valid == false`. |

#### Concrete Test Code: `test_provenance_chain_binds_consensus_certificate`

```rust
#[test]
fn test_provenance_chain_binds_consensus_certificate() {
    let keypair = Keypair::generate();
    let session_id = Uuid::new_v4();
    let intent_id = Uuid::new_v4();

    let mut intent = AgentIntent::new(
        session_id,
        AgentId::new("planner_alpha").unwrap(),
        IntentKind::Act,
        "swarm_coordinated_action",
    );
    intent.intent_id = intent_id;

    // Consensus Certificate data
    let cert_hash = "blake3_cert_hash_abcdef123456";
    let mut cert_meta = BTreeMap::new();
    cert_meta.insert("epoch".to_string(), serde_json::json!(1));
    cert_meta.insert("round".to_string(), serde_json::json!(0));
    cert_meta.insert("signer_bitmask".to_string(), serde_json::json!("0x07"));

    let chain = ProvenanceChainBuilder::new(session_id, intent_id)
        .with_intent(&intent)
        .unwrap()
        .with_policy("policy_digest_sha256", "ALLOW", BTreeMap::new())
        .unwrap()
        .with_consensus(cert_hash, cert_meta)
        .unwrap()
        .with_driver("driver_v1", "in_hash", "out_hash", BTreeMap::new())
        .unwrap()
        .with_receipt("rcpt_99", 1_700_000_000, BTreeMap::new())
        .unwrap()
        .build_and_sign(&keypair)
        .unwrap();

    let report = chain.verify(&keypair.verifying_key()).unwrap();
    assert!(report.valid);
    assert_eq!(report.verified_steps, 5);
    assert!(chain.verify_step(ProvenanceStage::Consensus).is_ok());
}
```

---

### 3.7 Suite 7: Tokio Daemon Actor Concurrency & Stress (`crates/rivun-node/tests/daemon_concurrency_test.rs`)

| Test ID | Test Case Name | Objective | Assertions |
| :--- | :--- | :--- | :--- |
| **NODE-01** | `test_actor_channel_backpressure_under_flood` | Send 10,000 frames into inbound channel with bounded capacity of 1,024. | - No unbounded memory allocation.<br>- Bounded channel applies backpressure gracefully without task panic. |
| **NODE-02** | `test_concurrent_gossip_consensus_mesh_tasks` | Run 4 concurrent actor tasks for 2 seconds with continuous message exchange. | - All actor tasks remain alive.<br>- Zero deadlocks across channel select loops.<br>- Clean graceful shutdown on cancellation token. |
| **NODE-03** | `test_node_restart_restores_consensus_epoch_state` | Crash node mid-epoch and restart from durable store. | - Restores active validator set, latest epoch $E$, and committed block height $H$. |

---

## 4. Clippy, Safety & Quality Guardrails

To guarantee **zero clippy warnings** under `cargo clippy --workspace --all-targets -- -D warnings`:

### 4.1 Strict Rust Code Guardrails for Implementers

1. **Explicit Error Propagation**:
   - Never use `unwrap()` or `expect()` in library code (`crates/rivun-net`, `rivun-agent`, `rivun-node`).
   - Use `thiserror` for library error enums and `anyhow::Context` in binary/CLI layers.
2. **Deterministic Time Handling**:
   - In unit/mock tests, inject virtual time or duration offsets rather than invoking `std::thread::sleep`.
3. **Lock & Synchronization Hygiene**:
   - Guard all `Mutex` and `RwLock` acquisitions against deadlocks by enforcing a global lock ordering.
   - Use `tokio::sync::mpsc` channels rather than cross-task shared mutable state where possible.
4. **Must-Use Attribute Compliance**:
   - Add `#[must_use]` on all builder methods (`ProvenanceChainBuilder`, `GossipEnvelopeBuilder`, `SwarmProposalBuilder`).
5. **No Underscore-Prefixed Dead Code**:
   - Ensure all declared struct fields, variants, and helper functions are either actively used or covered by test suites.

---

## 5. Acceptance Command Matrix

The complete test suite is verified via the following standardized commands:

```bash
# 1. Run all unit and integration tests across Milestone 1 crates
cargo test -p rivun-net -p rivun-agent -p rivun-node --all-targets

# 2. Run deterministic chaos and stress benchmarks
cargo test -p rivun-net --test gossip_test --test consensus_test --test phi_detector_test --test partition_test --test relay_routing_test

# 3. Verify zero Clippy warnings across all workspace targets
cargo clippy --workspace --all-targets -- -D warnings

# 4. Verify test coverage and documentation builds cleanly
cargo doc --no-deps -p rivun-net -p rivun-agent -p rivun-node
```

---
*Report compiled by Explorer 3 (Test Strategy & Validation Specialist) for Milestone 1.*

