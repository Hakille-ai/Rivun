# Milestone 3 Exploration Report: Async WASM Driver Pipeline & Inter-Driver IPC

## 1. Executive Summary
This report provides a comprehensive architectural and technical investigation of the ZAP codebase to prepare for **Milestone 3: Async WASM Driver Pipeline & Inter-Driver IPC**. 

Milestone 3 expands ZAP's execution runtime to support:
1. Non-blocking asynchronous host execution on Tokio tasks with strict fuel metering and isolated memory sandboxing (`crates/zap-runtime`).
2. High-throughput lock-free circular ring-buffers supporting async streaming I/O (TCP stream buffers, Modbus industrial framing buffers, and SPSC ring buffers) with backpressure (`crates/zap-runtime`).
3. Deterministic zero-copy inter-driver IPC pipes chaining perception, safety policy, and physical actuator drivers with strict aggregate fuel budgeting (`crates/zap-runtime`, `crates/zap-driver-sdk`).
4. The `AsyncZapDriver` trait, zero-copy pinned buffer views (`PinnedBuffer`, `BufferSlice`), and driver IPC channel primitives (`crates/zap-driver-sdk`).

---

## 2. Workspace Structure & Dependency Analysis

### 2.1 Workspace Overview
The ZAP workspace consists of 23 crates defined in `Cargo.toml`:
- **Core Execution Runtime**: `zap-runtime`, `zap-driver-sdk`, `zap-core`, `zap-capability`, `zap-machine`, `zap-memory`, `zap-node`.
- **Consensus & Swarm Mesh (M1)**: `zap-net`, `zap-agent`, `zap-node`.
- **Ledger & MMR Cryptography (M2)**: `zap-ledger`, `zap-crypto`.
- **Pacts & Policy Disputes (M4)**: `zap-pact`, `zap-policy`.
- **CLI & Cluster Simulation (M5)**: `zap-cli`, `zap-telemetry`.
- **Storage, Packaging & Gateway**: `zap-store`, `zap-pack`, `zap-envelope`, `zap-gateway`, `zap-journal`, `zap-ops`, `zap-router`, `zap-schema`.

### 2.2 Workspace Dependencies
From root `Cargo.toml`:
- `wasmtime = { version = "45.0.1", default-features = false, features = ["cranelift", "runtime", "std", "wat"] }`
- `tokio = { version = "1", features = ["macros", "net", "rt-multi-thread", "sync", "time"] }`
- `bytes = "1"`
- `blake3 = "1"`
- `thiserror = "2"`
- `serde = { version = "1", features = ["derive"] }`, `serde_json = "1"`
- `criterion = { version = "0.7", features = ["async_tokio"] }`
- `wat = "1"`
- `uuid = { version = "1", features = ["serde", "v4", "v5"] }`

### 2.3 Crate Dependencies for Milestone 3
- `crates/zap-driver-sdk/Cargo.toml`:
  - Needs: `bytes.workspace = true`, `thiserror.workspace = true`, `serde.workspace = true`.
- `crates/zap-runtime/Cargo.toml`:
  - Currently has: `blake3`, `serde`, `thiserror`, `wasmtime`, `zap-capability`.
  - Needs addition of: `tokio.workspace = true`, `bytes.workspace = true`, `zap-driver-sdk = { path = "../zap-driver-sdk" }`, `zap-core = { path = "../zap-core" }`.

---

## 3. Current Architecture & Implementation (M1 & M2 Baseline)

### 3.1 WASM Execution Engine (`zap-runtime/src/lib.rs`)
- **Engine Configuration**:
  - `wasmtime::Engine` configured with `consume_fuel(true)`, `epoch_interruption(true)`, and `wasm_backtrace_details(Enable)`.
- **Module Caching**:
  - `WasmModuleCache` stores compiled `WasmDriver` instances keyed by 32-byte Blake3 hash (`wasm_module_cache_key(wasm)`).
- **Timeouts & Interruptions**:
  - `EngineEpochTicker` spawns a background thread ticking every 1ms (`SHARED_EPOCH_TICK_MS = 1`).
  - Store deadline is configured via `store.set_epoch_deadline(ticks)` and `store.epoch_deadline_trap()`.
