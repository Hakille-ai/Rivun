# Handoff Report: R3 (Fleet Topology, Health & Incident Telemetry) & R4 (AI Agent Gateway & Multi-Transport Integration)

**Agent ID**: `teamwork_preview_spec_miner_survey_2`  
**Working Directory**: `c:\Users\Stagiaire\Documents\Amadou PGC\Prs\rivun\.agents\teamwork_preview_spec_miner_survey_2`  
**Date**: 2026-08-14T01:33:07Z  

---

## 1. Observation

A full static and dynamic audit of the rivun codebase (`crates/`, `docs/`, `fixtures/`, `tools/`, `website/`) was performed to evaluate current capabilities, interfaces, CLI subcommands, protocol specifications, and gaps for Requirements **R3** and **R4**.

### Key Code & Specs Inspected:
- `crates/rivun-cli/src/main.rs`: Main CLI parser containing 24 command groups (`Keygen`, `Run`, `CheckConfig`, `Doctor`, `Send`, `Inspect`, `Capability`, `Discovery`, `Memory`, `Route`, `Trust`, `Peer`, `Schema`, `Agent`, `Pact`, `Policy`, `Pack`, `Fixtures`, `DriverManifest`, `Registry`, `Receipts`, `Incident`, `Poa`, `Bench`).
  - **Observation 1**: `Commands` enum in `rivun-cli/src/main.rs` lines 91-276 contains `Doctor` (single-node) and `Incident` (`Snapshot` subcommand), but **lacks a `Fleet` command group** (`rivun fleet doctor`, `rivun fleet topology`, `rivun fleet list`).
  - **Observation 2**: `incident_snapshot` in `rivun-cli/src/main.rs` lines 3531-3534 explicitly lists limitations: *"snapshot omits key material... runtime process state, network captures, and live /metrics HTTP output are not collected"*.
- `crates/rivun-node/src/lib.rs`: In-process node implementation providing metrics via `metrics_prometheus_text()` and health via `health_snapshot()`, `health_json()`, `healthz_text()`.
  - **Observation 3**: `metrics_prometheus_text()` emits 8 metrics: `@@rivun_HEADER@@frames_sent_total`, `@@rivun_HEADER@@frames_received_total`, `@@rivun_HEADER@@frames_rejected_total`, `@@rivun_HEADER@@driver_execution_errors_total`, `@@rivun_HEADER@@peer_trust_status`, `@@rivun_HEADER@@registry_signature_valid`, `@@rivun_HEADER@@receipt_log_verify_failures_total`, `@@rivun_HEADER@@capability_cache_age_seconds`, and `@@rivun_HEADER@@poa_attestation_failures_total`.
  - **Observation 4**: Lacks driver execution duration histograms, policy evaluation counters, PoA attestation total counters, replay duplicate counters, journal segment rotation counters, active peer gauges, and MCP gateway metrics.
- `crates/rivun-agent/src/lib.rs`: Agent protocol schema definitions for `AgentIntent`, `AgentSession`, `DelegationRequest`, `DelegationResponse`, `CapabilityNegotiationRequest`, `CapabilityNegotiationResponse`, `AgentStatusUpdate`, `AgentResult`, `AgentErrorReport`.
  - **Observation 5**: `rivun-agent` contracts are schema/validation only; there is **no runtime gateway daemon, HTTP endpoint, WebSocket server, or MCP server** bridging LLM frameworks to `rivun-agent` messages.
- `crates/rivun-ledger/src/lib.rs`: Signed action receipts (`ActionReceipt`, `SignedActionReceipt`, `PoaReceipt`, `PactReceiptReference`).
  - **Observation 6**: `ActionReceipt` carries frame hash, payload hash, output hash, POA receipt, and PACT reference, but **lacks explicit provenance tracking fields** linking back to `session_id`, `intent_id`, `negotiation_id`, `policy_set_hash`, `driver_manifest_hash`, or an aggregated `provenance_chain_digest`.
