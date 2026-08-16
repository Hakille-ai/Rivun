# Implementation Blueprint & Technical Architecture: `crates/zap-net`
## Milestone 1 (R1) — P2P Swarm Gossip Consensus & Adaptive Quorum Mesh

**Document Reference**: `ZAP-NET-M1-BLUEPRINT-2026`  
**Target Crate**: `crates/zap-net` (with integration bindings for `crates/zap-agent`, `crates/zap-node`, `crates/zap-crypto`, `crates/zap-core`)  
**Author**: Explorer 1 (Milestone 1)  
**Status**: Comprehensive Technical Specification & Implementation Plan  

---

## 1. Executive Summary & Problem Scope

The **Milestone 1 (R1)** objective is to engineer a resilient, hyper-scalable, decentralized P2P networking and consensus engine in `crates/zap-net`. This engine transforms ZAP from a static, point-to-point UDP transport into an autonomous multi-agent swarm fabric with epidemic state broadcast, Byzantine Fault Tolerant (BFT) swarm consensus, adaptive mesh failure detection, and automatic 2-hop failover relay routing.

### Scope Breakdown & Architecture
```
                                     +-------------------------------------------------------------+
                                     |                     crates/zap-net                          |
                                     +-------------------------------------------------------------+
                                     |                                                             |
                 +-------------------+-----------------------------+-------------------------------+
                 |                                                 |                               |
  +--------------v--------------+                   +--------------v--------------+  +-------------v---------------+
  |       src/gossip/           |                   |      src/consensus/         |  |         src/mesh/           |
  | - GossipEnvelope ('ZGSP')   |                   | - 2-Phase BFT State Machine |  | - Phi Accrual Detector      |
  | - Epidemic Fanout Dispatcher|                   | - SwarmProposal / SwarmVote |  | - Jittered Heartbeats       |
  | - Deduplication Cache (LRU) |                   | - SwarmCommitCert ('ZSC1')  |  | - Partition Detection       |
  | - Peer Exchange (PEX)       |                   | - Bitmask Signer Indexing   |  | - Dynamic 2-Hop Relay Router|
  | - Anti-Entropy Sync Digest  |                   | - Batch Ed25519 Verify      |  | - MeshTopology Health Engine|
  +--------------+--------------+                   | - Dynamic Validator Sets    |  +-------------+---------------+
                 |                                  | - Equivocation Slashing     |                |
                 |                                  +--------------+--------------+                |
                 +-------------------------------------------------+-------------------------------+
                                                                   |
                                                    +--------------v--------------+
                                                    |     ZapEndpoint Integration |
                                                    |  (src/lib.rs & src/peer.rs) |
                                                    | - Encrypted UDP Transport   |
                                                    | - Nonce Replay WAL Journal  |
                                                    | - Wire Frame Encapsulation  |
                                                    +-----------------------------+
```

---

## 2. Existing Codebase Audit & Backward Compatibility Guardrails

### 2.1 Existing Structure in `crates/zap-net`
1. **`src/lib.rs` (1,140 lines)**:
   - Contains `ZapEndpoint`, `ZapEndpointConfig`, `Peer`, `TransportKey`, `InboundZap`, `DatagramEnvelope`, `NonceReplayCache`, Noise handshake helper (`noise::NoiseHandshake`).
   - Uses ChaCha20-Poly1305 AEAD over UDP with 52-byte header (`ZAPD` magic, version 1, source/target UUIDs, 12-byte nonce).
   - Re-exports `durable_replay` and legacy placeholder `gossip`.
2. **`src/durable_replay.rs` (215 lines)**:
   - Binary Write-Ahead Log (`ZAPNONC1`) for persistent anti-replay nonce tracking across node restarts.
3. **`src/gossip.rs` (397 lines)**:
   - Initial prototype containing `VectorClock`, `Causality`, `PeerHealth`, `SwarmPeer`, `QuorumProposal`, and `GossipMesh`.
4. **Existing Test Suite**:
   - 22 unit tests in `src/lib.rs` and `src/gossip.rs`.
   - 5 stress tests in `tests/durable_replay_stress.rs` (10,000+ nonce flood, crash recovery, clock jumps, compaction, concurrency).
   - Benchmark in `benches/round_trip.rs`.

### 2.2 Backward Compatibility Requirements
- All existing public symbols in `zap-net` root (`ZapEndpoint`, `ZapEndpointConfig`, `Peer`, `TransportKey`, `InboundZap`, `MAX_DATAGRAM_SIZE`, `ZapNetError`) **must remain strictly unchanged** in signature and behavior.
- Existing tests across `zap-cli`, `zap-node`, `zap-agent`, and `tests/e2e` must continue to pass without modification.
- New modules will be exposed under `zap_net::gossip`, `zap_net::consensus`, and `zap_net::mesh`. Legacy types (`VectorClock`, `PeerHealth`, `QuorumProposal`, `GossipMesh`) will be migrated/aliased so no downstream imports break.

---

## 3. Module Blueprint: Decentralized Epidemic Gossip Subsystem (`src/gossip/`)

### 3.1 Mathematical Dissemination Model
1. **Epidemic $k$-Fanout**:
   For any broadcast wave on topic $T$, node $u$ randomly selects $k$ peers from its Active View ($k_{\text{active}} \in [6, 8]$):
   $$k = \min\left(k_{\text{active}}, \max\left(3, \lceil \log_2 N \rceil + 1\right)\right)$$
2. **Message Identification & Deduplication**:
   $$M_{\text{id}} = \text{Blake3}(\text{"ZAP-GOSSIP-MSG-v1"} \parallel \text{topic} \parallel \text{origin\_node} \parallel \text{seq} \parallel \text{payload\_digest})$$
   - Cache capacity: 65,536 message IDs with 60-second sliding TTL window.
3. **Hop Count & TTL Damping**:
   - `max_hops = 16`, `current_hop` incremented at each hop. If `current_hop >= max_hops`, the envelope is dropped immediately to prevent network amplification loops.

