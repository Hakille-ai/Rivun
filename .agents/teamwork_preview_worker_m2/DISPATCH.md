## 2026-08-14T00:04:08Z
You are teamwork_preview_worker_m2 operating in working directory: c:\Users\Stagiaire\Documents\Amadou PGC\Prs\ZAP\.agents\teamwork_preview_worker_m2.
Read ORIGINAL_REQUEST.md at c:\Users\Stagiaire\Documents\Amadou PGC\Prs\ZAP\.agents\ORIGINAL_REQUEST.md, PROJECT.md at c:\Users\Stagiaire\Documents\Amadou PGC\Prs\ZAP\.agents\orchestrator\PROJECT.md, and Explorer M2 blueprint at c:\Users\Stagiaire\Documents\Amadou PGC\Prs\ZAP\.agents\teamwork_preview_explorer_m2\handoff.md.

MANDATORY INTEGRITY WARNING:
DO NOT CHEAT. All implementations must be genuine. DO NOT hardcode test results, create dummy/facade implementations, or circumvent the intended task. A teamwork_preview_auditor will independently verify your work. Integrity violations WILL be detected and your work WILL be rejected.

Objective: Implement Milestone 2 (R2: Signed Domain Pack Lifecycle & Marketplace).
Tasks:
1. Implement full `zap pack` CLI subcommands in `crates/zap-cli` (and `crates/zap-pack`): `init`, `build`, `sign`, `verify`, `install`, `audit`.
2. Implement ZapStore registry offline verification, `DomainPackBundle`, detached signature verification, dependency graph resolver, and policy/route validator in `crates/zap-store`.
3. Run unit and integration tests across affected crates (`cargo test -p zap-cli -p zap-pack -p zap-store`, `cargo clippy --workspace --all-targets -- -D warnings`).

Write your handoff report to `c:\Users\Stagiaire\Documents\Amadou PGC\Prs\ZAP\.agents\teamwork_preview_worker_m2\handoff.md` with complete test output logs. Notify orchestrator via send_message when finished.
