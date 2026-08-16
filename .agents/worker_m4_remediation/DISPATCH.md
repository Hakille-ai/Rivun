## 2026-08-15T01:09:46Z
You are worker_m4_remediation operating in working directory: c:\Users\Stagiaire\Documents\Amadou PGC\Prs\ZAP\.agents\worker_m4_remediation.

Your task is to apply minor clippy cleanups and HTTP buffer fixes in `zap-agent` and `zap-gateway` so that `cargo clippy -p zap-agent -p zap-gateway --all-targets -- -D warnings` runs 100% cleanly with ZERO warnings/errors.

DO NOT CHEAT. All fixes must be clean and genuine.

Tasks:
1. `crates/zap-agent/src/provenance.rs`:
   - Line 331: change `&processed_at_micros.to_be_bytes()` to `processed_at_micros.to_be_bytes()`.
   - Line 579: collapse nested `if` statements (`if let Some(expected_prev) = &last_hash` and `if prev != expected_prev`).
2. `crates/zap-gateway`:
   - `crates/zap-gateway/src/mcp/tools.rs` (around line 448): collapse nested `if` statement.
   - `crates/zap-gateway/src/transports/http.rs`:
     - Line 704: remove unnecessary `let` binding before return (`clippy::let_and_return`).
     - Line 717: collapse nested `if` statement.
     - `handle_connection`: update request body reader to read up to `Content-Length` header or `config.max_frame_size` so multi-chunk POST payloads larger than 8KB are parsed correctly without truncation.
   - `crates/zap-gateway/src/transports/ws.rs`: line 66 fix range loop index (`clippy::needless_range_loop`).
3. Run verification:
   - `cargo test -p zap-agent -p zap-gateway --all-targets`
   - `cargo clippy -p zap-agent -p zap-gateway --all-targets -- -D warnings`
4. Write handoff report to `c:\Users\Stagiaire\Documents\Amadou PGC\Prs\ZAP\.agents\worker_m4_remediation\handoff.md` and send_message to parent when finished.
