# Handoff Report: Explorer M3 Remediation — Fix Roadmap for Fleet Telemetry & Doctor

## 1. Observation

Direct code inspection of `crates/zap-telemetry`, `crates/zap-cli`, `crates/zap-node`, and existing unit/adversarial tests (`crates/zap-telemetry/tests/adversarial_m3_tests.rs`) revealed four distinct implementation defects:

1. **Hardcoded Facade in `FleetDoctor` Health Criteria (`crates/zap-telemetry/src/doctor.rs:156-199`)**:
   - `FleetDoctor::evaluate` hardcodes checks 3, 4, 5, and 6 to `FleetDoctorStatus::Passed`:
     - Line 157-165 (`replay_guard`): Returns `Passed` unconditionally without inspecting WAL file magic (`b"ZAPFRM01"`), directory state, or clock skew parameters.
     - Line 168-176 (`journal`): Returns `Passed` unconditionally without verifying `SignedReceiptSegmentManifest` signatures or journal segment sequence continuity.
     - Line 179-187 (`pack_registry`): Returns `Passed` unconditionally without inspecting `registry.json` index files or cryptographic signatures (`registry_signature_valid`).
     - Line 190-198 (`certificate_validity`): Returns `Passed` unconditionally without validating node Ed25519 identity keypairs or verifying PACT validator quorum ($T \le N$).
   - `overall_status` is never updated via `.merge()` for any of these four checks.

2. **Dummy Mock Values in `IncidentCapturer` (`crates/zap-telemetry/src/incident.rs:146-147`)**:
   - `IncidentCapturer::capture` assigns `process: ProcessState::default()` and `sockets: SocketState::default()`.
   - `ProcessState::default()` returns static mock constants: RSS 16 MB, VMS 64 MB, CPU 0.5%, 4 threads, 12 FDs, 120s uptime.
   - `SocketState::default()` returns static mock ports (`[9090, 8080]`).
   - No runtime OS process inspection (memory, CPU, threads, open descriptors, runtime) or socket enumeration is executed.

3. **Secret Redactor Bypasses, Leaks & JSON Corruption (`crates/zap-telemetry/src/incident.rs:65-114`)**:
   - `keywords` list is limited to 7 terms (`private_key`, `node_private_key`, `secret_key`, `auth_token`, `bearer`, `password`, `ed25519_private_key`), missing `transport_key`, `pact_private_key`, `api_key`, `access_token`, `client_secret`, `bearer_token`, `secret`, `token`.
   - `regex_simple_hex64` splits lines by whitespace (`input.split_whitespace()`). A key formatted without surrounding spaces (`transport_key=0123456789abcdef...`) yields a string length of 77 (`!= 64`), leaking raw 64-char hex transport keys.
   - Lines containing PEM private key headers (`-----BEGIN ... PRIVATE KEY-----`) do not contain `=` or `:`, causing base64 private key payload lines to leak unredacted.
   - Splitting inline JSON lines (e.g. `{"secret_key": "val", "node_id": "n1"}`) at `:` truncates everything after `[REDACTED]`, destroying trailing JSON syntax.

4. **Uncompressed POSIX Tar Bytes in `.tar.gz` Files (`crates/zap-cli/src/main.rs:3634-3650` & `crates/zap-telemetry/src/incident.rs:186-259`)**:
   - `TarBuilder` generates raw POSIX ustar tar bytes without gzip compression.
   - `zap-cli` writes raw uncompressed tar bytes directly to files specified with `.tar.gz` or `.tgz` extensions.
   - Standard archive utilities (`tar -xzf`, `gzip -d`) fail with `not in gzip format`.

5. **Prometheus Metrics Anomalies (`crates/zap-node/src/lib.rs:1822-1826` & `crates/zap-telemetry/src/metrics.rs:207`)**:
   - In `ZapNode::metrics_snapshot`, `peers_active` falls back to `self.peers.len()` when active peer count is 0, incorrectly reporting offline configured peers as active.
   - `zap_replay_drops_total` duplicates `self.replay_rejections_total` instead of emitting a dedicated drop counter.