- **Memory & Sandboxing**:
  - `StoreLimitsBuilder` bounds memory (`max_memory_bytes`), instances (1), memories (1), and tables (1).
  - Validates `DriverPermissions` (from `zap-capability`), rejecting unauthorized host capabilities (such as direct filesystem, ungranted network, or arbitrary device calls).
- **Fuel Accounting**:
  - Initial fuel set via `store.set_fuel(limits.fuel)`.
  - Fuel consumed computed via `limits.fuel.saturating_sub(store.get_fuel())`.
- **Host Imports (Driver ABI v1)**:
  - `zap:emit_event(ptr, len) -> i32`
  - `zap:memory_read(key_ptr, key_len, out_ptr, out_len) -> i32`
  - `zap:memory_write(ptr, len) -> i32`
  - `zap:device_call(ptr, len) -> i32`
  - Calls are recorded into `StoreState.host_calls: Vec<HostCallRecord>`.
- **WASM Driver Export ABI**:
  - `memory`: WASM linear memory export.
  - `zap_alloc(len: i32) -> i32`: Heap allocator in guest.
  - `zap_dealloc(ptr: i32, len: i32)`: Heap deallocator in guest.
  - `zap_execute(action_ptr: i32, action_len: i32, payload_ptr: i32, payload_len: i32) -> i64`: Main entry point returning `(result_ptr << 32) | result_len`.

### 3.2 Current `zap-driver-sdk` (`zap-driver-sdk/src/lib.rs`)
- Exposes `PackedResult`, `pack_result`, `unpack_result`.
- Exposes synchronous `ZapDriver` trait with `fn execute(&self, input: DriverInput<'_>) -> Result<Vec<u8>, DriverError>`.
- Exposes `execute_driver` helper.

### 3.3 Milestone 1 & 2 Status
- **Milestone 1 (`zap-net`)**: `GossipMesh` with vector clocks, 2/3 + 1 Byzantine dynamic quorum proposals, peer health tracking, and failover capability routing. All compiling and verified.
- **Milestone 2 (`zap-ledger`, `zap-crypto`)**: `MerkleMountainRange` accumulator, peak bagging, logarithmic inclusion proofs (`MmrInclusionProof`), and batch rollup commitments (`MmrRollupCommitment`). All compiling and verified.
- **Existing Crate Diagnostics**:
  - `zap-runtime/src/pipeline.rs:6` has an unresolved import `crate::WasmActionRuntime` which needs to be replaced with `WasmExecutor` / `AsyncWasmExecutor`.

---

## 4. Milestone 3 Detailed Requirements & Design

```
+-------------------------------------------------------------------------------------------------------+
|                                           crates/zap-runtime                                          |
|                                                                                                       |
|  +-------------------------------------+   +-------------------------------------------------------+  |
|  |           async_engine.rs           |   |                     streaming.rs                      |  |
|  |  - AsyncWasmExecutor                |   |  - SpscRingBuffer (lock-free circular ring buffer)    |  |
|  |  - FuelMeter                        |   |  - StreamingBufferPool (buffer allocation pool)       |  |
|  |  - Tokio async task host execution  |   |  - TcpStreamBuffer (async TCP adapter)                |  |
|  |  - Async host call handlers         |   |  - ModbusStreamBuffer (industrial framing simulator)  |  |
|  +-------------------------------------+   +-------------------------------------------------------+  |
|                     ^                                                  ^                              |
|                     |                                                  |                              |
|                     v                                                  v                              |
|  +-------------------------------------------------------------------------------------------------+  |
|  |                                              ipc.rs                                             |  |
|  |  - InterDriverIpcPipe (deterministic zero-copy IPC channel with memory sandboxing)              |  |
|  |  - IpcRouter (multi-stage inter-driver message routing)                                         |  |
|  +-------------------------------------------------------------------------------------------------+  |
|                                                     |                                                 |
|                                                     v                                                 |
|  +-------------------------------------------------------------------------------------------------+  |
|  |                                           pipeline.rs                                           |  |
|  |  - DriverPipeline (orchestrating Perception -> Safety Policy -> Actuator)                       |  |
|  |  - Aggregate Fuel Budgeting & Monitoring (strict budget enforcement across all stages)          |  |
|  |  - Composite Blake3 Causal Chain Hashing & Execution Reports                                    |  |
|  +-------------------------------------------------------------------------------------------------+  |
+-------------------------------------------------------------------------------------------------------+
                                                     |
                                                     v
+-------------------------------------------------------------------------------------------------------+
|                                         crates/zap-driver-sdk                                         |
|                                                                                                       |
|  +-------------------------------------+   +-------------------------------------------------------+  |
|  |             async_driver            |   |                 ring_buffer / pinned                  |  |
|  |  - AsyncZapDriver trait             |   |  - PinnedBuffer (cache-aligned zero-copy pinned views)|  |
|  |  - Async lifecycle (init/exec/stop) |   |  - BufferSlice (lightweight non-owning slice helper)  |  |
|  |  - DriverPipeEndpoint primitives    |   |  - IpcMessage & IpcChannelTopology                    |  |
|  +-------------------------------------+   +-------------------------------------------------------+  |
+-------------------------------------------------------------------------------------------------------+
```