- Documentation (`docs/observability.md`, `docs/operations.md`, `docs/agent-protocol.md`, `docs/pact.md`, `docs/discovery.md`, `docs/roadmap-status.md`, `docs/roadmap.md`).
  - **Observation 7**: `docs/roadmap-status.md` line 76 explicitly lists: *"Fleet doctor | planned | Add fleet topology inspection across peers"*, and line 77 lists: *"Stream/gateway transports | planned | Add HTTP, WebSocket, gRPC, MCP bridges"*.

---

## 2. Logic Chain

1. **Assessment of R3 (Fleet Topology, Health & Incident Telemetry)**:
   - *Topology*: Currently, discovery operates on single service announcements (`DiscoveryQuery` / `DiscoveryResponse`). A multi-node operator has no single CLI interface to discover and view the aggregated multi-node fleet topology graph (nodes, UDP addresses, public keys, trust states, capability routes, validator sets).
   - *Fleet Doctor*: While `ZapNode` exposes single-node `/healthz.json` and `rivun doctor` checks local TOML config, there is no cluster-wide `rivun fleet doctor` engine to query all active fleet peers in parallel, detect cross-node posture issues (clock skew, stale capability caches, missing validator attestations), or output an aggregated `ZapFleetHealthSnapshot`.
   - *Incident Snapshot*: The existing `rivun incident snapshot` subcommand captures local config summary, local doctor report, local memory, receipt, and capability cache summaries. However, it lacks live process state (CPU/RAM/FDs), live Prometheus telemetry, active socket state, and fleet-wide multi-node peer snapshots.
   - *Prometheus Metrics*: 8 core metrics exist, but high-value operational metrics (driver execution latency histograms, policy evaluation counters, replay duplicate counters, journal rotation counters, active peer counts) are absent.

2. **Assessment of R4 (AI Agent Gateway & Multi-Transport Integration)**:
   - *AI Agent Protocol*: `rivun-agent` provides robust JSON schemas and validation for agent communication (`rivun.agent.*` subjects). However, these exist in isolation without a runtime gateway.
   - *MCP Integration*: Model Context Protocol (JSON-RPC 2.0 over stdio/SSE/WebSocket) is unimplemented. LLM agents (Claude Desktop, Cursor, LangChain, LlamaIndex, AutoGen) currently cannot discover rivun capabilities as MCP Tools or inspect memory/receipts as MCP Resources.
   - *Multi-Transport Bridge*: rivun nodes only bind a UDP transport and a simple HTTP metrics/health listener. No HTTP REST API (`/v1/agent/*`), WebSocket endpoint (`/v1/agent/ws`), or SSE streaming endpoint (`/v1/agent/session/{id}/stream`) exists for external AI agent integration.
   - *Provenential Evidence Chain*: While `rivun-policy` evaluates policy, `rivun-crypto` handles PoA, `rivun-runtime` executes drivers, and `rivun-ledger` writes signed receipts, there is no end-to-end provenance pipeline binding `AgentIntent` -> `CapabilityNegotiation` -> `PolicyEvaluation` -> `WASM Execution` -> `SignedActionReceipt` into a cryptographic `provenance_chain_digest`.

---

## 3. Caveats

- **Read-Only Scope**: This report is produced by a Specification Miner agent. No implementation files or production code were modified during this turn.
- **Integration Dependencies**: The proposed interfaces for R4 (MCP / HTTP / WebSocket gateway) build directly on the existing `rivun-agent`, `rivun-policy`, `rivun-ledger`, and `rivun-runtime` crates without requiring changes to the binary `ZENV` wire frame format.
- **Network Environment**: Multi-node fleet discovery relies on standard UDP transport and control frame dispatch across configured or discovered peers.

---

## 4. Conclusion

