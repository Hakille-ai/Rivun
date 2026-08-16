# Milestone 4 Adversarial Challenge & Empirical Verification Report

## Final Verdict: `REQUEST_CHANGES`

**Summary Assessment**:
- **Core Milestone 4 Functional Features (APPROVED)**:
  1. 6-stage `ProvenanceChainDigest` with causal cryptographic linking ($H_{\text{intent}} \to H_{\text{negotiation}} \to H_{\text{policy}} \to H_{\text{driver}} \to H_{\text{poa}} \to H_{\text{receipt}} \to H_{\text{root}}$) and Ed25519 signing: **VERIFIED & ROBUST**.
  2. Adversarial Tamper Detection across all 6 stages, broken causal links, missing `previous_hash`, out-of-order stages, root hash mutation, and signature bit corruption: **VERIFIED & ROBUST**.
  3. SSE Event Stream Broadcasting (`/v1/agent/stream`, `/v1/agent/events`), multi-line formatting, and high-fanout concurrency: **VERIFIED & ROBUST**.
  4. Full-Duplex WebSocket RFC 6455 bridge, handshake computation, ping/pong, and 4MB / configurable frame size limit enforcement (code 1009): **VERIFIED & ROBUST**.
  5. Multi-Transport E2E AI Agent Workflow (`tc_rw_001`, REST sessions, negotiation, delegation, MCP JSON-RPC 2.0 tool execution): **VERIFIED & ROBUST**.
  6. Unit & integration test suites in `zap-agent` and `zap-gateway`: **47 of 47 tests PASSED**.

- **Required Changes (BLOCKING ACCEPTANCE CRITERIA)**:
  1. **Clippy Errors in `crates/zap-agent/src/provenance.rs`**: 2 warnings break `cargo clippy --workspace --all-targets -- -D warnings`.
  2. **Compilation Failures in `tests/e2e/tests/e2e_suite.rs`**: Outdated test stubs in later tiers prevent `cargo test --package zap-e2e --test e2e` from compiling cleanly.

---

## 1. Observation

Direct observations and execution outputs from empirical test runs:

1. **`cargo test -p zap-agent -p zap-gateway --all-targets`**:
   - `zap-agent` unit tests (12 passed, 0 failed):
     - `provenance::tests::test_full_provenance_chain_generation_and_verification`: PASSED
     - `provenance::tests::test_tampered_step_fails_verification`: PASSED
     - `provenance::tests::test_tampered_signature_fails_verification`: PASSED
     - `tests::capability_negotiation_must_not_be_empty`: PASSED
     - `tests::accepted_delegation_requires_assignee`: PASSED
     - `tests::exports_agent_message_json_schema_metadata`: PASSED
     - `tests::agent_message_roundtrips_with_subject`: PASSED
     - `tests::deserializing_agent_id_rejects_unstable_identifier`: PASSED
     - `tests::result_requires_terminal_status_and_error_for_failure`: PASSED
     - `tests::validates_nested_error_depth`: PASSED
     - `tests::validates_session_timestamps`: PASSED
     - `tests::serializes_intent_json_stably`: PASSED
   - `zap-agent` fixtures (6 passed, 0 failed)
   - `zap-gateway` unit tests (1 passed: `transports::ws::tests::test_rfc6455_accept_calculation`)
   - `zap-gateway` adversarial challenger suite (`adversarial_challenger_m4_2.rs`, 10 passed, 0 failed):
     - `test_empirical_6_stage_provenance_causal_chain_integrity`: PASSED
     - `test_empirical_adversarial_tamper_matrix_all_6_stages`: PASSED
     - `test_empirical_out_of_order_and_missing_link_rejection`: PASSED
     - `test_empirical_sse_event_wire_formatting_and_multiline`: PASSED
     - `test_empirical_sse_broker_high_fanout_concurrency`: PASSED
     - `test_empirical_sse_http_streaming_over_wire`: PASSED
     - `test_empirical_ws_duplex_handshake_and_frames`: PASSED
     - `test_empirical_ws_frame_size_overflow_rejection`: PASSED
     - `test_empirical_http_cors_and_bearer_auth_and_routing`: PASSED
     - `test_empirical_full_e2e_ai_agent_workflow`: PASSED
   - `zap-gateway` adversarial stress tests (`adversarial_stress_tests.rs`, 9 passed, 0 failed)
   - `zap-gateway` integration tests (`gateway_tests.rs`, 9 passed, 0 failed)
   - **Total Passed**: 47 tests.

2. **`cargo clippy --workspace --all-targets -- -D warnings` Failure**:
   ```
   error: the borrowed expression implements the required traits
      --> crates\zap-agent\src\provenance.rs:331:28
       |
   331 |         data_hasher.update(&processed_at_micros.to_be_bytes());
       |                            ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ help: change this to: `processed_at_micros.to_be_bytes()`
       |
       = note: `-D clippy::needless-borrows-for-generic-args` implied by `-D warnings`

   error: this `if` statement can be collapsed
      --> crates\zap-agent\src\provenance.rs:579:17
       |
   579 | /                 if let Some(expected_prev) = &last_hash {
   580 | |                     if prev != expected_prev {
   581 | |                         return Ok(ProvenanceVerificationReport {
   ...   |
   594 | |                 }
       | |_________________^
       |
       = note: `-D clippy::collapsible-if` implied by `-D warnings`
   ```

