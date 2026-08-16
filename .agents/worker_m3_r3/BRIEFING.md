# BRIEFING — 2026-08-15T20:10:00Z

## Mission
Implement Milestone 3 (R3): Async WASM Driver Pipeline & Inter-Driver IPC across `crates/zap-driver-sdk` and `crates/zap-runtime`.

## 🔒 My Identity
- Archetype: worker
- Roles: implementer, qa, specialist
- Working directory: c:\Users\Stagiaire\Documents\Amadou PGC\Prs\ZAP\.agents\worker_m3_r3
- Original parent: 9d3624de-782a-448d-bf0a-112246fc69a7
- Milestone: Milestone 3 (R3)

## 🔒 Key Constraints
- Production-ready, genuine implementation — no dummy/facade implementations, no hardcoded values.
- Backward compatibility with existing synchronous APIs (`WasmExecutor`, etc.).
- Strict fuel tracking, backpressure, lock-free/SPSC ring-buffers, inter-driver IPC zero-copy memory views.
- Clean compilation: `cargo test --workspace --all-targets` passes with 0 failures, `cargo clippy --workspace --all-targets -- -D warnings` with 0 warnings.

## Current Parent
- Conversation ID: 9d3624de-782a-448d-bf0a-112246fc69a7
- Updated: 2026-08-15T20:10:00Z

## Task Summary
- **What to build**:
  1. `crates/zap-driver-sdk`: `AsyncZapDriver`, `AsyncStreamReader`, `AsyncStreamWriter`, `PinnedBuffer`, `BufferSlice`, `ZeroCopyBuffer`, `IpcBufferView`, IPC channel endpoints & ring-buffer primitives.
  2. `crates/zap-runtime`:
     - `async_engine.rs`: `AsyncWasmExecutor`, async host bindings (`zap.async_stream_read`, `zap.async_stream_write`, `zap.async_device_call`), Tokio async execution, async module cache.
     - `streaming.rs`: `StreamingBufferPool`, `StreamTransport` (TCP, Modbus primitives, circular lock-free `SpscRingBuffer`), backpressure policies (`DropOldest`, `DropNewest`, `BlockWithTimeout`).
     - `ipc.rs`: Deterministic zero-copy inter-driver IPC pipes, channel topologies, memory isolation.
     - `pipeline.rs`: `DriverPipeline` orchestrating multi-stage driver graphs (perception -> safety filter -> actuator) with end-to-end latency monitoring, backpressure, aggregate fuel budget enforcement (`PipelineFuelExhausted`).
- **Success criteria**: Full test coverage, 0 clippy warnings, all workspace tests pass.
- **Interface contracts**: `crates/zap-driver-sdk/src/lib.rs`, `crates/zap-runtime/src/lib.rs`

## Key Decisions Made
- Integrate async support into Wasmtime Engine with Cranelift async execution.
- Maintain full backward compatibility for `WasmExecutor` while exposing `AsyncWasmExecutor` and async modules.
- Ensure clean module separation in `zap-runtime`: `async_engine`, `streaming`, `ipc`, `pipeline`.

## Change Tracker
- **Files modified**: [TBD]
- **Build status**: Initializing
- **Pending issues**: None

## Quality Status
- **Build/test result**: Running initial workspace test
- **Lint status**: Pending
- **Tests added/modified**: Pending

## Loaded Skills
- None
