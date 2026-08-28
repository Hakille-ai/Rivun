# Handoff Report: Milestone 3 (Fleet Telemetry & Doctor)

## 1. Observation

Direct observations from codebase inspection across `Cargo.toml`, `crates/rivun-node/src/lib.rs`, `crates/rivun-cli/src/main.rs`, `docs/observability.md`, `TEST_INFRA.md`, and `tests/e2e/tests/e2e_suite.rs`:

1. **Crate Layout (`Cargo.toml`)**:
   - `crates/rivun-telemetry` is listed in `PROJECT.md:59` and `TEST_INFRA.md:27-29` as the owner crate for telemetry, doctor diagnostics, and metrics, but the directory `crates/rivun-telemetry` does **not exist** on disk and is absent from `Cargo.toml` workspace `members` (lines 1-27).

2. **Metrics Parity (`crates/rivun-node/src/lib.rs`)**:
   - `ZapNodeMetricsSnapshot` (lines 1261-1272) and `to_prometheus_text()` (lines 1363-1451) currently emit 9 metrics: `@@rivun_HEADER@@frames_sent_total`, `@@rivun_HEADER@@frames_received_total`, `@@rivun_HEADER@@frames_rejected_total`, `@@rivun_HEADER@@driver_execution_errors_total`, `@@rivun_HEADER@@peer_trust_status`, `@@rivun_HEADER@@registry_signature_valid`, `@@rivun_HEADER@@capability_cache_age_seconds`, `@@rivun_HEADER@@receipt_log_verify_failures_total`, `@@rivun_HEADER@@poa_attestation_failures_total`.
   - The required missing Prometheus metrics specified in `TEST_INFRA.md:90-94` and `PROJECT.md:22` are missing:
     - `@@rivun_HEADER@@replay_rejections_total` / `@@rivun_HEADER@@replay_drops_total` (counter)
     - `@@rivun_HEADER@@journal_segment_rotations_total` (counter)
     - `@@rivun_HEADER@@segment_manifest_errors_total` (counter)
     - `@@rivun_HEADER@@pack_verification_failures_total` / `@@rivun_HEADER@@store_verifications_total` (counter)
     - `@@rivun_HEADER@@agent_gateway_requests_total` (counter, labelled by transport/status)
     - `@@rivun_HEADER@@agent_sessions_active` (gauge)
     - `@@rivun_HEADER@@provenance_verification_failures_total` (counter)
     - `@@rivun_HEADER@@peers_active` (gauge)

3. **Single Node vs Fleet Doctor (`crates/rivun-cli/src/main.rs`)**:
   - `rivun doctor` exists (lines 120-128, 2075-2160) and validates a single local `rivun.toml` configuration (`build_doctor_report`).
   - `rivun fleet doctor` subcommand is **not implemented** in `Commands` enum (lines 91-276). Multi-node peer reachability, storage mounts, replay guard status, journal rotation/manifest checks, registry signature checks, and certificate/key epoch validity across the cluster are not aggregated.

4. **Incident Snapshot Limitations (`crates/rivun-cli/src/main.rs`)**:
   - `IncidentCommand::Snapshot` (lines 1012-1030, 3432-3609) generates a static JSON snapshot of config validation, local doctor, memory summary, and receipt summary.
   - Lines 3603-3606 explicitly list limitations: `"runtime process state, network captures, and live /metrics HTTP output are not collected by this bounded CLI snapshot"`.
   - `rivun incident snapshot` does not capture PID/CPU/RAM/FDs, active socket connections, live Prometheus `/metrics`, or peer mesh states, and does not support creating `.tar.gz` archive bundles.

5. **Test Specifications (`TEST_INFRA.md` & `tests/e2e/tests/e2e_suite.rs`)**:
   - `TEST_INFRA.md` defines test specs for F06 (`TC-F06-001` .. `005`), F07 (`TC-F07-001` .. `005`), and F08 (`TC-F08-001` .. `005`).
   - `tests/e2e/tests/e2e_suite.rs` (lines 381-521) contains placeholder test skeletons for F06, F07, and F08 that need full implementation against live node & CLI APIs.

