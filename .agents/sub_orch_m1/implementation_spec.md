# Milestone 1 (R1) Implementation Specification

## Overview
Milestone 1 implements the complete P2P Swarm Gossip Consensus, Adaptive Quorum Mesh, Swarm Agent Coordinator, and Concurrent Tokio Node Actor Daemon for the rivun Next-Gen Frontier project.

## Target Crates & File Boundaries
1. **`crates/rivun-net/`**:
   - `Cargo.toml`: Add necessary workspace dependencies (`ed25519-dalek` with batch feature, `rivun-crypto` if needed).
   - `src/gossip/mod.rs` (or `src/gossip/*.rs`):
     - Wire magic `ZGSP`, version 1.
     - `GossipMessageId`, `GossipEnvelope` (origin, topic, sequence, max_hops, current_hop, timestamp, payload, signature).
     - `GossipDeduplicationCache` (LRU 65,536 entries with 60s TTL).
     - Peer Exchange (PEX) messages (`PeerExchangeRequest`, `PeerExchangeResponse`, `DiscoveredPeerEntry`).
     - Anti-entropy sync digests (`StateDigest`, `DigestReconciliation`).
     - `SwarmGossipEngine` trait & concrete implementation.
   - `src/consensus/mod.rs` (or `src/consensus/*.rs`):
     - Wire magic `ZSC1`, version 1.
     - `VoteKind` (`Prevote`, `Precommit`), `SwarmProposal`, `SwarmVote`, `SwarmCommitCertificate`.
     - Bitmask signer indexing (`ceil(N/8)` bytes) and batch Ed25519 threshold verification (`ed25519_dalek::verify_batch`).
     - Dynamic validator set transitions (`ValidatorSet`, epoch reconfiguration).
     - Equivocation detection & slashing proof (`EquivocationProof`).
     - `SwarmConsensusEngine` trait & BFT state machine implementation.
   - `src/mesh/mod.rs` (or `src/mesh/*.rs`):
     - `PhiAccrualDetector` with continuous Gaussian normal distribution ($\mu, \sigma^2$) sliding window ($W=100$) and erf calculation.
     - Jittered exponential heartbeat scheduler ($T_{\text{next}} = \min(T_{\text{max}}, T_{\text{base}} \cdot \gamma^k) + \text{Uniform}(0, J_{\text{max}})$).
     - Split-brain partition detector ($R = N_{\text{reach}} / N$, threshold $T/N$), `PartitionStatus` (`Normal`, `DegradedReadOnly`, `Healing`).
     - Dynamic 2-hop failover relay routing (`ZapRelayEnvelope` with magic `ZRLY`, forwarding trust checks, loop prevention).
   - `src/lib.rs`: Expose modules, maintain full backwards compatibility for `ZapEndpoint` and existing public API.

2. **`crates/rivun-agent/`**:
   - `src/swarm.rs`:
     - `SwarmAgentCoordinator` managing `AgentIntent` lifecycle through consensus.
     - `SwarmCapabilityIndex` with multi-factor scoring formula ($\text{Score} = w_1 \cdot \text{Trust} + w_2 \cdot (1 - \text{Latency}) + w_3 \cdot \text{Load}$).
     - `SwarmIntentProposal`, `SwarmCommitCertificateRef`.
   - `src/provenance.rs`:
     - Extend `ProvenanceStage` with `Consensus` variant.
     - Add `ProvenanceChainBuilder::with_consensus()` cryptographically binding certificate hash, epoch, round, threshold, validator count, and signer bitmask.
   - `src/lib.rs`: Expose `swarm` module and updated provenance methods.

3. **`crates/rivun-node/`**:
   - `src/config.rs`: Add `SwarmConfig`, `GossipConfig`, `MeshConfig` to `ZapNodeConfig` with `#[serde(default)]`.
   - `src/node.rs` / `src/actors/`:
     - Concurrent Tokio actor daemon architecture: `UdpRxTask`, `GossipTask`, `ConsensusTask`, `MeshTask`, `ExecutionTask`.
     - Structured graceful shutdown.
   - `src/lib.rs`: Expose node actors and configuration.

4. **Testing & Verification**:
   - Comprehensive unit and integration tests across all three crates.
   - Deterministic test fixtures (`MockSwarmRouter` or in-process multi-node clusters) testing:
     - Gossip fanout & anti-entropy sync under packet loss.
     - BFT 4-phase consensus, leader rotation, 1-node Byzantine fault tolerance ($f=1$), and equivocation slashing.
     - Phi accrual detector accuracy & jittered heartbeats.
     - Network partition degradation & post-partition healing.
     - Dynamic 2-hop relay routing.
     - Provenance consensus chain verification.
   - Ensure `cargo test -p rivun-net -p rivun-agent -p rivun-node` passes with 0 failures and `cargo clippy` has 0 warnings.