---

## 2. Logic Chain

1. **Requirement Integrity**:
   - Requirement R3 and Gate Criteria require strict production `zap fleet doctor` health verification and live `zap incident snapshot` diagnostic collection.
   - Hardcoding doctor criteria to `Passed` without evaluation constitutes a facade implementation.
   - Serving static mock process RAM/CPU and listening socket constants without querying live system state constitutes a dummy implementation.

2. **Security & Diagnostics**:
   - Failing to redact `transport_key=...`, PEM private keys, and API tokens risks leaking live cluster secret material in incident archives.
   - Truncating inline JSON objects corrupts diagnostic config structures.

3. **Archive Interoperability**:
   - Incident snapshot files with `.tar.gz` extensions must conform to RFC 1952 gzip compression wrapping POSIX ustar tar streams (`flate2::write::GzEncoder`).

4. **Telemetry Parity**:
   - Reporting active peer count as total configured peers when zero peers are connected corrupts cluster observability.

---

## 3. Caveats

- OS process inspection may require fallback to `ProcessState::default()` and `SocketState::default()` when system permissions or platform APIs (e.g. sandboxed containers or restricted environments) prevent reading process memory/FDs.
- `TarBuilder` uncompressed tar generation remains valid for `.tar` output files; `flate2::write::GzEncoder` should be applied when format is `tar.gz`/`tgz` or filename ends in `.tar.gz`/`.tgz`.

---

## 4. Conclusion & Actionable Fix Roadmap for `worker_m3_fix`

### Step 1: Real `FleetDoctor` Health Checks (`crates/zap-telemetry/src/doctor.rs`)
1. In `FleetDoctor::evaluate`, replace hardcoded `FleetDoctorStatus::Passed` responses with genuine checks for all 4 missing categories:
   - **`replay_guard` check**: Read `memory_dir` / `receipts_dir` or working path for WAL files (`.wal`). Verify `DURABLE_FRAME_MAGIC` (`b"ZAPFRM01"`) and file integrity. If directory missing or file corrupted, set status to `Warning` / `Failed`. Merge status with `overall_status.merge(...)`.
   - **`journal` check**: Search `receipts_dir` for `.zj` segment files and `.zjmanifest.json` / `.sig` signed manifests. Parse JSON and verify Ed25519 signature validity. If directory is missing or signatures fail, set status to `Warning` / `Failed`. Merge status with `overall_status.merge(...)`.
   - **`pack_registry` check**: Look in `config_path.parent()` / working directory for ZapStore index files (`registry.json`, `.zapstore/index.json`). Check signature validity (`registry_signature_valid == 1`). If missing or signature invalid, set status to `Warning` / `Failed`. Merge status with `overall_status.merge(...)`.
   - **`certificate_validity` check**: Validate Ed25519 node keypair in `config_path` / identity file. If `topology` is provided, verify active node count $N \ge 1$ and validator set quorum threshold $T \le N$. If threshold $T > N$ or key is invalid, set status to `Failed`. Merge status with `overall_status.merge(...)`.

### Step 2: Real System Process & Socket Collection (`crates/zap-telemetry/src/incident.rs`)
1. Implement `ProcessState::collect() -> ProcessState`:
   - Inspect PID (`std::process::id()`), memory (RSS/VMS), CPU, thread count, open file descriptors, and uptime via platform system APIs or procfs (`/proc/self/status`, `/proc/self/fd`, `/proc/self/stat`) or `sysinfo`.
   - Fall back gracefully to `ProcessState::default()` if platform inspection fails.
2. Implement `SocketState::collect() -> SocketState`:
   - Inspect listening TCP/UDP ports and active sockets (or parse `/proc/net/tcp`, `/proc/net/udp` / OS socket state).
   - Fall back gracefully to `SocketState::default()` if inspection fails.
3. Update `IncidentCapturer::capture()` to call `ProcessState::collect()` and `SocketState::collect()`.

