# Capability, Router, and Memory

ZAP now includes three future-core crates that stay inside the existing safety
model: capability discovery is descriptive, routing is deterministic, and memory
is local and auditable.

## Capability Discovery

`zap-capability` defines capability ids, driver permission declarations, and
JSON control messages. Nodes answer signed `ZENV` control envelopes:

- `zap.capability.query`
- `zap.capability.response`
- `zap.capability.announce`

Inspect local capabilities:

```bash
cargo run -p zap-cli -- capability list --config zap.toml --json
cargo run -p zap-cli -- capability inspect-manifest --manifest examples/wasm-drivers/echo/echo.manifest.toml --json
cargo run -p zap-cli -- capability query --config zap.toml --target <uuid> --cache .zap/capabilities.jsonl --json
cargo run -p zap-cli -- capability cache verify --path .zap/capabilities.jsonl
cargo run -p zap-cli -- capability cache list --path .zap/capabilities.jsonl --peer <uuid> --json
```

Discovery never grants authority. A peer can advertise `driver.execute:echo`,
but execution still depends on transport trust, frame signatures, node config,
manifests, registry policy, route rules, and runtime permission checks.

Node configs can attach policy grants and requirements to advertised
capabilities:

```toml
[capability_policy]
require_grants_for_advertised = true

[[capability_policy.grants]]
capability = "driver.execute:echo"
reason = "operator-approved signed echo driver"

[[capability_policy.requirements]]
capability = "poa.validator"
required = true
reason = "critical actions require validator quorum"
```

Grants must refer to capabilities the node actually advertises; validation
rejects grants for missing local capabilities. Requirements may describe
external prerequisites. When `require_grants_for_advertised = true`,
`zap check-config` rejects any advertised capability without an explicit grant.

Remote query responses can be appended to a local capability cache with
`capability query --cache`. The cache is JSONL with `previous_entry_hash` and
`entry_hash`, so `capability cache verify` detects tampering, removed middle
entries, duplicate ids, peer/ad node mismatches, and grants that reference
capabilities not present in the advertisement.

Node configs can require peer routes to be backed by a cached grant:

```toml
[capability_cache]
path = ".zap/capabilities.jsonl"
max_age_micros = 86400000000

[[routes]]
name = "thermostat-peer"
requires_peer_grant = "driver.execute:thermostat.setpoint"

[routes.match]
kind = "action"
subject = "thermostat.setpoint"

[routes.target]
peer = "00000000-0000-4000-8000-000000000000"
```

`zap check-config` verifies the cache hash chain, finds the latest cached
advertisement for the peer, and requires that advertisement to both include and
grant the requested capability.

## Router

`zap-router` evaluates `[[routes]]` entries after inbound frames pass signature,
PoA, timestamp, and replay validation. If no route matches, action messages keep
the legacy behavior and dispatch to a local driver named by the action subject.

Example:

```toml
[[routes]]
name = "thermostat-peer"

[routes.match]
kind = "action"
subject = "thermostat.*"

[routes.target]
peer = "00000000-0000-4000-8000-000000000000"
```

Explain a route:

```bash
cargo run -p zap-cli -- route explain --config zap.toml --kind action --subject thermostat.setpoint --json
```

Route targets can be `local_driver`, `peer`, `capability`, `broadcast`, or
`drop`. Peer and broadcast routes create new signed frames from the routing
node. Consensus-protected frames are not forwarded in v1.

## Memory

`zap-memory` is an append-only JSONL store with record body hashes,
entry-to-entry hash chaining, tombstones, queries, pruning, and verification.

```bash
cargo run -p zap-cli -- memory put --path .zap/memory.jsonl --subject note --payload hello
cargo run -p zap-cli -- memory query --path .zap/memory.jsonl --subject note --json
cargo run -p zap-cli -- memory verify --path .zap/memory.jsonl
cargo run -p zap-cli -- memory prune --path .zap/memory.jsonl --before-created-at-micros 1735689600000000 --out .zap/memory.retained.jsonl
```

Node config can expose memory in local capability advertisements:

```toml
[memory]
path = ".zap/memory.jsonl"
max_record_bytes = 1048576
allow_driver_read = false
allow_driver_write = false
```

Driver memory access remains denied unless future host imports are enabled by
manifest, config, and explicit capability policy together.

Newly appended memory entries include `previous_entry_hash` and `entry_hash`.
`zap memory verify` recalculates body hashes, validates entry hashes, checks the
append-only chain, rejects duplicate entry ids, and rejects tombstones whose
source record is missing. `zap memory prune` rewrites retained entries into a
fresh verifiable chain and drops tombstones whose source record was pruned.