### 3.2 File Layout for `src/gossip/`
```
crates/zap-net/src/gossip/
├── mod.rs             // Module exports, constants, GossipError taxonomy
├── envelope.rs        // GossipEnvelope wire struct, signing, verification, hop management
├── cache.rs           // GossipDeduplicationCache (LRU + TTL sliding window)
├── pex.rs             // Peer Exchange (PEX) protocol, active/passive views, XOR distance
├── sync.rs            // Anti-Entropy digest synchronization & range reconciliation
├── vector_clock.rs    // Monotonic VectorClock, causality comparison & merging
└── engine.rs          // SwarmGossipEngine trait & Concrete SwarmGossipDispatcher
```

### 3.3 Exact Data Structures & Trait Definitions

#### `src/gossip/envelope.rs`
```rust
use bytes::Bytes;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use ed25519_dalek::{SigningKey, VerifyingKey, Signature, Signer, Verifier};

pub const GOSSIP_ENVELOPE_MAGIC: [u8; 4] = *b"ZGSP";
pub const GOSSIP_ENVELOPE_VERSION: u8 = 1;
pub const GOSSIP_SIGNING_DOMAIN: &[u8] = b"ZAP-GOSSIP-ENVELOPE-v1";
pub const DEFAULT_MAX_HOPS: u8 = 16;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct GossipMessageId(pub [u8; 32]);

impl GossipMessageId {
    pub fn compute(topic: &str, origin: &Uuid, seq: u64, payload: &[u8]) -> Self {
        let mut hasher = blake3::Hasher::new_derive_key(GOSSIP_SIGNING_DOMAIN);
        hasher.update(topic.as_bytes());
        hasher.update(origin.as_bytes());
        hasher.update(&seq.to_be_bytes());
        hasher.update(&blake3::hash(payload).as_bytes()[..]);
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
    pub signature: [u8; 64],
}

impl GossipEnvelope {
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

    pub fn verify_signature(&self, verifying_key: &VerifyingKey) -> bool {
        if self.magic != GOSSIP_ENVELOPE_MAGIC || self.version != GOSSIP_ENVELOPE_VERSION {
            return false;
        }
        let expected_id = GossipMessageId::compute(
            &self.topic,
            &self.origin_node,
            self.sequence,
            &self.payload,
        );
        if self.message_id != expected_id {
            return false;
        }
        let digest = Self::signing_digest(&self.message_id, self.timestamp_micros, self.max_hops);
        let sig = Signature::from_bytes(&self.signature);
        verifying_key.verify(&digest, &sig).is_ok()
    }

    pub fn forward(&self) -> Option<Self> {
        if self.current_hop + 1 >= self.max_hops {
            return None;
        }
        let mut forwarded = self.clone();
        forwarded.current_hop += 1;
        Some(forwarded)
    }

    fn signing_digest(message_id: &GossipMessageId, timestamp_micros: u64, max_hops: u8) -> [u8; 32] {
        let mut hasher = blake3::Hasher::new_derive_key(GOSSIP_SIGNING_DOMAIN);
        hasher.update(&message_id.0);
        hasher.update(&timestamp_micros.to_be_bytes());
        hasher.update(&[max_hops]);
        *hasher.finalize().as_bytes()
    }
}
```

#### `src/gossip/cache.rs`
```rust
use std::{
    collections::{HashSet, VecDeque},
    time::{Duration, Instant},
};
use super::envelope::GossipMessageId;

pub struct GossipDeduplicationCache {
    capacity: usize,
    ttl: Duration,
    seen: HashSet<GossipMessageId>,
    order: VecDeque<(GossipMessageId, Instant)>,
}

impl GossipDeduplicationCache {
    pub fn new(capacity: usize, ttl: Duration) -> Self {
        Self {
            capacity,
            ttl,
            seen: HashSet::with_capacity(capacity.min(65536)),
            order: VecDeque::with_capacity(capacity.min(65536)),
        }
    }

    pub fn contains(&self, id: &GossipMessageId) -> bool {
        self.seen.contains(id)
    }

    pub fn insert(&mut self, id: GossipMessageId) -> bool {
        self.prune_expired();
        if self.seen.contains(&id) {
            return false;
        }
        if self.order.len() >= self.capacity {
            if let Some((old_id, _)) = self.order.pop_front() {
                self.seen.remove(&old_id);
            }
        }
        self.seen.insert(id);
        self.order.push_back((id, Instant::now()));
        true
    }

    pub fn prune_expired(&mut self) {
        let now = Instant::now();
        while let Some((_, timestamp)) = self.order.front() {
            if now.duration_since(*timestamp) > self.ttl {
                let (old_id, _) = self.order.pop_front().unwrap();
                self.seen.remove(&old_id);
            } else {
                break;
            }
        }
    }

    pub fn len(&self) -> usize {
        self.seen.len()
    }
}
```

#### `src/gossip/pex.rs`
```rust
use std::net::SocketAddr;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DiscoveredPeerEntry {
    pub node_id: Uuid,
    pub public_key: [u8; 32],
    pub socket_addr: SocketAddr,
    pub transport_key_epoch: u64,
    pub capabilities_digest: [u8; 32],
    pub last_seen_micros: u64,
    pub signature: [u8; 64],
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PeerExchangeRequest {
    pub requester: Uuid,
    pub max_peers_requested: u16,
    pub known_peer_ids: Vec<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PeerExchangeResponse {
    pub responder: Uuid,
    pub peers: Vec<DiscoveredPeerEntry>,
}

pub fn xor_distance(a: &Uuid, b: &Uuid) -> [u8; 16] {
    let a_bytes = a.as_bytes();
    let b_bytes = b.as_bytes();
    let mut dist = [0_u8; 16];
    for i in 0..16 {
        dist[i] = a_bytes[i] ^ b_bytes[i];
    }
    dist
}
```

