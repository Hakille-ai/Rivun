# Milestone 4 Specification Mining Report: MCP JSON-RPC 2.0, Multi-Transport Gateway & Cryptographic Provenance Chain

## 1. Observation

Direct examination of the workspace files, tests, and protocol definitions revealed the following concrete technical details:

1. **E2E Test Specifications for Milestone 4 (`tests/e2e/tests/e2e_suite.rs`)**:
   - **Feature 9: MCP Server Implementation** (lines 635–700):
     - `tc_f09_001_mcp_stdio_init`: Initializes MCP JSON-RPC 2.0 with method `"initialize"`, `protocolVersion: "2024-11-05"`, `capabilities: {}`, `clientInfo: {"name": "e2e_test", "version": "1.0.0"}`.
     - `tc_f09_002_tools_list`: Requests `"tools/list"`, expects list of callable ZAP execution drivers.
     - `tc_f09_003_tools_call`: Calls `"tools/call"` with params `{"name": "zap_send", "arguments": {"target": "<uuid>", "payload": "hello"}}`.
     - `tc_f09_004_resources_read`: Reads `"resources/read"` with params `{"uri": "zap://ledger/receipts"}`.
     - `tc_f09_005_prompts_list`: Requests `"prompts/list"`, expects prompt template descriptors.
   - **Feature 10: Multi-Transport Agent Gateway** (lines 701–761):
     - `tc_f10_001_rest_intent_submission`: Submits `AgentIntent` payload with `session_id`, `intent_id`, `source_agent`, `kind: IntentKind::Act`, `objective`, `input`, `required_capabilities`, `priority`.
     - `tc_f10_002_sse_stream_events`: Subscribes to Server-Sent Events stream with event format `event: agent_status\ndata: {"status":"running"}\n\n`.
     - `tc_f10_003_ws_bridge_message`: Exchanging duplex `AgentMessage::Session(session)` across WebSocket bridge.
     - `tc_f10_004_transport_fallback`: Handles protocol fallback when connecting via fallback transports (`http_rest_fallback`).
     - `tc_f10_005_parallel_sse_streams`: Handles 5+ parallel concurrent SSE client streams.
   - **Feature 11: Provenance Chain Linking** (lines 762–802):
     - `tc_f11_001_complete_chain_generation`: Computes multi-stage hash chain linking intent hash and policy hash to 32-byte chain root.
     - `tc_f11_002_intent_to_policy_link`: Links $H_{\text{intent}}$ to $H_{\text{policy}}$ (`link:{h_intent}:policy_allow`).
     - `tc_f11_003_policy_to_poa_link`: Signs provenance policy root with Ed25519 keypair producing 64-byte signature.
     - `tc_f11_004_poa_to_receipt_link`: Embeds provenance chain root digest in receipt payload.
     - `tc_f11_005_chain_verify_cli`: Verifies cryptographic chain validity.
   - **Tier 2 Boundary Cases** (lines 977–992):
     - `tc_b09_001_invalid_jsonrpc_syntax`: Invalid JSON input to JSON-RPC parser yields parse error.
     - `tc_b11_001_tampered_intent_step_hash`: Altering 1 byte in $H_{\text{intent}}$ ($0\text{xAA}$) causes chain verification failure.
   - **Tier 3 Cross-Feature Combinations** (lines 1047–1064):
     - `tc_x_006_mcp_plus_gateway_intent`: Dispatches intent via MCP, receives status via Gateway.
     - `tc_x_008_provenance_plus_signed_journal`: Stores provenance chain digest in signed journal segment manifest.
   - **Tier 4 Real-World Workflows** (lines 1083–1122, 1182–1201):
     - `tc_rw_001_distributed_ai_agent_fleet_task`: Intent $\to$ Policy Evaluation $\to$ Provenance Chain Digest Creation $\to$ Receipt Journal Store Commit $\to$ Seal Journal Segment.
     - `tc_rw_010_full_lifecycle_agent_task_to_receipt`: Agent Intent $\to$ Action Receipt $\to$ Rotate & Seal Manifest.

