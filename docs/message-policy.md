# Message Policy

ZAP does not compile natural language into actions. AI models, operator tools,
or gateways should emit strict typed messages: `kind`, `subject`,
`content_type`, and payload bytes. ZAP then signs, encrypts, routes, audits, and
enforces deterministic policy on those messages.

## Sending Typed Actions

Send an action envelope directly:

```bash
cargo run -p zap-cli -- send --config zap.toml --target <uuid> \
  --kind action --subject thermostat.setpoint \
  --payload '{"temperature_c":20}' --content-type application/json
```

For actions that must carry a Proof-of-Action certificate, mark the frame as
consensus-protected:

```bash
cargo run -p zap-cli -- send --config zap.toml --target <uuid> \
  --kind action --subject safety.emergency_stop \
  --payload '{"reason":"operator_request"}' --content-type application/json \
  --requires-consensus --poa-network
```

`--requires-consensus` sets the `REQUIRES_CONSENSUS` frame flag and uses either
local `--poa-validator-key` signers or configured `--poa-network` validators to
attach a `ZPOA` certificate before the frame is sent.

## Receiver Policy

Receiver configs can define deterministic message policy rules:

```toml
[message_policy]
default_decision = "deny"

[[message_policy.rules]]
kind = "action"
subject = "safety.*"
decision = "require_poa"
reason = "safety actions require validator quorum"

[[message_policy.rules]]
kind = "action"
subject = "debug.*"
decision = "deny"
reason = "debug drivers are disabled in production"
```

Rules are evaluated in order. `kind` and `subject` are optional wildcards when
omitted. `subject` supports `*` for all subjects and suffix wildcards such as
`safety.*`.

When no rule matches, `message_policy.default_decision` decides whether the
message continues. The value can be `allow` or `deny`. Omitted configs default
to `allow` for backward compatibility; production receivers should set
`default_decision = "deny"` and add explicit `allow` or gated rules for expected
traffic.

Decisions:

- `allow`: accept the message and continue with routing or local dispatch;
- `deny`: reject the message before routing or driver execution;
- `require_poa`: require a frame marked `REQUIRES_CONSENSUS` with a valid PoA
  certificate before routing or driver execution.
- `require_grant`: require an explicit configured capability grant before the
  message can continue;
- `human_approval`: fail closed until a trusted approval subsystem supplies
  human approval evidence;
- `simulate_first`: fail closed until a trusted simulation subsystem supplies
  successful simulation evidence.

This keeps model-specific planning outside the protocol while preserving ZAP's
receiver-side safety boundary.

## Typed Message Contracts

Agent gateways and machine adapters can validate strict contracts before
sending, and nodes can enforce the same contracts on receipt.

Example contract:

```toml
schema_version = 1
name = "thermostat setpoint"
kind = "action"
subject = "thermostat.setpoint"
content_type = "application/json"
max_body_bytes = 4096

[body]
format = "json_object"
required_json_fields = ["temperature_c"]
allowed_json_fields = ["temperature_c", "reason"]

[metadata]
max_bytes = 2048
json_object = true
required_json_fields = ["source"]
```

Validate an encoded `ZENV` payload:

```bash
cargo run -p zap-cli -- schema validate \
  --contract thermostat.contract.toml --envelope action.zenv --json
```

Enable receiver-side schema enforcement:

```toml
[message_schema]
require_match = true

[[message_schema.contracts]]
path = "thermostat.contract.toml"
```

When `require_match = false`, matching contracts are enforced but unmatched
messages keep the existing behavior. When `require_match = true`, contracts are
an allowlist for typed messages.

## Policy Dry Runs

Policy files use the same rule shape as node config:

```toml
default_decision = "deny"

[[rules]]
name = "safety quorum"
kind = "action"
subject = "safety.*"
decision = "require_poa"
reason = "safety actions require validator quorum"
```

Evaluate before deployment:

```bash
cargo run -p zap-cli -- policy evaluate --policy policy.toml \
  --kind action --subject safety.emergency_stop \
  --requires-consensus --strict --json
```
