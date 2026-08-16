# Milestone 4 Handoff Report: AI Agent Gateway, MCP Protocol & Provenance Engine

## 1. Observation

Codebase inspection across `Cargo.toml`, `crates/zap-agent/src/lib.rs`, `crates/zap-node/src/lib.rs`, `crates/zap-telemetry/src/metrics.rs`, `crates/zap-cli/src/main.rs`, `TEST_INFRA.md`, and `tests/e2e/tests/e2e_suite.rs` revealed the following:

1. **Missing `zap-gateway` Crate (`Cargo.toml:1-28`)**:
   - `crates/zap-gateway` is declared in `PROJECT.md:9,60` and `TEST_INFRA.md:30-31` as the owner crate for the AI agent gateway, Model Context Protocol (MCP) server, multi-transport bridges, and provenance chain linkage.
   - Currently, directory `crates/zap-gateway` does **not exist** on disk and is absent from `members` in the root `Cargo.toml`.

2. **Agent Protocol Contracts (`crates/zap-agent/src/lib.rs`)**:
   - `zap-agent` defines complete data structures for `AgentIntent`, `AgentSession`, `DelegationRequest`, `DelegationResponse`, `CapabilityNegotiationRequest`, `CapabilityNegotiationResponse`, `AgentStatusUpdate`, `AgentResult`, `AgentErrorReport`, and `AgentMessage`.
   - `AgentMessage` provides serialization/deserialization to JSON with standard subjects (`zap.agent.intent`, `zap.agent.session`, `zap.agent.delegation.request`, etc.), but does not include runtime network daemon, MCP JSON-RPC protocol parser, or HTTP/SSE/WebSocket listener logic.

3. **Node Engine Metrics & Transport Hooks (`crates/zap-node/src/lib.rs` & `crates/zap-telemetry/src/metrics.rs`)**:
   - `ZapNode` in `crates/zap-node/src/lib.rs` (lines 2285-2315) already has metric recording methods:
     - `record_agent_gateway_request(&self, transport: &str, status: &str)`
     - `inc_agent_session(&self)`
     - `dec_agent_session(&self)`
     - `record_provenance_verification_failure(&self)`
   - `ZapNodeMetricsSnapshot` in `crates/zap-telemetry/src/metrics.rs` (lines 54-56, 240-268) already includes Prometheus export formats for `zap_agent_gateway_requests_total`, `zap_agent_sessions_active`, and `zap_provenance_verification_failures_total`.
   - `ZapNode` has an async TCP listener pattern in `spawn_observability_http` (lines 1695-1737) that demonstrates the project's native nonblocking HTTP handling.

4. **CLI Subcommand Gaps (`crates/zap-cli/src/main.rs`)**:
   - `Commands` enum (lines 91-281) contains `Agent` (which only builds/validates local JSON structs) and `Receipts` (which verifies binary journals), but lacks:
     - `zap gateway start`: daemon startup with HTTP, SSE, WebSocket, and MCP stdio flags.
     - `zap gateway status`: runtime health and session inspection.
     - `zap provenance verify`: verifying cryptographic provenance chains ($H_{\text{intent}} \dots H_{\text{receipt}}$) against receipts and node keys.
     - `--provenance` flag under `zap receipts verify`.

5. **E2E & Test Specifications (`TEST_INFRA.md:96-115, 191-211` & `tests/e2e/tests/e2e_suite.rs:636-800`)**:
   - `TEST_INFRA.md` specifies requirements for:
     - **F09 (MCP Server)**: `initialize`, `tools/list`, `tools/call`, `resources/read`, `prompts/list`, standard JSON-RPC 2.0 error handling (`-32700`, `-32601`, `-32602`).
     - **F10 (Multi-Transport Gateway)**: HTTP REST `/v1/agent/intents`, SSE streaming `/v1/agent/stream` (or `/v1/agent/events`) with disconnect cleanup, WebSocket bridge `/v1/agent/ws` with 4MB max frame size limit.
     - **F11 (Provenance Chain Linking)**: 6-stage hash chain ($H_{\text{intent}} \to H_{\text{negotiation}} \to H_{\text{policy}} \to H_{\text{driver}} \to H_{\text{poa}} \to H_{\text{receipt}} \to H_{\text{root}}$) signed by node Ed25519 keypair.
   - `tests/e2e/tests/e2e_suite.rs` contains placeholder skeletons for F09, F10, F11, boundary tests (`TC-B09`, `TC-B10`, `TC-B11`), cross-feature tests (`TC-X-006` .. `009`, `TC-X-014`, `TC-X-015`), and real-world workflows (`TC-RW-001`, `TC-RW-006`, `TC-RW-009`, `TC-RW-010`).

