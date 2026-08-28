# Investigation Report: Milestone 4 (AI Agent Gateway & MCP Server)

## 1. Observation

Direct code observations from the repository inspection:

### 1.1 Crate Layout and Workspace Architecture
- **`Cargo.toml` Workspace Members** (`Cargo.toml:1-28`):
  - Currently includes: `crates/rivun-agent`, `crates/rivun-capability`, `crates/rivun-cli`, `crates/rivun-core`, `crates/rivun-crypto`, `crates/rivun-driver-sdk`, `crates/rivun-envelope`, `crates/rivun-journal`, `crates/rivun-ledger`, `crates/rivun-machine`, `crates/rivun-memory`, `crates/rivun-net`, `crates/rivun-node`, `crates/rivun-ops`, `crates/rivun-pact`, `crates/rivun-pack`, `crates/rivun-policy`, `crates/rivun-router`, `crates/rivun-runtime`, `crates/rivun-schema`, `crates/rivun-store`, `crates/rivun-telemetry`, `examples`, `tools/xtask`, `tests/e2e`.
  - **`crates/rivun-gateway` does NOT exist** on the filesystem and is missing from `Cargo.toml`.
- **`crates/rivun-agent`** (`crates/rivun-agent/src/lib.rs:1-1190`):
  - Implements core agent protocol models and serialization: `AgentIntent`, `AgentSession`, `DelegationRequest`, `DelegationResponse`, `CapabilityNegotiationRequest`, `CapabilityNegotiationResponse`, `AgentStatusUpdate`, `AgentResult`, `AgentErrorReport`, `AgentMessage`, and validation routines (`Validate`).
  - Supported subjects: `rivun.agent.intent`, `rivun.agent.session`, `rivun.agent.delegation.request`, `rivun.agent.delegation.response`, `rivun.agent.capability_negotiation.request`, `rivun.agent.capability_negotiation.response`, `rivun.agent.status`, `rivun.agent.result`, `rivun.agent.error`.
  - **Missing**: No MCP protocol implementation, no transport gateway, and no `ProvenanceChainDigest` cryptographic struct or verification logic.

### 1.2 MCP Server Implementation (Feature 9)
- **Status in Codebase**: Completely absent.
- **Requirements from `PROJECT.md` & `ORIGINAL_REQUEST.md` (R4, F09)**:
  - JSON-RPC 2.0 protocol endpoint over stdio / HTTP SSE.
  - Mandatory methods:
    1. `initialize`: Returns protocol version (`2024-11-05`), server capabilities (`tools`, `resources`, `prompts`), and server info (`name: "rivun-gateway"`, `version: "0.1.0"`).
    2. `tools/list`: Exposes rivun execution capabilities as callable tools:
       - `@@rivun_HEADER@@send`: Send typed messages / envelopes to rivun nodes.
       - `@@rivun_HEADER@@query`: Query node status, capabilities, or journal.
       - `@@rivun_HEADER@@agent_intent`: Submit an `AgentIntent` through the policy and driver pipeline.
       - `@@rivun_HEADER@@receipts_verify`: Verify cryptographic receipts and provenance chains.
    3. `tools/call`: Invokes a named tool with JSON parameters, enforcing policy checks and returning structured tool execution results or JSON-RPC errors (`-32700` parse error, `-32601` method not found, `-32602` invalid params, `-32603` internal error).
    4. `resources/list` & `resources/read`: Exposes read-only audit streams and URIs:
       - `rivun://ledger/receipts`: Read recent action receipts.
       - `rivun://node/status`: Read node health, peer count, and runtime stats.
       - `rivun://fleet/topology`: Read discovered cluster peer nodes.
    5. `prompts/list` & `prompts/get`: Exposes parameterized prompt templates for LLM agents (e.g. goal decomposition, capability negotiation, and safe execution verification).

### 1.3 Multi-Transport Agent Gateway (Feature 10)
- **Status in Codebase**: Completely absent.
- **Requirements from `PROJECT.md` & `ORIGINAL_REQUEST.md` (R4, F10)**:
  - **HTTP REST API**:
    - `POST /v1/agent/intents`: Ingests `AgentIntent`, validates payload, evaluates policy, creates session if needed, triggers driver execution or routing, returns `202 Accepted` or `200 OK` with session ID and intent ID.
    - `GET /v1/agent/sessions/{session_id}` / `POST /v1/agent/sessions`: Inspect or initialize agent sessions.
    - `GET /v1/agent/receipts`: Query ledger receipts by session or intent.
    - Standard status codes: `200 OK`, `202 Accepted`, `400 Bad Request` (schema failure), `401 Unauthorized`, `403 Forbidden` (policy denied), `429 Too Many Requests` (rate limited).
  - **SSE (Server-Sent Events) Streaming**:
    - `GET /v1/agent/events` / `GET /v1/agent/events?session_id={id}`: Content-Type `text/event-stream`.
    - Real-time event streaming (`event: agent_status\ndata: ...\n\n`, `event: agent_result\ndata: ...\n\n`).
    - Clean disconnection handling and multi-client broadcast without memory leaks.
  - **WebSocket Bridge**:
    - Route `GET /v1/agent/ws`: Bi-directional full-duplex message stream for agent runtimes.
    - Frame framing supporting text JSON `AgentMessage` payloads.
    - Enforces maximum frame size (e.g. 4MB, sending close code `1009 Message Too Big` if exceeded).
    - Subprotocol negotiation and graceful fallback.

