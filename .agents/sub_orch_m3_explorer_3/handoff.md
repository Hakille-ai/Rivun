# Milestone 3 Handoff Report: Async WASM Driver Pipeline & Inter-Driver IPC

**Author**: `explorer_m3_3`  
**Date**: 2026-08-15  
**Directory**: `c:\Users\Stagiaire\Documents\Amadou PGC\Prs\rivun\.agents\sub_orch_m3_explorer_3`  
**Report**: `c:\Users\Stagiaire\Documents\Amadou PGC\Prs\rivun\.agents\sub_orch_m3_explorer_3\analysis.md`

---

### 1. Observation
1. **Existing Crate & Code Structure**:
   - `crates/rivun-runtime/src/lib.rs` (lines 142–286) defines synchronous `WasmExecutor` using `wasmtime 45.0.1` (`consume_fuel(true)`, `epoch_interruption(true)`).
   - `crates/rivun-runtime/src/pipeline.rs` (line 6) contains an unresolved import error:
     ```
     error[E0432]: unresolved import `crate::WasmActionRuntime`
      --> crates\rivun-runtime\src\pipeline.rs:6:32
       |
     6 | use crate::{DriverPermissions, WasmActionRuntime, ZapRuntimeError};
       |                                ^^^^^^^^^^^^^^^^^ no `WasmActionRuntime` in the root
     ```
   - `crates/rivun-node/src/lib.rs` (lines 57, 1202, 1549, 3192, 4188, 4246) and `benches/runtime.rs` depend directly on synchronous `WasmExecutor`.
2. **Workspace Configuration**:
   - Root `Cargo.toml` lines 58 and 63 specify:
     ```toml
     tokio = { version = "1", features = ["macros", "net", "rt-multi-thread", "sync", "time"] }
     wasmtime = { version = "45.0.1", default-features = false, features = ["cranelift", "runtime", "std", "wat"] }
     ```
   - `crates/rivun-runtime/Cargo.toml` currently lacks `bytes`, `tokio`, and `wasmtime` async feature flags.
3. **M3 Scope & Requirements**:
   - `ORIGINAL_REQUEST.md §R3` and `PROJECT.md §Feature 7, 8, 9` mandate:
     - Non-blocking asynchronous WASM driver host execution on Tokio tasks with memory sandboxing and strict fuel metering.
     - Lock-free circular ring-buffers supporting async streaming I/O (TCP, Modbus, Ring-Buffers).
     - Deterministic zero-copy inter-driver IPC chaining (Perception -> Policy -> Actuator) with aggregate fuel budgeting.

---

### 2. Logic Chain
1. *From Obs 1 & 2*: Synchronous `WasmExecutor` cannot be replaced or broken without breaking `rivun-node` and existing unit tests. Therefore, `AsyncWasmExecutor` must be introduced alongside `WasmExecutor` in `crates/rivun-runtime/src/async_engine.rs`.
2. *From Obs 2*: Wasmtime supports non-blocking async execution when configured with `config.async_support(true)`, requiring the `async` feature on the `wasmtime` crate. Enabling this allows `instantiate_async`, `call_async`, and `func_wrap_async` for cooperative yields, async IPC, and streaming I/O without blocking OS worker threads.
3. *From Obs 1*: The compilation error in `pipeline.rs` stems from an out-of-sync import (`WasmActionRuntime` vs `WasmExecutor`). Fixing `pipeline.rs` to use `WasmExecutor` (for sync) and `AsyncWasmExecutor` (for async) with exact `store.get_fuel()` delta calculation resolves the error and ensures strict aggregate fuel budgeting across pipeline stages.
4. *From Obs 3*: Lock-free streaming I/O requires a dedicated Single-Producer Single-Consumer circular ring buffer (`SpscRingBuffer` in `streaming.rs`) exposing zero-copy contiguous slices `(&[u8], &[u8])` with `tokio::sync::Notify` backpressure, connected to `TcpStreamAdapter` and `ModbusStreamAdapter` (with a simulated register bank for industrial testing).
5. *From Obs 3*: Inter-driver IPC (`ipc.rs`) must maintain strict WebAssembly memory sandboxing (no shared guest pointers across isolated instances). Host-mediated message passing using `bytes::Bytes` and monotonic sequence numbering guarantees deterministic FIFO ordering and causal integrity without compromising memory isolation.

---

### 3. Caveats
- **Async Stack Size**: Wasmtime's async fibers allocate an async stack (default 512KB). For hundreds of concurrent long-lived drivers, async stack size configuration (`Config::async_stack_size`) should be tuned if memory pressure is tight.
- **Modbus Protocol Scope**: Modbus adapter focuses on TCP ADU and RTU standard function codes (`0x01`, `0x02`, `0x03`, `0x04`, `0x05`, `0x06`, `0x0F`, `0x10`). Proprietary manufacturer function codes are not modeled.
- **Zero-Copy Invariant**: True zero-copy across separate WASM instances is impossible at the hardware level without shared linear memory; host-mediated slice copy directly from instance A's memory into instance B's `@@rivun_HEADER@@alloc` buffer is the optimal zero-copy/single-copy memory-safe pattern.

---

### 4. Conclusion
The detailed architecture for `crates/rivun-runtime` is fully specified and ready for implementation by the M3 worker. It comprises four core modules:
1. `async_engine.rs`: `AsyncWasmExecutor`, async host imports (`rivun:yield_now`, `rivun:ipc_send`, `rivun:ipc_recv`), and Tokio task spawning.
2. `streaming.rs`: `SpscRingBuffer` (zero-copy slices, async backpressure), `TcpStreamAdapter`, `ModbusStreamAdapter`, and `ModbusSimulator`.
3. `ipc.rs`: `IpcPipe`, `IpcRouter`, `IpcMessage`, and Blake3 causal transcript tracking.
4. `pipeline.rs`: `DriverPipeline` orchestrating Perception $\rightarrow$ Safety $\rightarrow$ Actuator chains with strict aggregate fuel budgeting, microsecond latency profiling, and causal chain hashing.

---

### 5. Verification Method
1. **Compilation & Unit Tests**:
   - `cargo test -p rivun-runtime` (verifies `async_engine`, `streaming`, `ipc`, and `pipeline` unit tests).
   - `cargo test --workspace --all-targets` (verifies zero regressions across `rivun-node`, `rivun-core`, etc.).
2. **Files to Inspect**:
   - `crates/rivun-runtime/src/async_engine.rs`
   - `crates/rivun-runtime/src/streaming.rs`
   - `crates/rivun-runtime/src/ipc.rs`
   - `crates/rivun-runtime/src/pipeline.rs`
   - `crates/rivun-runtime/src/lib.rs`
3. **Invalidation Conditions**:
   - If `pipeline.rs` exceeds `max_total_fuel` without returning `PipelineError::FuelLimitExceeded`.
   - If guest drivers can read/write memory outside their own sandboxed linear memory.
   - If Tokio worker threads block synchronously during driver I/O.