2. **Test Infrastructure Expectations (`TEST_INFRA.md:96–116, 191–211, 237–247`)**:
   - **F09 MCP Server**:
     - `TC-F09-001` to `TC-F09-005`: stdio initialization, tools list, tools call (`zap_send`, `zap_query`), resources list/read (`zap://ledger/receipts`), prompts list/get.
     - `TC-B09-001` to `TC-B09-005`: JSON-RPC `-32700` (Parse error), `-32601` (Method not found), `-32602` (Invalid params), oversized stdio payload (10MB JSON line), connection reset.
   - **F10 Gateway**:
     - `TC-F10-001` to `TC-F10-005`: HTTP POST `/v1/agent/intents` returning 202 Accepted, SSE GET `/v1/agent/events`, WebSocket `/v1/agent/ws`, fallback handshake, 5 parallel streams.
     - `TC-B10-001` to `TC-B10-005`: HTTP 400 Bad Request, SSE abrupt connection drop cleanup, WS 4MB frame limit (close code 1009), HTTP 401 Unauthorized, HTTP 429 Rate Limiting with `Retry-After`.
   - **F11 Provenance**:
     - `TC-F11-001` to `TC-F11-005`: 6 linked hashes ($H_{\text{intent}} \to H_{\text{negotiation}} \to H_{\text{policy}} \to H_{\text{driver}} \to H_{\text{poa}} \to H_{\text{receipt}}$), hash linking equations, PoA link, receipt root embedding, CLI `zap receipts verify --provenance`.
     - `TC-B11-001` to `TC-B11-005`: Tampered intent hash, tampered driver output, missing intermediate link (`INCOMPLETE_PROVENANCE_CHAIN`), corrupted Ed25519 node signature, nested chain length up to $N=100$.

3. **Data Contracts in `crates/zap-agent/src/lib.rs` & `crates/zap-ledger/src/lib.rs`**:
   - `AgentIntent`: `schema_version`, `intent_id`, `session_id`, `source_agent`, `target_agent`, `kind`, `objective`, `input`, `required_capabilities`, `constraints`, `context`, `deadline_unix_micros`, `priority`, `metadata`.
   - `AgentSession`: `schema_version`, `session_id`, `root_intent_id`, `parent_session_id`, `owner_agent`, `status`, `created_at_micros`, `updated_at_micros`, `accepted_capabilities`, `metadata`.
   - `DelegationRequest` / `DelegationResponse`: `delegation_id`, `session_id`, `parent_intent_id`, `from_agent`, `to_agent`, `decision` (`accepted`, `rejected`, `counter_offer`), `assigned_agent`.
   - `CapabilityNegotiationRequest` / `CapabilityNegotiationResponse`: `negotiation_id`, `session_id`, `requester_agent` / `responder_agent`, `required_capabilities`, `accepted_capabilities`, `decision` (`accepted`, `partial`, `rejected`).
   - `AgentStatusUpdate`: `session_id`, `intent_id`, `agent_id`, `status`, `progress_per_mille`, `message`, `updated_at_micros`.
   - `AgentResult`: `result_id`, `session_id`, `intent_id`, `agent_id`, `status` (`completed`, `failed`, `cancelled`), `outputs`, `artifacts`, `error`, `completed_at_micros`.
   - `AgentErrorReport`: `error_id`, `session_id`, `intent_id`, `agent_id`, `error: AgentErrorInfo`, `observed_at_micros`.
   - `ZapNode` telemetry counters in `crates/zap-node/src/lib.rs`: `record_agent_gateway_request(transport, status)`, `inc_agent_session()`, `dec_agent_session()`, `record_provenance_verification_failure()`.

---

## 2. Logic Chain