---

## 2. Logic Chain

1. **Creating `crates/rivun-telemetry`**:
   - Creating a dedicated `crates/rivun-telemetry` crate establishes clean modular boundaries as defined in `PROJECT.md`.
   - It will encapsulate:
     - `metrics`: Metric definitions, counters, gauges, and Prometheus text exporter logic.
     - `topology`: Peer discovery mesh aggregation, peer state polling, node status summary (`healthy`, `degraded`, `critical`, `unreachable`).
     - `doctor`: Fleet doctor evaluation logic checking network, storage, replay guard, journal, pack registry, certificate validity.
     - `incident`: Live process state (PID, CPU, RSS, FDs), socket state, peer mesh state, secret redaction, JSON and `.tar.gz` archive generator.

2. **Metrics Parity Implementation (`rivun-node` & `rivun-telemetry`)**:
   - Expand `NodeMetricsCounters` and `ZapNodeMetricsSnapshot` in `rivun-node` (or `rivun-telemetry`) to track:
     - `replay_rejections_total` (incremented on `DurableReplayStore` drops)
     - `journal_segment_rotations_total` (incremented on `JournalRotator::rotate_and_seal`)
     - `segment_manifest_errors_total` (incremented on manifest verify/sign failure)
     - `pack_verification_failures_total` / `store_verifications_total` (incremented in pack/store verification)
     - `agent_gateway_requests_total` (incremented on MCP/REST/SSE/WS request dispatch)
     - `agent_sessions_active` (incremented/decremented on session start/close)
     - `provenance_verification_failures_total` (incremented on provenance digest chain failure)
     - `peers_active` (updated during peer discovery refresh)
   - Expose helper methods on `ZapNode`: `record_replay_drop()`, `record_segment_rotation()`, `record_segment_manifest_error()`, `record_pack_verification_failure()`, `record_agent_gateway_request()`, `inc_agent_session()`, `dec_agent_session()`, `record_provenance_verification_failure()`, `set_active_peers()`.
   - Update `to_prometheus_text()` to format all 16 metrics per Prometheus text exposition format 0.0.4.

3. **Fleet Topology & `rivun fleet doctor` Implementation**:
   - Add CLI subcommand `Fleet` / `FleetDoctor` under `Commands` in `crates/rivun-cli/src/main.rs`:
     `rivun fleet doctor [--config <path>] [--strict] [--json] [--timeout-ms <ms>] [--peer <uuid>]`
   - Fleet Doctor multi-node check pipeline:
     1. **Network**: Ping/query each configured peer's UDP bind or HTTP `/healthz.json` endpoint. Check socket reachability, latency, transport key epoch freshness.
     2. **Storage**: Verify receipt journal directory (`[receipts].dir`) and memory directory (`[memory].dir`) are mounted, writable, and have sufficient disk space.
     3. **Replay Guard**: Verify `DurableReplayStore` WAL file exists and is writable, check clock skew tolerance, verify replay rejection rate is zero/low.
     4. **Journal**: Verify receipt journal segment rotation status, check segment manifest signatures (`.zjmanifest.json.sig`), verify index integrity.
     5. **Pack Registry**: Verify local RivunStore registry index is loadable, verify `@@rivun_HEADER@@registry_signature_valid` == 1, check bundle manifest integrity.
     6. **Certificate & Key Validity**: Verify Ed25519 node identity keypair, check PACT signature validity, verify PoA validator set reachability and quorum threshold ($T \le N$).
   - Output structured JSON when `--json` is supplied or readable diagnostic report. Return non-zero exit code on `--strict` when warnings exist or when status is critical/failed.