### 4.1 Sub-Milestone M3.1: `zap-driver-sdk` Async & Zero-Copy Primitives
1. **`AsyncZapDriver` trait**:
   - Non-blocking asynchronous lifecycle trait:
     - `initialize(&mut self, config: &[u8]) -> impl Future<Output = Result<(), DriverError>> + Send`
     - `execute_async(&mut self, input: DriverInput<'_>) -> impl Future<Output = Result<Vec<u8>, DriverError>> + Send`
     - `process_stream(&mut self, rx: &mut PinnedBuffer, tx: &mut PinnedBuffer) -> impl Future<Output = Result<usize, DriverError>> + Send`
     - `handle_event(&mut self, event: &[u8]) -> impl Future<Output = Result<Option<Vec<u8>>, DriverError>> + Send`
     - `shutdown(&mut self) -> impl Future<Output = Result<(), DriverError>> + Send`
   - Default implementations for optional lifecycle methods.
2. **Zero-Copy Pinned Buffers (`PinnedBuffer`, `BufferSlice`)**:
   - `PinnedBuffer`: Pre-allocated, page/cache-aligned contiguous byte buffer supporting zero-copy read/write cursors, capacity checks, and slice views.
   - `BufferSlice<'a>`: Zero-copy borrowed sub-slice with start/len indexing and sub-slicing methods.
3. **Driver IPC Channel Endpoints**:
   - `DriverPipeEndpoint`: Channel endpoint metadata (sender/receiver role, channel ID, buffer capacity).
   - `IpcMessage`: Frame envelope with sequence number, stage name, action, payload, and timestamp.

### 4.2 Sub-Milestone M3.2: `zap-runtime` Async Engine (`async_engine.rs`)
1. **`AsyncWasmExecutor`**:
   - Tokio-based asynchronous WASM executor.
   - Non-blocking execution API: `execute_async(&self, driver: &WasmDriver, action: &str, payload: &[u8], limits: ExecutionLimits) -> Result<WasmExecutionResult, ZapRuntimeError>`.
   - Spawns execution cleanly on Tokio blocking tasks (`tokio::task::spawn_blocking`) with cooperative epoch timeout management and isolated `Store` instances.
2. **Strict `FuelMeter` Accounting**:
   - `FuelMeter`: Manages initial budget, remaining fuel, fuel consumption per stage, and aggregate bounds checking.
   - Enforces deterministic fuel limits so runaway or infinite loops are cleanly trapped.
3. **Sandboxed Memory Isolation**:
   - Per-instance `StoreLimitsBuilder` setting `max_memory_bytes`.
   - Permissions enforcement preventing unauthorized host calls.