#### `src/gossip/sync.rs`
```rust
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use super::envelope::GossipEnvelope;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StateDigest {
    pub topic: String,
    pub origin_node: Uuid,
    pub highest_sequence: u64,
    pub state_merkle_root: [u8; 32],
    pub timestamp_micros: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AntiEntropyDigestRequest {
    pub requester: Uuid,
    pub digests: Vec<StateDigest>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AntiEntropyDigestResponse {
    pub responder: Uuid,
    pub missing_ranges: Vec<MissingRange>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MissingRange {
    pub topic: String,
    pub origin_node: Uuid,
    pub start_seq: u64,
    pub end_seq: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AntiEntropyBatchResponse {
    pub responder: Uuid,
    pub envelopes: Vec<GossipEnvelope>,
}
```

#### `src/gossip/engine.rs`
```rust
use bytes::Bytes;
use std::sync::Arc;
use tokio::sync::mpsc;
use uuid::Uuid;
use super::{envelope::GossipEnvelope, mod_types::GossipError};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GossipReceipt {
    pub message_id: super::envelope::GossipMessageId,
    pub topic: String,
    pub sequence: u64,
    pub fanout_peers: usize,
}

pub trait SwarmGossipEngine: Send + Sync {
    fn broadcast_state(&self, topic: &str, payload: Bytes) -> Result<GossipReceipt, GossipError>;
    fn handle_inbound_envelope(&self, envelope: GossipEnvelope) -> Result<Option<GossipEnvelope>, GossipError>;
    fn subscribe(&self, topic: &str) -> mpsc::Receiver<GossipEnvelope>;
    fn active_peer_count(&self) -> usize;
}
```

---

## 4. Module Blueprint: BFT Swarm Consensus Subsystem (`src/consensus/`)

### 4.1 Quorum Consensus Model & Quorum Mathematics
1. **Fault Model & Quorum Bound**:
   - In a swarm of $N$ validators, up to $f$ Byzantine/faulty nodes are tolerated:
     $$N \ge 3f + 1 \implies f = \left\lfloor \frac{N - 1}{3} \right\rfloor$$
   - Supermajority Quorum Threshold:
     $$T = \left\lfloor \frac{2N}{3} \right\rfloor + 1$$
     - $N=3 \implies T=2, f=0$
     - $N=4 \implies T=3, f=1$
     - $N=7 \implies T=5, f=2$
     - $N=10 \implies T=7, f=3$
2. **2-Phase BFT State Machine Progression**:
   ```
   [Proposer] -> Propose(Round r, View v) -> Broadcast SwarmProposal
                     |
                     v
   [Validators] -> Verify Proposal & State -> Prevote(Digest) -> Broadcast SwarmVote::Prevote
                     |
                     v  (Collect >= T Prevotes: "Polka Certificate")
   [Validators] -> Precommit(Digest) -> Broadcast SwarmVote::Precommit
                     |
                     v  (Collect >= T Precommits)
   [Aggregator] -> Assemble SwarmCommitCertificate (Bitmask + T Signatures)
                     |
                     v
   [Node Runtime] -> Attach SwarmConsensusTrailer ('ZSC1') -> Execute & Record in Ledger
   ```

### 4.2 File Layout for `src/consensus/`
```
crates/zap-net/src/consensus/
├── mod.rs             // Module exports, consensus errors, constants
├── proposal.rs        // SwarmProposal struct, validation, proposer rotation
├── vote.rs            // SwarmVote, VoteKind (Prevote, Precommit), signing
├── certificate.rs     // SwarmCommitCertificate, SwarmConsensusTrailer ('ZSC1')
├── validator_set.rs   // ValidatorSet, ValidatorEntry, EpochTransition
├── batch_verify.rs    // Ed25519 threshold batch verification via ed25519_dalek
├── equivocation.rs    // EquivocationProof detection and slashing logic
└── engine.rs          // SwarmConsensusEngine trait & BftConsensusStateMachine
```

### 4.3 Compact Dynamic Threshold Signature (`SwarmConsensusTrailer` — `ZSC1`)
Binary Wire Layout for `SwarmConsensusTrailer`:
```
+-----------------------------------------------------------------------------------------------+
| Magic: 'ZSC1' (4B) | Version (2B) | Threshold T (2B) | Total Validators N (2B)                 |
+-----------------------------------------------------------------------------------------------+
| Epoch (8B)         | View (8B)    | Round (8B)       | Block Height (8B)                      |
+-----------------------------------------------------------------------------------------------+
| Proposal Digest (32B)                                                                         |
+-----------------------------------------------------------------------------------------------+
| Bitmask Length (2B)| Signer Bitmask (ceil(N/8) bytes)                                         |
+-----------------------------------------------------------------------------------------------+
| Signatures: T x 64 bytes (Ed25519 signatures in bitmask set-bit order)                        |
+-----------------------------------------------------------------------------------------------+
```

### 4.4 Exact Data Structures & Trait Definitions