3. **`cargo test --package zap-e2e --test e2e` Compilation Failure**:
   `tests/e2e/tests/e2e_suite.rs` contains 61 compilation errors due to unadapted tests in later tiers (`tc_x_001`, `tc_x_002`, `tc_x_003`, `tc_x_010`, `tc_x_015`, `tc_rw_004`, `tc_rw_006`, `tc_rw_009`).

---

## 2. Logic Chain

1. **Cryptographic Provenance Linking ($H_0 \to H_5$)**:
   - Verified that `ProvenanceChainBuilder` constructs an unambiguous 6-stage chain: Intent $\to$ Negotiation $\to$ Policy $\to$ Driver $\to$ PoA $\to$ Receipt.
   - Transition hashes bind the previous step's hash $H_{k-1}$ with the canonical data hash of stage $k$: $H_k = \text{SHA256}(H_{k-1} \parallel \text{input\_data\_hash})$.
   - The Merkle root $H_{\text{root}} = \text{SHA256}(\sum \text{stage}:H_k)$ is signed with Ed25519 using domain separator `ZAP-PROVENANCE-CHAIN-v1\0`.
   - Modifying any stage's data hash or link pointer breaks the subsequent step and triggers an explicit `ProvenanceVerificationReport` failure with exact `failed_stage`.

2. **SSE Streaming & Concurrency**:
   - `SseBroker` implements fanout broadcasting over `tokio::sync::broadcast`.
   - Verified wire serialization complies with the W3C SSE standard: `event: <type>\ndata: <line1>\ndata: <line2>\n\n`.
   - Multi-client subscription test proved 20+ concurrent receivers maintain order without dropping messages.

3. **WebSocket Bridge & Boundary Defense**:
   - Handshake performs standard SHA-1 + base64 digest calculation over `Sec-WebSocket-Key` + `258EAFA5-E914-47DA-95CA-C5AB0DC85B11`.
   - Inbound text and binary frames are parsed, and the 4MB / configured limit (`max_frame_size`) is strictly enforced; frames exceeding the limit trigger an immediate `WS_CLOSE_MESSAGE_TOO_BIG` (1009) frame and connection termination.

4. **Multi-Transport Full E2E Workflow (`tc_rw_001`)**:
   - Verified full end-to-end lifecycle: Session Creation $\to$ Capability Negotiation $\to$ Intent Submission $\to$ Policy Evaluation $\to$ Signed Journal Action Receipt $\to$ 6-Stage Provenance Chain Digest $\to$ SSE Broadcast $\to$ REST Verification $\to$ Delegation $\to$ MCP Tool Invocation.

5. **Acceptance Criteria Discrepancies**:
   - While the functional implementation of Milestone 4 is solid and bug-free, `cargo clippy --workspace --all-targets -- -D warnings` and `cargo test --package zap-e2e --test e2e` fail. In accordance with the project integrity rules, these must be resolved before final approval.

---

## 3. Caveats

- **Active Sliding-Window Rate Limiting**: `GatewayConfig` defines `rate_limit_per_second`, which is ready for rate limiting middleware, but in-memory IP token-bucket throttling is not currently hooked into `handle_connection`.
- **E2E Suite Scope**: The compilation errors in `tests/e2e/tests/e2e_suite.rs` stem from stub tests written for M5 (SDK conformance) and unadapted M2/M3 helpers.

---

## 4. Conclusion & Actionable Fixes

**Verdict**: `REQUEST_CHANGES`

### Required Fixes:
1. **Fix Clippy Warnings in `crates/zap-agent/src/provenance.rs`**:
   - Line 331: Change `data_hasher.update(&processed_at_micros.to_be_bytes());` to `data_hasher.update(processed_at_micros.to_be_bytes());`.
   - Line 579: Collapse nested `if` statements:
     ```rust
     if let Some(expected_prev) = &last_hash {
         if prev != expected_prev { ... }
     }
     ```
     into:
     ```rust
     if let Some(expected_prev) = &last_hash {
         if prev != expected_prev {
             return Ok(ProvenanceVerificationReport {
                 valid: false,
                 chain_id: self.chain_id,
                 root_hash: self.root_hash.clone(),
                 node_id: self.node_id,
                 verified_steps: verified_count,
                 failed_stage: Some(step.stage),
                 failure_reason: Some(format!(
                     "Causal break at stage {:?}: previous_hash {} != prior step_hash {}",
                     step.stage, prev, expected_prev
                 )),
             });
         }
     }
     ```
     (or `if Some(prev) != last_hash.as_ref()`).

2. **Clean up / Fix compilation in `tests/e2e/tests/e2e_suite.rs`**:
   - Align the outdated type signatures in `tier3_cross_feature` and `tier4_real_world_workflows` with current crate APIs or feature-gate them so `cargo test --package zap-e2e --test e2e` compiles cleanly.

---

## 5. Verification Method

To verify these fixes:

```bash
# 1. Run zap-agent and zap-gateway tests (including adversarial challenger suite)
cargo test -p zap-agent -p zap-gateway --all-targets

# 2. Run clippy with strict warnings
cargo clippy --workspace --all-targets -- -D warnings

# 3. Run E2E test suite
cargo test --package zap-e2e --test e2e
```