4. **Live Incident Snapshot Capture (`rivun incident snapshot`)**:
   - Enhance `IncidentCommand::Snapshot` in `crates/rivun-cli/src/main.rs`:
     `rivun incident snapshot [--config <path>] [--memory <path>] [--receipts <path>] [--capability-cache <path>] [--format json|tar] [--out <path>] [--force]`
   - Capture live diagnostic elements:
     1. **Process state**: PID (`std::process::id()`), memory usage (RSS/VMS), CPU time/usage, thread count, open file descriptors count, process start time / uptime.
     2. **Metrics**: Live Prometheus text scrape from local `/metrics` HTTP endpoint or `metrics_prometheus_text()`.
     3. **Socket state**: Active listening ports, bound UDP/TCP sockets, peer connection counts.
     4. **Peer mesh state**: Reachable peers, transport key epochs, peer trust statuses, last seen timestamps.
     5. **Secret Redaction**: Filter all captured configs, logs, and diagnostic files through strict redaction masking Ed25519 private keys, transport keys, authorization headers, and raw payload contents with `[REDACTED]`.
     6. **Archive Creation**: When `--format tar` or `--archive` is specified, generate a `.tar.gz` archive containing `snapshot.json`, `metrics.prom`, `config.redacted.toml`, `health.json`, `diagnostics.txt`.

---

## 3. Caveats

1. **Platform Differences for Process & Socket Metrics**:
   - Collecting process RSS, CPU usage, and open file descriptor count differs between OS platforms (Windows, Linux, macOS).
   - On Windows, `sysinfo` or standard Win32 API / `std::process` fallback should be handled gracefully with fallback default values if system APIs are unavailable, so tests pass reliably on all OS environments.
2. **Network Timeouts during Fleet Doctor**:
   - Peer node reachability checks in `rivun fleet doctor` must execute asynchronously with strict per-peer timeout (`--timeout-ms`, default 2000ms) to prevent CLI blocking if a peer node is offline.
3. **Archive Tar Creation**:
   - For `.tar.gz` creation, `tar` and `flate2` crates (or standard `tar` implementation) should be used cleanly without external system binary dependencies (`tar` CLI executable).

---

## 4. Conclusion & Implementation Roadmap for `worker_m3`

### Step-by-Step Execution Plan

#### Phase 1: Create `crates/rivun-telemetry` & Update Workspace Workspace
1. Create directory `crates/rivun-telemetry` with `Cargo.toml`:
   - Name: `rivun-telemetry`
   - Dependencies: `anyhow`, `serde`, `serde_json`, `tokio`, `uuid`, `tracing`, `rivun-core`, `rivun-crypto`, `rivun-net`, `rivun-ledger`, `rivun-store`.
2. Add `"crates/rivun-telemetry"` to `Cargo.toml` root workspace `members` and `[workspace.dependencies]`.
3. Create `crates/rivun-telemetry/src/lib.rs` with modules: `metrics`, `topology`, `doctor`, `incident`.

#### Phase 2: Expand Prometheus Metrics Parity in `rivun-node` & `rivun-telemetry`
1. Update `NodeMetricsCounters` and `ZapNodeMetricsSnapshot` in `crates/rivun-node/src/lib.rs` (and/or `rivun-telemetry`):
   - Add counters: `replay_rejections_total`, `journal_segment_rotations_total`, `segment_manifest_errors_total`, `pack_verification_failures_total`, `agent_gateway_requests_total`, `provenance_verification_failures_total`.
   - Add gauges: `agent_sessions_active`, `peers_active`.
2. Add public recorder methods to `ZapNode`:
   - `record_replay_drop(&self, peer: Uuid)`
   - `record_segment_rotation(&self)`
   - `record_segment_manifest_error(&self)`
   - `record_pack_verification_failure(&self)`
   - `record_agent_gateway_request(&self, transport: &str, status: &str)`
   - `inc_agent_session(&self)`, `dec_agent_session(&self)`
   - `record_provenance_verification_failure(&self)`
   - `set_active_peers(&self, count: usize)`
3. Update `to_prometheus_text()` in `ZapNodeMetricsSnapshot` to output all 16 metrics formatted according to Prometheus text 0.0.4 exposition standard with `# HELP` and `# TYPE`.
4. Connect `DurableReplayStore` rejection events in `rivun-node` / `rivun-net` and `JournalRotator::rotate_and_seal()` in `rivun-journal` to call metric recorders.

