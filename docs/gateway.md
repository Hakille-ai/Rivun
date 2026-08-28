# rivun Gateway

`rivun-gateway` is the inbound integration surface for rivun: a Model Context
Protocol (MCP) server, a native HTTP REST/SSE/WebSocket server, and the
cryptographic provenance chain engine that links agent stages into a single
verifiable audit trail.

It is an optional component. The core node (`rivun-node`), wire protocol, and
transport remain fully functional without a gateway.

## Capabilities

- **MCP server** (Model Context Protocol, JSON-RPC 2.0) over stdio or HTTP:
  tools, resources, and prompts for agent runtimes.
- **HTTP REST API** for intents, sessions, capability negotiation, delegation,
  and receipt queries.
- **Server-Sent Events (SSE)** stream broker for long-running agent status.
- **WebSocket** transport with native framing (RFC 6455 handshake validation,
  4 MB frame limit).
- **Provenance chain engine**: a root-signed chain of stage digests from intent
  through receipt.

## Configuration

`rivun gateway start` accepts:

| Flag | Default | Description |
| --- | --- | --- |
| `--config` | — | Optional `rivun.toml` providing node/peer context |
| `--http-bind` | `127.0.0.1:8080` | REST/SSE/WebSocket bind address |
| `--mcp-stdio` | off | Enable MCP server over stdin/stdout |
| `--auth-token` | — | Optional bearer token for HTTP endpoints |
| `--max-frame-size` | 4 MiB | Maximum WebSocket/HTTP frame payload |
| `--journal-dir` | — | Receipt journal directory |
| `--memory-dir` | — | Memory journal directory |

Programmatic configuration uses `GatewayConfig` in the crate.

## Transports

### MCP (stdio or HTTP)

The MCP engine implements JSON-RPC 2.0 with protocol version `2024-11-05` and
standard error codes. Supported methods include `initialize`,
`tools/list`, `tools/call`, `resources/list`, `resources/read`,
`prompts/list`, and `prompts/get`.

Available MCP tools wrap core rivun actions:

- `@@rivun_HEADER@@send` / `@@rivun_HEADER@@send_transaction` — dispatch typed action envelopes;
- receipt and memory tools backed by `ReceiptJournalStore` and
  `MemoryJournalStore`.

Resources are exposed as `rivun://` URIs:

- `rivun://ledger/receipts`
- `rivun://node/status`
- `rivun://fleet/topology`
- `rivun://fleet/status`

Prompts include `goal_decomposition`, `capability_negotiation`, and
`safe_execution_verification`.

### HTTP REST

Endpoints cover ordering agent intents, reading signed receipts, and querying
fleet state. Responses are JSON; mutations produce signed frames through the
node, so ordinary node policy, PoA, and receipt flows apply unchanged.

### SSE

`SseBroker` fans events out to subscribed clients (capacity 1024 per broker).
It is used to stream agent status updates and long-running execution progress.

### WebSocket

The WS implementation performs the RFC 6455 handshake itself (SHA-1 accept
key computation included) and enforces a 4 MB frame size limit. Messages are
typed, authenticated frames, so WS sessions inherit the same trust model as the
UDP transport.

## Provenance Chain

The provenance engine (`rivun-agent::provenance`) records a root-signed chain of
stage digests:

```
intent → negotiation → policy → driver → poa → receipt → root
```

Each stage produces a digest over the previous stage hash plus its own
canonical payload (SHA-256, domain-separated `rivun-PROVENANCE-CHAIN-v1`). The
root stage signs the full chain with the node Ed25519 key, so any modification
of an intermediate stage invalidates the root signature.

Stages in the default flow:

1. `Intent` — the agent intent JSON;
2. `Negotiation` — capability negotiation outcome;
3. `Policy` — the deterministic policy decision;
4. `Driver` — driver manifest and execution;
5. `Poa` — Proof-of-Action attestations (when gated);
6. `Receipt` — the signed action receipt.

Verify a provenance chain digest offline:

```bash
cargo run -p rivun-cli -- provenance verify \
  --chain chain.json --key .rivun/node.key --json
```

Receipt journals can also be verified with their provenance digests:

```bash
cargo run -p rivun-cli -- receipts verify --dir logs/receipts --provenance
```

A missing or tampered stage link (`MissingStep`,
`StepVerificationFailed`) rejects the chain and is surfaced as a
`ZapGatewayError` and in gateway HTTP error responses.

## Agent workflows

The gateway accepts the agent protocol messages documented in
[Agent Protocol](agent-protocol.md):

- `rivun.agent.intent` — order work from an agent;
- `rivun.agent.session` — manage a session lifecycle;
- `rivun.agent.delegation.request/response` — hand scoped work to another agent;
- `rivun.agent.capability_negotiation.request/response` — negotiate capabilities;
- `rivun.agent.status/result/error` — report progress and terminal outcomes.

## Security

- HTTP endpoints accept an optional bearer token (`--auth-token`).
- MCP stdio mode is intended for trusted local agent runtimes.
- All mutations are executed through the node pipeline: signatures, replay
  checks, policy, PoA, and receipts still apply.
- Frame size limits are enforced on every transport (4 MB default).
- The provenance chain root signature must verify before a digest is accepted.

## Tests

`crates/rivun-gateway/tests/` covers MCP initialize/list/call, REST intents and
receipts, sessions/negotiation/delegation, WebSocket framing and handshake,
SSE fan-out, CORS and auth behavior, and adversarial stress cases.