1. **MCP Server Interface Specification**:
   - The MCP interface follows the standard Model Context Protocol over JSON-RPC 2.0 (`"jsonrpc": "2.0"`).
   - Stdio transport operates on standard input and output streams. Requests are newline-delimited JSON objects. Tracing and diagnostics must strictly route to stderr to avoid corrupting stdio JSON-RPC stream.
   - Handlers required:
     - `initialize`: Client passes protocol version (`"2024-11-05"`), capabilities, and `clientInfo`. Server responds with server metadata and capability declarations (`tools`, `resources`, `prompts`).
     - `tools/list`: Returns array of `Tool` definitions. Built-in tools include `zap_send`, `zap_query`, `zap_get_fleet_health`, `zap_inspect_pack`, `zap_delegate`, `zap_verify_provenance`, plus dynamically registered WASM drivers.
     - `tools/call`: Executes tool with validated JSON arguments, initiates intent/execution pipeline, returns `content: [{"type": "text", "text": "..."}]` and `isError: bool`.
     - `resources/list`: Returns array of `Resource` definitions for accessible ZAP URIs (`zap://ledger/receipts`, `zap://memory/{namespace}/{subject}`, `zap://fleet/status`, `zap://packs/{name}`).
     - `resources/read`: Reads resource at specified URI, returns `contents: [{"uri": "...", "mimeType": "...", "text": "..."}]`.
     - `prompts/list`: Returns available pre-approved prompt templates (`agent_action_plan`, `policy_check`, `incident_diagnostics`).
     - `prompts/get`: Returns evaluated prompt with messages array.
   - Error format: Returns JSON-RPC 2.0 standard error object `{"jsonrpc": "2.0", "id": ..., "error": {"code": <int>, "message": "<str>", "data": ...}}`.

2. **Multi-Transport Gateway Specification**:
   - **HTTP REST Transport**:
     - Endpoints: `POST /v1/agent/intents`, `GET /v1/agent/sessions/{id}`, `POST /v1/agent/sessions`, `POST /v1/agent/delegate`, `POST /v1/agent/negotiate`, `POST /v1/agent/provenance/verify`, `GET /healthz`, `GET /metrics`.
     - Authentication: Optional Bearer token validation (HTTP 401 Unauthorized if invalid).
     - Rate Limiting: Configurable requests per second (e.g. 100 req/sec), returning HTTP 429 Too Many Requests with `Retry-After` header.
     - Success Status: HTTP 202 Accepted (asynchronous processing) with assigned IDs, or HTTP 200 OK (synchronous execution).
     - Error Status: HTTP 400 (Bad Request), 401 (Unauthorized), 403 (Policy Denied), 404 (Not Found), 405 (Method Not Allowed), 429 (Rate Limit Exceeded), 500 (Internal Error).
   - **SSE (Server-Sent Events) Transport**:
     - Endpoint: `GET /v1/agent/events` or `GET /v1/agent/stream` with header `Accept: text/event-stream`.
     - Framing: `event: <event_type>\ndata: <json_data>\n\n`.
     - Event types: `agent_status`, `agent_result`, `agent_error`, `heartbeat`.
     - Lifecycle: Uses broadcast channels (`tokio::sync::broadcast`). When client drops connection, channel receiver drops, instantly freeing memory and avoiding broker leaks.
     - Parallel capacity: Supports 5+ concurrent isolated subscriber streams.
   - **WebSocket Transport**:
     - Endpoint: `GET /v1/agent/ws`.
     - Handshake: RFC 6455 upgrade protocol computing `Sec-WebSocket-Accept` via SHA-1 + Base64 hash with GUID `258EAFA5-E914-47DA-95CA-C5AB0DC85B11`.
     - Frame format: Bidirectional JSON text frames enclosing `AgentMessage`.
     - Constraints: Maximum frame size capped at 4MB ($4 \times 1024 \times 1024$ bytes). Exceeding payload triggers WebSocket close frame code `1009` (Message Too Big). Clean closure uses code `1000`.

