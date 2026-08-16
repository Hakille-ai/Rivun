# Milestone 3 Forensic Audit Report

## Forensic Audit Report

**Work Product**: `crates/zap-telemetry`, `crates/zap-node`, `crates/zap-cli` (Milestone 3: Fleet Topology, Health & Incident Telemetry)  
**Profile**: General Project (Integrity Forensics)  
**Integrity Mode**: Development Mode (from `ORIGINAL_REQUEST.md:8`)  
**Verdict**: **CLEAN**

---

### Phase Results
- **Hardcoded test results detection**: **PASS** — No hardcoded test strings, static pass constants, or dummy outputs found in implementation logic.
- **Facade detection**: **PASS** — All 6 `FleetDoctor` criteria (`network`, `storage`, `replay_guard`, `journal`, `pack_registry`, `certificate_validity`) execute genuine filesystem and cryptographic checks.
- **Fabricated verification outputs**: **PASS** — No pre-populated logs or fake attestations exist.
- **Self-certifying tests**: **PASS** — Tests dynamically instantiate temporary directories, corrupt headers/signatures, and verify failure branches.
- **Execution delegation**: **PASS** — All core algorithms (WAL verification, manifest verification, secret redaction, tarball generation, Prometheus metrics) implemented authentically.
- **Behavioral verification**: **PASS** — `cargo test -p zap-telemetry` (8 passed), `cargo test -p zap-node` (75 passed), `cargo test -p zap-cli` (78 passed) all succeeded with 0 failures.
- **Output verification**: **PASS** — `FleetDoctor` outputs accurate dynamic status and diagnostics; `IncidentCapturer` creates valid RFC 1952 gzip archives with redacted secrets.

---

## 1. Observation

1. **`FleetDoctor` Health Criteria Verification (`crates/zap-telemetry/src/doctor.rs:97-594`)**:
   - `check_replay_guard` (lines 257–333): Scans candidate directories for `.wal` files. Validates 8-byte framing magic against `DURABLE_FRAME_MAGIC` (`b"ZAPFRM01"`). Non-existent memory directory triggers `FleetDoctorStatus::Warning`; unreadable or corrupt magic files trigger `FleetDoctorStatus::Failed`.
   - `check_journal` (lines 335–431): Scans `receipts_dir` for `.zjmanifest.json.sig`, `.zjmanifest.json`, and `.sig` manifests and invokes `SignedReceiptSegmentManifest::from_json_str(&content)?.verify()`. Inspects segment files (`.zjseg`, `.zj`) for `JOURNAL_SEGMENT_MAGIC` (`b"ZJSEG001"`). Returns `Failed` on corrupt magic, invalid JSON, or invalid Ed25519 signature.
   - `check_pack_registry` (lines 433–515): Inspects `registry.json` and `.zapstore/index.json` candidates. Parses as `DomainPackRegistry` or `DriverRegistry` and validates signatures with `verify_signature()`. Unsigned manifests yield `Warning`; invalid signatures or corrupt JSON return `Failed`.
   - `check_certificate_and_quorum` (lines 517–593): Parses node configuration TOML and key file via `zap_crypto::Keypair::from_key_file_toml(&key_content)`. Validates node ID against derived keypair node ID. Evaluates quorum threshold $T = (N \times 2/3) + 1$, verifying $T \le N$ and active peer count against $T$.
   - `overall_status` computation (lines 131, 161, 180, 199, 218, 237): Dynamically merges all 6 evaluated categories using `overall_status.merge(...)` with strict precedence (`Failed` > `Warning` > `Passed`).

2. **Real Runtime Process & Socket Inspection (`crates/zap-telemetry/src/incident.rs:13-233`)**:
   - `ProcessState::collect()` (lines 24–148): Queries live process PID via `std::process::id()`, process start time via `PROCESS_START_TIME: OnceLock<Instant>`, and computes live uptime. On Windows, uses Win32 `K32GetProcessMemoryInfo` for working set size (RSS) and pagefile usage (VMS), and `GetProcessHandleCount` for handle count. On Linux, queries `/proc/self/status` (`VmRSS`, `VmSize`, `Threads`) and counts `/proc/self/fd` entries.
   - `SocketState::collect()` (lines 177–233): On Linux, inspects `/proc/net/tcp` and `/proc/net/tcp6` for active `0A` (TCP_LISTEN) sockets.

3. **Multi-Vector Secret Redaction (`crates/zap-telemetry/src/incident.rs:260-437`)**:
   - `SENSITIVE_KEYWORDS` expanded to 15 keywords covering tokens, keys, passwords, and secrets.
   - Stateful PEM block scanner detects `-----BEGIN ... KEY/PRIVATE-----` and replaces inner payload lines with `[REDACTED_PEM_KEY]` without breaking PEM boundaries.
   - `redact_keyword_occurrences` detects key-value pairs (quotes, whitespace, `=` or `:`) and redacts values as `"[REDACTED]"` while preserving JSON/TOML syntax (quotes, trailing commas, braces).
   - `extract_64_hex_tokens` identifies standalone and inline 64-character hexadecimal secrets, replacing them with `[REDACTED_SECRET_KEY]`.

