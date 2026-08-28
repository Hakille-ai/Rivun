# Scope: E2E Testing Track (rivun Next-Gen Frontier)

## Architecture & Test Philosophy
- **Opaque-Box & Requirement-Driven**: Tests are designed directly against `ORIGINAL_REQUEST.md` requirements and user-facing CLI/interfaces, independent of internal crate implementation details.
- **4-Tier Comprehensive Methodology**:
  - **Tier 1 - Feature Coverage**: Direct positive functional tests for all 15 features in the Feature Inventory (>=5 tests per feature).
  - **Tier 2 - Boundary & Corner Cases**: Edge cases, extremes, zero/empty inputs, oversized payloads, invalid transitions, timeout limits, and error handling (>=5 tests per feature).
  - **Tier 3 - Cross-Feature Combinations**: Pairwise and multi-module interaction tests (Gossip + Consensus, Consensus + MMR, Pact + Dispute + ZK Rollup, WASM IPC + MMR Sealing, Swarm Chaos + Partition Recovery, etc.).
  - **Tier 4 - Real-World Application Scenarios**: End-to-end multi-agent topologies, decentralized swarm settlements, live cluster benchmarking workloads, Byzantine fault tolerance stress simulations.
- **Test Infrastructure**:
  - Test runner entry point and harness in `tests/e2e/`.
  - Comprehensive documentation in `TEST_INFRA.md`.
  - Readiness publication via `TEST_READY.md`.

## Feature Inventory Coverage (All 15 Features)
| # | Feature | Scope / Requirement | Tier 1 Target | Tier 2 Target | Tier 3 Target | Tier 4 Target |
|---|---------|---------------------|:-------------:|:-------------:|:-------------:|:-------------:|
| 1 | P2P Swarm Gossip Protocol | Epidemic dissemination, peer sampling, deduplication | >=5 | >=5 | Yes | Yes |
| 2 | Swarm Consensus Engine | BFT consensus, T-of-N threshold signatures | >=5 | >=5 | Yes | Yes |
| 3 | Network Partition & Failover Mesh | Phi accrual detector, jitter heartbeats, 2-hop relay | >=5 | >=5 | Yes | Yes |
| 4 | Incremental MMR Accumulator | $O(\log N)$ peak accumulator, peak-bagging root | >=5 | >=5 | Yes | Yes |
| 5 | Compact Batch Receipts & Proofs | Batch inclusion/exclusion proofs, batch sealing | >=5 | >=5 | Yes | Yes |
| 6 | ZK Verifiable Receipt Rollups | Blinded commitments, verifiable zero-knowledge rollups | >=5 | >=5 | Yes | Yes |
| 7 | Async WASM Driver Pipeline | Non-blocking host execution, fuel metering, sandbox | >=5 | >=5 | Yes | Yes |
| 8 | Streaming I/O Buffers | Lock-free circular ring-buffers, TCP/Modbus streaming | >=5 | >=5 | Yes | Yes |
| 9 | Inter-Driver IPC Pipes | Zero-copy pipeline chaining, aggregate fuel budgets | >=5 | >=5 | Yes | Yes |
| 10 | Multi-Party Conditional Pacts | `MultiPartyPact`, escrow locks, multi-sig release | >=5 | >=5 | Yes | Yes |
| 11 | Dispute Resolution Engine | Deterministic adjudication, SLA breaches, slash | >=5 | >=5 | Yes | Yes |
| 12 | Causal Execution Chains | Provenance causal binding (Pact -> Escrow -> MMR) | >=5 | >=5 | Yes | Yes |
| 13 | Cluster Simulator CLI | `rivun cluster up / status / down`, topology manager | >=5 | >=5 | Yes | Yes |
| 14 | Swarm Benchmarking Tooling | `rivun swarm bench / partition-test`, chaos fixtures | >=5 | >=5 | Yes | Yes |
| 15 | E2E Integration & Audit | 100% E2E test pass, zero clippy warnings, integrity | >=5 | >=5 | Yes | Yes |

## Milestones & Work Items
| # | Milestone | Scope | Deliverables | Status |
|---|-----------|-------|--------------|--------|
| 1 | Test Harness & Framework | Standalone test harness, assertions, mock environments, runner | `tests/e2e/harness.rs`, `tests/e2e/mod.rs` | IN_PROGRESS |
| 2 | Tier 1: Feature Coverage | >=5 tests per feature (Features 1-15) | `tests/e2e/tier1_feature_tests.rs` (75+ tests) | IN_PROGRESS |
| 3 | Tier 2: Boundary & Corner | >=5 tests per feature (Features 1-15) | `tests/e2e/tier2_boundary_tests.rs` (75+ tests) | IN_PROGRESS |
| 4 | Tier 3: Cross-Feature Interactions | Pairwise & cross-module integration tests | `tests/e2e/tier3_combination_tests.rs` (15+ tests) | IN_PROGRESS |
| 5 | Tier 4: Real-World Scenarios | High-complexity multi-agent & cluster workloads | `tests/e2e/tier4_realworld_tests.rs` (8+ scenarios) | IN_PROGRESS |
| 6 | Infrastructure & Readiness Signaling | Complete E2E docs and runner commands | `TEST_INFRA.md`, `TEST_READY.md` | IN_PROGRESS |

## Write Ownership
- `tests/e2e/**`: Owned exclusively by E2E test worker
- `TEST_INFRA.md`: Project root E2E documentation
- `TEST_READY.md`: Project root readiness signal

