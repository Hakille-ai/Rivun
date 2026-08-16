# Architectural Survey & Technical Specification: R1 — P2P Swarm Gossip Consensus & Adaptive Quorum Mesh

**Document Reference**: `ZAP-SURVEY-R1-2026`  
**Working Directory**: `c:\Users\Stagiaire\Documents\Amadou PGC\Prs\ZAP\.agents\explorer_survey_1`  
**Target Crates**: `crates/zap-net`, `crates/zap-agent`, `crates/zap-node`, with cross-cutting integration in `crates/zap-core`, `crates/zap-crypto`, `crates/zap-router`, `crates/zap-ledger`  
**Status**: Comprehensive Technical Survey & Architectural Blueprint  

---

## 1. Executive Summary

The ZAP Next-Gen Frontier objective is to transform ZAP into an autonomous, hyper-scalable, cross-cluster decentralized execution and verification fabric. 

Requirement **R1 (P2P Swarm Gossip Consensus & Adaptive Quorum Mesh)** establishes the foundational networking and distributed consensus layer required for autonomous multi-agent nodes to discover peers dynamically, disseminate state and capability metadata via gossip, reach high-throughput Byzantine-fault-tolerant (BFT) swarm consensus using dynamic threshold signatures ($T$-of-$N$), detect network partitions, and execute multi-peer failover routing.

### Key Survey Findings:
1. **Current Networking (`zap-net`)**: Operates on ChaCha20-Poly1305 AEAD encrypted UDP datagrams (`ZAPD` magic, version 1, 52-byte header, sliding window nonce anti-replay with WAL durability). However, discovery is strictly static (`Vec<Peer>` in config), broadcast is an $O(N)$ unicast loop, and there is no gossip dissemination, heartbeat liveness tracking, partition detector, or dynamic relay mesh.
2. **Current Agent Coordination (`zap-agent`)**: Provides robust JSON contracts for intents, sessions, delegations, and 1-to-1 capability negotiation, along with a 6-stage cryptographic Provenance Chain Engine ($H_{\text{intent}} \to H_{\text{negotiation}} \to H_{\text{policy}} \to H_{\text{driver}} \to H_{\text{poa}} \to H_{\text{receipt}} \to H_{\text{root}}$). However, it lacks swarm-level coordination state machines, multi-agent quorum voting, and decentralized capability indexing.
3. **Current Node & Consensus Daemon (`zap-node` & `zap-crypto`)**: The node runs an async `handle_once()` loop supporting static Proof-of-Action (PoA) attestation gathering via synchronous point-to-point requests. In PoA, $M$ individual Ed25519 signatures are concatenated in a `ZPOA` trailer. There is no BFT state machine replication (rounds/views/epochs, 2-phase/3-phase commit, leaderless or rotating leader consensus), no dynamic threshold signature aggregation, and no background mesh daemon.

This document presents the detailed gap analysis, mathematical models, wire protocols, state machines, Rust trait/struct/enum definitions, and integration blueprints necessary to implement R1 cleanly without breaking existing wire format contracts.

---

## 2. Current State Deep-Dive Analysis

### 2.1 `crates/zap-net`

| Component | Current Implementation | Capabilities | Gaps for R1 |
| :--- | :--- | :--- | :--- |
| **Transport Protocol** | Encrypted UDP (`ZapEndpoint`) | ChaCha20-Poly1305 AEAD with 12-byte nonces (4B prefix + 8B counter), MTU limit 65,507 bytes, Noise NN handshake helper. | Point-to-point UDP only; no multi-hop mesh forwarding or relay encapsulation. |
| **Peer Discovery** | Static configuration (`PeerTables`) | In-memory hash maps (`by_id`, `by_addr`), static manual peer insertion (`add_peer`). | No Kademlia DHT, no Peer Exchange (PEX), no automatic neighbor discovery or bootnode syncing. |
| **Broadcast Dissemination** | Sequential Unicast | Iterates through all known peer IDs and calls `send_frame(peer, frame)`. $O(N)$ egress bandwidth. | No epidemic gossip (fanout, bloom filter anti-entropy, deduplication cache, hop count/TTL control). |
| **Anti-Replay & Durability** | Sliding window + WAL | `NonceReplayCache` + `DurableNonceStore` (`ZAPNONC1` binary record journal). | Designed for unicast nonces per peer; no distributed message ID deduplication for gossip waves. |
| **Failure Detection** | None | No liveness checks; dead peers remain in `PeerTables` indefinitely. | No ping/pong heartbeats, no jitter backoff, no Phi Accrual failure detection, no partition detection. |