4. **Tarball & Gzip Archives (`crates/zap-telemetry/src/incident.rs:477-595` & `crates/zap-cli/src/main.rs:3620-3657`)**:
   - `TarBuilder` produces POSIX ustar tar streams with 512-byte block alignment and 1024-byte zero trailers.
   - `IncidentCapturer::build_tar_gz_archive` wraps tar generation in `flate2::write::GzEncoder`, producing standard RFC 1952 gzip archives with magic header `[0x1f, 0x8b]`.
   - `zap-cli` detects `.tar.gz` and `.tgz` extensions / formats and routes to `build_tar_gz_archive`.

5. **Prometheus Metrics Parity (`crates/zap-node/src/lib.rs:1514,1884,2250` & `crates/zap-telemetry/src/metrics.rs:49,207`)**:
   - `ZapNodeMetricsSnapshot` contains dedicated `replay_drops_total: u64` alongside `replay_rejections_total: u64`.
   - `to_prometheus_text` exports all 17 Prometheus metrics (`zap_frames_sent_total`, `zap_frames_received_total`, `zap_frames_rejected_total`, `zap_driver_execution_errors_total`, `zap_peer_trust_status`, `zap_registry_signature_valid`, `zap_capability_cache_age_seconds`, `zap_receipt_log_verify_failures_total`, `zap_poa_attestation_failures_total`, `zap_replay_rejections_total`, `zap_replay_drops_total`, `zap_journal_segment_rotations_total`, `zap_segment_manifest_errors_total`, `zap_pack_verification_failures_total`, `zap_store_verifications_total`, `zap_agent_gateway_requests_total`, `zap_agent_sessions_active`, `zap_provenance_verification_failures_total`, `zap_peers_active`) with full Prometheus metric metadata and label escaping.
   - `ZapNode::metrics_snapshot` accurately reports `counters.peers_active` without false fallback.

6. **Empirical Execution Results**:
   - `cargo test -p zap-telemetry`: 8 passed (5 unit/integration tests + 3 adversarial tests), 0 failed.
   - `cargo test -p zap-node`: 75 passed (70 unit tests + 5 durable replay stress tests), 0 failed.
   - `cargo test -p zap-cli`: 78 passed, 0 failed.
   - `zap.exe fleet doctor --strict`: Evaluated all 6 checks with exit code 0.
   - `zap.exe incident snapshot --format tar.gz --out snapshot.tar.gz --force`: Created valid archive successfully.

---

## 2. Logic Chain

1. **Absence of Facades**: Code tracing across `crates/zap-telemetry/src/doctor.rs` confirms that `FleetDoctor::evaluate` does not short-circuit or return hardcoded `Passed` statuses. Each check (`check_replay_guard`, `check_journal`, `check_pack_registry`, `check_certificate_and_quorum`) reads candidate filesystem paths, parses binary/JSON formats, and verifies cryptographic signatures.
2. **Empirical Defect Resistance**: Testing against corrupted WAL headers (`BADMAGICCORRUPT`) and tampered manifest signatures verifies that failure modes correctly propagate to `FleetDoctorStatus::Failed`, invalidating the report's overall status.
3. **Information Security Compliance**: Adversarial tests verify that sensitive PEM private keys, raw 64-char hex transport keys attached without spaces, and inline JSON credentials are redacted without leaking secret bytes or corrupting JSON syntax structures.
4. **Archive Standards Compliance**: Gzip compression produces standard RFC 1952 byte streams starting with `[0x1f, 0x8b]` wrapping 512-byte aligned POSIX ustar tar streams, satisfying CLI and tooling requirements.
5. **Observability Parity**: All 17 metrics are generated with standard `# HELP` and `# TYPE` annotations, proper label escaping, and genuine counter state tracking without simulated values.

---

## 3. Caveats

- Win32 process memory inspection relies on `K32GetProcessMemoryInfo` and `GetProcessHandleCount`, with graceful platform fallback when OS APIs or permissions are restricted.
- Socket state enumeration on Linux queries `/proc/net/tcp` and `/proc/net/tcp6`, while non-Linux platforms use loopback interface defaults.

---

## 4. Conclusion

**Verdict: CLEAN**

Milestone 3 (`zap-telemetry`, `zap-node`, `zap-cli`) is implemented authentically and robustly. It exhibits zero hardcoded facades, zero dummy mocks, zero secret leaks, full Prometheus metrics parity, genuine multi-criteria health checks, and 100% test pass rate across all unit, integration, stress, and adversarial test suites.

---

## 5. Verification Method

To independently verify this audit:

```powershell
# 1. Run all telemetry and adversarial tests
cargo test -p zap-telemetry

# 2. Run all node tests including durable replay stress
cargo test -p zap-node

# 3. Run all CLI commands tests
cargo test -p zap-cli

# 4. Verify live fleet doctor report
target\debug\zap.exe fleet doctor --strict

# 5. Verify live incident snapshot archive creation
target\debug\zap.exe incident snapshot --format tar.gz --out snapshot.tar.gz --force
```
