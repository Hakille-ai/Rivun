# Milestone 4 Exploration Report: AI Agent Gateway & MCP Server

## 1. Observation

### 1.1 `crates/zap-agent` Existing State
- **File**: `crates/zap-agent/src/lib.rs` (1190 lines).
  - Implements JSON message data structures and validation: `AgentId`, `IntentKind`, `Priority`, `AgentStatus`, `IntentConstraint`, `ContextReference`, `AgentIntent`, `AgentSession`, `DelegationRequest`, `DelegationDecision`, `DelegationResponse`, `CapabilityNegotiationRequest`, `CapabilityNegotiationDecision`, `CapabilityNegotiationResponse`, `AgentStatusUpdate`, `AgentArtifact`, `AgentErrorCategory`, `AgentErrorInfo`, `AgentResult`, `AgentErrorReport`, and `AgentMessage`.
  - Implements JSON schema export (`agent_message_json_schema()`), subject mappings (`agent_message_subjects()`), and `Validate` traits.
  - Tests in `zap-agent`: All 9 unit tests and 6 fixture tests pass cleanly (`cargo test --package zap-agent` exits 0).
  - **Absence**: Contains **no runtime daemon, no MCP server, no transport handlers (HTTP/SSE/WebSocket), and no cryptographic provenance engine**.

### 1.2 `crates/zap-gateway` Non-Existence
- **Workspace Manifest**: `Cargo.toml` lines 1-28 does NOT contain `"crates/zap-gateway"`.
- **Filesystem**: Search for `c:\Users\Stagiaire\Documents\Amadou PGC\Prs\ZAP\crates\zap-gateway` failed with error: `search directory ... does not exist`.
- `zap-gateway` must be newly created to fulfill R4 requirements as specified in `PROJECT.md` line 60 (`crates/zap-agent/ & crates/zap-gateway/: Agent protocol, MCP server, transport bridge`).

### 1.3 Missing Provenance Fields in `crates/zap-ledger`
- **File**: `crates/zap-ledger/src/lib.rs` lines 155-176 (`pub struct ActionReceipt`).
  - Contains fields: `schema_version`, `node_id`, `source_node`, `target_node`, `kind`, `subject`, `action`, `frame_hash`, `payload_hash`, `output_hash`, `frame_timestamp_micros`, `processed_at_micros`, `flags`, `consensus_required`, `poa`, `pact`.
  - **Missing**: Provenance linking fields (`session_id`, `intent_id`, `negotiation_id`, `policy_decision`, `provenance_chain_digest`).

### 1.4 Missing CLI Subcommands in `crates/zap-cli`
- **File**: `crates/zap-cli/src/main.rs` lines 91-281 (`enum Commands`).
  - Currently defines: `Keygen`, `Run`, `CheckConfig`, `Doctor`, `Send`, `Inspect`, `Capability`, `Discovery`, `Memory`, `Route`, `Trust`, `Peer`, `Schema`, `Agent`, `Pact`, `Policy`, `Pack`, `Fixtures`, `DriverManifest`, `Registry`, `Receipts`, `Incident`, `Fleet`, `Poa`, `Bench`.
  - **Missing**: `Gateway` subcommand (`zap gateway start`, `zap gateway status`) and `Provenance` verification command (`zap provenance verify` or `zap receipts verify --provenance`).

### 1.5 Facades and Compilation Errors in `tests/e2e`
- **Compilation Failure**: `cargo test --workspace` fails in `tests/e2e/tests/e2e_suite.rs` with 71 errors due to type signature mismatches (e.g. `AgentIntent` struct fields in test mismatched with `zap_agent::AgentIntent`, `ReceiptReplicationRequest` fields mismatched with `zap_ledger`, `DriverManifest::new` parameter mismatches).
- **Facade Implementations in E2E Suite**:
  - `tc_f09_001` through `tc_f09_005`: Merely create JSON objects and check `assert_eq!(req["method"], "initialize")` or `assert_eq!(req["method"], "tools/list")` without running an MCP server.
  - `tc_f10_001` through `tc_f10_005`: Check hardcoded string assertions (`let sse_event = "event: ..."; assert!(sse_event.starts_with(...));`).
  - `tc_f11_001` through `tc_f11_005`: Check basic sha2 assertions without verifying a unified provenance chain.
  - Cross-feature tests (`tc_x_006`, `tc_x_007`, `tc_x_008`, `tc_x_009`, `tc_x_014`, `tc_x_015`) and real-world workflows (`tc_rw_001`, `tc_rw_006`, `tc_rw_010`) are stubs.

