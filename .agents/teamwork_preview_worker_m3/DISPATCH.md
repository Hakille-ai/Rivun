## 2026-08-14T02:28:46Z
You are Worker M3 (Fleet Telemetry & Doctor).
Working Directory: c:\Users\Stagiaire\Documents\Amadou PGC\Prs\rivun\.agents\teamwork_preview_worker_m3
Read ORIGINAL_REQUEST.md at c:\Users\Stagiaire\Documents\Amadou PGC\Prs\rivun\.agents\ORIGINAL_REQUEST.md
Read PROJECT.md at c:\Users\Stagiaire\Documents\Amadou PGC\Prs\rivun\.agents\orchestrator\PROJECT.md
Read Explorer Roadmap at c:\Users\Stagiaire\Documents\Amadou PGC\Prs\rivun\.agents\explorer_m3\handoff.md

MANDATORY INTEGRITY WARNING:
DO NOT CHEAT. All implementations must be genuine. DO NOT hardcode test results, create dummy/facade implementations, or circumvent the intended task. A teamwork_preview_auditor will independently verify your work. Integrity violations WILL be detected and your work WILL be rejected.

Execute the 5-phase implementation plan detailed in Explorer's handoff:
1. Create `crates/rivun-telemetry` crate and register in workspace `Cargo.toml`.
2. Implement Prometheus metrics parity in `rivun-node` and `rivun-telemetry` (all 16 metrics: replay drops, journal rotations, segment errors, pack failures, agent requests, active sessions/peers, provenance errors).
3. Implement `FleetTopology` and `FleetDoctor` in `rivun-telemetry` evaluating 6 core criteria (network, storage, replay guard, journal, pack registry, certificate validity). Add `rivun fleet doctor` CLI subcommand in `crates/rivun-cli`.
4. Upgrade `rivun incident snapshot` in `crates/rivun-cli` and `rivun-telemetry` to capture live process state (PID/CPU/RAM/FDs), sockets, live metrics, secret redaction, and `.tar.gz` archive creation.
5. Add tests in `crates/rivun-telemetry`, `crates/rivun-cli`, and update `tests/e2e/tests/e2e_suite.rs` (F06, F07, F08).

Run verification commands:
- `cargo test -p rivun-telemetry -p rivun-node -p rivun-cli`
- `cargo clippy --workspace --all-targets -- -D warnings`

Write handoff.md in your working directory when implementation and verification are complete. Notify parent when finished.

