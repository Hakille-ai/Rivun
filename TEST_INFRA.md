# Rivun Next-Gen Frontier E2E Test Infrastructure (`TEST_INFRA.md`)

## 1. Executive Summary & Test Philosophy

The Rivun End-to-End (E2E) Test Suite provides a rigorous, requirement-driven, opaque-box testing framework for the Rivun decentralized agent protocol stack, marketing portal, documentation platform, and cloud architecture. The test suite exercises all **25 features** defined in `PROJECT.md` and `crate_and_protocol_specs.md` across four structured testing tiers without mocking or circumventing core protocol, cryptographic, or ledger invariants.

### Core Testing Invariants
- **Genuine Binary Codecs**: Strict big-endian 64-byte `RivunHeader`, 74-byte `ZENV` universal envelopes, Ed25519 `AuthTrailer` (`ZSIG`), and Proof-of-Action `PoaTrailer` (`ZPOA`).
- **Cryptographic Ground Truth**: Real Ed25519 signature validation (RFC 8032), standard BLAKE3 hash trees with domain separation prefixes (`Rivun-NODE-ID-v1`, `Rivun-POA-DIGEST-v1`, `Rivun-BLINDED-COMMITMENT-v1`), and ChaCha20-Poly1305 AEAD datagram transport.
- **BFT Consensus & Ledger Invariants**: 2-Phase BFT consensus state machine with $T = \lfloor 2N/3 \rfloor + 1$ quorum thresholds, bitmask attestation aggregation, equivocation detection & slashing, and carry-over Merkle Mountain Range (MMR) peak folding.
- **WASM Guest Sandbox & Streaming**: Linear memory allocation bounds, fuel metering, epoch timeout interrupts, and SPSC lock-free circular ring buffers with backpressure policies.

---

## 2. Test Architecture

The E2E test harness and test suites reside in `tests/e2e/`:

```
tests/e2e/
├── harness/
│   ├── blake3.mjs          # Standard BLAKE3 cryptographic hash engine
│   ├── crypto.mjs          # Ed25519, UUID v8, blinded commitments, ChaCha20-Poly1305
│   ├── wireCodec.mjs       # 64B wire header, frame, trailers, signing & PoA certification
│   ├── zenvCodec.mjs       # 74B universal envelope codec (8 message kinds)
│   ├── consensus.mjs       # 2-Phase BFT consensus engine & equivocation slashing
│   ├── mmr.mjs             # Merkle Mountain Range accumulator & inclusion/batch proofs
│   ├── wasmSim.mjs         # WASM guest sandbox simulator with fuel metering & ABI v1
│   ├── spscRingBuffer.mjs  # Lock-free SPSC circular ring buffer & backpressure policies
│   ├── provenance.mjs      # Causal execution DAG provenance chain & root signing
│   ├── pactDispute.mjs     # Multi-party conditional escrow PACT with arbitration
│   ├── domainPacks.mjs     # Manifests and risk matrices for all 7 official domain packs
│   ├── doctor.mjs          # 7-Point Fleet Doctor diagnostics engine
│   ├── searchEngine.mjs    # Inverted full-text search index with BM25 ranking
│   ├── pricingEngine.mjs   # 4-tier pricing model, volume sliders, SLA model & ROI analysis
│   └── assert.mjs          # Unit & integration assertions library
├── tier1-features.test.mjs    # Tier 1: 125 functional feature tests (5 per feature)
├── tier2-boundaries.test.mjs  # Tier 2: 125 boundary, negative & corner case tests (5 per feature)
├── tier3-integration.test.mjs # Tier 3: 20 cross-feature integration flows
├── tier4-scenarios.test.mjs   # Tier 4: 10 real-world multi-agent decentralized workloads
└── test-runner.mjs            # Standalone master test runner executing all 280 tests
```

---

## 3. Four-Tier Testing Strategy

| Tier | Purpose | Coverage Criteria | Test Count |
|---|---|---|---|
| **Tier 1: Feature Coverage** | Direct positive functional verification for all 25 features | $\ge 5$ tests per feature | **125 tests** |
| **Tier 2: Boundary & Corner Cases** | Negative testing, invalid schemas, timeouts, signature tampering, corrupt hashes, fuel limits | $\ge 5$ tests per feature | **125 tests** |
| **Tier 3: Cross-Feature Integrations** | Multi-crate interaction, pairwise flows (e.g. Wire + ZENV + Consensus + MMR + WASM) | Complex multi-stage flows | **20 tests** |
| **Tier 4: Real-World Workloads** | Full end-to-end multi-agent topologies and realistic workflows | DevOps PR, HVAC incident, SCADA e-stop, EHR PHI, Arbitrage | **10 tests** |
| **Total** | | | **280 tests** |

---

## 4. Comprehensive 25-Feature Inventory Matrix

