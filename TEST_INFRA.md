# ZAP Next-Gen Frontier E2E Test Infrastructure (`TEST_INFRA.md`)

## 1. Executive Summary

The ZAP End-to-End (E2E) Test Suite (`zap-e2e`) provides a comprehensive, requirement-driven, opaque-box testing framework for the ZAP Next-Gen Frontier decentralized mesh runtime. The test suite exercises all 15 features defined in `PROJECT.md § Feature Inventory` across four structured testing tiers without mocking or modifying core production logic.

## 2. Test Architecture

The E2E test harness resides in `tests/e2e` as a first-class workspace member (`zap-e2e`).

```
tests/e2e/
├── Cargo.toml                   # Workspace test crate configuration & dependencies
├── src/
│   ├── lib.rs                   # Crate export and version metadata
│   └── harness.rs               # In-process mock cluster, WASM fixtures, assertion helpers
└── tests/
    ├── e2e_suite.rs             # Master test runner integrating all 4 tiers
    ├── tier1_feature_tests.rs   # Tier 1: 75 positive functional tests (5 per feature)
    ├── tier2_boundary_tests.rs  # Tier 2: 75 boundary, negative & corner case tests (5 per feature)
    ├── tier3_combination_tests.rs # Tier 3: 15 cross-feature interaction & pairwise tests
    └── tier4_realworld_tests.rs # Tier 4: 8 multi-agent real-world decentralized workloads
```

### 2.1 Harness Components (`tests/e2e/src/harness.rs`)

1. **`SimulatedNode`**:
   - In-memory node instance with dedicated Ed25519 `Keypair`, ephemeral UDP socket address, `GossipMesh`, `ReceiptJournalStore`, `MemoryJournalStore`, and `FleetTopology`.
   - Helper `record_action(action, payload)` for generating genuine signed receipts.
2. **`SimulatedCluster`**:
   - Manages an N-node decentralized swarm topology.
   - Cross-registers node endpoints in gossip meshes and fleet topologies.
   - `broadcast_heartbeat(node_id, load_factor)` for clock synchronization and failure detection testing.
   - `simulate_partition(isolated_ids)` for testing split-brain, Phi Accrual dead transitions, and failover.
   - `reach_consensus(proposer_id, topic, terms_hash, voters)` for $T$-of-$N$ quorum proposal generation and vote finalization.
3. **WASM Bytecode Fixture Generators**:
   - `compile_echo_wasm()`: Compiles standard WAT echo driver ABI (`zap_alloc`, `zap_dealloc`, `zap_execute`).
   - `compile_reverse_wasm()`: Compiles transforming WAT driver that reverses arbitrary payload byte buffers.

---

## 3. Four-Tier Test Strategy

| Tier | Purpose | Coverage Criteria | Test Count |
|---|---|---|---|
| **Tier 1: Feature Coverage** | Direct positive functional verification for all 15 features | $\ge 5$ tests per feature | **75 tests** |
| **Tier 2: Boundary & Corner Cases** | Negative testing, invalid schemas, timeouts, signature tampering, corrupt hashes, fuel limits | $\ge 5$ tests per feature | **75 tests** |
| **Tier 3: Cross-Feature Combinations** | Multi-crate integration, pairwise flows (e.g. Gossip + Consensus + MMR + Provenance) | Complex multi-stage flows | **15 tests** |
| **Tier 4: Real-World Application Workloads** | Full end-to-end multi-agent topologies and realistic workflows | Swarm settlement, chaos healing, SLA dispute arbitration | **8 tests** |
| **Sanity & Harness Checks** | Master harness metadata and cryptographic sanity check | Infrastructure verification | **1 test** |
| **Total** | | | **174 tests** |

---

## 4. Feature Inventory Mapping