- **Requirement R3** requires creating a `rivun fleet` subcommand group (`rivun fleet doctor`, `rivun fleet topology`, `rivun fleet list`), expanding `rivun incident snapshot` to capture live process metrics, Prometheus metrics, and peer snapshot queries, and adding 7 missing metrics to `metrics_prometheus_text()` for full Prometheus parity.
- **Requirement R4** requires creating a new `rivun-gateway` / `rivun-mcp` runtime daemon / module with:
  1. An MCP server (Stdio, SSE, WebSocket) mapping rivun WASM capabilities to MCP Tools, memory to MCP Resources, and sessions to MCP Prompts.
  2. Multi-transport bridge interfaces (HTTP REST, WebSocket, SSE streaming).
  3. Automated pipeline connecting Agent Intent -> Capability Negotiation -> Policy Evaluation -> PoA Attestation -> WASM Driver Execution -> Receipt Logging.
  4. Cryptographic provenance linkage (`provenance_chain_digest`) in `SignedActionReceipt`.

---

## 5. Verification Method

- **CLI Inspection Verification**:
  - `cargo run -p rivun-cli -- --help` (verify available CLI subcommands)
  - `cargo run -p rivun-cli -- doctor --help` (verify single-node doctor interface)
  - `cargo run -p rivun-cli -- incident snapshot --help` (verify current incident snapshot parameters)
  - `cargo run -p rivun-cli -- agent --help` (verify existing agent protocol builders)
- **Unit & Integration Test Verification**:
  - `cargo test -p rivun-node --lib` (verify existing health snapshot and Prometheus text metrics)
  - `cargo test -p rivun-agent --lib` (verify agent protocol schemas and validation)
  - `cargo test -p rivun-ledger --lib` (verify receipt record structure and validation)
  - `cargo test -p rivun-policy --lib` (verify deterministic policy evaluation rules)

---

## Features Discovered

