# BRIEFING — 2026-08-15T20:07:22Z

## Mission
Implement Milestone 3 (Async WASM Driver Pipeline & Inter-Driver IPC) across `crates/zap-driver-sdk` and `crates/zap-runtime` with full unit & integration tests, zero clippy warnings, and genuine logic.

## 🔒 My Identity
- Archetype: Implementer / QA / Specialist
- Roles: implementer, qa, specialist
- Working directory: c:\Users\Stagiaire\Documents\Amadou PGC\Prs\ZAP\.agents\sub_orch_m3_worker_2
- Original parent: 3b4ab3a6-4146-4f38-a23d-cba01d0ffde7
- Milestone: Milestone 3 - Async WASM Driver Pipeline & Inter-Driver IPC

## 🔒 Key Constraints
- Genuine implementation only, no dummy/facade implementations, no hardcoded test results.
- Zero-copy streaming views, SPSC ring buffer cache-line aligned, Blake3 causal transcripts, async Wasmtime lifecycle and host hooks.
- Workspace targets must build, pass all tests, and pass clippy with `-D warnings`.

## Current Parent
- Conversation ID: 3b4ab3a6-4146-4f38-a23d-cba01d0ffde7
- Updated: 2026-08-15T20:07:22Z

## Task Summary
- **What to build**:
  - `zap-driver-sdk`: `AsyncZapDriver` trait, zero-copy buffer views (`PinnedBuffer`, `BufferSlice`, `BufferSliceMut`, memory mapping / slice utilities), IPC primitives (`IpcMessage`, `IpcChannelConfig`, `IpcPipe`, `IpcFlags`, `BackpressureStrategy`), `SyncDriverAdapter`.
  - `zap-runtime`: `async_engine.rs` (`AsyncWasmExecutor` with Wasmtime async, Tokio runner, fuel metering, host functions), `streaming.rs` (`SpscRingBuffer`, `StreamingBufferPool`, `TcpStreamAdapter`, `ModbusStreamAdapter`), `ipc.rs` (`InterDriverIpcPipe`, `IpcRouter`, `IpcMessage`, Blake3 causal transcripts, WASM sandboxing), `pipeline.rs` (`DriverPipeline` 3-stage Perception->Safety->Actuator with rolling Blake3 causal hashes and fuel budgeting).
- **Success criteria**: All tests passing, clippy clean with zero warnings, all features implemented genuinely.
- **Interface contracts**: PROJECT.md, SCOPE.md.
- **Code layout**: `crates/zap-driver-sdk`, `crates/zap-runtime`.

## Change Tracker
- **Files modified**: None yet
- **Build status**: Pending
- **Pending issues**: None

## Quality Status
- **Build/test result**: Pending
- **Lint status**: Pending
- **Tests added/modified**: Pending

## Loaded Skills
- None

## Key Decisions Made
- Starting investigation of existing files and Explorer reports.

## Artifact Index
- `.agents/sub_orch_m3_worker_2/DISPATCH.md` — Assignment
- `.agents/sub_orch_m3_worker_2/BRIEFING.md` — Agent working memory
- `.agents/sub_orch_m3_worker_2/progress.md` — Liveness & task progress
