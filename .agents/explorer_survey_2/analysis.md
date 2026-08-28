# rivun Next-Gen Frontier: Architectural Survey & Technical Specification
## Focus: R2 (Merkle Mountain Range & Compact Batch Receipts) & R3 (Async WASM Driver Pipeline & Inter-Driver IPC)

**Date:** 2026-08-15  
**Author:** Explorer 2 (Teamwork Explorer Agent)  
**Status:** Completed Architectural Survey  
**Target Crates:** `crates/rivun-ledger`, `crates/rivun-crypto`, `crates/rivun-runtime`, `crates/rivun-driver-sdk`, `crates/rivun-machine`, `crates/rivun-capability`, `crates/rivun-node`

---

## Table of Contents
1. [Executive Summary & Scope Definition](#1-executive-summary--scope-definition)
2. [Survey of Existing Architecture & Baseline Codebase](#2-survey-of-existing-architecture--baseline-codebase)
   - 2.1 `rivun-ledger`: Receipts, Journaling, Batch Verification, and Baseline MMR
   - 2.2 `rivun-crypto`: Identity, Signatures, PoA Consensus, and Domain Separation
   - 2.3 `rivun-runtime`: Wasmtime Engine, Fuel Metering, Epoch Ticking, and Memory Sandboxing
   - 2.4 `rivun-driver-sdk`: Driver Trait, Memory Layout, and ABI Interface
   - 2.5 `rivun-machine` & `rivun-capability`: Device Profiles and Capability Matrix
3. [Requirement 2 (R2): Merkle Mountain Range (MMR) & Compact Cryptographic Batch Receipts](#3-requirement-2-r2-merkle-mountain-range-mmr--compact-cryptographic-batch-receipts)
   - 3.1 MMR Theoretical Model & Bitwise Indexing Mathematics
   - 3.2 Gap Analysis of Current MMR Implementation
   - 3.3 Detailed Data Structures & Types for Next-Gen MMR
   - 3.4 Algorithms & Protocols: Incremental Appending, Peak Bagging, Batch Inclusion & Non-Membership (Exclusion) Proofs
   - 3.5 Cryptographic Batch Receipt Sealing & Swarm Quorum Binding
   - 3.6 Zero-Knowledge Verifiable Receipt Rollups (Private Execution Correctness Proofs)
4. [Requirement 3 (R3): Async WASM Driver Pipeline & Inter-Driver IPC](#4-requirement-3-r3-async-wasm-driver-pipeline--inter-driver-ipc)
   - 4.1 Gap Analysis of Current WASM Runtime & Driver SDK
   - 4.2 Non-Blocking Asynchronous WASM Host Execution Architecture
   - 4.3 Streaming I/O Buffers (Async TCP, Modbus, SPSC Shared Ring-Buffers)
   - 4.4 Deterministic Zero-Copy Inter-Driver IPC Pipes & Pipeline Chaining
   - 4.5 Unified Fuel Budgeting & Deterministic Pipeline Audit Trail
   - 4.6 Next-Gen `rivun-driver-sdk` Extensions
5. [Cross-Crate Dependency Architecture & Interface Contracts](#5-cross-crate-dependency-architecture--interface-contracts)
6. [Implementation Roadmap, Verification Strategy & Action Plan](#6-implementation-roadmap-verification-strategy--action-plan)

---

## 1. Executive Summary & Scope Definition

The rivun Next-Gen Frontier transformation upgrades rivun into an autonomous, hyper-scalable, cross-cluster decentralized execution and verification fabric. This survey provides an exhaustive technical analysis, architectural gap analysis, and implementation specification for two foundational pillars:

1. **Requirement 2 (R2): Merkle Mountain Range (MMR) & Compact Cryptographic Batch Receipts** (`rivun-ledger`, `rivun-crypto`)
   - High-throughput batch receipt sealing over append-only journals.
   - Strict $O(\log N)$ compact inclusion and non-membership (exclusion) cryptographic proofs.
   - Peak-bagging root calculation and multi-leaf batch proof aggregation.
   - Zero-Knowledge verifiable receipt rollups allowing agents to cryptographically prove execution correctness without disclosing confidential payload bytes, internal memory, or proprietary sensor data.

2. **Requirement 3 (R3): Async WASM Driver Pipeline & Inter-Driver IPC** (`rivun-runtime`, `rivun-driver-sdk`)
   - Non-blocking asynchronous WASM driver host execution integrated with Tokio.
   - Streaming I/O buffers for high-bandwidth telemetry and hardware protocols (Async TCP, Modbus TCP/RTU, Lock-free SPSC Ring-Buffers).
   - Deterministic zero-copy inter-driver IPC pipes enabling sequential and DAG driver chaining (e.g. Machine Perception $\to$ Safety Policy $\to$ Actuator).
   - Shared deterministic fuel budgeting across multi-driver pipelines with causal execution audit trails.

---

## 2. Survey of Existing Architecture & Baseline Codebase

### 2.1 `rivun-ledger`: Receipts, Journaling, Batch Verification, and Baseline MMR
- **Receipt Representation (`SignedActionReceipt`)**:
  - Encapsulates `ActionReceipt` containing `schema_version`, `node_id`, `source_node`, `target_node`, `kind`, `subject`, `action`, `frame_hash`, `payload_hash`, `output_hash`, `frame_timestamp_micros`, `processed_at_micros`, `flags`, `consensus_required`, `poa: Option<PoaReceipt>`, and `pact: Option<PactReceiptReference>`.
  - Signed via Ed25519 using domain separator `rivun-ACTION-RECEIPT-v1`.
  - Enforces static validation on Blake3 artifact hashes (`blake3:<64-hex>`), timestamp monotonicity, and consensus flag compliance.
- **Receipt Journal Store (`ReceiptJournalStore`)**:
  - Implements disk-backed, append-only segmented storage via `rivun-journal` (`.zjseg` files).
  - Generates signed segment manifests (`SignedReceiptSegmentManifest`, `.zjmanifest.json.sig`) linking segments cryptographically via `previous_segment_hash`.
  - Provides index-accelerated query filtering (`query_fast`) based on `ReceiptSegmentIndex`.
  - Supports full crash-tail recovery and JSONL export/import.
- **Batch Verification Engine (`verify_action_receipts`)**:
  - Scalar verification for $< 4$ receipts.
  - Chunked batch verification via `ed25519-dalek::verify_batch` for $\ge 4$ receipts.
  - Parallel multi-threaded chunk processing via Rayon (`par_chunks`) for $\ge 128$ receipts.
- **Baseline MMR Accumulator (`crates/rivun-ledger/src/mmr.rs`)**:
  - Implements `hash_leaf` (domain `rivun-MMR-LEAF-v1:`), `hash_nodes` (domain `rivun-MMR-NODE-v1:`), and `bag_peaks` (domain `rivun-MMR-PEAK-BAG-v1:`).
  - `MerkleMountainRange`: Stores all leaves in memory (`Vec<MmrHash>`), recomputes peaks via bit shifts, generates single-leaf inclusion proofs (`MmrInclusionProof`), and verifies single proofs.
  - Contains basic `MmrRollupCommitment` structure (root hash, leaf count, min/max timestamp).

### 2.2 `rivun-crypto`: Identity, Signatures, PoA Consensus, and Domain Separation
- **Key Material & Node Identity**:
  - Ed25519 `SigningKey` and `VerifyingKey` (`Keypair`, `PublicKey`).
  - Node IDs are derived deterministically: $\text{UUIDv8}(\text{Blake3}(\text{"rivun-NODE-ID-v1"} \parallel \text{public\_key\_bytes}))$.
- **Fast Synchronous Wire Filtering**:
  - Generates an 8-byte `@@rivun_HEADER@@SIGN` signature hint: $\text{Blake3}(\text{"rivun-SIGN-HINT-v1"} \parallel \text{signature})[0..8]$.
  - Embedded in the 64-byte `ZapFrame` header for $O(1)$ pre-filtering before full cryptographic verification.
- **Proof-of-Action (PoA) Consensus Primitives**:
  - Multi-validator threshold signing ($T$-of-$N$) over frame digests (`poa_frame_digest`).
  - `PoaAttestation`, `PoaTrailer`, `PoaValidatorSet`, `SignedPoaValidatorSet`.
  - Validator sets enforce epoch monotonicity, threshold bounds, and authority signature verification.

### 2.3 `rivun-runtime`: Wasmtime Engine, Fuel Metering, Epoch Ticking, and Memory Sandboxing
- **Execution Architecture**:
  - Built on `wasmtime 45.0.1` with Cranelift compiler.
  - Synchronous execution model (`WasmExecutor::execute(&driver, action, payload, limits)`).
  - Deterministic fuel metering (`consume_fuel(true)`).
  - Epoch-based interruption (`epoch_interruption(true)` + `EngineEpochTicker` ticking every 1ms).
- **Sandboxing & Limits (`ExecutionLimits`)**:
  - `max_memory_bytes` (default 16MB) enforced via Wasmtime `StoreLimitsBuilder` (max 1 instance, 1 memory, 1 table).
  - `timeout_ms` (default 1000ms), `fuel` (default 10,000,000 units), `max_output_bytes` (default 1MB).
  - Strict capability isolation via `DriverPermissions`: blocks filesystem, network, wall clock, and environment access unless explicitly granted.
- **Module Caching**:
  - LRU compiled module cache (`WasmModuleCache`) indexed by `Blake3(wasm_bytes)`.
- **Host ABI**:
  - WASM Exports: `memory`, `@@rivun_HEADER@@alloc(i32) -> i32`, `@@rivun_HEADER@@dealloc(i32, i32)`, `@@rivun_HEADER@@execute(i32, i32, i32, i32) -> i64` (packed `(ptr << 32) | len`).
  - Host Imports: `rivun.emit_event`, `rivun.memory_read`, `rivun.memory_write`, `rivun.device_call`.

### 2.4 `rivun-driver-sdk`: Driver Trait, Memory Layout, and ABI Interface
- Minimal SDK containing `ZapDriver` trait (`fn execute(&self, input: DriverInput) -> Result<Vec<u8>, DriverError>`).
- Bit-packed result encoding/decoding helper (`PackedResult`).

### 2.5 `rivun-machine` & `rivun-capability`: Device Profiles and Capability Matrix
- `rivun-capability`: Capability hierarchy (`CapabilityId`, e.g. `driver.execute:<action>`), permission sets (`DriverPermissions`).
- `rivun-machine`: Device profiles (`DeviceProfile`), adapter types (`Mock`, `Serial`, `Tcp`, `ModbusLike`), command payload validation. Currently synchronous only.

---

## 3. Requirement 2 (R2): Merkle Mountain Range (MMR) & Compact Cryptographic Batch Receipts

### 3.1 MMR Theoretical Model & Bitwise Indexing Mathematics
A Merkle Mountain Range (MMR) is an append-only cryptographic accumulator structured as an ordered list of perfectly balanced binary Merkle trees ("mountains") of strictly decreasing heights $h_0 > h_1 > \dots > h_k$.

#### 1. Node Count & Leaf Index Mapping
For an MMR with $N$ leaves:
- Total nodes in the MMR $M = 2N - \text{popcount}(N)$, where $\text{popcount}(N)$ is the number of set bits (peaks) in the binary representation of $N$.
- Peaks correspond exactly to the set bit positions of $N = \sum_{i=0}^k 2^{b_i}$ with $b_0 > b_1 > \dots > b_k$.
- Peak $i$ is the root of a full binary tree containing $2^{b_i}$ leaves and $2^{b_i + 1} - 1$ total nodes.

#### 2. Peak Bagging Root
To produce a single 32-byte cryptographic root $R$ from peaks $[P_0, P_1, \dots, P_k]$:
$$R = \text{BagPeaks}([P_0, P_1, \dots, P_k])$$
$$\text{Bag}(P_0) = P_0$$
$$\text{Bag}(P_0, P_1) = \text{Blake3}(\text{"rivun-MMR-PEAK-BAG-v1:"} \parallel P_0 \parallel P_1)$$
$$\text{Bag}(P_0, \dots, P_k) = \text{Blake3}(\text{"rivun-MMR-PEAK-BAG-v1:"} \parallel \text{Bag}(P_0, \dots, P_{k-1}) \parallel P_k)$$

### 3.2 Gap Analysis of Current MMR Implementation
| Feature | Current `rivun-ledger` (`mmr.rs`) | Required Next-Gen Frontier Architecture |
|---|---|---|
| **Memory Footprint** | $O(N)$ RAM: stores entire `Vec<MmrHash>` in memory; recomputes trees recursively. | $O(\log N)$ RAM: incremental peak accumulator maintaining only active peak hashes ($\le 64$ hashes in RAM). |
| **Disk Persistence** | In-memory only; rebuilt from journal records upon request. | Persistent MMR node storage integrated directly into `ReceiptJournalStore` segments (`.zmmr` files). |
| **Multi-Leaf Batch Proofs** | Single-leaf inclusion only (`MmrInclusionProof`). | Multi-inclusion proof (`MmrBatchInclusionProof`) with deduplicated sister DAG for cross-cluster syncing. |
| **Non-Membership / Exclusion Proofs** | None. | Monotonic sequence exclusion proofs and sorted/indexed neighbor bounding proofs (`MmrExclusionProof`). |
| **Cryptographic Batch Sealing** | Segment manifest with flat segment hash. | Formal `ReceiptBatchSeal` binding sequence range, MMR root, state transitions, and Swarm Quorum multi-signatures. |
| **Zero-Knowledge Verifiable Rollups** | None (receipts expose raw payload hashes and actions). | Blinded receipt commitments, execution witness trace, state delta circuit, and succinct ZK verification proofs. |

### 3.3 Detailed Data Structures & Types for Next-Gen MMR

```rust
/// Compact 32-byte Blake3 digest used across MMR operations.
pub type MmrHash = [u8; 32];

/// Incremental MMR peak accumulator maintaining O(log N) state.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct IncrementalMmr {
    /// Total number of leaves appended.
    pub leaf_count: u64,
    /// Active peaks of subtrees, ordered from highest to lowest.
    pub peaks: Vec<MmrHash>,
    /// Cached bagged peak root hash.
    pub cached_root: Option<MmrHash>,
}

/// Compact multi-leaf batch inclusion proof.
/// Deduplicates sister hashes across shared internal tree paths.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct MmrBatchInclusionProof {
    /// Total leaves in the MMR at generation time.
    pub total_leaves: u64,
    /// Sorted list of leaf indices being proven.
    pub leaf_indices: Vec<u64>,
    /// Leaf hashes corresponding to the leaf indices.
    pub leaf_hashes: Vec<String>,
    /// Minimal deduplicated set of internal sister hashes needed for reconstruction.
    pub sister_hashes: Vec<String>,
    /// Active peak hashes of the MMR.
    pub peak_hashes: Vec<String>,
}

/// Cryptographic non-membership (exclusion) proof.
/// Proves that a given sequence number or receipt hash does NOT exist in the batch.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum MmrExclusionProof {
    /// Target sequence is strictly less than the first receipt in the batch.
    BeforeRange {
        target_sequence: u64,
        first_sequence: u64,
        first_inclusion_proof: Box<MmrInclusionProof>,
    },
    /// Target sequence is strictly greater than the last receipt in the batch.
    AfterRange {
        target_sequence: u64,
        last_sequence: u64,
        last_inclusion_proof: Box<MmrInclusionProof>,
    },
    /// Target sequence falls in a verified gap between two adjacent receipts S_i < S < S_{i+1}.
    SequenceGap {
        target_sequence: u64,
        lower_sequence: u64,
        lower_inclusion_proof: Box<MmrInclusionProof>,
        upper_sequence: u64,
        upper_inclusion_proof: Box<MmrInclusionProof>,
    },
    /// Target receipt hash is bounded by sorted index neighbors.
    HashBound {
        target_hash: String,
        lower_bound_leaf: String,
        lower_inclusion_proof: Box<MmrInclusionProof>,
        upper_bound_leaf: String,
        upper_inclusion_proof: Box<MmrInclusionProof>,
    },
}

/// Cryptographically sealed receipt batch linking MMR root, state transitions,
/// and Swarm Quorum multi-signatures.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReceiptBatchSeal {
    pub schema_version: u8,
    pub batch_id: Uuid,
    pub node_id: Uuid,
    pub segment_sequence: u64,
    pub start_sequence: u64,
    pub end_sequence: u64,
    pub receipt_count: u64,
    pub first_processed_at_micros: u64,
    pub last_processed_at_micros: u64,
    pub mmr_root: String,
    pub initial_state_hash: String,
    pub final_state_hash: String,
    pub total_fuel_consumed: u64,
    pub quorum_threshold: u16,
    pub validator_signatures: Vec<BatchValidatorSignature>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct BatchValidatorSignature {
    pub validator_node: Uuid,
    pub validator_public_key: String,
    pub signature: String,
}
```

### 3.4 Algorithms & Protocols

#### 1. Incremental Leaf Append ($O(1)$ Amortized)
When appending a new leaf $L$ to `IncrementalMmr`:
1. Hash the leaf: $H = \text{hash\_leaf}(L)$.
2. Increment `leaf_count`.
3. Let $h = 0$, $\text{current} = H$.
4. While the lowest peak in `peaks` has height $h$:
   - Pop the left peak $P_{left}$ of height $h$.
   - Compute parent node: $\text{current} = \text{hash\_nodes}(P_{left}, \text{current})$.
   - $h = h + 1$.
5. Push $\text{current}$ as new peak of height $h$.
6. Invalidate `cached_root`.

#### 2. Multi-Leaf Batch Proof Generation & Verification
1. Given leaf indices $I = \{i_1, i_2, \dots, i_k\}$:
2. Identify target peak trees $T(i)$ for each index.
3. Traverse target trees from leaves to root, building the minimal subtree DAG of needed sibling nodes.
4. Filter out nodes whose children are already known in the proof DAG.
5. Serialize the minimal sister array and peak hashes.
6. **Verification**: Bottom-up DAG reduction computing peak candidates, matching them against `peak_hashes`, and computing $\text{BagPeaks}(\text{peak\_hashes}) == \text{expected\_root}$.

#### 3. Cryptographic Non-Membership Proof Verification
- For `SequenceGap { target_sequence, lower_sequence, lower_proof, upper_sequence, upper_proof }`:
  1. Verify `lower_proof` against `mmr_root` $\implies$ valid leaf at index $k$ with sequence `lower_sequence`.
  2. Verify `upper_proof` against `mmr_root` $\implies$ valid leaf at index $k+1$ with sequence `upper_sequence`.
  3. Assert $k + 1 == \text{upper\_index}$ (strictly adjacent leaves in the MMR).
  4. Assert $\text{lower\_sequence} < \text{target\_sequence} < \text{upper\_sequence}$.
  5. Cryptographic proof of non-existence is complete.

### 3.5 Cryptographic Batch Receipt Sealing & Swarm Quorum Binding
The batch sealing process operates during journal segment rotation:
1. `ReceiptJournalStore` rotates active segment $S$.
2. Generates `IncrementalMmr` from all receipts in segment $S$, deriving `mmr_root`.
3. Constructs `ReceiptBatchSealPayload` containing `(batch_id, segment_sequence, mmr_root, initial_state_hash, final_state_hash, receipt_count)`.
4. Broadcasts `ReceiptBatchSealPayload` to Swarm Quorum / PoA Validators via P2P Gossip (`rivun.quorum.seal_request`).
5. Validators verify local receipt segment records against `mmr_root`, signing the batch payload.
6. Node collects $T$-of-$N$ threshold signatures, assembling `ReceiptBatchSeal`.
7. Persists seal to `.zjseal.json` alongside `.zjseg` and `.zjmanifest.json.sig`.

### 3.6 Zero-Knowledge Verifiable Receipt Rollups (Private Execution Correctness)

#### 1. Motivation & Privacy Model
In cross-cluster enterprise fabrics and untrusted multi-agent swarms:
- Nodes must prove that actions were executed faithfully according to certified drivers and policies.
- However, payload data (e.g. medical records, financial balances, secret device keys, proprietary vision tensors) cannot be disclosed.
- Solution: **Zero-Knowledge Receipt Rollup**.

#### 2. ZK Rollup Data Structures
```rust
/// Blinded receipt commitment for private execution audit.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct BlindedReceiptCommitment {
    pub receipt_id: Uuid,
    /// Salt blinding factor: r
    pub blinding_salt: String,
    /// C = Blake3(rivun-ZK-RECEIPT-v1 || frame_hash || payload_hash || output_hash || salt)
    pub commitment_hash: String,
    /// Public execution metadata
    pub action: String,
    pub fuel_consumed: u64,
    pub status: u8,
}

/// Zero-Knowledge Verifiable Receipt Rollup Proof.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ZkReceiptBatchProof {
    pub schema_version: u8,
    pub batch_id: Uuid,
    pub mmr_root: String,
    pub public_inputs: ZkRollupPublicInputs,
    /// Cryptographic proof payload (SNARK/STARK/Polynomial Commitment Proof)
    pub proof_bytes: Vec<u8>,
    pub verifier_id: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ZkRollupPublicInputs {
    pub initial_state_root: String,
    pub final_state_root: String,
    pub batch_mmr_root: String,
    pub total_receipts: u64,
    pub total_fuel_consumed: u64,
    pub quorum_commitment: String,
}
```

#### 3. Verification Protocol
1. **Public Statement**: "There exists an ordered set of private receipts $R_0, \dots, R_{M-1}$ such that each receipt has valid blinded commitment $C_i$, the MMR accumulation yields `batch_mmr_root`, the state transition $S_{init} \to S_{final}$ is valid under driver transition rules, fuel consumed $\le F_{max}$, and all validator signatures are valid."
2. **Verification Time**: $O(1)$ constant time or $O(\text{polylog}(M))$, independent of payload size, sub-millisecond on edge nodes.

---

## 4. Requirement 3 (R3): Async WASM Driver Pipeline & Inter-Driver IPC

### 4.1 Gap Analysis of Current WASM Runtime & Driver SDK
| Dimension | Current `rivun-runtime` / `rivun-driver-sdk` | Next-Gen Async & IPC Architecture |
|---|---|---|
| **Host Execution** | Synchronous, blocking OS thread during execution. | Fully asynchronous host execution on Tokio tasks (`wasmtime::Config::async_support(true)`). |
| **Streaming I/O** | Discrete single request/response buffers. | Streaming I/O buffers for TCP, Modbus, and lock-free Shared Memory Ring-Buffers. |
| **Driver Inter-Communication** | None; host must serialize intermediate results to JSON/bytes and invoke next driver independently. | Deterministic zero-copy Inter-Driver IPC pipes chaining perception, policy, and actuator micro-drivers. |
| **Fuel Metering Across Pipelines** | Single driver fuel limits only. | Unified deterministic pipeline fuel budget shared across all chained driver stages with automatic fail-fast abortion. |
| **SDK Interface** | Synchronous `ZapDriver` trait with `execute(...)`. | `AsyncZapDriver` trait, zero-copy buffer views (`IpcBufferView`), and pipeline macros. |

### 4.2 Non-Blocking Asynchronous WASM Host Execution Architecture

```rust
#[derive(Clone)]
pub struct AsyncWasmExecutor {
    engine: Engine,
    module_cache: Arc<tokio::sync::RwLock<AsyncWasmModuleCache>>,
}

impl AsyncWasmExecutor {
    pub fn new() -> Result<Self> {
        let mut config = Config::new();
        config.async_support(true); // Enables non-blocking async execution
        config.consume_fuel(true);
        config.epoch_interruption(true);
        config.wasm_backtrace_details(wasmtime::WasmBacktraceDetails::Enable);
        let engine = Engine::new(&config)?;
        Ok(Self {
            engine,
            module_cache: Arc::new(tokio::sync::RwLock::new(AsyncWasmModuleCache::new())),
        })
    }

    /// Asynchronously executes a compiled WASM driver with timeout and fuel limits.
    pub async fn execute_async(
        &self,
        driver: &AsyncWasmDriver,
        action: &str,
        payload: &[u8],
        limits: ExecutionLimits,
    ) -> Result<AsyncWasmExecutionResult> {
        // Asynchronously instantiates store and linker
        // Dispatches execute typed function via .call_async(&mut store, ...).await
    }
}
```

#### Async Host Call Dispatch
Host calls (`rivun.async_stream_read`, `rivun.async_stream_write`, `rivun.async_device_call`) use Wasmtime's `linker.func_wrap_async`:
```rust
linker.func_wrap_async(
    "rivun",
    "async_stream_read",
    |mut caller: Caller<'_, AsyncStoreState>, (stream_id, ptr, max_len): (i32, i32, i32)| {
        Box::new(async move {
            let buffer = caller.data_mut().stream_pool.read_async(stream_id, max_len as usize).await?;
            // Write buffer directly into WASM caller memory
            Ok(buffer.len() as i32)
        })
    },
)?;
```

### 4.3 Streaming I/O Buffers (TCP, Modbus, Shared Ring-Buffers)

#### 1. Architecture of `StreamingBufferPool`
```rust
pub enum StreamTransport {
    /// Asynchronous framed TCP stream
    Tcp(tokio::net::TcpStream),
    /// Industrial Modbus TCP / RTU stream with register caching
    Modbus(AsyncModbusConnection),
    /// Lock-free Single-Producer Single-Consumer (SPSC) circular shared-memory ring buffer
    SharedRingBuffer(Arc<SpscRingBuffer>),
}

pub struct StreamingBufferPool {
    streams: HashMap<u32, StreamTransport>,
    buffer_capacity: usize,
}
```

#### 2. Lock-Free SPSC Ring Buffer
For high-rate machine perception (100Hz+ camera frames, LiDAR, IMU), the host and WASM runtime communicate via lock-free circular memory with atomic read/write indices:
- Atomic `head` and `tail` indices aligned to cache lines (64-byte padding).
- Zero memory allocation on steady-state streaming.
- Backpressure policies: `DropOldest`, `DropNewest`, or `BlockWithTimeout`.

### 4.4 Deterministic Zero-Copy Inter-Driver IPC Pipes & Pipeline Chaining

#### 1. The Autonomous Machine Chaining Model
```
┌─────────────────────────┐      IPC Pipe 1       ┌─────────────────────────┐      IPC Pipe 2       ┌─────────────────────────┐
│ Stage 0: Machine Vision │ ────────────────────> │ Stage 1: Safety Policy  │ ────────────────────> │ Stage 2: Physical Motor │
│ (Perception Driver)     │   Zero-Copy Buffer    │ (Pact Envelope Check)   │   Zero-Copy Buffer    │ (Actuator Driver)       │
└─────────────────────────┘                       └─────────────────────────┘                       └─────────────────────────┘
         │                                                 │                                                 │
         └────────────────────────────┬────────────────────┴─────────────────────────────────────────────────┘
                                      │
                                      ▼
                      ┌───────────────────────────────┐
                      │  Composite Execution Receipt  │
                      │  & Cumulative Fuel Audit      │
                      └───────────────────────────────┘
```

#### 2. Pipeline Orchestrator Data Structures (`DriverPipeline`)
```rust
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct PipelineStageConfig {
    pub stage_id: u32,
    pub driver_action: String,
    pub allocated_fuel_share: Option<u64>,
    pub permissions: DriverPermissions,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct DriverPipelineConfig {
    pub pipeline_id: Uuid,
    pub name: String,
    pub stages: Vec<PipelineStageConfig>,
    pub total_fuel_budget: u64,
    pub timeout_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PipelineExecutionResult {
    pub pipeline_id: Uuid,
    pub final_output: Vec<u8>,
    pub stage_receipts: Vec<StageExecutionReceipt>,
    pub total_fuel_consumed: u64,
    pub total_elapsed_ms: u128,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StageExecutionReceipt {
    pub stage_id: u32,
    pub driver_action: String,
    pub input_hash: String,
    pub output_hash: String,
    pub fuel_consumed: u64,
    pub elapsed_ms: u128,
}
```

#### 3. Zero-Copy Inter-Driver Buffer Passing Mechanism
1. Host allocates shared pinned memory page $B_{ipc}$.
2. Stage 0 runs in its WASM linear memory, writes output to $B_{ipc}$.
3. Host passes $B_{ipc}$ directly to Stage 1 via `@@rivun_HEADER@@execute` pointer mapping into Stage 1's address space without intermediate host heap copying.
4. Hash $H_0 = \text{Blake3}(B_{ipc})$ is recorded into the stage execution receipt for deterministic auditability.
5. Stage 1 executes, writing output to $B_{ipc}'$.
6. Stage 2 executes, consuming $B_{ipc}'$ and issuing physical device command.

### 4.5 Unified Fuel Budgeting & Deterministic Pipeline Audit Trail
- Single aggregate fuel budget $F_{total}$ for the pipeline.
- At start of Stage $k$:
  $$\text{Store::set\_fuel}(F_{remaining})$$
- After Stage $k$:
  $$F_{consumed, k} = F_{remaining} - \text{Store::get\_fuel}()$$
  $$F_{remaining} \leftarrow F_{remaining} - F_{consumed, k}$$
- If $F_{remaining} == 0$ or stage fails, pipeline terminates immediately, returning `PipelineFuelExhausted { stage_id }`.
- Determinism Guarantee: Given identical inputs and fuel costs, the pipeline execution trace and stage hashes are identical bit-for-bit across any node in the cluster.

### 4.6 Next-Gen `rivun-driver-sdk` Extensions
```rust
/// Asynchronous driver trait for high-performance streaming & IPC pipelines.
#[async_trait::async_trait]
pub trait AsyncZapDriver: Send + Sync {
    async fn execute(&self, input: DriverInput<'_>) -> Result<Vec<u8>, DriverError>;
    
    async fn process_stream(
        &self,
        stream_id: u32,
        reader: &mut dyn AsyncStreamReader,
        writer: &mut dyn AsyncStreamWriter,
    ) -> Result<(), DriverError> {
        Err(DriverError::new("streaming not implemented"))
    }
}

/// Zero-copy slice view into WASM memory.
pub struct ZeroCopyBuffer<'a> {
    ptr: *mut u8,
    len: usize,
    _marker: std::marker::PhantomData<&'a mut [u8]>,
}
```

---

## 5. Cross-Crate Dependency Architecture & Interface Contracts

```
                               ┌───────────────────────┐
                               │       rivun-core        │
                               │  (ZapFrame, Flags,    │
                               │   PoaTrailer, Hashes) │
                               └───────────┬───────────┘
                                           │
                     ┌─────────────────────┼─────────────────────┐
                     │                     │                     │
                     ▼                     ▼                     ▼
             ┌───────────────┐     ┌───────────────┐     ┌───────────────┐
             │  rivun-crypto   │     │rivun-capability │     │  rivun-journal  │
             │ (Keypair, PoA,│     │(Permissions,  │     │(Binary Segs,  │
             │  Signatures)  │     │ Capabilities) │     │ Hash Chaining)│
             └───────┬───────┘     └───────┬───────┘     └───────┬───────┘
                     │                     │                     │
                     ▼                     ▼                     │
             ┌───────────────┐     ┌───────────────┐             │
             │  rivun-ledger   │     │  rivun-machine  │             │
             │ (MMR, Rollups,│     │(Stream Buffer,│             │
             │  Batch Seals) │     │ Device Ports) │             │
             └───────┬───────┘     └───────┬───────┘             │
                     │                     │                     │
                     │             ┌───────▼───────┐             │
                     │             │rivun-driver-sdk │             │
                     │             │(Async, IPC SDK│             │
                     │             └───────┬───────┘             │
                     │                     │                     │
                     │             ┌───────▼───────┐             │
                     │             │  rivun-runtime  │             │
                     │             │(Async Executor│             │
                     │             │ Pipeline IPC) │             │
                     │             └───────┬───────┘             │
                     │                     │                     │
                     └───────────┬─────────┴─────────────────────┘
                                 │
                                 ▼
                          ┌───────────────┐
                          │   rivun-node    │
                          │ (Coordinator, │
                          │  P2P Engine)  │
                          └───────────────┘
```

### Key Interface Contracts
1. **`rivun-ledger` $\leftrightarrow$ `rivun-crypto`**:
   - `IncrementalMmr` and `ReceiptBatchSeal` consume `Keypair` and `PublicKey` domain-separated signing.
   - Batch verification links Ed25519 threshold validator certificates with peak-bagged MMR roots.
2. **`rivun-runtime` $\leftrightarrow$ `rivun-driver-sdk`**:
   - ABI contract version 2: `@@rivun_HEADER@@alloc`, `@@rivun_HEADER@@dealloc`, `@@rivun_HEADER@@execute_async`, `@@rivun_HEADER@@stream_read`, `@@rivun_HEADER@@stream_write`.
   - Zero-copy buffer pointer sharing via linear memory segments.
3. **`rivun-runtime` $\leftrightarrow$ `rivun-ledger`**:
   - When a multi-driver pipeline completes, `rivun-runtime` emits a `PipelineExecutionReceipt` directly ingestible by `ReceiptJournalStore`.
4. **`rivun-node` $\leftrightarrow$ `rivun-runtime` / `rivun-ledger`**:
   - P2P Gossip engine disseminates `ReceiptBatchSeal` and MMR inclusion proofs for cross-node replication and swarm consensus validation.

---

## 6. Implementation Roadmap, Verification Strategy & Action Plan

### 6.1 Phased Implementation Roadmap

#### Phase 1: MMR Accumulator & Batch Sealing Upgrade (`rivun-ledger`, `rivun-crypto`)
- [ ] Refactor `crates/rivun-ledger/src/mmr.rs`:
  - Implement `IncrementalMmr` with $O(\log N)$ peak array arithmetic.
  - Implement `MmrBatchInclusionProof` multi-leaf DAG deduplication algorithm.
  - Implement `MmrExclusionProof` (monotonic sequence gap and neighbor bounding proofs).
- [ ] Add `ReceiptBatchSeal` & `SignedReceiptBatch` with PoA validator multi-signature aggregation.
- [ ] Add `ZkReceiptRollupCommitment` and verification logic for private execution proofs.
- [ ] Extend `ReceiptJournalStore` to auto-commit MMR peaks to `.zmmr` indexes on segment rotation.

#### Phase 2: Async WASM Runtime & Streaming Engine (`rivun-runtime`, `rivun-driver-sdk`)
- [ ] Enable Wasmtime `async_support(true)` in `rivun-runtime`.
- [ ] Implement `AsyncWasmExecutor` with Tokio async task scheduling.
- [ ] Implement `StreamingBufferPool` supporting Async TCP, Modbus TCP/RTU, and SPSC lock-free ring buffers.
- [ ] Update host imports with async stream primitives (`rivun.async_stream_read`, `rivun.async_stream_write`).
- [ ] Update `rivun-driver-sdk` with `AsyncZapDriver` and zero-copy memory slice wrappers.

#### Phase 3: Inter-Driver IPC Pipes & Deterministic Chaining (`rivun-runtime`, `rivun-node`)
- [ ] Implement `DriverPipeline` orchestrator in `rivun-runtime`.
- [ ] Implement zero-copy buffer passing between chained pipeline stages (Perception $\to$ Safety $\to$ Actuator).
- [ ] Implement aggregate deterministic fuel pool metering across all pipeline stages.
- [ ] Integrate pipeline execution receipts with `ReceiptJournalStore`.

#### Phase 4: Benchmarking, Validation & Cluster Simulation
- [ ] Create benchmark `benches/mmr_scale.rs` in `rivun-ledger` verifying 100,000+ receipts with sub-millisecond MMR root derivation and proof checks.
- [ ] Create benchmark `benches/async_pipeline.rs` in `rivun-runtime` verifying high-concurrency streaming pipelines under strict fuel metering.
- [ ] Run full workspace test suite `cargo test --workspace --all-targets` and Clippy linting.

---

### 6.2 Acceptance & Invalidation Criteria
- **MMR Batch Inclusion Benchmark**: Inclusion verification for 1,000+ receipts in an MMR of 100,000+ receipts must complete in $< 1.0\text{ ms}$.
- **MMR Non-Membership Proof**: Exclusion verification must reject tampered sequences or existing leaves deterministically.
- **Async WASM Concurrency**: 500+ concurrent WASM streaming driver instances must run on Tokio threadpool without thread blocking.
- **Pipeline Zero-Copy IPC**: Passing a 1MB buffer across 3 driver stages must perform 0 heap allocations on the host inter-stage boundary.
- **Unified Fuel Budget**: A pipeline with 5M fuel limit must abort at the exact stage where aggregate fuel is exhausted.

