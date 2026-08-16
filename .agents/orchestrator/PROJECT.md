# Project: ZAP Next-Gen Frontier

## Architecture
ZAP Next-Gen Frontier transforms the ZAP architecture into an autonomous, hyper-scalable, cross-cluster decentralized execution and verification fabric.

```
+---------------------------------------------------------------------------------------+
|                                    ZAP CLI & NODE                                     |
|  zap cluster up / status / down  |  zap swarm bench / partition-test  |  zap run / ...  |
+---------------------------------------------------------------------------------------+
        |                                       |                                   |
        v                                       v                                   v
+------------------+                 +---------------------+               +-------------------+
|     zap-net      |                 |      zap-pact       |               |    zap-runtime    |
| - Gossip protocol| <--- mesh ----> | - MultiPartyPact    | <--- pipes - >| - AsyncWasmExec   |
| - BFT Consensus  |      pacts      | - Escrow lock/slash |               | - Stream Buffers  |
| - Failover mesh  |                 | - Dispute resolver  |               | - Zero-copy IPC   |
+------------------+                 +---------------------+               +-------------------+
        |                                       |                                   |
        v                                       v                                   v
+------------------+                 +---------------------+               +-------------------+
|    zap-agent     |                 |     zap-policy      |               |  zap-driver-sdk   |
| - Swarm coordinator                | - Dispute evaluator |               | - AsyncDriver     |
| - Causal chains  |                 | - Multi-party rule  |               | - Pinned buffers  |
+------------------+                 +---------------------+               +-------------------+
        \                                       |                                  /
         \--------------------------------------+---------------------------------/
                                                |
                                                v
                                     +---------------------+
                                     | zap-ledger & crypto |
                                     | - Incremental MMR   |
                                     | - Batch seal/proof  |
                                     | - ZK receipt rollup |
                                     | - Threshold sigs    |
                                     +---------------------+
```

## Feature Inventory
| # | Feature | Description | Milestone | Source |
|---|---------|-------------|-----------|--------|
| 1 | P2P Swarm Gossip Protocol | Epidemic gossip dissemination with k-fanout, message deduplication cache, peer sampling (PEX), and anti-entropy sync | M1 | ORIGINAL_REQUEST §R1 |
| 2 | Swarm Consensus Engine | Byzantine-fault-tolerant swarm consensus (Propose, Prevote, Precommit, Commit) with dynamic threshold signatures (T-of-N) | M1 | ORIGINAL_REQUEST §R1 |
| 3 | Network Partition & Failover Mesh | Phi Accrual Failure Detector, randomized jitter heartbeats, split-brain partition detection, dynamic 2-hop relay routing | M1 | ORIGINAL_REQUEST §R1 |
| 4 | Incremental MMR Accumulator | Merkle Mountain Range $O(\log N)$ peak accumulator with disk persistence, peak-bagging root calculation | M2 | ORIGINAL_REQUEST §R2 |
| 5 | Compact Batch Receipts & Proofs | Batch inclusion proofs, exclusion/non-membership proofs, cryptographic batch sealing | M2 | ORIGINAL_REQUEST §R2 |
| 6 | ZK Verifiable Receipt Rollups | Blinded commitments and verifiable execution rollups proving correctness without exposing private payload contents | M2 | ORIGINAL_REQUEST §R2 |
| 7 | Async WASM Driver Pipeline | Non-blocking asynchronous WASM driver host execution on Tokio tasks with memory sandboxing and strict fuel metering | M3 | ORIGINAL_REQUEST §R3 |
| 8 | Streaming I/O Buffers | Lock-free circular ring-buffers supporting async streaming I/O (TCP, Modbus, Ring-Buffers) | M3 | ORIGINAL_REQUEST §R3 |
| 9 | Inter-Driver IPC Pipes | Deterministic zero-copy inter-driver IPC chaining (Perception -> Policy -> Actuator) with aggregate fuel budgeting | M3 | ORIGINAL_REQUEST §R3 |
| 10 | Multi-Party Conditional Pacts | `MultiPartyPact`, multi-participant escrow locks, timeout slashes, and multi-signature release conditions | M4 | ORIGINAL_REQUEST §R4 |
| 11 | Dispute Resolution Engine | Deterministic dispute adjudication in `zap-policy` resolving SLA breaches, timeout claims, and payout distributions | M4 | ORIGINAL_REQUEST §R4 |
| 12 | Causal Execution Chains | Full provenance causal binding linking negotiation pacts, resource allocations, signed attestations, and settlement receipts | M4 | ORIGINAL_REQUEST §R4 |
| 13 | Cluster Simulator CLI | `zap cluster up --nodes N`, `zap cluster status`, `zap cluster down` managing in-process and multi-process topologies | M5 | ORIGINAL_REQUEST §R5 |
| 14 | Swarm Benchmarking Tooling | `zap swarm bench --rate R --duration D`, `zap swarm partition-test`, stress fixtures for 10,000+ consensus ops/sec under chaos | M5 | ORIGINAL_REQUEST §R5 |
| 15 | E2E Integration & Audit | 100% E2E test suite passing (Tiers 1-4), Tier 5 adversarial hardening, zero-warning clippy and zero-failure test suite | M6 / Final | ORIGINAL_REQUEST Acceptance Criteria |