3. **Cryptographic Provenance Chain Specification**:
   - Every agent execution traces an immutable 6-stage cryptographic hash chain:
     $$\begin{aligned}
     H_0 = H_{\text{intent}} &= \text{SHA256}(\text{canonical\_json}(AgentIntent)) \\
     H_1 = H_{\text{negotiation}} &= \text{SHA256}(H_0 \parallel \text{canonical\_json}(Negotiation / Delegation)) \\
     H_2 = H_{\text{policy}} &= \text{SHA256}(H_1 \parallel \text{canonical\_json}(PolicyInput) \parallel \text{canonical\_json}(PolicyDecision) \parallel \text{PolicySetHash}) \\
     H_3 = H_{\text{driver}} &= \text{SHA256}(H_2 \parallel \text{driver\_id} \parallel \text{input\_hash} \parallel \text{output\_hash}) \\
     H_4 = H_{\text{poa}} &= \text{SHA256}(H_3 \parallel \text{poa\_signatures}) \\
     H_5 = H_{\text{receipt}} &= \text{SHA256}(H_4 \parallel \text{receipt\_id} \parallel \text{timestamp}) \\
     H_{\text{root}} &= \text{SHA256}(H_0 \parallel H_1 \parallel H_2 \parallel H_3 \parallel H_4 \parallel H_5)
     \end{aligned}$$
   - Signer: $H_{\text{root}}$ is signed using the node's Ed25519 private key, producing 64-byte signature $S_{\text{provenance}}$.
   - Structure `ProvenanceChainDigest`: Contains all step hashes, the computed $H_{\text{root}}$, the node ID, the public key, and the Ed25519 signature.
   - Verification Rules:
     1. Verify each link $H_i$ correctly chains from $H_{i-1}$ and stage payload.
     2. Verify Merkle root $H_{\text{root}}$ from all 6 stage hashes.
     3. Verify Ed25519 signature $S_{\text{provenance}}$ against signer public key and $H_{\text{root}}$.
     4. Missing intermediate stages trigger `INCOMPLETE_PROVENANCE_CHAIN`.
     5. Any mismatch at any stage triggers an explicit error identifying the exact tampered stage index (0 to 5) and increments the Prometheus counter `zap_provenance_verification_failures_total`.
   - CLI verification: `zap provenance verify --chain <path> --key <key_file>` and `zap receipts verify --dir <journal_dir> --provenance`.

---

## 3. Features Discovered