| # | Category | Feature | Description | Inputs | Outputs | Error Behavior | Discovered Via |
|---|----------|---------|-------------|--------|---------|----------------|----------------|
| 1 | R3 Telemetry | `ZapNode::health_snapshot()` | Evaluates 8 local checks (`endpoint_bound`, `registry_signature`, `registry_bundle`, `receipt_log`, `capability_cache`, `message_policy`, `peer_trust`, `runtime_error`) | In-process node state | `ZapNodeHealthSnapshot` (`healthy`, `degraded`, `critical`) | Returns `critical` when registry signature invalid, receipt log corrupted, or peer revoked | `crates/rivun-node/src/lib.rs` |
| 2 | R3 Telemetry | Observability HTTP Listener | HTTP endpoint serving Prometheus metrics and text/JSON health snapshots | `GET /metrics`, `GET /healthz`, `GET /healthz.json` | Prometheus text, plain text health status, JSON health snapshot | Returns HTTP 503 Service Unavailable when status is `critical`; 200 OK for `healthy`/`degraded` | `docs/observability.md`, `crates/rivun-node/src/lib.rs` |
| 3 | R3 Diagnostic | `rivun doctor` | Single-node operator readiness diagnostic for node TOML config | `--config <path>`, `--json`, `--strict` | `DoctorReport` (score, checks, warnings) | Non-zero exit code on failure, or with `--strict` when warnings exist | `crates/rivun-cli/src/main.rs` |
| 4 | R3 Incident | `rivun incident snapshot` | Bounded diagnostic snapshot for incident triage | `--config`, `--memory`, `--receipts`, `--capability-cache`, `--out` | Bounded `IncidentSnapshot` JSON file | Returns JSON with `valid: false` if any component verification fails | `crates/rivun-cli/src/main.rs` |
| 5 | R3 Telemetry | Prometheus Metrics | Exported node operational metrics | Node metrics counters & gauges | 8 Prometheus text metrics | Formatted per Prometheus text 0.0.4 spec | `crates/rivun-node/src/lib.rs` |
| 6 | R3 Discovery | Peer Service Discovery | Signed service advertisement exchange | `DiscoveryQuery`, `SignedDiscoveryAdvertisement` | `DiscoveryResponse` with peer inventory and signed services | Rejects expired or invalid signatures | `crates/rivun-node/src/lib.rs`, `docs/discovery.md` |
| 7 | R4 Agent Protocol | `AgentIntent` | JSON payload representing high-level machine intent | `session_id`, `source_agent`, `target_agent`, `kind`, `objective`, `input`, `required_capabilities`, `constraints`, `context` | Validated `AgentIntent` JSON | Fails validation if objective empty or >16KB, or UUID is nil | `crates/rivun-agent/src/lib.rs`, `docs/agent-protocol.md` |
| 8 | R4 Agent Protocol | `AgentSession` | Units of related agent work tracking | `session_id`, `owner_agent`, `status`, timestamps, `accepted_capabilities` | Validated `AgentSession` JSON | Fails if updated timestamp < created timestamp | `crates/rivun-agent/src/lib.rs` |
| 9 | R4 Agent Protocol | Capability Negotiation | Exchange of required/optional capabilities and desired intents | `CapabilityNegotiationRequest`, `CapabilityNegotiationResponse` | Validated negotiation JSON | Fails if required/optional/intents fields are all empty | `crates/rivun-agent/src/lib.rs` |
| 10 | R4 Agent Protocol | Work Delegation | Scoped delegation of parent intent between agents | `DelegationRequest`, `DelegationResponse` | Accepted/Rejected delegation JSON | Accepted response requires `assigned_agent`; Rejected response requires `reason` | `crates/rivun-agent/src/lib.rs` |
| 11 | R4 Policy | Deterministic Policy Evaluation | Evaluates facts about typed messages against TOML rules | `PolicyInput` (`kind`, `subject`, `source_node`, `target_node`, `content_type`, `consensus_protected`, `granted_capabilities`, `human_approved`, `simulation_passed`) | `PolicyEvaluation` (`decision`: `Allow`, `Deny`, `RequirePoa`, `RequireGrant`, `HumanApproval`, `SimulateFirst`) | Evaluates rules sequentially; falls back to `default_decision` | `crates/rivun-policy/src/lib.rs`, `docs/message-policy.md` |
| 12 | R4 Ledger & PACT | Action Receipts & PACT Records | Durable signed action records and portable PACT action profiles | `ZapFrame`, `Keypair`, `PactRecord` | `SignedActionReceipt`, `PactReceiptReference` | Fails static validation if hashes or signatures are malformed | `crates/rivun-ledger/src/lib.rs`, `docs/pact.md` |

---

## Edge Cases

| # | Feature | Input | Observed Behavior |
|---|---------|-------|-------------------|
| 1 | `rivun doctor --strict` | Config with valid syntax but default-allow policy or missing receipt dir | Returns `DoctorReport` with status `degraded` and exits non-zero due to `--strict` flag |
| 2 | `rivun incident snapshot` | Environment where process lacks read access to receipt directory | Snapshot reports `receipts.verified: false` and lists error message under `receipts.errors`, setting `snapshot.valid = false` |
| 3 | `ZapNode::health_snapshot()` | Node with revoked peer in trust config | Health status transitions immediately to `critical` under `peer_trust` check |
| 4 | `AgentIntent` validation | `objective` string exceeding 16,384 bytes (16KB) | Fails validation with `ZapAgentError::FieldTooLong` |
| 5 | Capability Negotiation | Request with empty `required_capabilities`, `optional_capabilities`, and `desired_intents` | Fails validation with `ZapAgentError::EmptyCapabilityNegotiation` |
| 6 | Delegation Response | Decision = `accepted` but `assigned_agent` is `None` | Fails validation with `ZapAgentError::AcceptedDelegationMissingAssignee` |
| 7 | Policy Evaluation | Message matching `RequireGrant` rule when required capability is missing from `granted_capabilities` | `PolicyEvaluation` returns `allowed = false`, `decision = RequireGrant`, reason citing missing grant |
| 8 | PoA Receipt Validation | `consensus_required = true` in receipt but `poa` object is `None` | Fails static validation with `ZapLedgerError::InvalidReceiptField` for `poa` |