### 1.4 Provenance Chain Cryptographic Linking (Feature 11)
- **Status in Codebase**:
  - `@@rivun_HEADER@@telemetry` (`metrics.rs:56, 261`) and `@@rivun_HEADER@@node` (`lib.rs:1521, 2306`) declare counter `provenance_verification_failures_total`.
  - However, `ProvenanceChainDigest` data structure, cryptographic hash linking ($H_0 \to H_1 \to H_2 \to H_3 \to H_4 \to H_5$), and verification routines are completely missing.
- **Mathematical Cryptographic Chain Formulation**:
  - Six sequential stages:
    1. $H_{\text{intent}} = \text{BLAKE3/SHA256}(\text{canonical\_json}(\text{AgentIntent}))$
    2. $H_{\text{negotiation}} = \text{BLAKE3/SHA256}(H_{\text{intent}} \parallel \text{canonical\_json}(\text{CapabilityNegotiation}))$
    3. $H_{\text{policy}} = \text{BLAKE3/SHA256}(H_{\text{negotiation}} \parallel \text{canonical\_json}(\text{PolicyEvaluation}))$
    4. $H_{\text{driver}} = \text{BLAKE3/SHA256}(H_{\text{policy}} \parallel \text{driver\_output\_bytes})$
    5. $H_{\text{poa}} = \text{BLAKE3/SHA256}(H_{\text{driver}} \parallel \text{poa\_attestations\_bytes})$
    6. $H_{\text{receipt}} = \text{BLAKE3/SHA256}(H_{\text{poa}} \parallel \text{canonical\_json}(\text{ActionReceipt}))$
  - **Chain Root & Sealing**: $H_{\text{root}} = H_{\text{receipt}}$, signed by node Ed25519 signing key or embedded in `SignedActionReceipt`.
  - **Verification Properties**:
    - Tamper detection: Altering any intermediate stage ($H_{\text{intent}}$, negotiation, policy, driver output, PoA) breaks subsequent link hashes.
    - Incomplete chain detection: Missing intermediate steps fail with `IncompleteProvenanceChain` error.
    - Telemetry linkage: Verification failures increment `provenance_verification_failures_total`.

### 1.5 Existing E2E & Unit Test Suite
- **`tests/e2e/tests/e2e_suite.rs`**:
  - Contains test cases for F09 (`tc_f09_001`..`005`), F10 (`tc_f10_001`..`005`), F11 (`tc_f11_001`..`005`), boundary cases `tc_b09_001`, `tc_b11_001`, cross-feature `tc_x_006`, and real-world `tc_rw_001`.
  - Currently, these test cases contain superficial placeholder assertions (e.g. checking JSON literal keys, comparing hardcoded strings like `"event: agent_status"`, or hashing dummy strings).
  - Several tests (`tc_f10_001`, `tc_rw_001`) use struct field patterns that do not match `crates/rivun-agent/src/lib.rs` (e.g. `session_id: Some(Uuid)` vs `session_id: Uuid`, `IntentPriority` vs `Priority`).

---

## 2. Logic Chain

1. **Requirement R4 & PROJECT.md Interface Contracts**:
   - `ORIGINAL_REQUEST.md` (R4) demands MCP and streaming/HTTP/WebSocket bridge interfaces connecting LLM agent frameworks to rivun's deterministic policy, PoA, and signed receipt ledger, plus strict cryptographic provenance linking.
   - `PROJECT.md` Section "Interface Contracts" specifies:
     - `rivun-agent ↔ rivun-gateway (Provenance Chain)`: `ProvenanceChainDigest`: SHA256/Ed25519 chain ($H_{\text{intent}} \to H_{\text{negotiation}} \to H_{\text{policy}} \to H_{\text{driver}} \to H_{\text{poa}} \to H_{\text{receipt}}$).
     - `crates/rivun-agent/` & `crates/rivun-gateway/`: Agent protocol, MCP server, transport bridge.

