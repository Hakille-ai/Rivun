# DISPATCH

## 2026-08-15T20:07:22Z
You are worker_m3_r3_2 for Milestone 3 (Async WASM Driver Pipeline & Inter-Driver IPC).
Your working directory is: c:\Users\Stagiaire\Documents\Amadou PGC\Prs\rivun\.agents\sub_orch_m3_worker_2

Read:
- c:\Users\Stagiaire\Documents\Amadou PGC\Prs\rivun\.agents\ORIGINAL_REQUEST.md
- c:\Users\Stagiaire\Documents\Amadou PGC\Prs\rivun\PROJECT.md
- c:\Users\Stagiaire\Documents\Amadou PGC\Prs\rivun\.agents\sub_orch_m3\SCOPE.md
- Explorer reports:
  - c:\Users\Stagiaire\Documents\Amadou PGC\Prs\rivun\.agents\sub_orch_m3_explorer_1\analysis.md
  - c:\Users\Stagiaire\Documents\Amadou PGC\Prs\rivun\.agents\sub_orch_m3_explorer_2\analysis.md
  - c:\Users\Stagiaire\Documents\Amadou PGC\Prs\rivun\.agents\sub_orch_m3_explorer_3\analysis.md

MANDATORY INTEGRITY WARNING:
DO NOT CHEAT. All implementations must be genuine. DO NOT hardcode test results, create dummy/facade implementations, or circumvent the intended task. A teamwork_preview_auditor will independently verify your work. Integrity violations WILL be detected and your work WILL be rejected.

Your Write Ownership:
- `crates/rivun-driver-sdk/Cargo.toml`
- `crates/rivun-driver-sdk/src/*`
- `crates/rivun-runtime/Cargo.toml`
- `crates/rivun-runtime/src/*`
- `crates/rivun-runtime/tests/*` (if needed)

Your Task:
1. Update `crates/rivun-driver-sdk`:
   - In `Cargo.toml`, add `bytes.workspace = true`, `serde.workspace = true`, `thiserror.workspace = true`.
   - Implement `AsyncZapDriver` trait with async lifecycle (`init`, `process_stream`, `handle_event`, `shutdown`).
   - Implement zero-copy buffer views: `PinnedBuffer`, `BufferSlice`, `BufferSliceMut`, memory mapping helpers, memory slice utilities.
   - Implement IPC primitives: `IpcMessage`, `IpcChannelConfig`, `IpcPipe`, `IpcFlags`, `BackpressureStrategy`.
   - Implement `SyncDriverAdapter` for backward compatibility with `ZapDriver`.
   - Add unit tests for all SDK components.

2. Update `crates/rivun-runtime`:
   - In `Cargo.toml`, add `tokio.workspace = true`, `bytes.workspace = true`, `rivun-driver-sdk = { path = "../rivun-driver-sdk" }`.
   - Fix `src/pipeline.rs` import issue (`WasmExecutor` / `AsyncWasmExecutor`).
   - Implement `src/async_engine.rs`: `AsyncWasmExecutor` with Wasmtime async support, Tokio task runner, fuel metering, host functions (`rivun:yield_now`, `rivun:ipc_send`, `rivun:ipc_recv`), and async lifecycle execution.
   - Implement `src/streaming.rs`: `SpscRingBuffer` (cache-line aligned, zero-copy contiguous slices `(&[u8], &[u8])`, async backpressure using `tokio::sync::Notify`), `StreamingBufferPool`, `TcpStreamAdapter`/`TcpStreamBuffer`, `ModbusStreamAdapter`/`ModbusSimulator`.
   - Implement `src/ipc.rs`: `InterDriverIpcPipe`, `IpcRouter`, `IpcMessage`, monotonic sequence tracking, Blake3 causal transcripts, and strict WebAssembly memory sandboxing.
   - Implement / Update `src/pipeline.rs`: `DriverPipeline` chaining Perception -> Safety Policy -> Actuator drivers with strict aggregate fuel budgeting, latency profiling, and rolling Blake3 causal hashes.
   - Update `src/lib.rs` to re-export all new modules and types cleanly.
   - Add comprehensive unit tests in `src/async_engine.rs`, `src/streaming.rs`, `src/ipc.rs`, `src/pipeline.rs`, and integration tests.

3. Build and Verification:
   - Run `cargo check --workspace --all-targets`
   - Run `cargo test -p rivun-driver-sdk -p rivun-runtime`
   - Run `cargo test --workspace --all-targets`
   - Run `cargo clippy --workspace --all-targets -- -D warnings`
   - Ensure 100% tests pass with 0 failures and 0 clippy warnings.