### 2.2 `crates/zap-agent`

| Component | Current Implementation | Capabilities | Gaps for R1 |
| :--- | :--- | :--- | :--- |
| **Agent Contracts** | JSON schemas (v1) in `ZENV` envelopes | `AgentIntent`, `AgentSession`, `DelegationRequest`, `DelegationResponse`, `AgentStatusUpdate`, `AgentResult`. | 1-to-1 client-server / delegator-delegatee semantics; lacks multi-party swarm collective intent coordination. |
| **Capability Negotiation** | Point-to-Point 1:1 | `CapabilityNegotiationRequest`/`Response` matches required/optional capabilities. | No decentralized capability index broadcast or swarm-wide auction/scoring mechanism. |
| **Provenance Chain Engine** | `provenance.rs` | 6-stage causal hashing with Ed25519 signature of root Merkle hash. | Stage `Poa` only links static PoA signatures; needs extension to capture dynamic BFT swarm commit certificates. |

### 2.3 `crates/zap-node`

| Component | Current Implementation | Capabilities | Gaps for R1 |
| :--- | :--- | :--- | :--- |
| **Daemon Architecture** | `ZapNode` async loop | Sequential `handle_once()` processing inbound frames from `ZapEndpoint`. | Single receive loop without dedicated background mesh, gossip, or consensus worker tasks. |
| **Consensus Engine** | Static PoA Attestation | Checks `REQUIRES_CONSENSUS` flag, requests individual signatures from configured `poa.validators`, validates `verify_poa_certificate`. | Static validator set, no BFT voting phases (propose/prevote/precommit), no dynamic quorum reconfiguration, no equivocation slashing. |
| **Discovery Handling** | Point-to-point query/announce | `zap.discovery.query`, `zap.discovery.response`, `zap.discovery.announce` caching `SignedDiscoveryAdvertisement`. | Passive announcement cache; does not actively gossip advertisements across multi-hop topologies. |
| **Routing** | `RouteTable` (`zap-router`) | Subject pattern matching (`*`, `prefix.*`), peer unicast, local driver dispatch, drop. | Static routing decisions; no dynamic multi-peer failover, no path quality metrics (RTT, packet loss). |

### 2.4 `crates/zap-crypto` & `crates/zap-core`

- **Wire Header (`ZapHeader`)**: 64 bytes fixed big-endian header with magic `ZAP_`, version 1, flags (`ENCRYPTED`, `PRIORITY`, `REQUIRES_CONSENSUS`, `SIGNED`, `BROADCAST`), source/target UUIDs, timestamp (micros), payload length, and 8-byte `zap_sign` hint.
- **Trailers**:
  - `AuthTrailer` (`ZSIG`): 72 bytes containing 64-byte Ed25519 signature over header signing prefix + payload.
  - `PoaTrailer` (`ZPOA`): 44-byte header + $M \times 80$-byte attestations (16B node ID + 64B Ed25519 signature). Total size grows linearly with validator count ($44 + 80M$ bytes).
- **Crypto Primitives**: Ed25519 signing/verification (`ed25519_dalek`), Blake3 hashing for node ID derivation and signature hints, ChaCha20-Poly1305 AEAD.

---

## 3. Decentralized P2P Gossip Protocol Specification

To achieve autonomous swarm self-organization, `zap-net` must implement a two-tier decentralized P2P gossip layer combining **HyParView / Plumtree style epidemic dissemination** with **Kademlia-inspired peer distance metric** and **SWIM-based peer sampling**.

```
                           +-------------------------------------+
                           |          ZAP Node Runtime           |
                           +------------------+------------------+
                                              |
                     +------------------------+------------------------+
                     |                                                 |
       +-------------v-------------+                     +-------------v-------------+
       |   P2P Gossip Subsystem    |                     |    Adaptive Quorum Mesh   |
       |  (crates/zap-net/gossip)  |                     |   (crates/zap-net/mesh)   |
       +-------------+-------------+                     +-------------+-------------+
                     |                                                 |
   +-----------------+-----------------+             +-----------------+-----------------+
   |                 |                 |             |                 |                 |
+--v---------+ +-----v------+ +--------v---+     +---v--------+ +------v-------+ +---------v-----+
| Peer Disc. | | State Bcast| | Capab. Neg.|     | Heartbeats | | Partition Det.| | Failover Route |
| (PEX/Boot) | | (Anti-Entr)| | (Swarm Idx)|     | (Jittered) | | (Quorum Loss) | | (Relay Mesh) |
+------------+ +------------+ +------------+     +------------+ +-------------+ +---------------+
```