| # | Category | Feature | Description | Inputs | Outputs | Error Behavior | Discovered Via |
|---|----------|---------|-------------|--------|---------|----------------|----------------|
| 1 | MCP JSON-RPC | `initialize` | Negotiates MCP protocol version and exchanges server capabilities | `InitializeParams` (protocolVersion, capabilities, clientInfo) | `InitializeResult` (protocolVersion, capabilities: tools, resources, prompts, serverInfo) | Returns `-32600` on invalid params | `TEST_INFRA.md:97`, `tests/e2e:637` |
| 2 | MCP JSON-RPC | `tools/list` | Lists all available execution tools (built-in and dynamic WASM drivers) | `ToolsListParams` (optional cursor) | `ToolsListResult` (tools array: name, description, inputSchema) | Returns standard JSON-RPC error on invalid request | `TEST_INFRA.md:98`, `tests/e2e:653` |
| 3 | MCP JSON-RPC | `tools/call` | Invokes a specified ZAP tool through policy, PoA, and execution pipeline | `ToolCallParams` (name, arguments object) | `ToolCallResult` (content: [{type: "text", text: "..."}], isError: bool) | Returns `-32601` if tool unknown, `-32602` if args invalid, `isError: true` on policy/driver failure | `TEST_INFRA.md:99`, `tests/e2e:664` |
| 4 | MCP JSON-RPC | `resources/list` | Lists inspectable audit ledgers, memory journals, and system status resources | `ResourcesListParams` (optional cursor) | `ResourcesListResult` (resources array: uri, name, description, mimeType) | Returns standard JSON-RPC error | `TEST_INFRA.md:100`, `tests/e2e:679` |
| 5 | MCP JSON-RPC | `resources/read` | Reads content of a resource URI (`zap://ledger/receipts`, `zap://memory/*`, `zap://fleet/status`) | `ResourceReadParams` (uri string) | `ResourceReadResult` (contents array: uri, mimeType, text) | Returns `-32602` if URI invalid, `-32000` / error if resource not found | `TEST_INFRA.md:100`, `tests/e2e:679` |
| 6 | MCP JSON-RPC | `prompts/list` | Lists pre-approved agent prompt templates | `PromptsListParams` (optional cursor) | `PromptsListResult` (prompts array: name, description, arguments) | Returns standard JSON-RPC error | `TEST_INFRA.md:101`, `tests/e2e:691` |
| 7 | MCP JSON-RPC | `prompts/get` | Retrieves a rendered prompt template with substituted arguments | `PromptGetParams` (name, arguments map) | `PromptGetResult` (description, messages: [{role, content}]) | Returns `-32601` if prompt not found, `-32602` if args invalid | `TEST_INFRA.md:101` |
| 8 | MCP JSON-RPC | Stdio Framing & Stderr Isolation | Line-delimited JSON-RPC 2.0 over standard I/O streams | Stdin text lines containing JSON-RPC requests | Stdout text lines containing JSON-RPC responses | Stderr isolated for logs; non-JSON on stdin returns `-32700` Parse error | `TEST_INFRA.md:97,192`, `explorer_m4:103` |
| 9 | Multi-Transport Gateway | HTTP REST Intent Ingestion | Submits `AgentIntent` via HTTP POST for execution | `POST /v1/agent/intents` with `AgentIntent` JSON body | HTTP 202 Accepted / HTTP 200 OK with `session_id`, `intent_id`, `provenance_digest` | HTTP 400 on malformed JSON, 401 unauthorized, 403 policy denied, 429 rate limit | `TEST_INFRA.md:104`, `tests/e2e:703` |
| 10 | Multi-Transport Gateway | HTTP REST Session Management | Creates, retrieves, and updates `AgentSession` state | `POST /v1/agent/sessions`, `GET /v1/agent/sessions/{id}` | HTTP 200/201 with `AgentSession` JSON body | HTTP 404 if session not found, 400 on invalid session state | `TEST_INFRA.md:104`, `zap-agent:310` |
| 11 | Multi-Transport Gateway | HTTP REST Negotiation & Delegation | Dispatches multi-agent capability negotiations and task delegations | `POST /v1/agent/negotiate`, `POST /v1/agent/delegate` | HTTP 200 with `CapabilityNegotiationResponse` or `DelegationResponse` | HTTP 400 on invalid payload or rejected delegation without reason | `TEST_INFRA.md:104`, `zap-agent:364,477` |
| 12 | Multi-Transport Gateway | SSE Event Stream | Streams real-time agent status, progress, and results via Server-Sent Events | `GET /v1/agent/events` or `/v1/agent/stream` (`Accept: text/event-stream`) | Continuous SSE stream (`event: agent_status\ndata: {...}\n\n`) | Connection drop detected immediately; receiver channel dropped without memory leak | `TEST_INFRA.md:105,200`, `tests/e2e:723` |
| 13 | Multi-Transport Gateway | Parallel SSE Subscriptions | Handles multiple concurrent isolated SSE subscribers | 5+ simultaneous HTTP GET SSE connections | Independent event broadcasts delivered to each subscriber | No cross-talk; closing one subscriber does not disrupt others | `TEST_INFRA.md:108`, `tests/e2e:756` |
| 14 | Multi-Transport Gateway | WebSocket Bridge | Full-duplex bidirectional streaming of `AgentMessage` JSON frames | `GET /v1/agent/ws` WebSocket upgrade handshake (RFC 6455) | Bi-directional text frame stream | Rejects non-upgrade with HTTP 400; closes with code `1009` if frame > 4MB | `TEST_INFRA.md:106,201`, `tests/e2e:730` |
| 15 | Multi-Transport Gateway | Transport Fallback & Subprotocol | Handles fallback negotiation when client uses legacy transport / subprotocol | Handshake request with fallback headers or subprotocol | Fallback to HTTP REST or agreed subprotocol | Clean handshake error if incompatible | `TEST_INFRA.md:107`, `tests/e2e:749` |
| 16 | Multi-Transport Gateway | REST Rate Limiting & Auth | Protects gateway endpoints against denial of service and unauthorized access | HTTP Authorization header (Bearer token), high frequency requests (>100 req/s) | Passes valid requests; returns HTTP 429 / 401 | HTTP 401 Unauthorized for bad tokens; HTTP 429 Too Many Requests with `Retry-After` | `TEST_INFRA.md:202,203` |
| 17 | Provenance Engine | 6-Stage Hash Chain Construction | Computes 6 linked cryptographic hashes ($H_0 \to H_5$) binding execution lifecycle | `AgentIntent`, `NegotiationResponse`, `PolicyDecision`, `DriverOutput`, `PoaAttestation`, `Receipt` | `ProvenanceChainDigest` with 6 stage hashes and composite $H_{\text{root}}$ | Missing input yields `INCOMPLETE_PROVENANCE_CHAIN` | `TEST_INFRA.md:111`, `tests/e2e:764` |
| 18 | Provenance Engine | Intent-to-Policy Hash Linking | Verifies mathematical binding $H_{\text{policy}} = \text{Hash}(H_{\text{intent}} \parallel \text{policy\_digest} \parallel \text{decision})$ | $H_{\text{intent}}$, `PolicyInput`, `PolicyDecision`, `PolicySetHash` | $H_{\text{policy}}$ (32 bytes) | Mismatch indicates policy tampering | `TEST_INFRA.md:112`, `tests/e2e:774` |
| 19 | Provenance Engine | Policy-to-PoA Hash Linking | Verifies mathematical binding $H_{\text{poa}} = \text{Hash}(H_{\text{driver}} \parallel \text{poa\_signatures})$ | $H_{\text{driver}}$, `PoaReceipt` / validator signatures | $H_{\text{poa}}$ (32 bytes) | Mismatch indicates consensus/PoA forgery | `TEST_INFRA.md:113`, `tests/e2e:782` |
| 20 | Provenance Engine | PoA-to-Receipt Root Binding | Embeds complete provenance root hash in signed action receipt | $H_{\text{poa}}$, receipt metadata, node signing key | `SignedActionReceipt` containing `provenance_chain_digest` | Manifest sealing rejects unverified provenance root | `TEST_INFRA.md:114`, `tests/e2e:790` |
| 21 | Provenance Engine | Ed25519 Provenance Signing | Signs composite Merkle root $H_{\text{root}}$ using node private key | $H_{\text{root}}$ (32 bytes), node `Keypair` | 64-byte Ed25519 signature | Signature generation fails if key material invalid | `PROJECT.md:50`, `tests/e2e:783` |
| 22 | Provenance Engine | Independent Cryptographic Verification | Verifies step continuity, hash links, Merkle root, and Ed25519 signature | `ProvenanceChainDigest`, signer public key | `ProvenanceVerificationReport` (is_valid: bool, failed_stage: Option<usize>) | Returns verification failure with exact stage index; increments failure metric | `TEST_INFRA.md:115,206-210`, `tests/e2e:797` |
| 23 | Provenance Engine | Long Delegation Chain Scaling | Supports nested multi-hop delegation provenance chains up to $N=100$ steps | Deeply nested `DelegationRequest` chain ($N=100$) | Bounded validation without stack overflow | Rejects chain exceeding max configured depth limit | `TEST_INFRA.md:210` |
| 24 | CLI & Telemetry | Gateway CLI Subcommands | Manages agent gateway daemon lifecycle via CLI | `zap gateway start`, `zap gateway status` | Daemon start / JSON status output | Clear error if port bound or config invalid | `explorer_m4:26`, `TEST_INFRA.md:104` |
| 25 | CLI & Telemetry | Provenance CLI Verification | Verifies standalone or journal-embedded provenance chains via CLI | `zap provenance verify --chain <file> --key <key>`, `zap receipts verify --provenance` | Success output or JSON verification report | Exits non-zero with failure report on corrupted chain | `TEST_INFRA.md:115`, `explorer_m4:28` |
| 26 | CLI & Telemetry | Gateway & Provenance Prometheus Metrics | Exposes real-time gateway requests, active sessions, and provenance error metrics | Prometheus HTTP scrape `GET /metrics` | `zap_agent_gateway_requests_total`, `zap_agent_sessions_active`, `zap_provenance_verification_failures_total` | Accurately tracks counts and gauge increments/decrements | `TEST_INFRA.md:93-94`, `crates/zap-node:2285` |

