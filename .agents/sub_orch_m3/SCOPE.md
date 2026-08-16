# Scope: Milestone 3 — Async WASM Driver Pipeline & Inter-Driver IPC

## Architecture
Milestone 3 implements the asynchronous runtime engine, streaming I/O buffers, zero-copy inter-driver IPC pipes, and `DriverPipeline` orchestrator chaining perception, safety policy, and physical actuator drivers with strict aggregate fuel budgeting, along with the `AsyncZapDriver` trait and zero-copy slice helpers in `zap-driver-sdk`.

### Core Components
1. `crates/zap-driver-sdk`:
   - `AsyncZapDriver` trait (async lifecycle, process stream, handle events, IPC endpoints).
   - Zero-copy pinned buffer views (`PinnedBuffer`, `BufferSlice`, memory mapping helpers).
   - Driver IPC channel endpoints / pipe primitives.
2. `crates/zap-runtime`:
   - `async_engine.rs`: Tokio-based async WASM driver execution engine with fuel budget tracking, async host function bindings, and lifecycle management.
   - `streaming.rs`: Streaming I/O buffers (TCP, Modbus simulation/primitives, circular ring-buffers) with backpressure and zero-copy ring slices.
   - `ipc.rs`: Deterministic zero-copy inter-driver IPC pipes with ring buffers, channel topologies, and memory isolation.
   - `pipeline.rs`: `DriverPipeline` orchestrating multi-stage driver graphs (perception -> safety filter -> actuator) with end-to-end latency monitoring, backpressure, and aggregate fuel budget enforcement.

## Milestones
| # | Name | Scope | Dependencies | Status |
|---|------|-------|-------------|--------|
| M3.1 | zap-driver-sdk async & zero-copy | AsyncZapDriver, PinnedBuffer, IPC traits | M1, M2 | IN_PROGRESS |
| M3.2 | zap-runtime async engine | Non-blocking async host runtime on Tokio | M3.1 | IN_PROGRESS |
| M3.3 | zap-runtime streaming & ring-buffers | Streaming buffers, TCP/Modbus/Ring | M3.1 | IN_PROGRESS |
| M3.4 | zap-runtime IPC & DriverPipeline | Inter-driver IPC, multi-stage DriverPipeline, fuel budgeting | M3.2, M3.3 | IN_PROGRESS |