### 3.1 Peer Discovery & Swarm Topology

1. **Bootnodes & Seed Ingestion**:
   - Nodes initialize with a configured list of bootstrap nodes (`bootnodes = ["192.168.1.10:9000@<node_id>", ...]`).
   - On boot, the node sends a signed `PeerExchangeRequest` to all bootnodes.
2. **Peer Sampling (PEX - Peer Exchange)**:
   - Nodes maintain two peer views:
     - **Active View** ($k_{\text{active}} = 6 \text{ to } 8$ peers): Direct UDP transport connections actively maintained and monitored.
     - **Passive View** ($k_{\text{passive}} = 24 \text{ to } 32$ peers): Backup peer pool for rapid fault recovery.
   - Periodic PEX gossip cycle: Every $T_{\text{pex}}$ (default 10s), select random active peer $P$, exchange a subset of passive view entries (`PeerExchangeMessage`), and update tables based on XOR distance metric $d(A, B) = \text{Blake3}(\text{NodeId}_A) \oplus \text{Blake3}(\text{NodeId}_B)$.
3. **Signed Peer Advertisement (`SignedPeerAdvertisement`)**:
   - Contains node ID, Ed25519 public key, advertised socket addresses, transport key epoch, capabilities summary hash, sequence number, and timestamp.
   - Signed by the node's identity key. Propagated through the swarm via gossip.

### 3.2 Epidemic State Broadcast & Anti-Entropy Synchronization

1. **Fanout Parameter ($k_{\text{fanout}}$)**:
   - For every new broadcast message, the node randomly selects $k$ peers from its Active View (default $k=3$ for $N \le 20$, $k=\lceil \log_2 N \rceil + 1$ for large swarms).
2. **Message Deduplication & Cache Window**:
   - **Gossip Message ID**: $M_{\text{id}} = \text{Blake3}(\text{topic} \parallel \text{origin\_node} \parallel \text{seq} \parallel \text{payload\_digest})$.
   - In-memory LRU deduplication cache (`GossipDeduplicationCache`) stores up to 65,536 recent message IDs with 60-second TTL to eliminate broadcast storm amplification.
3. **Hop Count / Time-To-Live (TTL)**:
   - Every gossip envelope carries `max_hops` (default 16) and `current_hop`. Dropped if `current_hop >= max_hops`.
4. **Anti-Entropy Synchronization**:
   - Periodic digest reconciliation: Every $T_{\text{sync}}$ (default 5s), nodes exchange an **Invertible Bloom Lookup Table (IBLT)** or **Merkle State Digest** of recent state/consensus hashes over topic channels (`zap.gossip.sync`).
   - If a divergence is detected, missing messages are retrieved via unicast range fetch.

### 3.3 Dynamic Capability Negotiation & Swarm Indexing

1. **Capability Advertisement Gossip**:
   - When a node registers or modifies WASM drivers or policies, it generates a `SignedCapabilityAdvertisement` and broadcasts it on topic `zap.swarm.capabilities`.
2. **Swarm Capability Registry (`SwarmCapabilityRegistry`)**:
   - Maintains an in-memory inverted index mapping `CapabilityId` $\to$ `Vec<SwarmPeerCapabilityScore>`.
   - Dynamic scoring formula:
     $$\text{Score}(P, C) = w_1 \cdot \text{TrustScore}(P) + w_2 \cdot (1 - \text{LatencyNormalized}(P)) + w_3 \cdot \text{AvailableFuel}(P)$$
3. **Capability Negotiation Protocol**:
   - When an agent submits `AgentIntent` requiring capabilities $\{C_1, \dots, C_k\}$, the local node queries the Swarm Capability Registry, selects the top-scoring candidate nodes, and initiates consensus-backed task assignment.

### 3.4 Rust Data Structures for Gossip Protocol (`crates/zap-net/src/gossip/`)