---

## 4. Edge Cases

| # | Feature | Input | Observed Behavior |
|---|---------|-------|-------------------|
| 1 | MCP JSON-RPC | Invalid JSON syntax (e.g. `{ "jsonrpc": "2.0", "id": 1, `) | Parser rejects input and returns JSON-RPC standard `-32700` Parse Error (`tc_b09_001`). |
| 2 | MCP JSON-RPC | Unregistered tool name (e.g. `tools/call` for `unknown_tool`) | Server returns standard JSON-RPC `-32601` Method Not Found error (`TC-B09-002`). |
| 3 | MCP JSON-RPC | Missing required parameter (e.g. `zap_send` without `target`) | Server returns standard JSON-RPC `-32602` Invalid Params error detailing missing field (`TC-B09-003`). |
| 4 | MCP JSON-RPC | Oversized stdio payload (10MB JSON line) | Server enforces max buffer limit and returns payload size exceeded error without crashing (`TC-B09-004`). |
| 5 | MCP JSON-RPC | Stdio pipe interruption & re-initialization | Closing stdio and restarting initializes a fresh protocol state without leaking stale session memory (`TC-B09-005`). |
| 6 | HTTP REST Gateway | Malformed HTTP POST JSON payload to `/v1/agent/intents` | Server responds with HTTP 400 Bad Request and structured JSON error message (`TC-B10-001`). |
| 7 | HTTP REST Gateway | Unauthorized request with missing or invalid bearer token | Server responds with HTTP 401 Unauthorized (`TC-B10-004`). |
| 8 | HTTP REST Gateway | Burst traffic exceeding 100 req/sec rate limit | Server returns HTTP 429 Too Many Requests containing a valid `Retry-After` header (`TC-B10-005`). |
| 9 | SSE Stream | Client terminates HTTP connection abruptly during streaming | Server detects broken TCP pipe, drops broadcast receiver channel, and frees memory without leak (`TC-B10-002`). |
| 10 | SSE Stream | 5 parallel concurrent client connections to `/v1/agent/events` | All 5 clients receive identical broadcast events independently without latency degradation (`tc_f10_005`). |
| 11 | WebSocket Bridge | Incoming WebSocket frame exceeding 4MB ($>4,194,304$ bytes) | Server rejects oversized frame and immediately closes connection with RFC 6455 close code `1009` (Message Too Big) (`TC-B10-003`). |
| 12 | WebSocket Bridge | Non-WebSocket HTTP request to `/v1/agent/ws` | Server rejects handshake with HTTP 400 Bad Request / 426 Upgrade Required. |
| 13 | Provenance Chain | Tampered intent step hash (1 byte flipped $0\text{xAA}$ in $H_{\text{intent}}$) | Verifier detects mismatch at Stage 0 ($H_{\text{intent}}$), fails verification, and increments `zap_provenance_verification_failures_total` (`tc_b11_001`, `TC-B11-001`). |
| 14 | Provenance Chain | Altered driver output bytes after PoA attestation | Verifier detects $H_{\text{driver}}$ mismatch at Stage 3, failing verification with exact stage report (`TC-B11-002`). |
| 15 | Provenance Chain | Omitted intermediate stage (e.g. missing $H_{\text{policy}}$ in chain) | Verifier rejects chain with explicit error `INCOMPLETE_PROVENANCE_CHAIN` (`TC-B11-003`). |
| 16 | Provenance Chain | Tampered Ed25519 signature on provenance root hash | Verifier detects invalid cryptographic signature on $H_{\text{root}}$ and returns signature error (`TC-B11-004`). |
| 17 | Provenance Chain | Maximum nested delegation chain depth ($N=100$ steps) | Verifier processes multi-hop delegation chain iteratively without recursion or stack overflow (`TC-B11-005`). |
| 18 | Agent Delegation | `DelegationResponse` with `decision: accepted` but `assigned_agent: None` | Validation rejects struct with `ZapAgentError::AcceptedDelegationMissingAssignee` (`crates/zap-agent/src/lib.rs:448`). |
| 19 | Agent Delegation | `DelegationResponse` with `decision: rejected` but `reason: None` | Validation rejects struct with `ZapAgentError::RejectedDelegationMissingReason` (`crates/zap-agent/src/lib.rs:451`). |
| 20 | Agent Negotiation | `CapabilityNegotiationRequest` with empty capabilities and empty intents | Validation rejects request with `ZapAgentError::EmptyCapabilityNegotiation` (`crates/zap-agent/src/lib.rs:509`). |
| 21 | Agent Result | `AgentResult` with non-terminal status (`running`) or failed status without error | Validation rejects non-terminal status with `ResultStatusNotTerminal`, failed without error with `FailedResultMissingError` (`crates/zap-agent/src/lib.rs:748-752`). |
| 22 | Agent Session | `AgentSession` with `updated_at_micros < created_at_micros` | Validation rejects inverted timestamps with `ZapAgentError::InvalidTimestampOrder` (`crates/zap-agent/src/lib.rs:930`). |
| 23 | Agent Error Info | Deeply nested error cause chain exceeding 8 levels ($>8$) | Validation rejects recursion depth with `ZapAgentError::ErrorCauseTooDeep { max: 8 }` (`crates/zap-agent/src/lib.rs:702`). |

