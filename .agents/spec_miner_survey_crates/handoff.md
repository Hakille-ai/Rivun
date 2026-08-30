# Handoff Report — Specification Mining for Rivun Crate & Protocol Stack

**Agent Directory**: `.agents/spec_miner_survey_crates`  
**Specification Document**: `c:\Users\Stagiaire\Documents\Amadou PGC\Prs\Rivun\.agents\spec_miner_survey_crates\crate_and_protocol_specs.md`  
**Handoff Type**: Hard (Task Complete)

---

## 1. Observation

Direct inspection of the repository source files and documentation revealed the authoritative specifications for all 26 workspace crates:

1. **Workspace Crates**:
   - `Cargo.toml` lines 1-32 declares all 26 workspace members: 25 crates in `crates/*` and 1 desktop application in `apps/rivun-control/src-tauri`.
2. **Rivun-Wire Binary Framing**:
   - `crates/rivun-core/src/lib.rs` (lines 1-85) defines `MAGIC_NUMBER = 0x5A41505F` (`ZAP_`), `HEADER_LEN = 64`, `AUTH_TRAILER_LEN = 72` (`ZSIG`), `POA_TRAILER_MAGIC = 0x5A504F41` (`ZPOA`), and `RivunFlags` bitmasks (`ENCRYPTED=1, PRIORITY=2, REQUIRES_CONSENSUS=4, SIGNED=8, BROADCAST=16`).
3. **Cryptographic Signing Transcripts & Domains**:
   - `crates/rivun-crypto/src/lib.rs` (lines 20-35) defines domain strings: `Rivun-NODE-ID-v1`, `Rivun-SIGN-HINT-v1`, `Rivun-POA-DIGEST-v1`, `Rivun-POA-SIGNATURE-v1`, `Rivun-POA-VALIDATOR-SET-v1`, `Rivun-BLINDED-COMMITMENT-v1`, `Rivun-BLINDED-RECEIPT-v1`, `Rivun-BATCH-SEAL-v1`, and node ID derivation `UUID::from_bytes(blake3(pubkey)[..16])`.
4. **Universal Envelope**:
   - `crates/rivun-envelope/src/lib.rs` (lines 1-45) defines `MAGIC_BYTES = b"ZENV"` (`0x5A454E56`), `HEADER_LEN = 74`, and 8 message kinds: `Data=1, Event=2, Command=3, Query=4, Response=5, StreamChunk=6, Action=7, Control=8`.
5. **Network Mesh, BFT Consensus & Datagrams**:
   - `crates/rivun-net/src/lib.rs` and submodules define 52-byte `ZAPD` datagram header with ChaCha20-Poly1305 AEAD, `BftConsensusEngine` 2-phase state machine ($T \le N$, $T = \lfloor 2N/3 \rfloor + 1$) with equivocation slashing, epidemic gossip engine with vector clocks, and $\Phi$-accrual failure detection ($8.0 \le \phi \le 14.0$).
6. **Ledger & Merkle Mountain Ranges**:
   - `crates/rivun-ledger/src/lib.rs` and `src/mmr.rs` define `IncrementalMmr` binary carry-over tree peak accumulator, single-leaf (`MmrInclusionProof`), batch multi-leaf (`MmrBatchInclusionProof`), and non-membership (`MmrExclusionProof`) proofs, alongside `ZAPMMR01` binary disk format.
7. **WASM Sandbox & Zero-Copy Streaming**:
   - `crates/rivun-runtime/src/lib.rs`, `src/streaming.rs`, and `crates/rivun-driver-sdk/src/lib.rs` define Wasmtime execution with fuel metering, epoch timer interrupts, ABI v1 exports (`memory`, `rivun_alloc`, `rivun_dealloc`, `rivun_execute`), and lock-free SPSC circular byte ring-buffers (`SpscRingBuffer`).
8. **Agent Protocol & Multi-Party PACT**:
   - `crates/rivun-agent/src/lib.rs`, `src/provenance.rs`, and `crates/rivun-pact/src/lib.rs`, `src/dispute.rs` define agent intents, sessions, delegations, and full causal provenance chains ($H_{\text{intent}} \to H_{\text{negotiation}} \to H_{\text{policy}} \to H_{\text{consensus}} \to H_{\text{driver}} \to H_{\text{poa}} \to H_{\text{receipt}} \to H_{\text{root}}$) and multi-party escrow dispute resolution.