---

## 2. Logic Chain

1. **Creation of `crates/zap-gateway`**:
   - To satisfy Requirement R4 and Milestone 4, a dedicated crate `crates/zap-gateway` must be created and added to the root `Cargo.toml`.
   - It serves as the runtime gateway daemon bridging external AI agents (via MCP, HTTP REST, SSE, and WebSocket) to ZAP's deterministic execution core (`zap-node`, `zap-ledger`, `zap-policy`, `zap-capability`, `zap-pack`, `zap-runtime`).

2. **MCP JSON-RPC 2.0 Protocol Engine**:
   - Follows the Model Context Protocol specification (`protocolVersion: "2024-11-05"`).
   - Handles:
     - `initialize`: returns serverInfo `{"name": "zap-gateway", "version": "0.1.0"}` and capabilities for tools, resources, and prompts.
     - `tools/list`: exposes `zap_send_transaction`, `zap_query_state`, `zap_get_fleet_health`, `zap_inspect_pack`, `zap_delegate`, `zap_verify_provenance`.
     - `tools/call`: validates arguments against tool schemas, executes through policy/PoA/runtime, appends receipt to journal, records provenance, and returns text content.
     - `resources/list` & `resources/read`: exposes `zap://ledger/receipts`, `zap://memory/*`, `zap://fleet/status`, `zap://packs/*`.
     - `prompts/list` & `prompts/get`: provides agent templates (`agent_action_plan`, `policy_check`, `incident_diagnostics`).
     - JSON-RPC 2.0 error handling: strictly emits `-32700` (Parse error), `-32600` (Invalid Request), `-32601` (Method not found), `-32602` (Invalid params), `-32603` (Internal error).

3. **Multi-Transport Gateway Engine**:
   - **HTTP REST (`/v1/agent/*`)**:
     - `POST /v1/agent/intents`: receives `AgentIntent`, checks policy, executes driver, generates provenance digest, returns HTTP 202 Accepted or 200 OK.
     - `GET /v1/agent/sessions/{session_id}`: queries session status.
     - `POST /v1/agent/sessions`: initiates a session.
     - `POST /v1/agent/delegate`: processes `DelegationRequest` $\to$ `DelegationResponse`.
     - `POST /v1/agent/negotiate`: processes `CapabilityNegotiationRequest` $\to$ `CapabilityNegotiationResponse`.
     - `POST /v1/agent/provenance/verify`: validates `ProvenanceChainDigest`.
     - HTTP responses: 200, 202, 400 (Bad Request), 401 (Unauthorized), 403 (Policy Denied), 404 (Not Found), 405 (Method Not Allowed), 429 (Too Many Requests).
   - **SSE Stream (`/v1/agent/stream` / `/v1/agent/events`)**:
     - Establishes `text/event-stream` connection.
     - Broadcasts real-time events: `agent_status`, `agent_result`, `heartbeat`.
     - Uses `tokio::sync::broadcast` to handle multiple parallel clients (5+ concurrent connections) with immediate disconnect cleanup preventing channel memory leaks.
   - **WebSocket Bridge (`/v1/agent/ws`)**:
     - Implements RFC 6455 upgrade handshake (`Sec-WebSocket-Key` $\to$ `Sec-WebSocket-Accept` using SHA-1 + Base64).
     - Full-duplex bidirectional exchange of `AgentMessage` JSON frames.
     - Enforces 4MB max frame size limit (returns close code `1009` Message Too Big when exceeded).