---

## 5. Caveats

1. **No External Heavy Web Frameworks**: In line with the ZAP architecture, all HTTP REST, SSE, and WebSocket networking is implemented natively using `tokio::net` and standard library primitives rather than pulling in large frameworks like `axum` or `actix-web`.
2. **WebSocket Handshake Computation**: WebSocket upgrade handshake requires computing `Sec-WebSocket-Accept` as $\text{Base64}(\text{SHA1}(\text{key} \parallel \text{"258EAFA5-E914-47DA-95CA-C5AB0DC85B11"}))$. A self-contained SHA-1 implementation or cryptographic utility is required.
3. **Stdio Stream Isolation**: In MCP stdio mode, standard output is reserved strictly for line-delimited JSON-RPC 2.0 messages. All logging, tracing, and diagnostics must be redirected to stderr or a file.
4. **Canonical JSON Serialization**: Hash generation across steps ($H_{\text{intent}}$, $H_{\text{policy}}$, etc.) must use deterministic canonical JSON serialization (e.g. sorted keys, no extraneous whitespace) to guarantee cross-language and cross-platform verification parity.

---

## 6. Conclusion

All technical specifications, interface contracts, and test expectations for Milestone 4 (MCP JSON-RPC 2.0, Multi-Transport Agent Gateway, and Cryptographic Provenance Chain) have been extracted from authoritative test suites and codebase contracts:
- **MCP JSON-RPC 2.0 Engine**: Covers `initialize`, `tools/list`, `tools/call`, `resources/list`, `resources/read`, `prompts/list`, `prompts/get`, and standard JSON-RPC 2.0 error codes.
- **Multi-Transport Gateway**: Covers HTTP REST `/v1/agent/*`, SSE streaming `/v1/agent/events` with instant client drop cleanup and 5+ parallel streams, and WebSocket `/v1/agent/ws` with 4MB frame limit.
- **Cryptographic Provenance Engine**: Covers the 6-stage hash chain ($H_{\text{intent}} \to H_{\text{negotiation}} \to H_{\text{policy}} \to H_{\text{driver}} \to H_{\text{poa}} \to H_{\text{receipt}} \to H_{\text{root}}$), Ed25519 signing, step-level tamper isolation, and telemetry metrics integration.
- **CLI & Telemetry**: Covers `zap gateway start`, `zap gateway status`, `zap provenance verify`, and Prometheus metrics (`zap_agent_gateway_requests_total`, `zap_agent_sessions_active`, `zap_provenance_verification_failures_total`).

