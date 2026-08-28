## 2026-08-15T01:09:46Z
You are worker_m4_remediation operating in working directory: c:\Users\Stagiaire\Documents\Amadou PGC\Prs\rivun\.agents\worker_m4_remediation.

Your task is to apply minor clippy cleanups and HTTP buffer fixes in `rivun-agent` and `rivun-gateway` so that `cargo clippy -p rivun-agent -p rivun-gateway --all-targets -- -D warnings` runs 100% cleanly with ZERO warnings/errors.

DO NOT CHEAT. All fixes must be clean and genuine.

Tasks:
1. `crates/rivun-agent/src/provenance.rs`:
   - Line 331: change `&processed_at_micros.to_be_bytes()` to `processed_at_micros.to_be_bytes()`.
   - Line 579: collapse nested `if` statements (`if let Some(expected_prev) = &last_hash` and `if prev != expected_prev`).
2. `crates/rivun-gateway`:
   - `crates/rivun-gateway/src/mcp/tools.rs` (around line 448): collapse nested `if` statement.
   - `crates/rivun-gateway/src/transports/http.rs`:
     - Line 704: remove unnecessary `let` binding before return (`clippy::let_and_return`).
     - Line 717: collapse nested `if` statement.
     - `handle_connection`: update request body reader to read up to `Content-Length` header or `config.max_frame_size` so multi-chunk POST payloads larger than 8KB are parsed correctly without truncation.
   - `crates/rivun-gateway/src/transports/ws.rs`: line 66 fix range loop index (`clippy::needless_range_loop`).
3. Run verification:
   - `cargo test -p rivun-agent -p rivun-gateway --all-targets`
   - `cargo clippy -p rivun-agent -p rivun-gateway --all-targets -- -D warnings`
4. Write handoff report to `c:\Users\Stagiaire\Documents\Amadou PGC\Prs\rivun\.agents\worker_m4_remediation\handoff.md` and send_message to parent when finished.