2. **From Observations to Root Implementation Needs**:
   - **Step 1 — Create `rivun-gateway` crate**: Create `crates/rivun-gateway` (or module within workspace) and register in root `Cargo.toml` workspace members and dependencies.
   - **Step 2 — Implement Provenance Chain in `rivun-agent` (or `rivun-ledger`)**: Define `ProvenanceChainDigest`, `ProvenanceLink`, `ProvenanceStage`, step-by-step verification method `ProvenanceChainDigest::verify(...)`, and tamper detection logic.
   - **Step 3 — Implement JSON-RPC 2.0 MCP Server in `rivun-gateway`**:
     - JSON-RPC 2.0 wire protocol handling over stdio and SSE.
     - Implement `initialize`, `tools/list`, `tools/call`, `resources/list`, `resources/read`, `prompts/list`, `prompts/get`.
     - Connect `tools/call` to `rivun-node` / `rivun-policy` / `rivun-ledger` execution pipelines.
   - **Step 4 — Implement Multi-Transport HTTP / SSE / WebSocket Gateway in `rivun-gateway`**:
     - REST HTTP router for `/v1/agent/intents`, `/v1/agent/sessions`, `/v1/agent/receipts`.
     - SSE stream handler for `/v1/agent/events` streaming status and execution results.
     - WebSocket handler for `/v1/agent/ws` with framing, ping/pong, max frame size limits, and `AgentMessage` duplex exchange.
   - **Step 5 — Upgrade E2E & Unit Test Suites**:
     - Upgrade `tc_f09_001`..`005`, `tc_f10_001`..`005`, `tc_f11_001`..`005`, boundary tests, and `tc_rw_001` in `tests/e2e` to execute actual MCP calls, HTTP REST requests, SSE streams, WebSocket frames, and full cryptographic provenance chain verifications.

---

## 3. Caveats

1. **Standalone Crate vs Internal Modules**:
   - `PROJECT.md` specifies `crates/rivun-gateway` alongside `crates/rivun-agent`. Creating `crates/rivun-gateway` as a clean workspace crate ensures separation of concerns (protocol types in `rivun-agent`, network/transport/server engines in `rivun-gateway`).
2. **Network Transport Dependencies**:
   - To maintain fast compilation and minimal dependencies, HTTP/SSE/WebSocket server components should use lightweight async primitives (such as `tokio::net::TcpListener`, `tokio-stream`, or standard hyper/axum if compatible with workspace dependencies).
3. **Driver Execution Sandbox Interactivity**:
   - In MCP `tools/call` and Gateway REST intent execution, if no live WASM runtime is configured, the gateway should cleanly execute mock/fallback or deterministic simulated drivers, returning signed receipts with valid cryptographic provenance chains.

---

## 4. Conclusion

Milestone 4 requires four primary deliverables across `crates/rivun-agent`, `crates/rivun-gateway`, and `tests/e2e`:

1. **`crates/rivun-gateway` Creation**:
   - Crate initialization in `Cargo.toml`.
   - `McpServer`: JSON-RPC 2.0 server supporting `initialize`, `tools/list`, `tools/call`, `resources/list`, `resources/read`, `prompts/list`, `prompts/get`.
   - `AgentGateway`: Multi-transport server supporting HTTP REST (`/v1/agent/intents`, `/v1/agent/sessions`), SSE streaming (`/v1/agent/events`), and WebSocket (`/v1/agent/ws`).
2. **`ProvenanceChainDigest` in `crates/rivun-agent`**:
   - Cryptographic linking struct:
     ```rust
     pub struct ProvenanceChainDigest {
         pub intent_hash: String,        // H_0
         pub negotiation_hash: String,   // H_1
         pub policy_hash: String,        // H_2
         pub driver_hash: String,        // H_3
         pub poa_hash: String,           // H_4
         pub receipt_hash: String,       // H_5
         pub chain_root: String,
         pub node_signature: Option<String>,
     }
     ```
   - Step verification: `verify_step(...)`, `verify_chain(...)`, returning specific errors on broken links.
3. **Integration with `rivun-node` and `rivun-ledger`**:
   - Linking intent submission through capability negotiation, policy check, driver execution, PoA attestation, and receipt writing with complete `ProvenanceChainDigest`.
4. **E2E & Unit Test Hardening**:
   - Replace dummy assertions in `tests/e2e/tests/e2e_suite.rs` with functional assertions for all 15 M4 test cases (Tiers 1, 2, 3, 4).

---

## 5. Verification Method

To verify Milestone 4 implementations:

1. **Unit Tests**:
   - `cargo test -p rivun-agent --lib` — verifies agent protocol models, serialization, and `ProvenanceChainDigest` cryptographic link verification.
   - `cargo test -p rivun-gateway --lib` — verifies MCP JSON-RPC 2.0 dispatch, HTTP REST request handlers, SSE framing, and WebSocket bridge.
2. **E2E Integration Tests**:
   - `cargo test --package rivun-e2e --test e2e -- tc_f09` — verifies MCP server initialize, tools, resources, prompts.
   - `cargo test --package rivun-e2e --test e2e -- tc_f10` — verifies REST intent submission, SSE event streaming, WebSocket bridge.
   - `cargo test --package rivun-e2e --test e2e -- tc_f11` — verifies 6-step cryptographic provenance chain generation, link tampering detection, and verification.
   - `cargo test --package rivun-e2e --test e2e -- tc_rw_001` — verifies full distributed AI agent workflow from intent to signed receipt.
3. **Workspace Validation**:
   - `cargo clippy --workspace --all-targets -- -D warnings` — zero warnings.
   - `cargo test --workspace --all-targets` — 100% pass across all crates.