#### `src/consensus/proposal.rs`
```rust
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use ed25519_dalek::{SigningKey, VerifyingKey, Signature, Signer, Verifier};

pub const PROPOSAL_DOMAIN: &[u8] = b"ZAP-SWARM-PROPOSAL-v1";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SwarmProposal {
    pub epoch: u64,
    pub view: u64,
    pub round: u64,
    pub block_height: u64,
    pub proposer_node: Uuid,
    pub payload_digest: [u8; 32],
    pub state_merkle_root: [u8; 32],
    pub valid_round: Option<u64>,
    pub timestamp_micros: u64,
    pub signature: [u8; 64],
}

impl SwarmProposal {
    pub fn new_signed(
        epoch: u64,
        view: u64,
        round: u64,
        block_height: u64,
        proposer_node: Uuid,
        payload_digest: [u8; 32],
        state_merkle_root: [u8; 32],
        valid_round: Option<u64>,
        timestamp_micros: u64,
        signing_key: &SigningKey,
    ) -> Self {
        let digest = Self::compute_digest(
            epoch,
            view,
            round,
            block_height,
            &proposer_node,
            &payload_digest,
            &state_merkle_root,
            valid_round,
            timestamp_micros,
        );
        let signature = signing_key.sign(&digest).to_bytes();
        Self {
            epoch,
            view,
            round,
            block_height,
            proposer_node,
            payload_digest,
            state_merkle_root,
            valid_round,
            timestamp_micros,
            signature,
        }
    }

    pub fn compute_digest(
        epoch: u64,
        view: u64,
        round: u64,
        block_height: u64,
        proposer_node: &Uuid,
        payload_digest: &[u8; 32],
        state_merkle_root: &[u8; 32],
        valid_round: Option<u64>,
        timestamp_micros: u64,
    ) -> [u8; 32] {
        let mut hasher = blake3::Hasher::new_derive_key(PROPOSAL_DOMAIN);
        hasher.update(&epoch.to_be_bytes());
        hasher.update(&view.to_be_bytes());
        hasher.update(&round.to_be_bytes());
        hasher.update(&block_height.to_be_bytes());
        hasher.update(proposer_node.as_bytes());
        hasher.update(payload_digest);
        hasher.update(state_merkle_root);
        hasher.update(&valid_round.unwrap_or(u64::MAX).to_be_bytes());
        hasher.update(&timestamp_micros.to_be_bytes());
        *hasher.finalize().as_bytes()
    }

    pub fn verify_signature(&self, verifying_key: &VerifyingKey) -> bool {
        let digest = Self::compute_digest(
            self.epoch,
            self.view,
            self.round,
            self.block_height,
            &self.proposer_node,
            &self.payload_digest,
            &self.state_merkle_root,
            self.valid_round,
            self.timestamp_micros,
        );
        let sig = Signature::from_bytes(&self.signature);
        verifying_key.verify(&digest, &sig).is_ok()
    }
}
```

#### `src/consensus/vote.rs`
```rust
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use ed25519_dalek::{SigningKey, VerifyingKey, Signature, Signer, Verifier};

pub const VOTE_DOMAIN: &[u8] = b"ZAP-SWARM-VOTE-v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VoteKind {
    Prevote = 1,
    Precommit = 2,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SwarmVote {
    pub epoch: u64,
    pub view: u64,
    pub round: u64,
    pub vote_kind: VoteKind,
    pub proposal_digest: [u8; 32],
    pub voter_node: Uuid,
    pub timestamp_micros: u64,
    pub signature: [u8; 64],
}

impl SwarmVote {
    pub fn new_signed(
        epoch: u64,
        view: u64,
        round: u64,
        vote_kind: VoteKind,
        proposal_digest: [u8; 32],
        voter_node: Uuid,
        timestamp_micros: u64,
        signing_key: &SigningKey,
    ) -> Self {
        let digest = Self::compute_digest(
            epoch,
            view,
            round,
            vote_kind,
            &proposal_digest,
            &voter_node,
            timestamp_micros,
        );
        let signature = signing_key.sign(&digest).to_bytes();
        Self {
            epoch,
            view,
            round,
            vote_kind,
            proposal_digest,
            voter_node,
            timestamp_micros,
            signature,
        }
    }

    pub fn compute_digest(
        epoch: u64,
        view: u64,
        round: u64,
        vote_kind: VoteKind,
        proposal_digest: &[u8; 32],
        voter_node: &Uuid,
        timestamp_micros: u64,
    ) -> [u8; 32] {
        let mut hasher = blake3::Hasher::new_derive_key(VOTE_DOMAIN);
        hasher.update(&epoch.to_be_bytes());
        hasher.update(&view.to_be_bytes());
        hasher.update(&round.to_be_bytes());
        hasher.update(&[vote_kind as u8]);
        hasher.update(proposal_digest);
        hasher.update(voter_node.as_bytes());
        hasher.update(&timestamp_micros.to_be_bytes());
        *hasher.finalize().as_bytes()
    }

    pub fn verify_signature(&self, verifying_key: &VerifyingKey) -> bool {
        let digest = Self::compute_digest(
            self.epoch,
            self.view,
            self.round,
            self.vote_kind,
            &self.proposal_digest,
            &self.voter_node,
            self.timestamp_micros,
        );
        let sig = Signature::from_bytes(&self.signature);
        verifying_key.verify(&digest, &sig).is_ok()
    }
}
```