---

## 2. Logic Chain

1. **Requirement R4 & Milestone M4 Scope** (from `ORIGINAL_REQUEST.md` and `PROJECT.md`):
   - M4 requires a production-ready AI Agent Gateway and Model Context Protocol (MCP) server connecting LLMs to ZAP's deterministic policy, Proof-of-Action (PoA) consensus, WASM execution engine, and signed receipt ledger.
   - Strict cryptographic provenance linking must chain every execution step: $H_{\text{intent}} \to H_{\text{negotiation}} \to H_{\text{policy}} \to H_{\text{driver}} \to H_{\text{poa}} \to H_{\text{receipt}}$.

2. **Crate Boundary & Architectural Decomposition**:
   - `zap-agent` is the contract layer (pure data structures, validation rules, JSON schema).
   - `zap-gateway` must be the active transport and execution bridge crate, depending on `zap-agent`, `zap-core`, `zap-crypto`, `zap-policy`, `zap-capability`, `zap-runtime`, `zap-ledger`, `zap-memory`, `zap-telemetry`, and `zap-node`.
   - `zap-gateway` architecture must provide:
     1. **`mcp` module**: Stdio and network JSON-RPC 2.0 protocol engine:
        - `initialize` (MCP protocolVersion `2024-11-05`, capabilities, serverInfo).
        - `tools/list` & `tools/call` for execution tools: `zap_send_transaction`, `zap_query_state`, `zap_get_fleet_health`, `zap_inspect_pack`, `zap_intent_create`, `zap_delegate`.
        - `resources/list` & `resources/read` for URIs: `zap://ledger/receipts`, `zap://memory/{namespace}/{subject}`.
        - `prompts/list` & `prompts/get` for templates: `agent_task_execution`, `security_audit`, `delegation_request`.
        - Standard JSON-RPC 2.0 error handling: `-32700` (Parse Error), `-32600` (Invalid Request), `-32601` (Method Not Found), `-32602` (Invalid Params), `-32603` (Internal Error).
     2. **`transport` module**:
        - **HTTP REST**: `POST /v1/agent/intent`, `POST /v1/agent/negotiate`, `GET /v1/agent/session/{session_id}`, `GET /v1/capabilities`, `GET /healthz`, `GET /metrics`.
        - **SSE Streaming**: `GET /v1/agent/events`, `GET /v1/agent/stream`, `GET /v1/agent/session/{session_id}/stream` (emitting `AgentStatusUpdate` and terminal `AgentResult` Server-Sent Events).
        - **WebSocket Bridge**: `GET /v1/agent/ws` (RFC 6455 framing, duplex `AgentMessage` streaming, close code 1009 for frames exceeding 4MB).
        - **Telemetry Integration**: Record metrics on `ZapNode` (`record_agent_gateway_request(transport, status)`, `inc_agent_session()`, `dec_agent_session()`).
     3. **`provenance` module**:
        - `ProvenanceChainDigest` data structure linking:
          $$\begin{aligned}
          H_{\text{intent}} &= \text{blake3}(\text{canonical\_json}(\text{AgentIntent})) \\
          H_{\text{negotiation}} &= \text{blake3}(\text{canonical\_json}(\text{CapabilityNegotiationResponse})) \\
          H_{\text{policy}} &= \text{blake3}(H_{\text{intent}} \parallel \text{canonical\_json}(\text{PolicyInput}) \parallel \text{canonical\_json}(\text{PolicyDecision})) \\
          H_{\text{driver}} &= \text{blake3}(H_{\text{DriverManifest}} \parallel \text{output\_bytes}) \\
          H_{\text{poa}} &= \text{blake3}(\text{poa\_signatures}) \quad (\text{or zeros if not consensus protected}) \\
          H_{\text{receipt}} &= \text{blake3}(H_{\text{intent}} \parallel H_{\text{negotiation}} \parallel H_{\text{policy}} \parallel H_{\text{driver}} \parallel H_{\text{poa}})
          \end{aligned}$$
        - Step-by-step verification logic detecting intermediate link tampering, omitted steps, or corrupted signatures, incrementing `record_provenance_verification_failure()` on verification errors.