#### Phase 3: Implement Fleet Topology & `rivun fleet doctor`
1. Implement `FleetTopology` in `rivun-telemetry/src/topology.rs`:
   - Structure `FleetNodeState`: `node_id`, `addr`, `trust_status`, `health_status`, `capabilities`, `rtt_ms`, `last_seen_micros`.
   - Aggregate multi-node states across static configuration and dynamic discovery.
2. Implement `FleetDoctor` in `rivun-telemetry/src/doctor.rs`:
   - Evaluates 6 core health check criteria:
     1. Network (UDP socket reachability, transport key epoch)
     2. Storage (receipt/memory directory mounted & writable)
     3. Replay Guard (`DurableReplayStore` WAL file & clock skew)
     4. Journal (segment rotation, manifest signatures valid)
     5. Pack Registry (RivunStore index present & signature valid)
     6. Certificate Validity (node keypair, PACT signatures, PoA quorum)
3. Add CLI subcommand `rivun fleet doctor` in `crates/rivun-cli/src/main.rs`:
   - Accepts `--config`, `--strict`, `--json`, `--timeout-ms`, `--peer`.
   - Connects to `FleetDoctor`, formats tabular or JSON output, sets appropriate exit code (0 for healthy/degraded without `--strict`, 1 for critical or strict warnings).

#### Phase 4: Upgrade `rivun incident snapshot` with Live Capture & Tar Archives
1. Implement `IncidentCapturer` in `rivun-telemetry/src/incident.rs`:
   - Capture live process metrics: PID, RSS/VMS memory bytes, CPU time, thread count, open FDs.
   - Capture live Prometheus `/metrics` text exposition.
   - Capture active UDP/TCP socket state and connection counts.
   - Capture peer mesh topology state.
   - Apply secret redaction filter replacing private keys, transport keys, auth tokens, and raw payloads with `[REDACTED]`.
   - Support JSON output format and `.tar.gz` archive creation (`--format tar` / `--archive`).
2. Update `IncidentCommand::Snapshot` in `crates/rivun-cli/src/main.rs`:
   - Wire options `--format json|tar`, `--out <path>`, `--force`.

#### Phase 5: Implement Test Coverage
1. Update `tests/e2e/tests/e2e_suite.rs`:
   - Fill out tests for `TC-F06-001` .. `005`, `TC-F07-001` .. `005`, `TC-F08-001` .. `005`.
2. Add integration tests in `crates/rivun-cli/tests/cli.rs` and `crates/rivun-telemetry/tests/`.

---

## 5. Verification Method

To verify the implementation of M3:

1. **Workspace Compilation & Clippy**:
   ```bash
   cargo check --workspace --all-targets
   cargo clippy --workspace --all-targets -- -D warnings
   ```

2. **Metrics Parity Unit Tests**:
   ```bash
   cargo test -p rivun-node --lib
   cargo test -p rivun-telemetry
   ```
   Verify all 16 metrics appear in `GET /metrics` text output and counters increment on replay drops, journal rotations, agent sessions, and provenance errors.

3. **Fleet Doctor CLI Verification**:
   ```bash
   cargo run -p rivun-cli -- fleet doctor --config rivun.toml --json
   cargo run -p rivun-cli -- fleet doctor --config rivun.toml --strict
   ```
   Verify 6 criteria checks pass, output JSON matches schema, and `--strict` handles warnings correctly.

4. **Incident Snapshot Verification**:
   ```bash
   cargo run -p rivun-cli -- incident snapshot --config rivun.toml --format json --out snapshot.json
   cargo run -p rivun-cli -- incident snapshot --config rivun.toml --format tar --out snapshot.tar.gz
   ```
   Verify PID, CPU/RAM, socket state, live metrics are included, all secrets are redacted, and `.tar.gz` expands cleanly.

5. **E2E Test Suite Verification**:
   ```bash
   cargo test --test e2e_suite tc_f06
   cargo test --test e2e_suite tc_f07
   cargo test --test e2e_suite tc_f08
   ```

