## 2026-08-15T20:23:46Z
You are the Remediation and Integration Worker for the ZAP Next-Gen Frontier project.

Working directory: c:\Users\Stagiaire\Documents\Amadou PGC\Prs\ZAP\.agents\worker_remediation
Original Request: c:\Users\Stagiaire\Documents\Amadou PGC\Prs\ZAP\.agents\ORIGINAL_REQUEST.md
Project Definition: c:\Users\Stagiaire\Documents\Amadou PGC\Prs\ZAP\PROJECT.md
Audit Report: c:\Users\Stagiaire\Documents\Amadou PGC\Prs\ZAP\.agents\explorer_status_audit\handoff.md
Detailed Analysis: c:\Users\Stagiaire\Documents\Amadou PGC\Prs\ZAP\.agents\explorer_status_audit\analysis.md
Project Root: c:\Users\Stagiaire\Documents\Amadou PGC\Prs\ZAP

MANDATORY INTEGRITY WARNING:
DO NOT CHEAT. All implementations must be genuine. DO NOT hardcode test results, create dummy/facade implementations, or circumvent the intended task. A teamwork_preview_auditor will independently verify your work. Integrity violations WILL be detected and your work WILL be rejected.

Your Mission:
Apply the targeted fixes identified in the status audit to achieve 100% build, test, and clippy clean pass across the workspace and SDKs:

1. `crates/zap-net`:
   - Fix Serde derive / byte array serialization for `[u8; 64]` and `Bytes` (use `#[serde(with = "serde_bytes")]` or custom serializer / `hex` string / byte array wrapper).
   - Fix syntax typo `HashMap::new>` at `consensus/engine.rs:44:36`.
   - Fix format string positional argument indexing in `consensus/mod_types.rs:40:13`.
   - Fix clippy unused imports / ambiguous glob re-exports in `lib.rs:30`.

2. `crates/zap-driver-sdk`:
   - Add `hex.workspace = true` (or `hex = "0.4"`) to `crates/zap-driver-sdk/Cargo.toml`.
   - Fix `IpcMessage` usage/constructor in `crates/zap-driver-sdk/src/async_driver.rs` (lines 314, 322).
   - Fix `clippy::needless_lifetimes` on `translate_slice` and `translate_slice_mut` in `buffer.rs:311, 322`.

3. `crates/zap-ledger`:
   - Fix signature signing / verification in `batch.rs:448` (`batch_seal_quorum_verification` test: ensure keypair used for signing matches the validator set public key).
   - Remove unused import `ActionReceipt` in `batch.rs:14` and unused `mut mmr` in `mmr.rs:663:13`.

4. `sdks/rust`:
   - Update `sdks/rust/src/lib.rs` (lines 234, 246, 257, 268) to map actual `ZapEnvelopeError` variants (e.g. `InvalidMagic`, `PayloadTooLarge`, `CorruptedPayload`, etc.) instead of non-existent `InvalidHeader`.

5. Verification Commands to Execute:
   - `cargo test --workspace --all-targets`
   - `cargo clippy --workspace --all-targets -- -D warnings`
   - `cargo test -p zap-e2e --test e2e`
   - Multi-language SDK tests (Rust, Go, Python, TypeScript)

6. Deliverables:
   - Write comprehensive report to `c:\Users\Stagiaire\Documents\Amadou PGC\Prs\ZAP\.agents\worker_remediation\changes.md` and 5-component `handoff.md`.
   - Send completion message to parent.
