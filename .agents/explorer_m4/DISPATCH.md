## 2026-08-14T19:05:20Z
You are Explorer M4 (AI Agent Gateway & MCP).
Working Directory: c:\Users\Stagiaire\Documents\Amadou PGC\Prs\rivun\.agents\explorer_m4
Read ORIGINAL_REQUEST.md at c:\Users\Stagiaire\Documents\Amadou PGC\Prs\rivun\.agents\ORIGINAL_REQUEST.md
Read PROJECT.md at c:\Users\Stagiaire\Documents\Amadou PGC\Prs\rivun\.agents\orchestrator\PROJECT.md

Investigate `crates/rivun-net`, `crates/rivun-node`, `crates/rivun-cli`, `crates/rivun-telemetry`, and project structure for Milestone 4:
1. Architectural design for `crates/rivun-gateway` crate.
2. MCP JSON-RPC 2.0 protocol handler (`tools/list`, `tools/call`, `prompts/*`, `resources/*`, tools: `@@rivun_HEADER@@send_transaction`, `@@rivun_HEADER@@query_state`, `@@rivun_HEADER@@get_fleet_health`, `@@rivun_HEADER@@inspect_pack`).
3. Multi-transport gateway: HTTP REST (`/v1/agent/*`), SSE (`/v1/agent/stream`), WebSocket bridge (`/v1/agent/ws`).
4. Cryptographic Provenance Chain Engine ($H_{intent} \to H_{negotiation} \to H_{policy} \to H_{driver} \to H_{poa} \to H_{receipt}$) and `@@rivun_HEADER@@verify_provenance`.
5. CLI subcommands (`rivun gateway start`, `rivun gateway status`, `rivun provenance verify`) and test suites (`F09`, `F10`, `F11`).

Formulate a detailed, 5-phase implementation plan for worker_m4 and write handoff.md in your working directory. Report your findings to parent when done.