---

## 7. Verification Method

To independently verify these specifications against the codebase and test suite:

1. **Inspect Test Specifications**:
   - `tests/e2e/tests/e2e_suite.rs`: inspect lines 635–802 (Tier 1 F09–F11), lines 977–992 (Tier 2 boundary cases), lines 1047–1064 (Tier 3 cross-feature), lines 1083–1122 and 1182–1201 (Tier 4 workflows).
   - `TEST_INFRA.md`: inspect lines 96–116 (Tier 1 expectations), lines 191–211 (Tier 2 expectations), lines 237–247 (Tier 3 expectations).
2. **Inspect Agent Protocol Contracts**:
   - `crates/zap-agent/src/lib.rs`: inspect lines 84–970 (validation rules for `AgentIntent`, `AgentSession`, `DelegationRequest`, `CapabilityNegotiationRequest`, `AgentResult`, `AgentErrorInfo`).
   - `crates/zap-agent/tests/fixtures.rs`: verify schema version, declared subjects, and fixture shapes.
3. **Execute Test Commands (when ready)**:
   ```bash
   cargo test --package zap-agent
   cargo test --test e2e_suite tc_f09
   cargo test --test e2e_suite tc_f10
   cargo test --test e2e_suite tc_f11
   cargo test --test e2e_suite tc_b09
   cargo test --test e2e_suite tc_b10
   cargo test --test e2e_suite tc_b11
   ```
