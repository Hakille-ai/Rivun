# ZAP Agent Protocol

The ZAP agent protocol is a high-level JSON contract for model gateways,
planners, tools, and operator agents. It is designed to travel inside `ZENV`
envelopes with `content_type = application/zap-agent+json`; it does not change
the wire frame, signature, transport, PoA, policy, or runtime layers.

The Rust contracts live in `crates/zap-agent`.

## Goals

- express machine-readable intent before it becomes a node action;
- group related work into sessions;
- delegate work between agents without relying on natural-language parsing;
- negotiate required and optional capabilities before execution;
- report status, terminal results, and structured errors;
- keep JSON stable enough for CLI/node/SDK integration and golden tests.

## Envelope Subjects

| Subject | Payload |
| --- | --- |
| `zap.agent.intent` | `AgentMessage::Intent` |
| `zap.agent.session` | `AgentMessage::Session` |
| `zap.agent.delegation.request` | `AgentMessage::DelegationRequest` |
| `zap.agent.delegation.response` | `AgentMessage::DelegationResponse` |
| `zap.agent.capability_negotiation.request` | `AgentMessage::CapabilityNegotiationRequest` |
| `zap.agent.capability_negotiation.response` | `AgentMessage::CapabilityNegotiationResponse` |
| `zap.agent.status` | `AgentMessage::Status` |
| `zap.agent.result` | `AgentMessage::Result` |
| `zap.agent.error` | `AgentMessage::Error` |

Every payload uses `schema_version = 1`. Receivers should reject unsupported
schema versions before acting on the content.

## JSON Shape

Messages are encoded as an internally tagged JSON envelope:

```json
{
  "type": "intent",
  "payload": {
    "schema_version": 1,
    "intent_id": "22222222-2222-4222-8222-222222222222",
    "session_id": "11111111-1111-4111-8111-111111111111",
    "source_agent": "planner.main",
    "target_agent": "executor.safety",
    "kind": "act",
    "objective": "open valve",
    "input": { "valve": "v-7" },
    "required_capabilities": ["driver.execute:valve.open"],
    "priority": "high",
    "metadata": {}
  }
}
```

The crate uses ordered maps and sets for deterministic output. Optional fields
are omitted when absent; empty capability sets and vector fields are omitted
where they are not semantically required.

## Core Contracts

`AgentIntent` describes requested work. It includes an intent UUID, a session
UUID, source and optional target agent IDs, an `IntentKind`, objective text,
arbitrary JSON input, required `zap-capability` IDs, constraints, context
references, an optional deadline, priority, and metadata.

`AgentSession` tracks a related unit of work. It records owner agent, status,
created and updated timestamps, optional root and parent IDs, accepted
capabilities, and metadata.

`DelegationRequest` asks another agent, or any capable agent, to take scoped
work from a parent intent. `DelegationResponse` accepts, rejects, or returns a
counter-offer. Accepted responses must include `assigned_agent`; rejected
responses must include a reason.

`CapabilityNegotiationRequest` and `CapabilityNegotiationResponse` exchange
required capabilities, optional capabilities, desired intents, accepted
capabilities, unsupported capabilities, supported intents, expiry, and a reason.
Empty negotiations are invalid.

`AgentStatusUpdate` reports queued, negotiating, running, waiting, blocked,
completed, failed, or cancelled state. Progress uses `progress_per_mille` so
integer JSON can represent 0.1% increments.

`AgentResult` is terminal only: completed, failed, or cancelled. Failed results
must include `AgentErrorInfo`.

`AgentErrorReport` carries standalone structured errors with a stable code,
message, category, retryable flag, details, and optional nested cause.

## Validation Rules

The crate performs basic local validation:

- `schema_version` must be `1`;
- UUID fields that identify protocol objects must not be nil;
- agent IDs and error codes must be non-empty lowercase tokens containing only
  `a-z`, `0-9`, `.`, `:`, `_`, and `-`;
- objectives, messages, artifact names, URIs, context references, and reasons
  must not exceed bounded lengths;
- session `updated_at_micros` cannot precede `created_at_micros`;
- capability negotiations cannot be empty;
- result status must be terminal;
- failed results must include structured error details;
- nested error causes are bounded.

These checks are intentionally local. Node policy, peer trust, PoA, signed
receipts, and driver/runtime enforcement remain separate layers.

## Integration Notes

Future CLI or node integrations should wrap these JSON messages in `ZENV`
envelopes, set the subject from `AgentMessage::subject()`, and set
`content_type = application/zap-agent+json`. Receivers should deserialize with
`AgentMessage::from_json_slice` so validation runs before policy evaluation or
dispatch.

Agent capabilities reuse `zap-capability::CapabilityId`. A negotiated
capability is descriptive until existing node policy, manifest, registry, and
grant checks authorize execution.