### Step 3: Comprehensive `SecretRedactor` (`crates/zap-telemetry/src/incident.rs`)
1. Expand keyword list to include:
   `["private_key", "node_private_key", "secret_key", "auth_token", "bearer", "password", "ed25519_private_key", "transport_key", "pact_private_key", "api_key", "access_token", "client_secret", "bearer_token", "secret", "token"]`.
2. Add PEM Block Redaction:
   - Stateful parsing for lines between `-----BEGIN ... PRIVATE KEY-----` (or `-----BEGIN ... KEY-----`) and `-----END ... PRIVATE KEY-----`. Replace body lines with `[REDACTED_PEM_KEY]`.
3. Fix Key-Value Redaction for Inline JSON & Delimiters:
   - For JSON strings or multi-pair lines, use targeted regex replacement (`"keyword"\s*:\s*"[^"]*"`) to replace sensitive values with `"[REDACTED]"`, preserving trailing commas, braces `}`, and adjacent key-value pairs.
4. Regex Hex64 Matching (`key=hex64`):
   - Match any 64-char hex string (`\b[0-9a-fA-F]{64}\b`, excluding all-zero strings) regardless of preceding `=` or `:` without whitespace, and replace with `[REDACTED_SECRET_KEY]`.

### Step 4: Gzip Tarball Archive Compression (`crates/zap-telemetry` & `crates/zap-cli`)
1. Add `flate2 = "1.0"` to workspace dependencies in `Cargo.toml` and `crates/zap-telemetry/Cargo.toml` & `crates/zap-cli/Cargo.toml`.
2. In `crates/zap-telemetry/src/incident.rs`, implement:
   ```rust
   pub fn build_tar_gz_archive(snapshot: &IncidentSnapshot) -> Result<Vec<u8>> {
       let tar_bytes = Self::build_tar_archive(snapshot)?;
       let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
       std::io::Write::write_all(&mut encoder, &tar_bytes)?;
       Ok(encoder.finish()?)
   }
   ```
3. In `crates/zap-cli/src/main.rs`:
   - Detect gzip output format (`format == "tar.gz"` || `format == "tgz"` || filename ends with `.tar.gz` || `.tgz`).
   - Call `IncidentCapturer::build_tar_gz_archive(&live_snapshot)` for gzip targets, reserving `build_tar_archive` for uncompressed `.tar` targets.

### Step 5: Metrics Parity & Drops Counter Cleanup
1. In `crates/zap-node/src/lib.rs:1822`, remove `if counters.peers_active > 0 { ... } else { self.peers.len() as u64 }` fallback so `peers_active` accurately reflects connected peers.
2. In `crates/zap-telemetry/src/metrics.rs`, add `pub replay_drops_total: u64` to `ZapNodeMetricsSnapshot` and use `self.replay_drops_total` for `zap_replay_drops_total`.

---

## 5. Verification Method

Independent verification steps:

1. **Verify `FleetDoctor` Checks**:
   - Run `cargo test -p zap-telemetry --test telemetry_tests test_fleet_doctor_evaluation_6_criteria`.
   - Verify that corrupted WAL files, missing manifests, invalid ZapStore index signatures, or invalid node keypairs cause `FleetDoctor::evaluate` to return `Warning` or `Failed`.

2. **Verify Adversarial M3 Tests**:
   - Run `cargo test -p zap-telemetry --test adversarial_m3_tests`.
   - All tests (`test_adversarial_secret_redactor_leaks`, `test_adversarial_tar_builder_unpacking_and_gzip`, `test_adversarial_process_and_socket_state_hardcoding`) must PASS.

3. **Verify Gzip Decompression**:
   - Run `cargo run -p zap-cli -- incident snapshot --out snapshot.tar.gz --force`.
   - Verify file signature starts with `0x1f 0x8b` and can be extracted via standard archive tools or `tar -xzf snapshot.tar.gz`.

4. **Workspace Conformance**:
   - Run `cargo test --workspace --all-targets` and `cargo clippy --workspace --all-targets -- -D warnings`.