4. **Cryptographic Provenance Chain Engine**:
   - Constructs a verifiable 6-stage hash chain linking every phase of agent execution:
     $$\begin{aligned}
     H_{\text{intent}} &= \text{SHA256}(\text{canonical\_json}(AgentIntent)) \\
     H_{\text{negotiation}} &= \text{SHA256}(H_{\text{intent}} \parallel \text{canonical\_json}(Negotiation / Delegation)) \\
     H_{\text{policy}} &= \text{SHA256}(H_{\text{negotiation}} \parallel \text{policy\_digest} \parallel \text{decision}) \\
     H_{\text{driver}} &= \text{SHA256}(H_{\text{policy}} \parallel \text{driver\_id} \parallel \text{input\_hash} \parallel \text{output\_hash}) \\
     H_{\text{poa}} &= \text{SHA256}(H_{\text{driver}} \parallel \text{poa\_signatures}) \\
     H_{\text{receipt}} &= \text{SHA256}(H_{\text{poa}} \parallel \text{receipt\_id} \parallel \text{timestamp}) \\
     H_{\text{root}} &= \text{SHA256}(H_{\text{intent}} \parallel H_{\text{negotiation}} \parallel H_{\text{policy}} \parallel H_{\text{driver}} \parallel H_{\text{poa}} \parallel H_{\text{receipt}})
     \end{aligned}$$
   - Signs $H_{\text{root}}$ with the node's Ed25519 private key.
   - Provides step-level verification: any alteration at any stage ($H_{\text{intent}}$, $H_{\text{driver}}$, $H_{\text{policy}}$, etc.) fails verification with an explicit error identifying the exact tampered link and increments `zap_provenance_verification_failures_total`.

5. **CLI Integration & Metric Wiring**:
   - Add subcommands `zap gateway start`, `zap gateway status`, and `zap provenance verify` to `crates/zap-cli/src/main.rs`.
   - Wire all gateway requests to `ZapNode::record_agent_gateway_request(transport, status)` to ensure Prometheus metric parity for `zap_agent_gateway_requests_total`.
   - Track active sessions via `ZapNode::inc_agent_session()` and `ZapNode::dec_agent_session()`.
   - Record verification failures via `ZapNode::record_provenance_verification_failure()`.

---

## 3. Caveats

1. **Native Async Networking vs Heavy Frameworks**:
   - Consistent with ZAP's minimal-dependency, high-performance architecture, HTTP, SSE, and WebSocket handling is implemented directly using `tokio::net` and standard library primitives rather than adding heavy external web framework dependencies (e.g. `axum`, `actix`), keeping binary footprint small and deterministic.
2. **WebSocket Handshake Compatibility**:
   - WebSocket upgrade requires computing `Sec-WebSocket-Accept` as `base64(SHA1(key + "258EAFA5-E914-47DA-95CA-C5AB0DC85B11"))`. A self-contained SHA-1 implementation or standard crypto helper should be included in `zap-gateway`/`zap-crypto`.
3. **Stdio MCP Concurrency**:
   - In MCP stdio mode, standard input and output streams are shared with the parent process. All logs and tracing must be directed to `stderr` or a log file so that `stdout` remains pure line-delimited JSON-RPC 2.0 messages.

---

## 4. Conclusion & 5-Phase Implementation Plan for `worker_m4`

### Phase 1: Workspace Integration & `crates/zap-gateway` Foundation
1. **Workspace Configuration (`Cargo.toml`)**:
   - Add `"crates/zap-gateway"` to `workspace.members`.
   - Add `zap-gateway = { path = "crates/zap-gateway" }` to `[workspace.dependencies]`.
2. **Create `crates/zap-gateway/Cargo.toml`**:
   - Package name: `zap-gateway`, edition: `2024`.
   - Dependencies: `zap-agent`, `zap-core`, `zap-crypto`, `zap-envelope`, `zap-capability`, `zap-journal`, `zap-ledger`, `zap-node`, `zap-pack`, `zap-policy`, `zap-runtime`, `zap-store`, `zap-telemetry`, `tokio`, `serde`, `serde_json`, `uuid`, `sha2`, `ed25519-dalek`, `base64`, `hex`, `tracing`, `thiserror`, `anyhow`.
3. **Create Module Structure**:
   - `src/lib.rs`: public exports for gateway server, config, MCP, transports, provenance.
   - `src/error.rs`: `ZapGatewayError` enum with structured error codes.
   - `src/config.rs`: `GatewayConfig` with bind addresses, auth token, rate limits, frame size limits, CORS settings.

### Phase 2: Cryptographic Provenance Chain Engine
1. **Implement `crates/zap-gateway/src/provenance/mod.rs`**:
   - Data structures: `ProvenanceChainDigest`, `ProvenanceStage`, `ProvenanceStepHash`, `ProvenanceVerificationReport`.
   - `ProvenanceChainBuilder`: incremental stage hashing ($H_{\text{intent}} \to H_{\text{negotiation}} \to H_{\text{policy}} \to H_{\text{driver}} \to H_{\text{poa}} \to H_{\text{receipt}} \to H_{\text{root}}$).
   - Ed25519 signature generation over `H_root`.
   - Independent verification method `ProvenanceChainDigest::verify(&self, public_key: &PublicKey)` validating step continuity, root Merkle hash, and signature.
   - Unit tests covering full chain generation, step tampering detection, and missing link detection.