---

## Detailed Specifications & Gap Analysis

### Specification Requirement R3: Fleet Topology, Health & Incident Telemetry

#### 1. Fleet Topology Discovery & Management
- **Existing Capability**: Single-node discovery service (`DiscoveryService`) capable of publishing and querying signed service advertisements (`SignedDiscoveryAdvertisement`).
- **Missing Interface / CLI Commands**:
  - `rivun fleet`: Top-level CLI command group.
  - `rivun fleet topology` / `rivun fleet list`: Discovers and displays active multi-node network graph.
- **Proposed Interface Spec (`rivun fleet topology`)**:
  ```toml
  # Output of `rivun fleet topology --config rivun.toml --json`
  schema_version = 1
  cluster_name = "primary-fleet"
  discovered_at_micros = 1893457000000000
  total_nodes = 3
  
  [[nodes]]
  node_id = "11111111-1111-4111-8111-111111111111"
  udp_addr = "192.168.1.10:7000"
  public_key = "a1b2c3d4..."
  trust_status = "enrolled"
  capabilities = ["driver.execute:sensor.read", "driver.execute:valve.open"]
  transport_key_epoch = 4
  health_status = "healthy"
  
  [[nodes]]
  node_id = "22222222-2222-4222-8222-222222222222"
  udp_addr = "192.168.1.11:7000"
  public_key = "e5f6g7h8..."
  trust_status = "verified"
  capabilities = ["driver.execute:valve.open"]
  transport_key_epoch = 4
  health_status = "healthy"
  ```

#### 2. Fleet Health Aggregation (`rivun fleet doctor`)
- **Existing Capability**: Local single-node doctor (`rivun doctor`) and single-node HTTP readiness endpoints (`/healthz`, `/healthz.json`).
- **Missing Interface / CLI Commands**:
  - `rivun fleet doctor`: Multi-node health aggregator querying all configured/discovered peers.
- **Proposed Interface Spec (`rivun fleet doctor`)**:
  ```bash
  rivun fleet doctor --config rivun.toml [--strict] [--json] [--timeout-ms 3000]
  ```
- **Aggregate Fleet Doctor Logic**:
  1. Load local node config and resolve all peer endpoints.
  2. Send `rivun.fleet.health.query` control frames (or parallel HTTP requests to peer `/healthz.json` endpoints).
  3. Aggregate individual node health snapshots into `ZapFleetHealthSnapshot`.
  4. Perform cross-node production readiness checks:
     - Cross-node UDP socket reachability.
     - Transport key epoch parity across peers.
     - Capability cache freshness across peers (<24h).
     - Validator set consensus threshold reachability.
     - Receipt log segment verification status across nodes.
     - Clock skew check between nodes (<1000ms).
  5. Overall fleet status is `critical` if any node is `critical` or unreachable; `degraded` if any node is `degraded` or has stale capability cache; `healthy` otherwise. Exit non-zero if status != `healthy` when `--strict` is supplied.

#### 3. Live Incident Snapshot Capture (`rivun incident snapshot`)
- **Existing Capability**: Single-node bounded JSON snapshot capturing static config, doctor report, memory, receipt, and capability cache summaries.
- **Missing Features & Gaps to Fill**:
  1. **Live Process Metrics**: Add `process_metrics` section (CPU usage %, RSS/VMS memory bytes, thread count, open file descriptors, uptime seconds).
  2. **Live Prometheus Metrics Embed**: Embed current values of all node Prometheus counters and gauges (`metrics_snapshot`).
  3. **Live Network Transport State**: Add active socket bind status, frames sent/received/rejected counters, transport key epoch, per-peer round-trip latency (RTT), and packet loss rates.
  4. **Fleet-Wide Multi-Node Snapshot (`rivun fleet incident snapshot`)**: Collect live incident evidence from all active peer nodes in parallel and bundle into a cluster-wide incident artifact package.

