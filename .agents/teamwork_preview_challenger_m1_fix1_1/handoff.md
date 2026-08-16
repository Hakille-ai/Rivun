# Milestone 1 Durable Replay Remediation — Challenger Empirical Stress Test Report

## 1. Observation

Empirical stress testing of Milestone 1 durable replay remediation was conducted by running targeted stress test suites and workspace test targets across `zap-net`, `zap-node`, `zap-journal`, and `zap-ledger`.

### Test Execution Commands & Results

1. **`cargo test --test durable_replay_stress -p zap-net -p zap-node`**:
   - `zap-net`: 5/5 stress tests passed:
     - `stress_test_nonce_store_clock_jumps` ... ok
     - `stress_test_nonce_store_partial_write_corruption` ... ok
     - `stress_test_nonce_store_compaction_under_load` ... ok
     - `stress_test_nonce_store_concurrent_access` ... ok
     - `stress_test_nonce_store_crash_restart_replay_flood` ... ok
   - `zap-node`: 5/5 stress tests passed:
     - `stress_test_replay_store_clock_jumps_and_overflow` ... ok
     - `stress_test_replay_store_compaction_under_load` ... ok
     - `stress_test_replay_store_partial_write_corruption` ... ok
     - `stress_test_replay_store_concurrent_access` ... ok
     - `stress_test_replay_store_crash_restart_replay_flood` ... ok

2. **Full M1 Crates Test Suite (`cargo test -p zap-net -p zap-node -p zap-journal -p zap-ledger`)**:
   - Total tests executed: 65
   - Total passed: 65
   - Total failed: 0

3. **M1 Clippy Verification (`cargo clippy -p zap-net -p zap-node -p zap-journal -p zap-ledger --all-targets -- -D warnings`)**:
   - Status: Clean build, 0 warnings, 0 errors.

---

## 2. Logic Chain

1. **Partial Write Corruption Recovery**:
   - *Observation*: `DurableNonceStore::open()` (`crates/zap-net/src/durable_replay.rs:58-63`) and `DurableReplayStore::open()` (`crates/zap-node/src/durable_replay.rs:52-57`) calculate `valid_len = 8 + (valid_records * RECORD_LEN)` and invoke `file.set_len(valid_len)` when trailing unaligned bytes exist.
   - *Verification*: Tests `stress_test_nonce_store_partial_write_corruption` and `stress_test_replay_store_partial_write_corruption` append partial byte sequences to WAL files and verify that valid prior records are preserved, corrupt trailing bytes are truncated, and subsequent appends function properly across process restarts.

2. **Clock Jump Overflow Prevention**:
   - *Observation*: `DurableReplayStore::check_and_insert()` (`crates/zap-node/src/durable_replay.rs:88-89`) uses saturating addition (`ts.saturating_add(self.max_clock_skew_micros) < now_micros || ts > now_micros.saturating_add(self.max_clock_skew_micros)`).
   - *Verification*: Test `stress_test_replay_store_clock_jumps_and_overflow` submits extreme timestamp frames (`u64::MAX`) and verifies no integer overflow panic occurs. `stress_test_nonce_store_clock_jumps` verifies backward clock jumps are handled cleanly.

3. **Peer WAL Isolation**:
   - *Observation*: `ZapEndpoint::add_peer()` (`crates/zap-net/src/lib.rs`) constructs distinct per-peer WAL file paths using `{stem}.{peer.node_id}.{ext}` when `durable_nonce_store_path` is specified.
   - *Verification*: `test_endpoint_peer_wal_isolation` confirms that different peers do not overwrite or collide on each other's WAL files.

---

## 3. Caveats

No caveats. All stress test scenarios were executed directly against the compiled debug targets and passed with 100% success rate without flaky behavior or regressions.

---

## 4. Conclusion

**Verdict: APPROVE**

Milestone 1 Durable Replay Remediation passes 100% of empirical stress tests:
- Partial write corruption recovery: 100% PASS
- Clock jump overflow prevention: 100% PASS
- Peer WAL isolation: 100% PASS
- Workspace crates `zap-net`, `zap-node`, `zap-journal`, `zap-ledger`: 65/65 tests PASS, 0 clippy warnings.

---

## 5. Verification Method

To independently reproduce and verify this stress test result:

1. Run durable replay stress tests:
   ```powershell
   cargo test --test durable_replay_stress -p zap-net -p zap-node
   ```
   *Expected result*: 10/10 stress tests pass.

2. Run full M1 test suite:
   ```powershell
   cargo test -p zap-net -p zap-node -p zap-journal -p zap-ledger
   ```
   *Expected result*: 65/65 unit and stress tests pass.

3. Run clippy check:
   ```powershell
   cargo clippy -p zap-net -p zap-node -p zap-journal -p zap-ledger --all-targets -- -D warnings
   ```
   *Expected result*: 0 warnings/errors.
