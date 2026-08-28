# Scope: Milestone 1 (R1) - P2P Swarm Gossip Consensus & Adaptive Quorum Mesh

## Target Modules & Write Boundaries
- `crates/rivun-net/`: Epidemic gossip protocol (`src/gossip/`), BFT swarm consensus state machine with dynamic threshold signatures (`src/consensus/`), adaptive quorum mesh & failure detector (`src/mesh/`), dynamic 2-hop relay routing.
- `crates/rivun-agent/`: Swarm agent coordinator (`src/swarm.rs`), consensus commit certificate recording in provenance chain (`src/provenance.rs`).
- `crates/rivun-node/`: Concurrent Tokio actor daemon refactor (`UdpRxTask`, `GossipTask`, `ConsensusTask`, `MeshTask`), node config extensions (`rivun.toml`).

## Feature Inventory
| # | Feature | Description | Milestone | Source |
|---|---------|-------------|-----------|--------|
| 1 | P2P Swarm Gossip Protocol | Epidemic gossip dissemination with k-fanout, message deduplication cache, peer sampling (PEX), and anti-entropy sync | M1 | ORIGINAL_REQUEST §R1 |
| 2 | Swarm Consensus Engine | Byzantine-fault-tolerant swarm consensus (Propose, Prevote, Precommit, Commit) with dynamic threshold signatures (T-of-N) | M1 | ORIGINAL_REQUEST §R1 |
| 3 | Network Partition & Failover Mesh | Phi Accrual Failure Detector, randomized jitter heartbeats, split-brain partition detection, dynamic 2-hop relay routing | M1 | ORIGINAL_REQUEST §R1 |

## Interface Contracts
### `rivun-net` <-> `rivun-node` / `rivun-agent`
- `SwarmGossipEngine`: `broadcast_state(payload: Vec<u8>) -> Result<GossipReceipt, NetError>`
- `SwarmConsensusEngine`: `propose(round: u64, proposal: Vec<u8>) -> Result<ConsensusCertificate, ConsensusError>`
- `MeshTopology`: `get_peer_health(peer: &Uuid) -> PeerHealthState`, `detect_partition() -> PartitionStatus`

## Verification & Acceptance
- `cargo test -p rivun-net -p rivun-agent -p rivun-node` passes with 0 failures
- `cargo clippy -p rivun-net -p rivun-agent -p rivun-node -- -D warnings` runs with 0 warnings
- Rigorous unit & integration tests covering gossip, consensus state machine, threshold signatures, phi accrual detector, partition detection, and 2-hop relay routing.