| Feature # | Feature Name | Tier 1 Tests | Tier 2 Tests | Tier 3 & 4 Highlights |
|---|---|---|---|---|
| **F01** | P2P Swarm Gossip Protocol | `tc_f01_01` .. `tc_f01_05` | `tc_f01_06` .. `tc_f01_10` | `tc_t3_01`, `tc_t3_10`, `tc_t4_07` |
| **F02** | Swarm Consensus Engine | `tc_f02_01` .. `tc_f02_05` | `tc_f02_06` .. `tc_f02_10` | `tc_t3_02`, `tc_t3_11`, `tc_t4_05` |
| **F03** | Network Partition & Failover Mesh | `tc_f03_01` .. `tc_f03_05` | `tc_f03_06` .. `tc_f03_10` | `tc_t3_03`, `tc_t4_03` |
| **F04** | Incremental MMR Accumulator | `tc_f04_01` .. `tc_f04_05` | `tc_f04_06` .. `tc_f04_10` | `tc_t3_05`, `tc_t4_04` |
| **F05** | Compact Batch Receipts & Proofs | `tc_f05_01` .. `tc_f05_05` | `tc_f05_06` .. `tc_f05_10` | `tc_t3_04`, `tc_t3_14` |
| **F06** | ZK Verifiable Receipt Rollups | `tc_f06_01` .. `tc_f06_05` | `tc_f06_06` .. `tc_f06_10` | `tc_t3_05`, `tc_t4_01`, `tc_t4_04` |
| **F07** | Async WASM Driver Pipeline | `tc_f07_01` .. `tc_f07_05` | `tc_f07_06` .. `tc_f07_10` | `tc_t3_04`, `tc_t3_13`, `tc_t4_02` |
| **F08** | Streaming I/O Buffers | `tc_f08_01` .. `tc_f08_05` | `tc_f08_06` .. `tc_f08_10` | `tc_t3_09`, `tc_t4_02` |
| **F09** | Inter-Driver IPC Pipes | `tc_f09_01` .. `tc_f09_05` | `tc_f09_06` .. `tc_f09_10` | `tc_t3_06`, `tc_t4_02` |
| **F10** | Multi-Party Conditional Pacts | `tc_f10_01` .. `tc_f10_05` | `tc_f10_06` .. `tc_f10_10` | `tc_t3_07`, `tc_t3_08`, `tc_t4_01`, `tc_t4_06` |
| **F11** | Dispute Resolution Engine | `tc_f11_01` .. `tc_f11_05` | `tc_f11_06` .. `tc_f11_10` | `tc_t3_07`, `tc_t3_12`, `tc_t4_06` |
| **F12** | Causal Execution Chains | `tc_f12_01` .. `tc_f12_05` | `tc_f12_06` .. `tc_f12_10` | `tc_t3_06`, `tc_t3_13`, `tc_t4_01`, `tc_t4_02` |
| **F13** | Cluster Simulator CLI | `tc_f13_01` .. `tc_f13_05` | `tc_f13_06` .. `tc_f13_10` | `tc_t3_10`, `tc_t4_03`, `tc_t4_08` |
| **F14** | Swarm Benchmarking Tooling | `tc_f14_01` .. `tc_f14_05` | `tc_f14_06` .. `tc_f14_10` | `tc_t3_15`, `tc_t4_04`, `tc_t4_08` |
| **F15** | E2E Integration & Audit | `tc_f15_01` .. `tc_f15_05` | `tc_f15_06` .. `tc_f15_10` | `tc_t4_01` .. `tc_t4_08` |

---

## 5. Execution Commands

### Run Full E2E Test Suite
```bash
cargo test -p zap-e2e
```

### Run Specific Test Tiers
```bash
# Run Tier 1 Feature Tests
cargo test -p zap-e2e --test e2e tier1_feature_tests

# Run Tier 2 Boundary & Negative Tests
cargo test -p zap-e2e --test e2e tier2_boundary_tests

# Run Tier 3 Cross-Feature Combination Tests
cargo test -p zap-e2e --test e2e tier3_combination_tests

# Run Tier 4 Real-World Application Workload Scenarios
cargo test -p zap-e2e --test e2e tier4_realworld_tests
```

### Run Specific Real-World Scenario
```bash
cargo test -p zap-e2e tc_t4_01_autonomous_multi_agent_swarm_resource_settlement
```

---

## 6. Integrity and Verification Mandate

All tests in `zap-e2e` perform actual cryptographic signature generation, Blake3 hashing, WASM compilation and execution in Wasmtime, Merkle mountain range root computations, vector clock increments, and ChaCha20-Poly1305 frame encryption/decryption. No hardcoded results, mocked stubs, or bypasses are used.
