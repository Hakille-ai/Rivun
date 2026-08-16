## 2026-08-15T00:57:55Z
You are Worker M4 (AI Agent Gateway & MCP Server - Replacement).
Working Directory: c:\Users\Stagiaire\Documents\Amadou PGC\Prs\ZAP\.agents\teamwork_preview_worker_m4
Read ORIGINAL_REQUEST.md at c:\Users\Stagiaire\Documents\Amadou PGC\Prs\ZAP\.agents\ORIGINAL_REQUEST.md
Read PROJECT.md at c:\Users\Stagiaire\Documents\Amadou PGC\Prs\ZAP\.agents\orchestrator\PROJECT.md
Read Explorer Roadmap at c:\Users\Stagiaire\Documents\Amadou PGC\Prs\ZAP\.agents\explorer_m4\handoff.md

MANDATORY INTEGRITY WARNING:
DO NOT CHEAT. All implementations must be genuine. DO NOT hardcode test results, create dummy/facade implementations, or circumvent the intended task. A teamwork_preview_auditor will independently verify your work. Integrity violations WILL be detected and your work WILL be rejected.

Execute the 5-phase implementation plan detailed in Explorer M4's handoff:
1. Create `crates/zap-gateway` workspace crate and register in root `Cargo.toml`.
2. Implement Cryptographic Provenance Chain Engine ($H_{intent} \to H_{negotiation} \to H_{policy} \to H_{driver} \to H_{poa} \to H_{receipt} \to H_{root}$) with Ed25519 signatures and step-by-step verification API (`zap_verify_provenance`).
3. Implement MCP JSON-RPC 2.0 Protocol Engine (`initialize`, `tools/list`, `tools/call`, `resources/read`, `prompts/list`, standard error codes `-32700`, `-32601`, `-32602`, stdio transport loop).
4. Implement Multi-Transport Gateway (HTTP REST `/v1/agent/*`, SSE stream `/v1/agent/stream`, WebSocket bridge `/v1/agent/ws` with 4MB max frame limit). Wire node metrics (`zap_agent_gateway_requests_total`, `zap_agent_sessions_active`, `zap_provenance_verification_failures_total`).
5. Implement CLI subcommands (`zap gateway start`, `zap gateway status`, `zap provenance verify`) and test cases for F09, F10, F11 in `crates/zap-gateway` and `tests/e2e/tests/e2e_suite.rs`.

Run verification commands:
- `cargo test -p zap-gateway -p zap-agent -p zap-cli`
- `cargo test --test e2e_suite tc_f09 tc_f10 tc_f11`
- `cargo clippy --workspace --all-targets -- -D warnings`

Write handoff.md in your working directory summarizing your changes, build/test results, and verification commands. Notify parent when finished.