### Phase 3: MCP JSON-RPC 2.0 Protocol Engine
1. **Implement `crates/zap-gateway/src/mcp/protocol.rs`**:
   - `JsonRpcRequest`, `JsonRpcResponse`, `JsonRpcError`, `InitializeParams`, `InitializeResult`, `ToolsListResult`, `ToolDescriptor`, `ToolCallParams`, `ToolCallResult`, `ResourcesListResult`, `ResourceReadResult`, `PromptsListResult`, `PromptGetResult`.
   - Standard error codes: `-32700` (Parse error), `-32600` (Invalid Request), `-32601` (Method not found), `-32602` (Invalid params), `-32603` (Internal error).
2. **Implement `crates/zap-gateway/src/mcp/tools.rs`**:
   - Tool `zap_send_transaction`: executes actions, verifies PoA if required, writes receipt to journal.
   - Tool `zap_query_state`: queries memory journal and receipt store.
   - Tool `zap_get_fleet_health`: runs fleet doctor diagnostics.
   - Tool `zap_inspect_pack`: parses and validates domain packs.
   - Tool `zap_delegate`: handles multi-agent task delegation.
   - Tool `zap_verify_provenance`: verifies cryptographic provenance chains.
3. **Implement `crates/zap-gateway/src/mcp/resources.rs` & `prompts.rs`**:
   - Resource providers for `zap://ledger/receipts`, `zap://memory/*`, `zap://fleet/status`, `zap://packs/*`.
   - Prompt templates for `agent_action_plan`, `policy_check`, `incident_diagnostics`.
4. **Implement `crates/zap-gateway/src/mcp/stdio.rs` & `mod.rs`**:
   - Stdio loop processing line-by-line JSON-RPC requests on `stdin` and writing JSON-RPC responses to `stdout`.

### Phase 4: Multi-Transport Gateway (HTTP REST, SSE, WebSocket)
1. **Implement `crates/zap-gateway/src/transports/http.rs`**:
   - Async HTTP 1.1 server on `GatewayConfig::http_bind`.
   - Endpoints:
     - `POST /v1/agent/intents`: submits `AgentIntent`, runs policy check, executes driver/action, builds provenance digest, returns HTTP 202/200.
     - `GET /v1/agent/sessions/{session_id}`: returns session state.
     - `POST /v1/agent/sessions`: creates session.
     - `POST /v1/agent/delegate`: submits `DelegationRequest`.
     - `POST /v1/agent/negotiate`: submits `CapabilityNegotiationRequest`.
     - `POST /v1/agent/provenance/verify`: verifies `ProvenanceChainDigest`.
     - `GET /v1/health` / `/healthz`: health probe.
     - `GET /metrics`: Prometheus text metrics.
   - Strict status codes: 200, 202, 400, 401, 403, 404, 405, 429.
2. **Implement `crates/zap-gateway/src/transports/sse.rs`**:
   - `GET /v1/agent/stream` (or `/v1/agent/events`) EventSource stream.
   - `SseBroker` with `tokio::sync::broadcast` emitting `agent_status`, `agent_result`, `heartbeat`.
   - Connection drop detection with channel cleanup preventing leaks.
3. **Implement `crates/zap-gateway/src/transports/ws.rs`**:
   - `GET /v1/agent/ws` WebSocket upgrade handshake (RFC 6455).
   - Duplex message frame handler for `AgentMessage`.
   - 4MB max frame size enforcement (close code 1009).
4. **Implement `crates/zap-gateway/src/server.rs`**:
   - `AgentGatewayServer` integrating HTTP, SSE, WS, MCP, and `ZapNode` metric recorders.

### Phase 5: CLI Subcommands & Test Suite Verification
1. **CLI Commands in `crates/zap-cli/src/main.rs`**:
   - Add `GatewayCommand`:
     - `zap gateway start`: `--config <path>`, `--http-bind <addr>`, `--ws-bind <addr>`, `--mcp-stdio`, `--auth-token <token>`.
     - `zap gateway status`: `--addr <url>`, `--json`.
   - Add `ProvenanceCommand`:
     - `zap provenance verify`: `--chain <path>`, `--key <key_file>`, `--receipt <receipt_file>`, `--json`.
   - Update `ReceiptsCommand::Verify` to support `--provenance`.