```rust
// Proposed in crates/zap-net/src/gossip/mod.rs

pub const GOSSIP_MAGIC: [u8; 4] = *b"ZGSP";
pub const GOSSIP_VERSION: u8 = 1;

pub const GOSSIP_TOPIC_CONSENSUS: &str = "zap.gossip.consensus";
pub const GOSSIP_TOPIC_STATE: &str = "zap.gossip.state";
pub const GOSSIP_TOPIC_CAPABILITIES: &str = "zap.gossip.capabilities";
pub const GOSSIP_TOPIC_HEARTBEAT: &str = "zap.gossip.heartbeat";
pub const GOSSIP_TOPIC_MEMBERSHIP: &str = "zap.gossip.membership";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GossipMessageId(pub [u8; 32]);

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerExchangeRequest {
    pub requester: Uuid,
    pub max_peers_requested: u16,
    pub known_peer_bloom_filter: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerExchangeResponse {
    pub responder: Uuid,
    pub peers: Vec<DiscoveredPeerEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveredPeerEntry {
    pub node_id: Uuid,
    pub public_key: String,
    pub socket_addr: SocketAddr,
    pub transport_key_epoch: u64,
    pub capabilities_digest: [u8; 32],
    pub last_seen_micros: u64,
    pub signature: String,
}
```

---

## 4. Byzantine-Fault-Tolerant (BFT) Swarm Consensus with Dynamic Threshold Signatures (T-of-N)

### 4.1 Consensus Model & Quorum Mathematics

The ZAP Swarm BFT consensus engine provides deterministic, state-machine-replicated decision making across $N$ autonomous nodes under partial synchrony.

- **Fault Tolerance**: Tolerates up to $f$ Byzantine (malicious, compromised, crashed, or equivocating) nodes where:
  $$N \ge 3f + 1 \quad \implies \quad f \le \left\lfloor \frac{N - 1}{3} \right\rfloor$$
- **Quorum Threshold ($T$)**: The minimal number of valid votes required to finalize a consensus round:
  $$T = \left\lfloor \frac{2N}{3} \right\rfloor + 1$$
  *(For $N=3 \implies T=2, f=0$; for $N=4 \implies T=3, f=1$; for $N=7 \implies T=5, f=2$; for $N=10 \implies T=7, f=3$).*
- **Dynamic Adaptability**: When nodes join or leave via approved governance transactions, the active validator set $V_{\text{active}}$ and threshold $T$ are updated at epoch boundaries.

### 4.2 Multi-Stage Consensus Protocol Pipeline

```
  +------------------+
  | Client / Agent   |
  | Submits Action   |
  +--------+---------+
           |
           v
  +------------------+       Gossip Proposal
  | 1. PROPOSE       | --------------------------> [ Swarm Nodes ]
  | (Leader / Origin)|                              Validates State & Policy
  +--------+---------+                                     |
           |                                               v
           | <------------------------------------ +------------------+
           |       Gossip Prevotes (T-of-N)        | 2. PREVOTE       |
           |                                       | (Partial Sig)    |
           v                                       +------------------+
  +------------------+
  | 3. PRECOMMIT     | --------------------------> [ Swarm Nodes ]
  | (Polka Obtained) |                              Collects Precommits
  +--------+---------+                                     |
           |                                               v
           | <------------------------------------ +------------------+
           |       Gossip Precommits (T-of-N)      | 4. COMMIT        |
           |                                       | (Assemble Cert)  |
           v                                       +------------------+
  +------------------+
  | 5. FINALIZE      |
  | Attach Cert to   | ===> Durable Action Execution & Receipt Journaling
  | Frame & Execute  |
  +------------------+
```

1. **Phase 1 — Propose (Round $r$, View $v$)**:
   - The proposer (designated by deterministic rotating round-robin $\text{Leader}(v, r) = V[(v + r) \pmod N]$ or transaction initiator in leaderless mode) packages a batch of candidate frames/actions into a `SwarmProposal`.
   - `SwarmProposal` includes: `proposal_id`, `epoch`, `view`, `round`, `proposer_node`, `block_height`, `payload_digest`, `merkle_root`, `timestamp_micros`, and `proposer_signature`.
   - Broadcast via `zap.gossip.consensus`.