#### `src/consensus/certificate.rs`
```rust
use serde::{Deserialize, Serialize};
use super::validator_set::ValidatorSet;
use super::batch_verify::verify_threshold_signatures;
use super::mod_types::ConsensusError;

pub const CONSENSUS_TRAILER_MAGIC: [u8; 4] = *b"ZSC1";
pub const CONSENSUS_TRAILER_VERSION: u16 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SwarmCommitCertificate {
    pub epoch: u64,
    pub view: u64,
    pub round: u64,
    pub block_height: u64,
    pub proposal_digest: [u8; 32],
    pub threshold: u16,
    pub total_validators: u16,
    pub signer_bitmask: Vec<u8>,
    pub signatures: Vec<[u8; 64]>,
}

impl SwarmCommitCertificate {
    pub fn verify_against_set(&self, validator_set: &ValidatorSet) -> Result<(), ConsensusError> {
        if self.epoch != validator_set.epoch {
            return Err(ConsensusError::EpochMismatch {
                cert_epoch: self.epoch,
                set_epoch: validator_set.epoch,
            });
        }
        if self.threshold < validator_set.threshold {
            return Err(ConsensusError::ThresholdMismatch {
                cert_threshold: self.threshold,
                required_threshold: validator_set.threshold,
            });
        }
        let signers = validator_set.resolve_bitmask_signers(&self.signer_bitmask)?;
        if signers.len() < validator_set.threshold as usize {
            return Err(ConsensusError::InsufficientSignatures {
                received: signers.len(),
                required: validator_set.threshold as usize,
            });
        }
        if signers.len() != self.signatures.len() {
            return Err(ConsensusError::SignatureCountMismatch {
                signers_in_mask: signers.len(),
                signatures_provided: self.signatures.len(),
            });
        }

        verify_threshold_signatures(
            self.epoch,
            self.view,
            self.round,
            &self.proposal_digest,
            &signers,
            &self.signatures,
        )
    }

    pub fn encode_trailer(&self) -> Vec<u8> {
        let bitmask_len = self.signer_bitmask.len() as u16;
        let mut out = Vec::with_capacity(76 + self.signer_bitmask.len() + self.signatures.len() * 64);
        out.extend_from_slice(&CONSENSUS_TRAILER_MAGIC);
        out.extend_from_slice(&CONSENSUS_TRAILER_VERSION.to_be_bytes());
        out.extend_from_slice(&self.threshold.to_be_bytes());
        out.extend_from_slice(&self.total_validators.to_be_bytes());
        out.extend_from_slice(&self.epoch.to_be_bytes());
        out.extend_from_slice(&self.view.to_be_bytes());
        out.extend_from_slice(&self.round.to_be_bytes());
        out.extend_from_slice(&self.block_height.to_be_bytes());
        out.extend_from_slice(&self.proposal_digest);
        out.extend_from_slice(&bitmask_len.to_be_bytes());
        out.extend_from_slice(&self.signer_bitmask);
        for sig in &self.signatures {
            out.extend_from_slice(sig);
        }
        out
    }

    pub fn decode_trailer(bytes: &[u8]) -> Result<Self, ConsensusError> {
        if bytes.len() < 76 {
            return Err(ConsensusError::TrailerTruncated { expected: 76, actual: bytes.len() });
        }
        if bytes[0..4] != CONSENSUS_TRAILER_MAGIC {
            return Err(ConsensusError::InvalidTrailerMagic);
        }
        let version = u16::from_be_bytes([bytes[4], bytes[5]]);
        if version != CONSENSUS_TRAILER_VERSION {
            return Err(ConsensusError::UnsupportedTrailerVersion(version));
        }
        let threshold = u16::from_be_bytes([bytes[6], bytes[7]]);
        let total_validators = u16::from_be_bytes([bytes[8], bytes[9]]);
        let epoch = u64::from_be_bytes(bytes[10..18].try_into().unwrap());
        let view = u64::from_be_bytes(bytes[18..26].try_into().unwrap());
        let round = u64::from_be_bytes(bytes[26..34].try_into().unwrap());
        let block_height = u64::from_be_bytes(bytes[34..42].try_into().unwrap());
        let mut proposal_digest = [0_u8; 32];
        proposal_digest.copy_from_slice(&bytes[42..74]);
        let bitmask_len = u16::from_be_bytes([bytes[74], bytes[75]]) as usize;
        
        let mask_end = 76 + bitmask_len;
        if bytes.len() < mask_end {
            return Err(ConsensusError::TrailerTruncated { expected: mask_end, actual: bytes.len() });
        }
        let signer_bitmask = bytes[76..mask_end].to_vec();
        let sigs_bytes = &bytes[mask_end..];
        if sigs_bytes.len() % 64 != 0 {
            return Err(ConsensusError::InvalidSignaturePayloadLength(sigs_bytes.len()));
        }
        let sig_count = sigs_bytes.len() / 64;
        let mut signatures = Vec::with_capacity(sig_count);
        for chunk in sigs_bytes.chunks_exact(64) {
            let mut sig = [0_u8; 64];
            sig.copy_from_slice(chunk);
            signatures.push(sig);
        }

        Ok(Self {
            epoch,
            view,
            round,
            block_height,
            proposal_digest,
            threshold,
            total_validators,
            signer_bitmask,
            signatures,
        })
    }
}
```

#### `src/consensus/batch_verify.rs`
```rust
use ed25519_dalek::{ed25519::signature::Signature, Signature as DalekSignature, VerifyingKey};
use super::{vote::{SwarmVote, VoteKind}, mod_types::ConsensusError};

pub fn verify_threshold_signatures(
    epoch: u64,
    view: u64,
    round: u64,
    proposal_digest: &[u8; 32],
    verifying_keys: &[VerifyingKey],
    signatures: &[[u8; 64]],
) -> Result<(), ConsensusError> {
    if verifying_keys.len() != signatures.len() {
        return Err(ConsensusError::SignatureCountMismatch {
            signers_in_mask: verifying_keys.len(),
            signatures_provided: signatures.len(),
        });
    }

    let mut messages: Vec<Vec<u8>> = Vec::with_capacity(verifying_keys.len());
    let mut dalek_signatures: Vec<DalekSignature> = Vec::with_capacity(signatures.len());

    for (vk, sig_bytes) in verifying_keys.iter().zip(signatures.iter()) {
        let node_id = zap_crypto::node_id_from_public_key(&vk.to_bytes());
        // In commit cert, the signing message is the vote precommit digest
        let msg_digest = SwarmVote::compute_digest(
            epoch,
            view,
            round,
            VoteKind::Precommit,
            proposal_digest,
            &node_id,
            0, // Canonical timestamp normalization for batch aggregation or per-vote timestamp verification
        );
        messages.push(msg_digest.to_vec());
        dalek_signatures.push(DalekSignature::from_bytes(sig_bytes));
    }

    // ed25519-dalek batch verification
    let message_refs: Vec<&[u8]> = messages.iter().map(|m| m.as_slice()).collect();
    ed25519_dalek::verify_batch(&message_refs, &dalek_signatures, verifying_keys)
        .map_err(|_| ConsensusError::BatchVerificationFailed)
}
```

