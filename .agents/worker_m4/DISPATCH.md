# Dispatch Assignment

## 2026-08-14T22:57:34Z

You are worker_m4 operating in working directory: c:\Users\Stagiaire\Documents\Amadou PGC\Prs\ZAP\.agents\worker_m4.

Your objective is to execute Milestone 4 (AI Agent Gateway & Multi-Transport Integration) according to the blueprint in `c:\Users\Stagiaire\Documents\Amadou PGC\Prs\ZAP\.agents\explorer_m4_2\handoff.md` and user requirements in `c:\Users\Stagiaire\Documents\Amadou PGC\Prs\ZAP\.agents\ORIGINAL_REQUEST.md`.

DO NOT CHEAT. All implementations must be genuine. DO NOT hardcode test results, create dummy/facade implementations, or circumvent the intended task. A teamwork_preview_auditor will independently verify your work. Integrity violations WILL be detected and your work WILL be rejected.

Key Tasks:
1. Read `c:\Users\Stagiaire\Documents\Amadou PGC\Prs\ZAP\.agents\explorer_m4_2\handoff.md` and `c:\Users\Stagiaire\Documents\Amadou PGC\Prs\ZAP\.agents\ORIGINAL_REQUEST.md`.
2. Implement `ProvenanceChainDigest` in `crates/zap-agent`:
   - 6-stage cryptographic chain digest linking ($H_{\text{intent}} \to H_{\text{negotiation}} \to H_{\text{policy}} \to H_{\text{driver}} \to H_{\text{poa}} \to H_{\text{receipt}}$).
   - Compute step hashes using BLAKE3 or SHA256 canonical representations.
   - Implement `ProvenanceChainDigest::verify(...)` method with detailed error checking for missing links, broken hashes, and tamper detection.
3. Create crate `crates/zap-gateway` and register it in `Cargo.toml` workspace members:
   - Implement JSON-RPC 2.0 MCP Server (`initialize`, `tools/list`, `tools/call`, `resources/list`, `resources/read`, `prompts/list`, `prompts/get`).
   - Implement Multi-Transport Bridge:
     - HTTP REST API (`/v1/agent/intents`, `/v1/agent/sessions`, `/v1/agent/receipts`).
     - SSE stream handler (`/v1/agent/events` / `/v1/agent/session/{id}/stream`).
     - WebSocket bridge (`/v1/agent/ws`) with max frame size enforcement and framing.
4. Update tests in `tests/e2e/tests/e2e_suite.rs` for F09, F10, F11, boundary cases, and real-world scenarios to interact with real `zap-gateway` and `zap-agent` types and verify exact functionality.
5. Run verification:
   - `cargo build --workspace`
   - `cargo test -p zap-agent -p zap-gateway --all-targets`
   - `cargo test --package zap-e2e --test e2e`
   - `cargo clippy --workspace --all-targets -- -D warnings`
6. Write a comprehensive handoff report to `c:\Users\Stagiaire\Documents\Amadou PGC\Prs\ZAP\.agents\worker_m4\handoff.md`. Communicate via send_message to parent when finished.