2. **Phase 2 — Prevote / Prepare**:
   - Each node receives the proposal, verifies proposer authority, checks payload hash against local state machine rules and policies.
   - If valid, the node signs a `PrevoteVote` $( \text{epoch}, \text{view}, \text{round}, \text{proposal\_id}, \text{payload\_digest} )$ and gossips it.
   - Nodes collect prevotes until they observe $\ge T$ matching prevotes (**Polka Certificate**).
3. **Phase 3 — Precommit**:
   - Upon observing a Polka Certificate, each node broadcasts a `PrecommitVote` $( \text{epoch}, \text{view}, \text{round}, \text{proposal\_id}, \text{payload\_digest} )$.
4. **Phase 4 — Commit & Dynamic Threshold Signature Aggregation**:
   - When $\ge T$ valid precommit signatures from distinct active validators are collected, they are aggregated into a compact `SwarmCommitCertificate`.
5. **Phase 5 — Execution & Receipt Attachment**:
   - The frame is executed in WASM runtime, and the `SwarmCommitCertificate` is attached to the action receipt and stored in `zap-ledger`.

### 4.3 Compact Dynamic Threshold Signature Representation

Instead of the linear $80M$-byte `PoaTrailer`, the Next-Gen consensus engine introduces the `SwarmConsensusTrailer` (`ZSC1` magic):

```
+--------------------------------------------------------------------------------+
| Magic: 'ZSC1' (4B) | Version (2B) | Epoch (8B) | View (8B) | Round (8B)        |
+--------------------------------------------------------------------------------+
| Block Height (8B)  | Required Threshold T (2B) | Total Validators N (2B)       |
+--------------------------------------------------------------------------------+
| Proposal Hash (32B)| Merkle Root Hash (32B)                                    |
+--------------------------------------------------------------------------------+
| Signer Bitmask: ceil(N/8) bytes (e.g. 8 bytes for up to 64 nodes)              |
+--------------------------------------------------------------------------------+
| Aggregated Signature Payload: T x 64-byte Ed25519 signatures (or BLS/Schnorr)  |
+--------------------------------------------------------------------------------+
```

- **Bitmask Indexing**: Signer identities are encoded as bit positions in `signer_bitmask`, referencing the ordered validator set in the active epoch. This eliminates the 16-byte UUID overhead per signature.
- **Batch Verification**: Verifiers decode the bitmask, resolve public keys in a single slice, and verify all $T$ signatures concurrently using `ed25519_dalek::verify_batch()`, yielding sub-millisecond verification times even for $N=64$.

### 4.4 Quorum Reconfiguration & Equivocation Slashing

1. **Epoch Reconfiguration Transaction (`EpochTransition`)**:
   - Allows dynamic adding, removing, or weight-adjusting of validator nodes.
   - Proposed as a special consensus transaction. Requires $T_{\text{current}}$ signatures to activate at epoch boundary $\text{epoch} + 1$.
2. **Cryptographic Equivocation Detection (`EquivocationProof`)**:
   - If any validator signs two distinct proposals or votes for the same $(\text{epoch}, \text{view}, \text{round})$, any node observing both signatures constructs an `EquivocationProof`:
     $$\text{Proof} = \{ \text{validator\_node}, \text{epoch}, \text{view}, \text{round}, (\text{hash}_1, \text{sig}_1), (\text{hash}_2, \text{sig}_2) \}$$
   - When verified, the offending node is immediately quarantined in `PeerTrustConfig`, evicted from the validator set, and permanently slashed.

### 4.5 Rust Data Structures for Consensus Engine (`crates/zap-net/src/consensus/`)

```rust
// Proposed in crates/zap-net/src/consensus/mod.rs

pub const CONSENSUS_TRAILER_MAGIC: [u8; 4] = *b"ZSC1";
pub const CONSENSUS_TRAILER_VERSION: u16 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VoteKind {
    Prevote,
    Precommit,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EquivocationProof {
    pub offender_node: Uuid,
    pub epoch: u64,
    pub view: u64,
    pub round: u64,
    pub vote_kind: VoteKind,
    pub digest_a: [u8; 32],
    pub signature_a: [u8; 64],
    pub digest_b: [u8; 32],
    pub signature_b: [u8; 64],
}

pub trait SwarmConsensusEngine: Send + Sync {
    fn propose(&mut self, payload_digest: [u8; 32]) -> Result<SwarmProposal>;
    fn handle_proposal(&mut self, proposal: &SwarmProposal) -> Result<Option<SwarmVote>>;
    fn handle_vote(&mut self, vote: &SwarmVote) -> Result<Option<SwarmCommitCertificate>>;
    fn verify_certificate(&self, cert: &SwarmCommitCertificate) -> Result<()>;
}
```