#### `src/consensus/validator_set.rs`
```rust
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use ed25519_dalek::VerifyingKey;
use super::mod_types::ConsensusError;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ValidatorEntry {
    pub node_id: Uuid,
    pub public_key: [u8; 32],
    pub weight: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ValidatorSet {
    pub epoch: u64,
    pub validators: Vec<ValidatorEntry>,
    pub threshold: u16,
}

impl ValidatorSet {
    pub fn new(epoch: u64, validators: Vec<ValidatorEntry>) -> Result<Self, ConsensusError> {
        if validators.is_empty() {
            return Err(ConsensusError::EmptyValidatorSet);
        }
        let n = validators.len();
        let threshold = ((n * 2) / 3 + 1) as u16;
        Ok(Self {
            epoch,
            validators,
            threshold,
        })
    }

    pub fn proposer_for_round(&self, view: u64, round: u64) -> &ValidatorEntry {
        let idx = ((view + round) as usize) % self.validators.len();
        &self.validators[idx]
    }

    pub fn resolve_bitmask_signers(&self, bitmask: &[u8]) -> Result<Vec<VerifyingKey>, ConsensusError> {
        let mut signers = Vec::new();
        for (i, val) in self.validators.iter().enumerate() {
            let byte_idx = i / 8;
            let bit_idx = i % 8;
            if byte_idx < bitmask.len() && (bitmask[byte_idx] & (1 << bit_idx)) != 0 {
                let vk = VerifyingKey::from_bytes(&val.public_key)
                    .map_err(|_| ConsensusError::InvalidValidatorKey(val.node_id))?;
                signers.push(vk);
            }
        }
        Ok(signers)
    }

    pub fn create_bitmask(&self, signer_ids: &[Uuid]) -> Vec<u8> {
        let byte_len = (self.validators.len() + 7) / 8;
        let mut mask = vec![0_u8; byte_len];
        for id in signer_ids {
            if let Some(pos) = self.validators.iter().position(|v| v.node_id == *id) {
                let byte_idx = pos / 8;
                let bit_idx = pos % 8;
                mask[byte_idx] |= 1 << bit_idx;
            }
        }
        mask
    }
}
```

---

## 5. Module Blueprint: Adaptive Quorum Mesh & Failover Routing (`src/mesh/`)

### 5.1 Phi Accrual Failure Detector ($\Phi$)
The Phi Accrual failure detector computes a continuous suspicion metric $\Phi$ rather than a binary timeout:
1. Maintain sliding window of heartbeat intervals: $\{\Delta t_1, \Delta t_2, \dots, \Delta t_W\}$ ($W = 100$).
2. Compute mean $\mu$ and standard deviation $\sigma = \max(\sigma_{\text{computed}}, \sigma_{\text{min}})$ with $\sigma_{\text{min}} = 50.0\text{ ms}$.
3. For elapsed time $t = t_{\text{now}} - t_{\text{last\_heartbeat}}$, compute probability $P_{\text{later}}(t)$ that a heartbeat arrives later than $t$:
   $$P_{\text{later}}(t) = \frac{1}{\sigma \sqrt{2\pi}} \int_t^\infty \exp\left(-\frac{(u - \mu)^2}{2\sigma^2}\right) du \approx \frac{1}{2} \text{erfc}\left(\frac{t - \mu}{\sigma \sqrt{2}}\right)$$
4. Suspicion Metric:
   $$\Phi(t) = -\log_{10}(P_{\text{later}}(t))$$
5. Health State Classification:
   - **`Alive`**: $\Phi < 8.0$
   - **`Suspect`**: $8.0 \le \Phi < 14.0$
   - **`Dead`**: $\Phi \ge 14.0$

### 5.2 File Layout for `src/mesh/`
```
crates/zap-net/src/mesh/
├── mod.rs             // Module exports, constants, MeshError taxonomy
├── phi_detector.rs    // PhiAccrualDetector sliding window & normal CDF
├── heartbeat.rs       // HeartbeatPing, HeartbeatAck, jittered scheduler
├── partition.rs       // PartitionStatus (Normal, DegradedMinority, Isolated)
├── relay.rs           // ZapRelayEnvelope ('ZRLY'), 2-hop routing, cost metric
└── topology.rs        // MeshTopology health engine & dynamic routing tables
```

### 5.3 Exact Data Structures & Trait Definitions

#### `src/mesh/phi_detector.rs`
```rust
use std::collections::VecDeque;

const DEFAULT_WINDOW_SIZE: usize = 100;
const MIN_STD_DEV_MICROS: f64 = 50_000.0; // 50ms min std dev

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PeerHealthState {
    Alive,
    Suspect,
    Dead,
}

#[derive(Debug, Clone)]
pub struct PhiAccrualDetector {
    window_size: usize,
    intervals: VecDeque<f64>,
    last_heartbeat_micros: Option<u64>,
    phi_suspect: f64,
    phi_dead: f64,
}

impl PhiAccrualDetector {
    pub fn new(phi_suspect: f64, phi_dead: f64) -> Self {
        Self {
            window_size: DEFAULT_WINDOW_SIZE,
            intervals: VecDeque::with_capacity(DEFAULT_WINDOW_SIZE),
            last_heartbeat_micros: None,
            phi_suspect,
            phi_dead,
        }
    }

    pub fn record_heartbeat(&mut self, now_micros: u64) {
        if let Some(prev) = self.last_heartbeat_micros {
            if now_micros > prev {
                let interval = (now_micros - prev) as f64;
                if self.intervals.len() >= self.window_size {
                    self.intervals.pop_front();
                }
                self.intervals.push_back(interval);
            }
        }
        self.last_heartbeat_micros = Some(now_micros);
    }

    pub fn phi(&self, now_micros: u64) -> f64 {
        let last = match self.last_heartbeat_micros {
            Some(l) => l,
            None => return 0.0,
        };
        if now_micros <= last || self.intervals.len() < 2 {
            return 0.0;
        }
        let elapsed = (now_micros - last) as f64;
        let count = self.intervals.len() as f64;
        let mean = self.intervals.iter().sum::<f64>() / count;
        let variance = self.intervals.iter().map(|&x| (x - mean).powi(2)).sum::<f64>() / count;
        let std_dev = variance.sqrt().max(MIN_STD_DEV_MICROS);

        let y = (elapsed - mean) / (std_dev * std::f64::consts::SQRT_2);
        let p_later = 0.5 * erfc(y);
        if p_later <= 1e-15 {
            15.0
        } else {
            -p_later.log10()
        }
    }

    pub fn health(&self, now_micros: u64) -> PeerHealthState {
        let score = self.phi(now_micros);
        if score >= self.phi_dead {
            PeerHealthState::Dead
        } else if score >= self.phi_suspect {
            PeerHealthState::Suspect
        } else {
            PeerHealthState::Alive
        }
    }
}

// Complementary Error Function approximation (Abramowitz & Stegun 7.1.26)
fn erfc(x: f64) -> f64 {
    if x < 0.0 {
        return 2.0 - erfc(-x);
    }
    let p = 0.3275911;
    let a1 = 0.254829592;
    let a2 = -0.284496736;
    let a3 = 1.421413741;
    let a4 = -1.453152027;
    let a5 = 1.061405429;

    let t = 1.0 / (1.0 + p * x);
    let poly = t * (a1 + t * (a2 + t * (a3 + t * (a4 + t * a5))));
    poly * (-x * x).exp()
}
```