2. **Implement Test Suites in `tests/e2e/tests/e2e_suite.rs` & `crates/zap-gateway/tests/`**:
   - **Tier 1 (Feature Coverage)**:
     - `tc_f09_001` .. `tc_f09_005`: MCP stdio init, tools/list, tools/call, resources/read, prompts/list.
     - `tc_f10_001` .. `tc_f10_005`: REST intent submit, SSE events, WS bridge, transport fallback, parallel SSE streams.
     - `tc_f11_001` .. `tc_f11_005`: Complete chain generation, intent-to-policy link, policy-to-poa link, poa-to-receipt link, chain verify CLI.
   - **Tier 2 (Boundary Cases)**:
     - `tc_b09_001` .. `tc_b09_005`: JSON-RPC parse error, unknown tool, missing tool param, oversized payload, session reset.
     - `tc_b10_001` .. `tc_b10_005`: Malformed REST JSON, SSE connection drop, WS frame size exceeded (4MB), unauthorized access (401), rate limiting (429).
     - `tc_b11_001` .. `tc_b11_005`: Tampered intent hash, tampered driver output, omitted intermediate link, corrupted node signature, long chain.
   - **Tier 3 (Cross-Feature Combinations)**:
     - `tc_x_006`: MCP tool call emits event on Gateway SSE stream.
     - `tc_x_007`: Gateway HTTP REST response returns complete ProvenanceChainDigest.
     - `tc_x_008`: Signed receipt stored in journal contains verifiable provenance chain root.
     - `tc_x_009`: WASM driver execution digest incorporated into provenance chain.
     - `tc_x_014`: MCP `resources/read` on `zap://ledger/receipts` returns verified segment entries.
     - `tc_x_015`: Provenance verification failure captured by `zap incident snapshot`.
   - **Tier 4 (Real-World Workflows)**:
     - `tc_rw_001`: Distributed AI agent fleet task execution.
     - `tc_rw_006`: Multi-tenant agent delegation & capability negotiation.
     - `tc_rw_009`: Emergency PACT revocation & message policy enforcement.
     - `tc_rw_010`: Full lifecycle AI agent task to audit receipt.

---

## 5. Verification Method

To verify the complete Milestone 4 implementation:

1. **Compilation & Clippy Verification**:
   ```bash
   cargo check --workspace --all-targets
   cargo clippy --workspace --all-targets -- -D warnings
   ```

2. **Crate Unit & Integration Tests**:
   ```bash
   cargo test -p zap-gateway
   cargo test -p zap-agent
   cargo test -p zap-cli
   ```

3. **MCP JSON-RPC Stdio Verification**:
   ```bash
   echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test","version":"1.0.0"}}}' | cargo run -p zap-cli -- gateway start --mcp-stdio
   echo '{"jsonrpc":"2.0","id":2,"method":"tools/list"}' | cargo run -p zap-cli -- gateway start --mcp-stdio
   ```

4. **Multi-Transport HTTP / SSE / WS Verification**:
   ```bash
   # Start gateway daemon in background
   cargo run -p zap-cli -- gateway start --config zap.toml --http-bind 127.0.0.1:8080 &
   # Query health
   curl -s http://127.0.0.1:8080/v1/health
   # Submit agent intent
   curl -s -X POST http://127.0.0.1:8080/v1/agent/intents -H "Content-Type: application/json" -d '{"schema_version":1,"session_id":"11111111-1111-4111-8111-111111111111","intent_id":"22222222-2222-4222-8222-222222222222","source_agent":"agent_1","kind":"act","objective":"test"}'
   ```

5. **Provenance Chain Verification**:
   ```bash
   cargo run -p zap-cli -- provenance verify --chain chain.json --key .zap/node.key
   ```

6. **E2E Test Suite Execution**:
   ```bash
   cargo test --test e2e_suite tc_f09
   cargo test --test e2e_suite tc_f10
   cargo test --test e2e_suite tc_f11
   cargo test --test e2e_suite tc_b09
   cargo test --test e2e_suite tc_b10
   cargo test --test e2e_suite tc_b11
   ```
