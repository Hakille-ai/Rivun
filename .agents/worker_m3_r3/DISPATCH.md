## 2026-08-15T20:08:13Z

You are the Implementation Worker for Milestone 3 (R3): Async WASM Driver Pipeline & Inter-Driver IPC.

Working directory: c:\Users\Stagiaire\Documents\Amadou PGC\Prs\ZAP\.agents\worker_m3_r3
Project root: c:\Users\Stagiaire\Documents\Amadou PGC\Prs\ZAP
Original Request: c:\Users\Stagiaire\Documents\Amadou PGC\Prs\ZAP\.agents\ORIGINAL_REQUEST.md
Scope document: c:\Users\Stagiaire\Documents\Amadou PGC\Prs\ZAP\.agents\sub_orch_m3\SCOPE.md
Survey Analysis: c:\Users\Stagiaire\Documents\Amadou PGC\Prs\ZAP\.agents\explorer_survey_2\analysis.md

MANDATORY INTEGRITY WARNING:
DO NOT CHEAT. All implementations must be genuine. DO NOT hardcode test results, create dummy/facade implementations, or circumvent the intended task. A teamwork_preview_auditor will independently verify your work. Integrity violations WILL be detected and your work WILL be rejected.

Your Mission:
Implement the complete, production-ready, genuine Milestone 3 features across `crates/zap-driver-sdk` and `crates/zap-runtime`:

1. `crates/zap-driver-sdk`:
   - `AsyncZapDriver` trait supporting async lifecycle, execution, and streaming (`process_stream`).
   - `AsyncStreamReader` and `AsyncStreamWriter` streaming traits.
   - Zero-copy buffer views (`PinnedBuffer`, `BufferSlice`, `ZeroCopyBuffer`, `IpcBufferView`).
   - Inter-driver IPC channel endpoints and ring-buffer primitives.
   - Comprehensive unit tests in `crates/zap-driver-sdk`.

2. `crates/zap-runtime`:
   - `async_engine.rs`: Non-blocking asynchronous WASM driver host execution on Tokio tasks (`AsyncWasmExecutor`), fuel budget tracking, async host function bindings (`zap.async_stream_read`, `zap.async_stream_write`, `zap.async_device_call`), and compiled async module caching.
   - `streaming.rs`: Streaming I/O buffers (TCP streams, Modbus simulation/primitives, circular lock-free/SPSC ring-buffers `SpscRingBuffer`, `StreamingBufferPool`) with backpressure policies (`DropOldest`, `DropNewest`, `BlockWithTimeout`).
   - `ipc.rs`: Deterministic zero-copy inter-driver IPC pipes with ring buffers, channel topologies, and memory isolation.
   - `pipeline.rs`: `DriverPipeline` orchestrating multi-stage driver graphs (perception -> safety filter -> actuator) with end-to-end latency monitoring, backpressure, and aggregate fuel budget enforcement (`PipelineFuelExhausted`).
   - Ensure backward compatibility with existing synchronous APIs (`WasmExecutor`, etc.) while adding async pipeline capabilities.

3. Testing and Verification:
   - Run `cargo test -p zap-driver-sdk` and `cargo test -p zap-runtime`.
   - Run `cargo test --workspace --all-targets` and ensure all tests pass with 0 failures.
   - Run `cargo clippy --workspace --all-targets -- -D warnings` and ensure 0 warnings.
   - Document all changes and verification results in `c:\Users\Stagiaire\Documents\Amadou PGC\Prs\ZAP\.agents\worker_m3_r3\handoff.md`.

Send a completion message back to the orchestrator when finished.

## 2026-08-15T20:18:54Z
**Context**: Milestone 3 Implementation
**Content**: Checking in on implementation status and test/clippy verification. Please update progress.md and send handoff report when complete.
**Action**: Please report current status.