---

## 5. Network Partition Detection, Heartbeats & Dynamic Failover Routing

```
               +--------------------------------------------------+
               |        Adaptive Peer Mesh Health Tracker         |
               +------------------------+-------------------------+
                                        |
               +------------------------+-------------------------+
               |                                                  |
    +----------v----------+                            +----------v----------+
    | Phi Accrual Engine  |                            | Quorum Health Ratio |
    | (Sliding Window RTT)|                            |    R = N_reach / N  |
    +----------+----------+                            +----------+----------+
               |                                                  |
     +---------+---------+                              +---------+---------+
     |                   |                              |                   |
+----v-----+       +-----v----+                   +-----v----+        +-----v----+
|  Alive   |       |  Suspect |                   |  Quorum  |        | Partition|
| (Normal) |       |  (Phi>8) |                   | (Normal) |        | (Degraded|
+----+-----+       +-----+----+                   +----------+        | ReadOnly)|
     |                   |                                            +----------+
     |             +-----v----+
     +------------>|   Dead   | ===> Trigger Multi-Hop Failover Relay
                   | (Phi>12) |
                   +----------+
```

### 5.1 Heartbeat Protocol with Randomized Jitter Backoff

Nodes broadcast lightweight encrypted heartbeat probes (`zap.p2p.heartbeat`) to active peers.

1. **Jittered Scheduling Algorithm**:
   To prevent synchronization resonance (thundering herd), heartbeats calculate next trigger with exponential backoff and randomized uniform jitter:
   $$T_{\text{next}} = \min(T_{\text{max}}, T_{\text{base}} \cdot \gamma^{\text{consecutive\_failures}}) + \text{Uniform}(0, J_{\text{max}})$$
   - Standard parameters: $T_{\text{base}} = 1000\text{ ms}$, $\gamma = 1.5$, $T_{\text{max}} = 15000\text{ ms}$, $J_{\text{max}} = 250\text{ ms}$.
2. **Heartbeat Payload**:
   Carries `sender_node`, `sequence_number`, `current_epoch`, `last_committed_block`, `active_peer_count`, and `timestamp_micros`.
   Receiver immediately returns a signed `HeartbeatAck` with local RTT echo.

### 5.2 Phi Accrual Failure Detector ($\Phi$)

Rather than binary up/down timeouts, ZAP implements the **Phi Accrual Failure Detector** (Hayashibara et al.):
1. Maintain a sliding window of the last $W$ (default $W=100$) heartbeat arrival intervals $\{ \Delta t_1, \dots, \Delta t_W \}$.
2. Compute mean $\mu$ and variance $\sigma^2$ under normal distribution assumption.
3. For elapsed time $t_{\text{now}} - t_{\text{last\_heartbeat}}$, calculate cumulative probability $P_{\text{later}}(t)$ that a heartbeat arrives after $t$:
   $$P_{\text{later}}(t) = \frac{1}{\sigma \sqrt{2\pi}} \int_t^\infty \exp\left( -\frac{(u - \mu)^2}{2\sigma^2} \right) du$$
4. Compute suspicion metric $\Phi$:
   $$\Phi = -\log_{10}(P_{\text{later}}(t))$$
5. State Transitions:
   - $\Phi < \Phi_{\text{suspect}}$ (e.g., 8.0): Peer is **`Alive`** (RTT healthy).
   - $\Phi_{\text{suspect}} \le \Phi < \Phi_{\text{dead}}$ (e.g., 12.0): Peer is **`Suspect`** (degrade route priority, increase probe rate).
   - $\Phi \ge \Phi_{\text{dead}}$ (e.g., 16.0): Peer is **`Dead`** (mark unreachable, trigger failover routing).

### 5.3 Network Partition Detection & Split-Brain Mitigation

1. **Quorum Reachability Ratio ($R$)**:
   Nodes evaluate the reachable validator ratio continuously:
   $$R = \frac{|\{ v \in V_{\text{active}} \mid \text{Status}(v) \neq \text{Dead} \}|}{|V_{\text{active}}|}$$