#### `src/mesh/partition.rs`
```rust
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
    pub fn is_operational(&self) -> bool {
        matches!(self, PartitionStatus::Normal { .. })
    }
}
```

#### `src/mesh/relay.rs`
```rust
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
            return Err(MeshError::RelayHopLimitExceeded { max: MAX_RELAY_HOPS });
        }
        let mut forwarded = self.clone();
        forwarded.hop_count += 1;
        Ok(forwarded)
    }
}
```

#### `src/mesh/topology.rs`
```rust
use std::{collections::HashMap, net::SocketAddr, sync::Arc};
use tokio::sync::RwLock;
use uuid::Uuid;
use super::{phi_detector::{PhiAccrualDetector, PeerHealthState}, partition::PartitionStatus, relay::ZapRelayEnvelope, mod_types::MeshError};

#[derive(Debug, Clone)]
pub struct PeerMeshInfo {
    pub node_id: Uuid,
    pub addr: SocketAddr,
    pub rtt_micros: u64,
    pub loss_ratio: f64,
    pub queue_pressure: u8,
}

pub trait MeshTopology: Send + Sync {
    fn record_heartbeat(&self, peer_id: Uuid, rtt_micros: u64, now_micros: u64);
    fn peer_health(&self, peer_id: &Uuid, now_micros: u64) -> PeerHealthState;
    fn partition_status(&self, total_validators: usize, now_micros: u64) -> PartitionStatus;
    fn select_relay_route(&self, target_node: &Uuid) -> Result<Uuid, MeshError>;
}
```

---

## 6. Comprehensive Error Taxonomy (`src/error.rs`)

```rust
use thiserror::Error;
use uuid::Uuid;
use zap_core::ZapError as CoreError;

#[derive(Debug, Error)]
pub enum GossipError {
    #[error("peer {0} not found in gossip mesh")]
    PeerNotFound(Uuid),
    #[error("invalid gossip magic")]
    InvalidMagic,
    #[error("unsupported gossip version {0}")]
    UnsupportedVersion(u8),
    #[error("gossip hop limit exceeded: current {current}, max {max}")]
    HopLimitExceeded { current: u8, max: u8 },
    #[error("duplicate gossip message {0:?}")]
    DuplicateMessage(super::gossip::envelope::GossipMessageId),
    #[error("invalid gossip signature from {0}")]
    InvalidSignature(Uuid),
    #[error("channel send error: {0}")]
    Channel(String),
}

#[derive(Debug, Error)]
pub enum ConsensusError {
    #[error("quorum threshold not reached: received {received}, required {required}")]
    QuorumNotReached { received: usize, required: usize },
    #[error("epoch mismatch: cert epoch {cert_epoch}, validator set epoch {set_epoch}")]
    EpochMismatch { cert_epoch: u64, set_epoch: u64 },
    #[error("threshold mismatch: cert threshold {cert_threshold}, required {required_threshold}")]
    ThresholdMismatch { cert_threshold: u16, required_threshold: u16 },
    #[error("insufficient signatures: received {received}, required {required}")]
    InsufficientSignatures { received: usize, required: usize },
    #[error("signature count mismatch: {signers_in_mask} signers in bitmask vs {signatures_provided} signatures")]
    SignatureCountMismatch { signers_in_mask: usize, signatures_provided: usize },
    #[error("empty validator set")]
    EmptyValidatorSet,
    #[error("invalid validator key for {0}")]
    InvalidValidatorKey(Uuid),
    #[error("invalid trailer magic")]
    InvalidTrailerMagic,
    #[error("unsupported trailer version {0}")]
    UnsupportedTrailerVersion(u16),
    #[error("trailer truncated: expected {expected}, got {actual}")]
    TrailerTruncated { expected: usize, actual: usize },
    #[error("invalid signature payload length {0}")]
    InvalidSignaturePayloadLength(usize),
    #[error("batch verification failed")]
    BatchVerificationFailed,
    #[error("proposal {0} already expired or finalized")]
    ProposalClosed(Uuid),
    #[error("equivocation detected from validator {offender} in epoch {epoch}, round {round}")]
    EquivocationDetected { offender: Uuid, epoch: u64, round: u64 },
}

#[derive(Debug, Error)]
pub enum MeshError {
    #[error("network partition detected: in minority partition")]
    MinorityPartition,
    #[error("node {0} is dead or unreachable")]
    PeerUnreachable(Uuid),
    #[error("no healthy relay route available to destination {0}")]
    NoRelayRoute(Uuid),
    #[error("relay hop limit exceeded: max {max}")]
    RelayHopLimitExceeded { max: u8 },
    #[error("untrusted relay forwarder {0}")]
    UntrustedRelay(Uuid),
}
```