#### 4. Prometheus Metrics Parity
- **Existing 8 Emitted Metrics**:
  - `@@rivun_HEADER@@frames_sent_total{node_id, peer}`
  - `@@rivun_HEADER@@frames_received_total{node_id, peer}`
  - `@@rivun_HEADER@@frames_rejected_total{node_id, reason}`
  - `@@rivun_HEADER@@driver_execution_errors_total{node_id, action}`
  - `@@rivun_HEADER@@peer_trust_status{node_id, peer, status}`
  - `@@rivun_HEADER@@registry_signature_valid{node_id}`
  - `@@rivun_HEADER@@receipt_log_verify_failures_total{node_id}`
  - `@@rivun_HEADER@@capability_cache_age_seconds{node_id}`
  - `@@rivun_HEADER@@poa_attestation_failures_total{node_id}`
- **Required New Metrics for Parity**:
  - `@@rivun_HEADER@@driver_execution_duration_seconds{node_id, action}` (Histogram)
  - `@@rivun_HEADER@@policy_evaluations_total{node_id, decision}` (Counter for `allow`, `deny`, `require_poa`, `require_grant`, `human_approval`, `simulate_first`)
  - `@@rivun_HEADER@@poa_attestations_total{node_id, status}` (Counter for `success` vs `failure`)
  - `@@rivun_HEADER@@replay_window_duplicates_total{node_id}` (Counter for rejected duplicate frames)
  - `@@rivun_HEADER@@journal_segment_rotations_total{node_id}` (Counter for sealed journal segments)
  - `@@rivun_HEADER@@mcp_requests_total{node_id, transport, method, status}` (Counter for AI Gateway requests)
  - `@@rivun_HEADER@@active_peers_count{node_id, status}` (Gauge for active connected peers)

---

### Specification Requirement R4: AI Agent Gateway & Multi-Transport Integration

#### 1. MCP (Model Context Protocol) Integration Architecture
- **Protocol Standard**: Model Context Protocol (JSON-RPC 2.0) over Stdio, SSE (Server-Sent Events), and WebSocket.
- **Core MCP Feature Mappings**:
  - **MCP Tools**: Expose rivun WASM driver actions and capability functions as MCP tools.
    - `tools/list`: Dynamically converts active `DriverManifest` entries and `CapabilityId` grants into MCP Tool definitions:
      ```json
      {
        "name": "@@rivun_HEADER@@valve_open",
        "description": "Execute rivun WASM driver action to open target valve",
        "inputSchema": {
          "type": "object",
          "properties": {
            "valve_id": { "type": "string", "description": "Target valve identifier" },
            "flow_rate": { "type": "number", "description": "Desired flow rate percentage" }
          },
          "required": ["valve_id"]
        }
      }
      ```
    - `tools/call`: Receives tool execution request from LLM, wraps payload into `AgentIntent`, evaluates deterministic policy (`rivun-policy`), checks/obtains PoA attestation if required, executes WASM driver (`rivun-runtime`), writes `SignedActionReceipt`, and returns JSON output to LLM.
  - **MCP Resources**: Expose rivun durable memory journals and receipt ledgers as MCP resources.
    - URIs: `rivun://memory/{namespace}/{subject}`, `rivun://receipts/{receipt_id}`.
    - `resources/list` & `resources/read`: Allows LLM agents to inspect audit logs, memory context, and receipt manifests directly.
  - **MCP Prompts**: Expose pre-approved agent workflow templates and delegation contracts as MCP prompts.

#### 2. Multi-Transport Bridge Interfaces
- **HTTP REST API Gateway**:
  - `POST /v1/agent/intent`: Submit `AgentIntent` JSON envelope.
  - `POST /v1/agent/negotiate`: Initiate `CapabilityNegotiationRequest`.
  - `GET /v1/agent/session/{session_id}`: Retrieve current `AgentSession` state.
  - `GET /v1/capabilities`: List node advertised capabilities.
  - `GET /healthz`, `GET /healthz.json`, `GET /metrics`: Readiness & telemetry endpoints.
