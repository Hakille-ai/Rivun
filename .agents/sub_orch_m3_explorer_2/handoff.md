# Handoff Report: `rivun-driver-sdk` Specification & API Design for Milestone 3

**Agent**: `explorer_m3_2`  
**Milestone**: Milestone 3 — Async WASM Driver Pipeline & Inter-Driver IPC  
**Date**: 2026-08-15  
**Full Report**: `c:\Users\Stagiaire\Documents\Amadou PGC\Prs\rivun\.agents\sub_orch_m3_explorer_2\analysis.md`

---

## 1. Observation
1. **Existing Driver SDK (`crates/rivun-driver-sdk/src/lib.rs:13-74`)**:
   - SDK defines ABI version 1 with exports `memory`, `@@rivun_HEADER@@alloc`, `@@rivun_HEADER@@dealloc`, and `@@rivun_HEADER@@execute`.
   - Defines synchronous trait:
     ```rust
     pub trait ZapDriver {
         fn execute(&self, input: DriverInput<'_>) -> Result<Vec<u8>, DriverError>;
     }
     ```
   - Only dependencies in `Cargo.toml` is currently `criterion` in dev-dependencies.
2. **Current Runtime Pipeline (`crates/rivun-runtime/src/pipeline.rs:60-173`)**:
   - `DriverPipeline` executes stages sequentially, accumulating fuel and chaining intermediate outputs with BLAKE3 hashes.
   - Synchronous host execution requires non-blocking asynchronous streaming and zero-copy IPC pipes.
3. **Workspace Configuration (`Cargo.toml:31-63`)**:
   - Edition is `2024`, Rust version `1.93`.
   - Workspace already includes `bitflags`, `blake3`, `bytes`, `serde`, `thiserror`, `tokio`, `wasmtime`.
4. **Milestone 3 Requirements (`PROJECT.md:48-50`, `sub_orch_m3/SCOPE.md:6-16`)**:
   - Non-blocking async WASM driver host execution.
   - Lock-free circular ring-buffers for streaming I/O (TCP, Modbus, Ring-Buffers).
   - Deterministic zero-copy inter-driver IPC chaining (Perception -> Policy -> Actuator) with aggregate fuel budgeting.

---

## 2. Logic Chain
1. **From Observation 1 & 4**: Synchronous `ZapDriver::execute` requires dynamic memory allocation (`Vec<u8>`) on every invocation, which creates unacceptable overhead for streaming perception/telemetry. Adding `AsyncZapDriver` with `process_stream(input: &BufferSlice, output: &mut PinnedBuffer)` enables continuous zero-copy execution without re-allocation.
2. **From Observation 1 & 3**: Rust 2024 / 1.93 natively supports `async fn` in traits (AFIT) and RPITIT (`impl Future<Output = ...> + Send`). Adding `Send + Sync + 'static` bounds ensures safe dispatch on Tokio multi-threaded executors.
3. **From Observation 1 & 2**: Existing drivers must not break. Creating `SyncDriverAdapter<D: ZapDriver>` implementing `AsyncZapDriver` allows legacy sync drivers to run transparently in async pipelines.
4. **From Observation 2 & 4**: Inter-driver chaining requires structured messaging and deterministic provenance. Implementing `IpcMessage` (with BLAKE3 message digest) and `IpcPipe` (with incremental causal transcript hashing and sequence tracking) allows chaining stages with verified execution proofs.
5. **From Observation 3 & 4**: Memory isolation in WASM requires strict bounds checking. `MemoryMapper::validate_range` and `BufferSliceMut::split_at_mut` ensure that guest pointer translations and mutable slices never violate memory bounds or Rust aliasing rules.

---

## 3. Caveats
- Host-side Tokio scheduling engine (`AsyncWasmExecutor`) and lock-free ring-buffer internals are being investigated by companion explorer `explorer_m3_3` in `crates/rivun-runtime`. The interface contracts defined here in `rivun-driver-sdk` align directly with `rivun-runtime` host function bindings.
- WASM target compilation (`wasm32-unknown-unknown`) requires that non-WASM-compatible host system calls are avoided in guest code; all guest I/O goes through `rivun::ipc_send` and `rivun::ipc_recv` imports.

---

## 4. Conclusion
The specification and API design for `crates/rivun-driver-sdk` is complete and documented in detail in `analysis.md`. The design includes:
1. `AsyncZapDriver` trait with full lifecycle (`init`, `process_stream`, `handle_event`, `shutdown`).
2. Zero-copy buffer views: `PinnedBuffer`, `BufferSlice`, `BufferSliceMut`, and `MemoryMapper`.
3. Inter-driver IPC primitives: `IpcMessage`, `IpcChannelConfig`, `IpcPipe`, `IpcFlags`, `BackpressureStrategy`.
4. Backward-compatible `SyncDriverAdapter` bridging `ZapDriver` <-> `AsyncZapDriver`.
5. Strict memory isolation, data-race freedom, and deterministic causal chain guarantees.

---

## 5. Verification Method
1. Inspect the full specification report in `c:\Users\Stagiaire\Documents\Amadou PGC\Prs\rivun\.agents\sub_orch_m3_explorer_2\analysis.md`.
2. Verify workspace dependencies in `Cargo.toml` and existing SDK tests:
   ```powershell
   cargo test -p rivun-driver-sdk
   ```
3. Invalidation conditions: Any breaking change to `ZapDriver::execute` signature or `DRIVER_ABI_VERSION = 1` exports that breaks existing M1/M2 tests.