---

## 7. Implementation Roadmap & Concrete File Plan

### 7.1 `crates/zap-net/Cargo.toml` Additions
Ensure required workspace dependencies are activated:
```toml
[dependencies]
blake3.workspace = true
bytes.workspace = true
chacha20poly1305.workspace = true
ed25519-dalek = { workspace = true, features = ["batch", "rand_core"] }
hex.workspace = true
rand_core.workspace = true
serde.workspace = true
serde_json.workspace = true
snow.workspace = true
thiserror.workspace = true
tokio.workspace = true
tracing.workspace = true
uuid.workspace = true
zap-core = { path = "../zap-core" }
zap-crypto = { path = "../zap-crypto" }
```

### 7.2 File Creation & Modification Matrix
| Action | File Path | Scope / Description |
| :--- | :--- | :--- |
| **Modify** | `crates/zap-net/src/lib.rs` | Register submodules (`pub mod gossip; pub mod consensus; pub mod mesh;`), export unified `ZapNetError`, integrate relay unwrap in `recv()`. |
| **Create** | `crates/zap-net/src/gossip/mod.rs` | Submodule root, re-exports, `GossipError`. |
| **Create** | `crates/zap-net/src/gossip/envelope.rs` | `GossipEnvelope`, `GossipMessageId`, signature & hop methods. |
| **Create** | `crates/zap-net/src/gossip/cache.rs` | `GossipDeduplicationCache` (LRU + TTL). |
| **Create** | `crates/zap-net/src/gossip/pex.rs` | `DiscoveredPeerEntry`, `PeerExchangeRequest/Response`, XOR metric. |
| **Create** | `crates/zap-net/src/gossip/sync.rs` | `StateDigest`, `AntiEntropySync` protocols. |
| **Create** | `crates/zap-net/src/gossip/vector_clock.rs` | `VectorClock`, `Causality` (migrated & enhanced from `src/gossip.rs`). |
| **Create** | `crates/zap-net/src/gossip/engine.rs` | `SwarmGossipEngine` trait & dispatcher implementation. |
| **Create** | `crates/zap-net/src/consensus/mod.rs` | Submodule root, `ConsensusError`. |
| **Create** | `crates/zap-net/src/consensus/proposal.rs` | `SwarmProposal` with Ed25519 signing & digest computation. |
| **Create** | `crates/zap-net/src/consensus/vote.rs` | `SwarmVote`, `VoteKind` (Prevote, Precommit). |
| **Create** | `crates/zap-net/src/consensus/certificate.rs` | `SwarmCommitCertificate`, bitmask packing, `SwarmConsensusTrailer` (`ZSC1`). |
| **Create** | `crates/zap-net/src/consensus/validator_set.rs` | `ValidatorSet`, `ValidatorEntry`, bitmask resolution. |
| **Create** | `crates/zap-net/src/consensus/batch_verify.rs` | High-throughput `verify_threshold_signatures` using `ed25519_dalek::verify_batch`. |
| **Create** | `crates/zap-net/src/consensus/equivocation.rs` | `EquivocationProof`, conflicting vote detector. |
| **Create** | `crates/zap-net/src/consensus/engine.rs` | `SwarmConsensusEngine` state machine (Propose -> Prevote -> Precommit -> Finalize). |
| **Create** | `crates/zap-net/src/mesh/mod.rs` | Submodule root, `MeshError`. |
| **Create** | `crates/zap-net/src/mesh/phi_detector.rs` | `PhiAccrualDetector` with continuous erf normal distribution model. |
| **Create** | `crates/zap-net/src/mesh/heartbeat.rs` | `HeartbeatPing`, `HeartbeatAck`, randomized jitter backoff. |
| **Create** | `crates/zap-net/src/mesh/partition.rs` | `PartitionStatus`, minority degradation, quorum ratios. |
| **Create** | `crates/zap-net/src/mesh/relay.rs` | `ZapRelayEnvelope` (`ZRLY`), 2-hop forwarding engine. |
| **Create** | `crates/zap-net/src/mesh/topology.rs` | `MeshTopology` health tracker and route selector. |

---

## 8. Verification Strategy & Test Matrix

### 8.1 Unit & Integration Test Specifications
1. **Gossip Dissemination Test**:
   - Create 5 in-memory connected nodes; publish on `zap.test.topic`.
   - Assert all 5 nodes receive exactly 1 copy of the message within 100ms.
   - Assert duplicate transmissions are filtered by `GossipDeduplicationCache`.
2. **Consensus Quorum & Dynamic Bitmask Test**:
   - Build 4-validator set ($T=3$). Produce proposal, collect 3 prevotes and 3 precommits.
   - Assemble `SwarmCommitCertificate` with bitmask `[0b00000111]`.
   - Assert `verify_against_set()` completes in $< 1\text{ ms}$ via batch verification.
3. **Equivocation Slashing Test**:
   - Produce two conflicting Prevote votes from Validator 2 for same round with different proposal digests.
   - Assert `EquivocationProof` generates successfully and verifies.
4. **Phi Accrual Failure Detection Test**:
   - Feed regular 1,000ms intervals to `PhiAccrualDetector`.
   - Advance clock by 12 seconds with no heartbeats; assert state transitions from `Alive` $\to$ `Suspect` $\to$ `Dead` with $\Phi > 14.0$.
5. **Network Partition Test**:
   - Simulate a 5-node cluster split into partition A (3 nodes) and partition B (2 nodes).
   - Assert partition A reports `PartitionStatus::Normal` and proceeds with consensus.
   - Assert partition B reports `PartitionStatus::DegradedMinority` and refuses new proposals.
6. **2-Hop Dynamic Relay Test**:
   - Configure nodes A, B, C where link A-B is blocked.
   - Route packet from A to B via C using `ZapRelayEnvelope`.
   - Assert C unpacks and forwards to B with `hop_count == 2`.

---
*Document produced by Explorer 1 — Milestone 1 (R1).*