- **Streaming Event SSE Endpoint (`GET /v1/agent/session/{session_id}/stream`)**:
  - Emits real-time Server-Sent Events for `AgentStatusUpdate`, progress per mille, step intermediate outputs, and terminal `AgentResult`.
  - Content-Type: `text/event-stream`.
- **Full-Duplex WebSocket Bridge (`WS /v1/agent/ws`)**:
  - Bidirectional JSON-RPC 2.0 / ZENV frame streaming over WebSocket.
  - Enables browser-based dashboards and Python/JS SDKs to maintain low-latency agent control sessions.

#### 3. Deterministic Policy, PoA & Signed Receipt Ledger Integration
- **Execution Workflow**:
  ```
  LLM Agent (MCP / HTTP / WS)
       │
       ▼
  1. AgentIntent (`rivun.agent.intent`)
       │
       ▼
  2. Capability Negotiation (`rivun.agent.capability_negotiation`)
       │
       ▼
  3. Policy Evaluation (`rivun-policy`) ──[Deny]──► Return AgentErrorReport
       │ [Allow / RequirePoa / RequireGrant]
       ▼
  4. Consensus / PoA (`rivun-crypto`) ──[Failed]──► Return AgentErrorReport
       │ [Certified]
       ▼
  5. WASM Driver Execution (`rivun-runtime` / `rivun-capability`)
       │
       ▼
  6. Signed Action Receipt (`rivun-ledger`) ──► Write to Receipt Journal & return AgentResult
  ```

#### 4. Strict Provenance Tracking (Provenential Evidence Chain)
- **Specification for Provenance Linkage**:
  To guarantee complete auditability, every execution result must include a `provenance_chain_digest` in the `SignedActionReceipt` and `AgentResult.metadata`.
- **Cryptographic Hash Chain Formula**:
  - $H_{\text{intent}} = \text{blake3}(\text{canonical\_json}(\text{AgentIntent}))$
  - $H_{\text{negotiation}} = \text{blake3}(\text{canonical\_json}(\text{CapabilityNegotiationResponse}))$
  - $H_{\text{policy}} = \text{blake3}(\text{PolicyInput} \parallel \text{PolicyDecision} \parallel H_{\text{PolicySet}})$
  - $H_{\text{driver}} = \text{blake3}(H_{\text{DriverManifest}} \parallel H_{\text{WasmOutput}})$
  - $H_{\text{poa}} = \text{blake3}(\text{PoaAttestations})$ (if consensus required)
  - $H_{\text{provenance}} = \text{blake3}(H_{\text{intent}} \parallel H_{\text{negotiation}} \parallel H_{\text{policy}} \parallel H_{\text{driver}} \parallel H_{\text{poa}})$
- **Updated `ActionReceipt` Structure**:
  ```rust
  pub struct ActionReceipt {
      pub schema_version: u8,
      pub node_id: Uuid,
      pub source_node: Uuid,
      pub target_node: Uuid,
      pub kind: String,
      pub subject: String,
      pub action: String,
      pub frame_hash: String,
      pub payload_hash: String,
      pub output_hash: Option<String>,
      pub frame_timestamp_micros: u64,
      pub processed_at_micros: u64,
      pub flags: u16,
      pub consensus_required: bool,
      pub poa: Option<PoaReceipt>,
      pub pact: Option<PactReceiptReference>,
      // Provenance linking fields for R4:
      pub session_id: Option<Uuid>,
      pub intent_id: Option<Uuid>,
      pub negotiation_id: Option<Uuid>,
      pub policy_decision: Option<String>,
      pub provenance_chain_digest: Option<String>, // "blake3:<64 hex>"
  }
  ```

---
*End of Handoff Report.*

