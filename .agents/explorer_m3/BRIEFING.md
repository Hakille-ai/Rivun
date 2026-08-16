# BRIEFING — 2026-08-14T02:28:10Z

## Mission
Investigate ZAP crates (`zap-telemetry`, `zap-cli`, `zap-node`, etc.) and metrics/doctor infrastructure to formulate a complete implementation roadmap for M3 (Fleet Telemetry & Doctor).

## 🔒 My Identity
- Archetype: Teamwork explorer
- Roles: Fleet Telemetry & Doctor Explorer
- Working directory: c:\Users\Stagiaire\Documents\Amadou PGC\Prs\ZAP\.agents\explorer_m3
- Original parent: 095fccca-c79e-45c9-b11a-9b726328b7e6
- Milestone: M3 (Fleet Telemetry & Doctor)

## 🔒 Key Constraints
- Read-only investigation — do NOT implement
- Scope: `zap fleet doctor`, fleet topology & aggregation, `zap incident snapshot`, Prometheus exporter missing metrics.

## Current Parent
- Conversation ID: 095fccca-c79e-45c9-b11a-9b726328b7e6
- Updated: 2026-08-14T02:28:10Z

## Investigation State
- **Explored paths**: `Cargo.toml`, `docs/observability.md`, `crates/zap-cli/src/main.rs`, `crates/zap-node/src/lib.rs`, `tests/e2e/tests/e2e_suite.rs`, `TEST_INFRA.md`, `crates/zap-cli/tests/cli.rs`.
- **Key findings**:
  1. `crates/zap-telemetry` is currently missing from workspace `Cargo.toml` and needs to be created as a workspace crate.
  2. Fleet topology discovery & node state aggregation needs `FleetTopology` engine in `zap-telemetry` / `zap-cli` to poll peer `/healthz.json` and aggregate node states.
  3. `zap fleet doctor` CLI command needs implementation covering 6 health check criteria: network, storage, replay guard, journal, pack registry, certificate validity.
  4. `zap incident snapshot` currently lacks live process state (PID, CPU, RSS RAM, open FDs), live `/metrics` scrape, active socket state, and tar archive creation. Secret redaction must be enforced.
  5. Prometheus exporter in `ZapNodeMetricsSnapshot` is missing 7+ metrics: `zap_replay_rejections_total`, `zap_journal_segment_rotations_total`, `zap_segment_manifest_errors_total`, `zap_pack_verification_failures_total`, `zap_agent_gateway_requests_total`, `zap_agent_sessions_active`, `zap_provenance_verification_failures_total`, `zap_peers_active`.
- **Unexplored areas**: None within M3 scope.

## Key Decisions Made
- Formulated complete implementation roadmap for `worker_m3` covering crate creation, `zap-node` metrics expansion, `zap-cli` subcommand additions, and E2E test alignment.

## Artifact Index
- DISPATCH.md — Initial task dispatch
- BRIEFING.md — Working state briefing index
- progress.md — Progress log and liveness heartbeat
- handoff.md — 5-component handoff report and implementation roadmap for worker_m3