9. **7 Domain Packs & RivunStore**:
   - `docs/domain-packs.md`, `crates/rivun-pack/src/lib.rs`, and `crates/rivun-store/src/lib.rs` define the 7 domain packs (`agentic-dev`, `cloud-ops`, `finance`, `healthcare`, `industrial`, `personal-ai`, `smart-building`), `.zpack` packaging, and signed driver registries.
10. **7-Point Fleet Doctor**:
    - `crates/rivun-telemetry/src/doctor.rs` (lines 110-305) defines the 7 core evaluation criteria: `network`, `storage`, `replay_guard`, `journal`, `pack_registry`, `certificate_validity`, and `peer_trust`.

---

## 2. Logic Chain

1. **Step 1 (Scope Definition)**: Read `ORIGINAL_REQUEST.md` and `Cargo.toml` to identify the full workspace surface, resulting in 26 total crates (25 under `crates/*` and 1 under `apps/rivun-control/src-tauri`).
2. **Step 2 (Protocol Architecture Extraction)**: Inspected `rivun-core`, `rivun-crypto`, `rivun-envelope`, and `rivun-net` to establish exact byte-level framing, trailer geometries, cryptographic domain separation transcripts, and encrypted datagram structures.
3. **Step 3 (Consensus & Ledger Invariants)**: Inspected `rivun-net/src/consensus/` and `rivun-ledger/src/mmr.rs` to extract BFT quorum thresholds ($T = \lfloor 2N/3 \rfloor + 1$), equivocation slashing state transitions, and $O(\log N)$ peak-bagged Merkle accumulator proof logic.
4. **Step 4 (Execution Sandbox & Streaming)**: Inspected `rivun-runtime` and `rivun-driver-sdk` to document Wasmtime sandboxing limits (fuel, epoch interrupts, linear memory bounds), guest-host ABI signatures, lock-free SPSC circular ring buffers, and IPC pipeline chaining.
5. **Step 5 (Agent Layer & Domain Packs)**: Inspected `rivun-agent`, `rivun-pact`, `rivun-pack`, `rivun-store`, `docs/domain-packs.md`, and `docs/zapstore.md` to document the full causal provenance DAG, multi-party escrow dispute mediation, 7 domain packs, and RivunStore bundle lifecycle.
6. **Step 6 (Observability & Diagnostics)**: Inspected `rivun-telemetry/src/doctor.rs` and `rivun-ops` to extract the 7-Point Fleet Doctor readiness suite and incident forensics generation.
7. **Step 7 (Synthesis & Verification)**: Compiled all findings into `crate_and_protocol_specs.md` with exhaustive categorization, 20-row Features Discovered table, and 15-row Edge Cases table.

---

## 3. Caveats

- **External Toolchains for SDKs**: Full multi-language SDK execution (Go toolchain for `sdks/go`, Python `crypto` extras for `sdks/python`) requires respective language runtime installations on the host. In Rust-only environments, `sdks/rust` serves as the reference implementation.
- **WASM Driver Artifact Compilation**: The WASM runtime tests rely on pre-compiled `.wat` / `.wasm` guest binaries in `examples/wasm-drivers/`.

---

## 4. Conclusion

The specification mining for the Rivun project is complete. All 26 crates, wire framing, cryptographic domain signatures, 2-Phase BFT consensus engine, Merkle Mountain Range accumulators, WASM sandboxing runtime, 4 SDKs, 7 Domain Packs, and 7-Point Fleet Doctor diagnostics have been authoritatively analyzed and documented in `crate_and_protocol_specs.md`.

---

## 5. Verification Method

To independently verify the mined specification:

1. **Verify Workspace Crates & Documentation**:
   - Inspect `crate_and_protocol_specs.md` to review the crate mapping, binary layouts, consensus rules, and feature tables.
2. **Inspect Codebases**:
   - Review `crates/rivun-core/src/lib.rs` for header/trailer layouts.
   - Review `crates/rivun-crypto/src/lib.rs` for domain prefixes and signing transcripts.
   - Review `crates/rivun-envelope/src/lib.rs` for `ZENV` envelope framing.
   - Review `crates/rivun-ledger/src/mmr.rs` for MMR proofs and `.zmmr` binary layout.
   - Review `crates/rivun-telemetry/src/doctor.rs` for the 7-Point Fleet Doctor criteria.
3. **Run Existing Test Suite**:
   ```powershell
   cargo test --workspace --locked
   ```