## Milestones
| # | Name | Scope | Dependencies | Status |
|---|------|-------|-------------|--------|
| E2E | E2E Testing Track | Independent opaque-box test suite (Tiers 1-4) covering all 15 features | none | IN_PROGRESS |
| M1 | R1: P2P Swarm Gossip & Quorum Mesh | `crates/zap-net`, `crates/zap-agent`, `crates/zap-node` | none | PLANNED |
| M2 | R2: MMR & Compact Cryptographic Receipts | `crates/zap-ledger`, `crates/zap-crypto` | none | PLANNED |
| M3 | R3: Async WASM Driver Pipeline & IPC | `crates/zap-runtime`, `crates/zap-driver-sdk` | none | PLANNED |
| M4 | R4: Decentralized Agent Pact & Dispute Engine | `crates/zap-pact`, `crates/zap-policy`, `crates/zap-agent` | M1, M2 | PLANNED |
| M5 | R5: Cluster Simulator & Swarm Tooling | `crates/zap-cli`, `crates/zap-telemetry`, `benches/`, `tests/` | M1, M2, M3, M4 | PLANNED |
| M6 | Final: E2E Verification & Adversarial Hardening | Full workspace validation, 100% test pass, Tier 5 hardening, clippy zero warnings | E2E, M1, M2, M3, M4, M5 | PLANNED |

## Code Layout & Write Boundaries
- `crates/zap-net/`: Owned exclusively by M1 worker (Gossip, Consensus, Mesh modules)
- `crates/zap-agent/`: Shared by M1 (consensus binding) and M4 (pact provenance) with separate files (`src/swarm.rs` vs `src/provenance.rs`)
- `crates/zap-node/`: Owned by M1 (daemon actor refactor) and M5 (cluster runner integration)
- `crates/zap-ledger/`: Owned exclusively by M2 worker (`mmr.rs`, `batch.rs`, `zk.rs`)
- `crates/zap-crypto/`: Owned exclusively by M2 worker (batch threshold signatures, blinded commitments)
- `crates/zap-runtime/`: Owned exclusively by M3 worker (`async_engine.rs`, `ipc.rs`, `streaming.rs`)
- `crates/zap-driver-sdk/`: Owned exclusively by M3 worker (`async_driver.rs`, `ring_buffer.rs`)
- `crates/zap-pact/`: Owned exclusively by M4 worker (`multi_party.rs`, `escrow.rs`, `dispute.rs`)
- `crates/zap-policy/`: Owned exclusively by M4 worker (`dispute_eval.rs`)
- `crates/zap-cli/`: Owned exclusively by M5 worker (`src/commands/cluster.rs`, `src/commands/swarm.rs`)
- `crates/zap-telemetry/`: Owned exclusively by M5 worker (metrics export)
- `tests/e2e/`: Owned exclusively by E2E Testing Track worker
- `benches/`: Owned by M5 worker

## Interface Contracts
### `zap-net` <-> `zap-node` / `zap-agent`
- `SwarmGossipEngine`: `broadcast_state(payload: Vec<u8>) -> Result<GossipReceipt, NetError>`
- `SwarmConsensusEngine`: `propose(round: u64, proposal: Vec<u8>) -> Result<ConsensusCertificate, ConsensusError>`
- `MeshTopology`: `get_peer_health(peer: &Uuid) -> PeerHealthState`, `detect_partition() -> PartitionStatus`

### `zap-ledger` <-> `zap-crypto` <-> `zap-node`
- `IncrementalMmr`: `append_receipt(receipt: &SignedActionReceipt) -> Result<MmrLeafIndex, LedgerError>`, `get_root() -> MmrHash`
- `MmrBatchInclusionProof`: `verify(&self, root: &MmrHash) -> bool`
- `ZkReceiptBatchProof`: `generate_rollup(receipts: &[SignedActionReceipt]) -> ZkReceiptBatchProof`, `verify(&self, root: &MmrHash) -> bool`

### `zap-runtime` <-> `zap-driver-sdk`
- `AsyncWasmExecutor`: `execute_async(&self, driver: &AsyncZapDriver, input: &[u8], fuel_budget: u64) -> Result<DriverOutput, RuntimeError>`
- `DriverPipeline`: `pipe(stages: &[DriverStage]) -> Result<CompositeReceipt, RuntimeError>`
- `StreamingBufferPool`: `acquire_ring_buffer(capacity: usize) -> Arc<SpscRingBuffer>`

### `zap-pact` <-> `zap-policy` <-> `zap-agent`
- `MultiPartyPact`: `lock_escrow(&self, deposit: PactEscrowDeposit) -> Result<EscrowLockReceipt, PactError>`
- `DisputeEngine`: `evaluate_dispute(pact: &MultiPartyPact, claims: &[DisputeClaim]) -> DisputeMediationResult`
- `ProvenanceStage`: `PactCommit(Hash)`, `EscrowLock(Hash)`, `DisputeMediation(Hash)`, `MmrCommitment(Hash)`