| Feature # | Feature Name | Tier 1 Tests | Tier 2 Tests | Tier 3 & 4 Highlights |
|---|---|---|---|---|
| **F01** | Marketing Hero & Signed Frame Visualizer | `tc_f01_01` .. `tc_f01_05` | `tc_b01_01` .. `tc_b01_05` | `tc_t3_01`, `tc_t3_03`, `tc_t4_01` |
| **F02** | P2P Swarm & Gossip Particle Mesh | `tc_f02_01` .. `tc_f02_05` | `tc_b02_01` .. `tc_b02_05` | `tc_t3_01`, `tc_t4_06`, `tc_t4_07` |
| **F03** | 5 Core Protocol Innovations Showcase | `tc_f03_01` .. `tc_f03_05` | `tc_b03_01` .. `tc_b03_05` | `tc_t3_03`, `tc_t3_06`, `tc_t4_04` |
| **F04** | Rivun Cloud SaaS & Operator Workstation | `tc_f04_01` .. `tc_f04_05` | `tc_b04_01` .. `tc_b04_05` | `tc_t3_12`, `tc_t3_19`, `tc_t4_10` |
| **F05** | 7 Domain Packs Showcase | `tc_f05_01` .. `tc_f05_05` | `tc_b05_01` .. `tc_b05_05` | `tc_t3_08`, `tc_t3_10`, `tc_t4_01` |
| **F06** | Enterprise Security & Compliance Matrix | `tc_f06_01` .. `tc_f06_05` | `tc_b06_01` .. `tc_b06_05` | `tc_t3_13`, `tc_t4_04`, `tc_t4_10` |
| **F07** | Interactive Pricing & ROI Calculator | `tc_f07_01` .. `tc_f07_05` | `tc_b07_01` .. `tc_b07_05` | `tc_t3_11`, `tc_t4_10` |
| **F08** | Live Developer Sandbox & Code Gen | `tc_f08_01` .. `tc_f08_05` | `tc_b08_01` .. `tc_b08_05` | `tc_t3_02`, `tc_t4_01` |
| **F09** | Apple-Grade Aesthetics & Navigation | `tc_f09_01` .. `tc_f09_05` | `tc_b09_01` .. `tc_b09_05` | `tc_t3_10` |
| **F10** | Instant Client-Side Full-Text Search | `tc_f10_01` .. `tc_f10_05` | `tc_b10_01` .. `tc_b10_05` | `tc_t3_10` |
| **F11** | Multi-Level Sidebar & Scroll-Spy TOC | `tc_f11_01` .. `tc_f11_05` | `tc_b11_01` .. `tc_b11_05` | `tc_t3_10` |
| **F12** | Multi-Language Code Tabs & Callouts | `tc_f12_01` .. `tc_f12_05` | `tc_b12_01` .. `tc_b12_05` | `tc_t3_08` |
| **F13** | Mermaid & KaTeX Diagram Renderers | `tc_f13_01` .. `tc_f13_05` | `tc_b13_01` .. `tc_b13_05` | `tc_t3_04` |
| **F14** | Architecture & Core Protocol Specs | `tc_f14_01` .. `tc_f14_05` | `tc_b14_01` .. `tc_b14_05` | `tc_t3_01`, `tc_t3_02`, `tc_t3_06` |
| **F15** | Consensus Engine & BFT Quorum Docs | `tc_f15_01` .. `tc_f15_05` | `tc_b15_01` .. `tc_b15_05` | `tc_t3_04`, `tc_t3_15`, `tc_t4_08` |
| **F16** | WASM Sandbox & Zero-Copy Streaming | `tc_f16_01` .. `tc_f16_05` | `tc_b16_01` .. `tc_b16_05` | `tc_t3_05`, `tc_t3_14`, `tc_t4_05` |
| **F17** | Rivun Cloud SaaS & Key Vault Docs | `tc_f17_01` .. `tc_f17_05` | `tc_b17_01` .. `tc_b17_05` | `tc_t3_12`, `tc_t3_19` |
| **F18** | 26 Workspace Crates API Reference | `tc_f18_01` .. `tc_f18_05` | `tc_b18_01` .. `tc_b18_05` | `tc_t3_10` |
| **F19** | 4 SDK Developer Manuals | `tc_f19_01` .. `tc_f19_05` | `tc_b19_01` .. `tc_b19_05` | `tc_t3_01`, `tc_t3_02` |
| **F20** | 7 Domain Packs Guide & RivunStore | `tc_f20_01` .. `tc_f20_05` | `tc_b20_01` .. `tc_b20_05` | `tc_t3_08`, `tc_t3_10` |
| **F21** | 7-Point Fleet Doctor & MMR Forensics | `tc_f21_01` .. `tc_f21_05` | `tc_b21_01` .. `tc_b21_05` | `tc_t3_09`, `tc_t3_15`, `tc_t4_09` |
| **F22** | Interactive API Explorer & Sandbox | `tc_f22_01` .. `tc_f22_05` | `tc_b22_01` .. `tc_b22_05` | `tc_t3_02`, `tc_t3_19` |
| **F23** | Cross-Platform Build & Integration | `tc_f23_01` .. `tc_f23_05` | `tc_b23_01` .. `tc_b23_05` | `tc_t3_10` |
| **F24** | E2E Testing Suite (Tiers 1-4) | `tc_f24_01` .. `tc_f24_05` | `tc_b24_01` .. `tc_b24_05` | `tc_t3_20`, `tc_t4_01` .. `10` |
| **F25** | Adversarial Hardening (Tier 5) | `tc_f25_01` .. `tc_f25_05` | `tc_b25_01` .. `tc_b25_05` | `tc_t3_15`, `tc_t4_08` |

---

## 5. Execution Instructions

Run the master test runner from the project root:

```bash
node tests/e2e/test-runner.mjs
```

The test runner will execute all 280 tests in order, display the 25-feature matrix, and exit with code `0` on 100% pass or code `1` on failure.