3. **Receipt Ledger & ActionReceipt Integration**:
   - `crates/zap-ledger` must be updated to include optional provenance fields on `ActionReceipt`:
     - `session_id: Option<Uuid>`
     - `intent_id: Option<Uuid>`
     - `negotiation_id: Option<Uuid>`
     - `policy_decision: Option<String>`
     - `provenance_chain_digest: Option<String>`
   - `ReceiptJournalStore` serialization and index structures remain backward-compatible by using `#[serde(default, skip_serializing_if = "Option::is_none")]`.

4. **CLI Subcommand Integration**:
   - `crates/zap-cli` must expose:
     - `zap gateway start [--port <u16>] [--bind <addr>] [--config <zap.toml>] [--mcp-stdio]`
     - `zap gateway status [--port <u16>]`
     - `zap provenance verify --receipt <file.json> / --chain <digest>`

5. **E2E Test Suite Alignment**:
   - All 71 compilation errors in `tests/e2e/tests/e2e_suite.rs` must be corrected.
   - All facade tests in F09, F10, F11, cross-feature (Tier 3), and real-world (Tier 4) must be converted into genuine functional tests invoking the real gateway server, MCP stdio protocol, SSE stream subscriber, WebSocket duplex channel, and provenance chain verifier.

---

## 3. Caveats

1. **WASM Runtime Dependency**: Driver execution in `zap-gateway` depends on `zap-runtime` and `wasmtime`. In test environments where valid WASM bytecode is needed, minimal valid WAT (`(module (func (export "zap_execute") ...))`) should be supplied.
2. **Concurrency & Port Allocation**: E2E tests for HTTP/SSE/WebSocket gateway must bind to ephemeral free ports (`127.0.0.1:0`) to avoid port collision during parallel test runs.
3. **No External Network Dependencies**: All MCP, REST, SSE, and WebSocket tests must run entirely against local loopback interfaces (`127.0.0.1`) without external network access.

---

## 4. Conclusion

1. `crates/zap-agent` provides a solid, tested foundation of protocol contracts, but **Milestone 4 is currently 0% implemented in terms of runtime gateway, MCP JSON-RPC 2.0 handlers, multi-transport servers, and provenance chain verification**.
2. A new workspace crate **`crates/zap-gateway`** must be created and registered in `Cargo.toml`.
3. `crates/zap-ledger` must be extended with provenance fields in `ActionReceipt`.
4. `crates/zap-cli` must be extended with `gateway` and `provenance` subcommands.
5. `tests/e2e/tests/e2e_suite.rs` must be fixed and upgraded from placeholder/facade tests to comprehensive opaque-box integration tests.

---

## 5. Verification Method

To verify the implementation of Milestone 4:

1. **Unit & Integration Tests in `zap-gateway`**:
   ```powershell
   cargo test --package zap-gateway
   ```
   Must verify:
   - MCP `initialize`, `tools/list`, `tools/call` for `zap_send`, `zap_query`, `zap_get_fleet_health`, `zap_inspect_pack`.
   - MCP `resources/list`, `resources/read` for `zap://ledger/receipts` and `zap://memory/...`.
   - MCP `prompts/list`, `prompts/get`.
   - JSON-RPC 2.0 error handling (`-32700`, `-32600`, `-32601`, `-32602`, `-32603`).
   - HTTP REST `/v1/agent/intent`, `/v1/agent/negotiate`, `/v1/agent/session/{id}`, `/healthz`, `/metrics`.
   - SSE streaming `/v1/agent/events` with multi-client delivery and disconnect cleanup.
   - WebSocket `/v1/agent/ws` duplex framing and 4MB size limit handling.
   - Cryptographic `ProvenanceChainDigest` generation and step-by-step verification ($H_{\text{intent}} \to H_{\text{negotiation}} \to H_{\text{policy}} \to H_{\text{driver}} \to H_{\text{poa}} \to H_{\text{receipt}}$).

2. **CLI Commands**:
   ```powershell
   cargo run --package zap-cli --bin zap -- gateway --help
   cargo run --package zap-cli --bin zap -- provenance --help
   ```

3. **E2E Test Suite Validation**:
   ```powershell
   cargo test --package zap-e2e
   ```
   Must pass all Tier 1 (F01-F13), Tier 2 (B01-B13), Tier 3 (Cross-feature), and Tier 4 (Real-world) test cases with 0 compilation errors and 0 test failures.

4. **Workspace Clippy Cleanliness**:
   ```powershell
   cargo clippy --workspace --all-targets -- -D warnings
   ```
