# Milestone 3 Review & Adversarial Challenge Report

## 1. Observation

Direct inspection of code, tests, and build artifacts across `crates/zap-telemetry`, `crates/zap-node`, and `crates/zap-cli` confirms:

1. **FleetDoctor Verification Logic (`crates/zap-telemetry/src/doctor.rs:95-255`)**:
   - `check_replay_guard` (lines 257-333): Genuine inspection of candidate directories (`memory_dir`, `receipts_dir`, `config_path.parent()`) for `.wal` files. Validates framing magic against `DURABLE_FRAME_MAGIC` (`b"ZAPFRM01"`). Non-existent memory directory triggers `FleetDoctorStatus::Warning`; unreadable or corrupt magic files trigger `FleetDoctorStatus::Failed`.
   - `check_journal` (lines 335-431): Scans `receipts_dir` for `.zjmanifest.json.sig`, `.zjmanifest.json`, and `.sig` manifests and invokes `SignedReceiptSegmentManifest::from_json_str(&content)?.verify()`. Inspects segment files (`.zjseg`, `.zj`) for `JOURNAL_SEGMENT_MAGIC` (`b"ZJSEG001"`). Returns `Failed` on corrupt magic, invalid JSON, or invalid Ed25519 signature.
   - `check_pack_registry` (lines 433-515): Inspects `registry.json` and `.zapstore/index.json` candidates. Parses as `DomainPackRegistry` or `DriverRegistry` and validates signatures with `verify_signature()`. Missing signatures produce `Warning`; invalid signatures or corrupted JSON produce `Failed`.
   - `check_certificate_and_quorum` (lines 517-593): Parses node configuration TOML and key file via `zap_crypto::Keypair::from_key_file_toml(&key_content)`. Validates node ID against derived keypair node ID. Evaluates quorum threshold $T = (N \times 2/3) + 1$, verifying $T \le N$ and active peer count.
   - `overall_status` computation (lines 131, 161, 180, 199, 218, 237): Computes aggregate status using `overall_status.merge(...)` across all 6 evaluated categories.

2. **Real Runtime Process & Socket Inspection (`crates/zap-telemetry/src/incident.rs:13-233`)**:
   - `ProcessState::collect()` (lines 24-148): Queries live process PID via `std::process::id()`, process start time via `PROCESS_START_TIME: OnceLock<Instant>`, and computes live uptime. On Windows, uses Win32 `K32GetProcessMemoryInfo` for working set size (RSS) and pagefile usage (VMS), and `GetProcessHandleCount` for handle count. On Linux, queries `/proc/self/status` (`VmRSS`, `VmSize`, `Threads`) and counts `/proc/self/fd` entries.
   - `SocketState::collect()` (lines 177-233): On Linux, inspects `/proc/net/tcp` and `/proc/net/tcp6` for active `0A` (TCP_LISTEN) sockets.

3. **Multi-Vector Secret Redaction (`crates/zap-telemetry/src/incident.rs:260-437`)**:
   - `SENSITIVE_KEYWORDS` expanded to 15 keywords covering tokens, keys, passwords, and secrets.
   - Stateful PEM block scanner detects `-----BEGIN ... KEY/PRIVATE-----` and replaces inner payload lines with `[REDACTED_PEM_KEY]` without breaking PEM boundaries.
   - `redact_keyword_occurrences` detects key-value pairs (quotes, whitespace, `=` or `:`) and redacts values as `"[REDACTED]"` while preserving JSON/TOML syntax (quotes, trailing commas, braces).
   - `extract_64_hex_tokens` identifies standalone and inline 64-character hexadecimal secrets, replacing them with `[REDACTED_SECRET_KEY]`.

4. **Tarball & Gzip Archives (`crates/zap-telemetry/src/incident.rs:477-595` & `crates/zap-cli/src/main.rs:3620-3657`)**:
   - `TarBuilder` produces POSIX ustar tar streams with 512-byte block alignment and 1024-byte zero trailers.
   - `IncidentCapturer::build_tar_gz_archive` wraps tar generation in `flate2::write::GzEncoder`, producing standard RFC 1952 gzip archives with magic header `[0x1f, 0x8b]`.
   - `zap-cli` detects `.tar.gz` and `.tgz` extensions / formats and routes to `build_tar_gz_archive`.