### 4.3 Sub-Milestone M3.3: `zap-runtime` Streaming & Ring-Buffers (`streaming.rs`)
1. **`SpscRingBuffer`**:
   - High-performance, lock-free circular ring buffer for Single-Producer Single-Consumer streaming.
   - Atomic head/tail tracking.
   - Zero-copy read/write slice methods: `reserve_write_slice`, `commit_write`, `read_slice`, `commit_read`.
   - Backpressure notification using Tokio `Notify`.
2. **`StreamingBufferPool`**:
   - Manages pre-allocated pools of `SpscRingBuffer`s of varying standard sizes (4KB, 64KB, 1MB).
   - `acquire_ring_buffer(capacity: usize) -> Arc<SpscRingBuffer>`.
3. **Async Streaming Adapters**:
   - `TcpStreamBuffer`: Async TCP socket reader/writer feeding directly into / draining from an `SpscRingBuffer`.
   - `ModbusStreamBuffer`: Industrial protocol simulator/adapter handling Modbus RTU / TCP packet framing (Transaction ID, Function Code, Register addresses, data payloads).
   - `StreamingSession`: Binds an async I/O stream source to an async WASM driver with backpressure flow control.

### 4.4 Sub-Milestone M3.4: `zap-runtime` Inter-Driver IPC & `DriverPipeline` (`ipc.rs`, `pipeline.rs`)
1. **`InterDriverIpcPipe`**:
   - Zero-copy inter-driver message pipe connecting Stage A to Stage B.
   - Strict memory isolation: Each driver runs in its own sandboxed WASM memory; messages transfer across the pipe via pinned buffers / ring buffers.
2. **`DriverPipeline`**:
   - Multi-stage pipeline orchestrator (e.g. Perception -> Safety Policy Filter -> Physical Actuator).
   - Both synchronous (`execute`) and asynchronous (`execute_async`) execution methods.
   - Aggregate fuel budgeting: Total fuel consumed across all stages must not exceed `max_total_fuel`. If any stage exhausts the remaining aggregate fuel, execution aborts with `PipelineError::FuelLimitExceeded`.
   - Comprehensive `PipelineExecutionReport`:
     - Stage metrics: `PipelineStageResult` (stage index, name, action, fuel consumed, output length, output Blake3 hash, duration micros).
     - Blake3 causal chain digest: `causal_chain_hash` verifying the entire causal pipeline provenance.
     - Final processed payload.

---

## 5. Backward Compatibility & Verification Strategy

### 5.1 Backward Compatibility
- **`WasmExecutor`**: Existing synchronous APIs (`new()`, `compile()`, `compile_and_validate()`, `compile_and_validate_cached()`, `execute()`, `execute_bytes()`) remain completely intact and unchanged. `zap-node` and `zap-cli` will continue functioning without any breaking changes.
- **`ZapDriver`**: Existing synchronous driver trait in `zap-driver-sdk` remains unchanged.
- **ABI v1**: The WASM ABI (`zap_alloc`, `zap_dealloc`, `zap_execute`, host imports) remains fully compatible.

### 5.2 Verification Matrix
| Component | Verification Target | Test Method |
|---|---|---|
| `zap-driver-sdk` | `AsyncZapDriver` trait & `PinnedBuffer` | Unit tests in `zap-driver-sdk` testing async lifecycle, zero-copy buffer slicing, and IPC endpoint structs. |
| `zap-runtime` | `AsyncWasmExecutor` & `FuelMeter` | Unit & integration tests executing WASM/WAT text drivers on Tokio tasks, verifying fuel consumption, timeouts, and sandboxing. |
| `zap-runtime` | `SpscRingBuffer`, TCP & Modbus stream adapters | Ring buffer wrap-around tests, multi-threaded SPSC streaming benchmarks, TCP streaming simulator, Modbus framing tests. |
| `zap-runtime` | `InterDriverIpcPipe` & `DriverPipeline` | Multi-stage pipeline tests (Perception -> Policy -> Actuator) verifying aggregate fuel limits, causal Blake3 chain hash, and zero-copy data passing. |
| Workspace | Full regression validation | `cargo test --workspace --all-targets` and `cargo clippy --workspace --all-targets -- -D warnings`. |