2. **State Transition on Partition**:
   - If $R \ge \frac{T}{N}$ ($\ge \frac{2}{3}$): Node is in the **Majority Partition**. Consensus continues normally.
   - If $R < \frac{T}{N}$: Node detects **Quorum Loss / Minority Partition**.
     - Transitions immediately to `PartitionDegraded` mode:
       - Refuses new state mutation proposals.
       - Rejects non-idempotent action dispatch.
       - Enters read-only queries with a `PARTITION_WARNING` flag.
3. **Partition Healing & Anti-Entropy Reconciliation**:
   - When heartbeats resume and $R \ge \frac{T}{N}$, node initiates `AntiEntropySync`:
     - Compares its block height and MMR root with the majority swarm.
     - Fetches and applies missed consensus blocks sequentially before resuming normal proposer/voter roles.

### 5.4 Multi-Peer Dynamic Failover Routing

When a direct UDP route to peer $B$ degrades ($\Phi_B \ge 8$) or fails ($\Phi_B \ge 16$), the mesh router calculates an alternative 2-hop relay path through an intermediary peer $C$:

```
[ Node A ] ----- (Degraded / Blocked Direct Path) - - - - > [ Node B ]
     \                                                         ^
      \ (Relay Hop 1: C_AC)                                    / (Relay Hop 2: C_CB)
       \                                                      /
        +------------------> [ Relay Node C ] ---------------+
```

1. **Composite Path Cost Metric**:
   $$\text{Cost}(A \to C \to B) = (\text{RTT}_{AC} + \text{RTT}_{CB}) + \alpha \cdot (\text{Loss}_{AC} + \text{Loss}_{CB}) + \beta \cdot \text{QueuePressure}_C$$
2. **Relay Encapsulation Header (`ZapRelayEnvelope`)**:
   - Node $A$ wraps the original `ZapFrame` in a `ZapRelayEnvelope` targeting Node $C$ with destination $B$.
   - Node $C$ validates forwarding trust permissions (`PeerTrustConfig::allow_forward`) and relays the inner frame to $B$.

---

## 6. Integration Architecture with `zap-agent` and `zap-node`

### 6.1 `zap-agent` Integration

1. **Swarm Agent State Machine (`SwarmAgentCoordinator`)**:
   - Bridges agent intents to swarm consensus.
   - When an agent initiates a high-value action (`IntentKind::Act` or multi-party `Pact`), the coordinator compiles a consensus proposal and dispatches it through the node's swarm consensus engine.
2. **Provenance Chain Stage Integration**:
   - Extends `ProvenanceStep` at `ProvenanceStage::Poa` / `ProvenanceStage::Consensus` to cryptographically store the `SwarmCommitCertificate` hash, epoch, round, and signer bitmask.
   - Ensures end-to-end mathematical proof linking Agent Intent $\to$ Swarm Consensus Finalization $\to$ WASM Execution $\to$ Signed Receipt.

### 6.2 `zap-node` Runtime Daemon Refactoring

```
+-----------------------------------------------------------------------------------+
|                                  ZAP NODE DAEMON                                  |
+-----------------------------------------------------------------------------------+
|                                                                                   |
|  +-----------------------------------------------------------------------------+  |
|  |                           Tokio Task Orchestration                          |  |
|  |                                                                             |  |
|  |  +-------------------+  +-------------------+  +-------------------------+  |  |
|  |  |  UdpRxTask        |  |  GossipDisseminator| |  ConsensusWorkerTask    |  |  |
|  |  |  (Inbound Socket) |  |  (Fanout / Sync)  |  |  (Propose/Prevote/Commit|  |  |
|  |  +---------+---------+  +---------+---------+  +------------+------------+  |  |
|  |            |                      |                         |               |  |
|  |            +----------------------+-------------------------+               |  |
|  |                                   |                                         |  |
|  |  +-------------------+  +---------v---------+  +-------------------------+  |  |
|  |  |  MeshHeartbeatTask|  | Node State Engine |  |  ObservabilityServer    |  |  |
|  |  |  (Phi Detector)   |  | (Routing / WASM)  |  |  (Prometheus / HTTP)    |  |  |
|  |  +-------------------+  +-------------------+  +-------------------------+  |  |
|  +-----------------------------------------------------------------------------+  |
+-----------------------------------------------------------------------------------+
```

