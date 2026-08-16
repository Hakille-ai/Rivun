## 2026-08-14T17:30:16Z
You are Worker M3 Fix responsible for implementing Milestone 3 remediation.
Working Directory: c:\Users\Stagiaire\Documents\Amadou PGC\Prs\ZAP\.agents\teamwork_preview_worker_m3_fix
Read ORIGINAL_REQUEST.md at c:\Users\Stagiaire\Documents\Amadou PGC\Prs\ZAP\.agents\ORIGINAL_REQUEST.md
Read PROJECT.md at c:\Users\Stagiaire\Documents\Amadou PGC\Prs\ZAP\.agents\orchestrator\PROJECT.md
Read Explorer Fix Roadmap at c:\Users\Stagiaire\Documents\Amadou PGC\Prs\ZAP\.agents\explorer_m3_remediation\handoff.md

MANDATORY INTEGRITY WARNING:
DO NOT CHEAT. All implementations must be genuine. DO NOT hardcode test results, create dummy/facade implementations, or circumvent the intended task. A teamwork_preview_auditor will independently verify your work. Integrity violations WILL be detected and your work WILL be rejected.

Execute the 5-step remediation plan detailed in Explorer's handoff:
1. Real `FleetDoctor` Health Checks (`crates/zap-telemetry/src/doctor.rs`): Replace hardcoded `FleetDoctorStatus::Passed` for `replay_guard`, `journal`, `pack_registry`, `certificate_validity` with real dynamic evaluation logic (inspect WAL headers `b"ZAPFRM01"`, receipt segment manifest Ed25519 signatures, ZapStore index signatures, Ed25519 identity keypair & PoA quorum $T \le N$), updating `overall_status.merge(...)`.
2. Real Process & Socket State Collection (`crates/zap-telemetry/src/incident.rs`): Implement `ProcessState::collect()` and `SocketState::collect()` to query live OS process RSS/VMS/CPU/threads/FDs and socket bindings, falling back gracefully to defaults if restricted.
3. Comprehensive `SecretRedactor` (`crates/zap-telemetry/src/incident.rs`): Expand keywords (`transport_key`, `pact_private_key`, `api_key`, `access_token`, `client_secret`), handle unspaced `key=hex64` regex matching (`\b[0-9a-fA-F]{64}\b`), redact PEM private key blocks (`-----BEGIN ... PRIVATE KEY-----`), and preserve inline JSON structure.
4. Gzip Tarball Archives (`crates/zap-telemetry` & `crates/zap-cli`): Add `flate2` dependency, implement `IncidentCapturer::build_tar_gz_archive`, and apply `flate2::write::GzEncoder` when `.tar.gz`/`.tgz` format or output extension is requested.
5. Metrics Parity Cleanup (`crates/zap-node/src/lib.rs` & `metrics.rs`): Remove `peers_active` zero-peer fallback and emit dedicated `zap_replay_drops_total` counter.

Run verification commands:
- `cargo test -p zap-telemetry -p zap-node -p zap-cli`
- `cargo clippy --workspace --all-targets -- -D warnings`

Write handoff.md in your working directory summarizing your changes, build/test results, and verification commands. Notify parent when finished.