5. **Prometheus Metrics Parity & Replay Drops Counter (`crates/zap-node/src/lib.rs:1514,1884,2250` & `crates/zap-telemetry/src/metrics.rs:49,207`)**:
   - `ZapNodeMetricsSnapshot` contains dedicated `replay_drops_total: u64` alongside `replay_rejections_total: u64`.
   - `to_prometheus_text` exports `zap_replay_drops_total` with full Prometheus metric metadata.
   - `ZapNode::metrics_snapshot` accurately reports `counters.peers_active` without dummy fallback.

6. **Automated Test Results**:
   - `cargo test -p zap-telemetry`: 8 passed (5 unit/integration tests + 3 adversarial tests), 0 failed.
   - `cargo test -p zap-node`: 75 passed (70 unit tests + 5 durable replay stress tests), 0 failed.
   - `cargo test -p zap-cli`: 78 passed, 0 failed.
   - `cargo clippy --workspace --all-targets --exclude zap-e2e -- -D warnings`: 0 warnings.

---

## 2. Logic Chain

- **Step 1 (Integrity Verification)**:
  - Scanned for hardcoded `FleetDoctorStatus::Passed` or mocked values. All 6 doctor criteria now execute domain checks against filesystem paths, WAL headers, signed manifests, registry structures, and keypair cryptographic validity.
  - Verified that corrupted inputs (e.g. invalid WAL magic `BADMAGICCORRUPT` or corrupted manifest JSON) directly trigger `FleetDoctorStatus::Failed` and `has_failures() == true`.
- **Step 2 (Diagnostic Redaction & Safety)**:
  - Tested adversarial inputs including PEM private keys, inline JSON secrets (`{"secret_key": "...", "node_id": "..."}`), and attached transport keys (`transport_key=hex64`). All secret payloads are redacted while preserving JSON syntax structure.
- **Step 3 (Archive Correctness)**:
  - Verified that `IncidentCapturer::build_tar_gz_archive` produces valid gzip bytes starting with `0x1f 0x8b`, decompressible with `flate2::read::GzDecoder`, yielding 512-byte aligned tar bytes with snapshot files (`snapshot.json`, `metrics.prom`, `diagnostics.txt`, `config.redacted.toml`, `health.json`).
- **Step 4 (Metrics Parity)**:
  - Verified that all 17 Prometheus metrics in `ZapNodeMetricsSnapshot::to_prometheus_text` match naming, type, and labeling requirements.
- **Step 5 (Code Cleanliness)**:
  - Clippy and test suites across `zap-telemetry`, `zap-node`, and `zap-cli` executed with zero errors and zero warnings.

---

## 3. Caveats

- On non-Linux and non-Windows platforms, `ProcessState::collect()` and `SocketState::collect()` fallback gracefully to safe default values when OS-specific procfs or Win32 APIs are unavailable.

---

## 4. Conclusion

**Verdict**: `APPROVE`

Milestone 3 telemetry, diagnostics, doctor checks, and node metrics parity meet all architectural requirements, pass all unit, integration, stress, and adversarial test suites, adhere to project conventions, and exhibit zero integrity violations.

---

## 5. Verification Method

To independently verify this review:

1. **Verify Fleet Telemetry & Adversarial Tests**:
   ```powershell
   cargo test -p zap-telemetry
   ```
2. **Verify Node Unit & Durable Replay Stress Tests**:
   ```powershell
   cargo test -p zap-node
   ```
3. **Verify CLI Commands & Doctor Checks**:
   ```powershell
   cargo test -p zap-cli
   ```
4. **Verify Workspace Clippy Cleanliness**:
   ```powershell
   cargo clippy --workspace --all-targets --exclude zap-e2e -- -D warnings
   ```
