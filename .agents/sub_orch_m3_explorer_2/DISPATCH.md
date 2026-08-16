## 2026-08-15T15:02:51Z
You are explorer_m3_2 for Milestone 3 (Async WASM Driver Pipeline & Inter-Driver IPC).
Your working directory is: c:\Users\Stagiaire\Documents\Amadou PGC\Prs\ZAP\.agents\sub_orch_m3_explorer_2
Read:
- c:\Users\Stagiaire\Documents\Amadou PGC\Prs\ZAP\.agents\ORIGINAL_REQUEST.md
- c:\Users\Stagiaire\Documents\Amadou PGC\Prs\ZAP\PROJECT.md
- c:\Users\Stagiaire\Documents\Amadou PGC\Prs\ZAP\.agents\sub_orch_m3\SCOPE.md
- crates/zap-driver-sdk/src/*

Investigate:
1. Exact specification and API design for `crates/zap-driver-sdk`:
   - `AsyncZapDriver` trait (async driver lifecycle: `init`, `process_stream`, `handle_event`, `shutdown`).
   - Zero-copy buffer views: `PinnedBuffer`, `BufferSlice`, memory mapping helpers, memory slice utilities for safe guest-host pointer translation and zero-copy access.
   - IPC primitives: `IpcChannel`, `IpcEndpoint`, `IpcMessage`, `IpcPipe` abstractions for guest driver usage.
2. Interaction between sync `ZapDriver` (from M2) and `AsyncZapDriver` (M3).
3. Rust traits, async-trait / native async fn in trait considerations, safety invariants, Send + Sync bounds.

Write your full structured report to: c:\Users\Stagiaire\Documents\Amadou PGC\Prs\ZAP\.agents\sub_orch_m3_explorer_2\analysis.md
and a concise handoff to: c:\Users\Stagiaire\Documents\Amadou PGC\Prs\ZAP\.agents\sub_orch_m3_explorer_2\handoff.md
Send a message when done referencing your report path.