1. **Multi-Task Architecture**:
   - `UdpRxTask`: Non-blocking datagram receive, decryption, and message classification (Gossip, Consensus, Unicast Action, Control).
   - `GossipDisseminatorTask`: Manages outbound gossip queues, fanout batching, deduplication tables, and anti-entropy synchronization.
   - `ConsensusWorkerTask`: Evaluates state machine replication, signs prevotes/precommits, aggregates threshold signatures into certificates.
   - `MeshHeartbeatTask`: Drives jittered heartbeats, calculates $\Phi$-accrual scores, updates reachability ratios, triggers partition failovers.
2. **Node Configuration Extensions (`zap.toml`)**:

```toml
[node]
bind = "0.0.0.0:9000"
key_file = ".zap/node.key"
require_signed = true

[swarm]
enabled = true
cluster_id = "zap-mainnet-alpha"
min_quorum_threshold = 3
auto_rebalance = true

[gossip]
fanout = 3
max_hops = 16
anti_entropy_interval_ms = 5000
dedup_cache_size = 65536
bootnodes = [
    "192.168.1.10:9000@01914b10-0000-7000-8000-000000000001",
    "192.168.1.11:9000@01914b10-0000-7000-8000-000000000002"
]

[mesh]
heartbeat_interval_ms = 1000
heartbeat_jitter_ms = 250
phi_suspect_threshold = 8.0
phi_dead_threshold = 14.0
partition_quorum_ratio = 0.67
enable_relay_failover = true
```

---

## 7. Implementation Roadmap & Technical Recommendations

### Phase 1: Gossip Subsystem & Dynamic Mesh Foundation (`zap-net`)
1. Implement `crates/zap-net/src/gossip/` module: `GossipEnvelope`, message deduplication cache, peer exchange (PEX), epidemic fanout dispatcher.
2. Implement `crates/zap-net/src/mesh/` module: `MeshHeartbeat`, Phi Accrual Failure Detector, partition detector, and dynamic 2-hop relay routing.
3. Unit and property test gossip fanout under simulated packet drops.

### Phase 2: Byzantine Swarm Consensus & Threshold Signatures (`zap-net`, `zap-crypto`, `zap-core`)
1. Define `SwarmConsensusTrailer` (`ZSC1`) in `zap-core` and threshold signature verification routines in `zap-crypto`.
2. Implement BFT consensus state machine (`SwarmConsensusEngine`) in `zap-net/src/consensus/` (Propose $\to$ Prevote $\to$ Precommit $\to$ Finalize).
3. Implement dynamic validator set transitions, bitmask signature encoding, and batch Ed25519 verification.

### Phase 3: Agent & Node Daemon Integration (`zap-agent`, `zap-node`, `zap-cli`)
1. Integrate Swarm Coordinator with `zap-agent`, updating `ProvenanceChainEngine` stage `Consensus`.
2. Refactor `ZapNode` daemon into concurrent Tokio actor tasks (`GossipTask`, `ConsensusTask`, `MeshTask`, `ExecutionTask`).
3. Add CLI cluster & swarm simulation commands (`zap cluster up`, `zap swarm bench`, `zap swarm partition-test`).

---

## 8. Verification & Acceptance Criteria Alignment

| Requirement Item | Acceptance Criterion | Verification Method |
| :--- | :--- | :--- |
| **P2P Peer Discovery & Gossip** | Autonomous peer discovery & state propagation across $\ge 3$ nodes. | Spawn $N \ge 3$ in-process nodes with 1 bootnode; verify complete discovery and gossip convergence in $< 500\text{ ms}$. |
| **BFT Swarm Consensus** | Reaches BFT consensus on action proposals with $T$-of-$N$ threshold signatures. | Execute 1,000 consensus rounds across 4 nodes tolerating 1 Byzantine node dropping / corrupting votes. |
| **Dynamic Threshold Signatures** | Compact bitmask encoding + batch Ed25519 verification. | Benchmark verification speed for $N=16, T=11$; assert sub-millisecond verification time. |
| **Partition Detection & Healing** | Detects loss of quorum, enters degraded mode, and reconciles state after healing. | Simulate network partition isolating 2 nodes from a 5-node cluster; assert minority stops state mutation and recovers upon reconnection. |
| **Heartbeats & Dynamic Failover** | Jittered heartbeats, Phi accrual failure detection, and automatic relay routing. | Block direct UDP link between Node A and Node B; verify transparent relay delivery through Node C. |

---
*Report compiled by Explorer 1 — ZAP Next-Gen Frontier Survey Phase.*
