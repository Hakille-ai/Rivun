# Capability, Router, and Memory

rivun now includes three future-core crates that stay inside the existing safety
model: capability discovery is descriptive, routing is deterministic, and memory
is local and auditable.

## Capability Discovery

`rivun-capability` defines capability ids, driver permission declarations, and
JSON control messages. Nodes answer signed `ZENV` control envelopes:

- `rivun.capability.query`
- `rivun.capability.response`
- `rivun.capability.announce`

Inspect local capabilities (generate
`examples/wasm-drivers/echo/echo.manifest.toml` first with
`rivun driver-manifest create`, see [RivunStore](RivunStore.md#cli)):

```bash
cargo run -p rivun-cli -- capability list --config rivun.toml --json
cargo run -p rivun-cli -- capability inspect-manifest --manifest examples/wasm-drivers/echo/echo.manifest.toml --json
cargo run -p rivun-cli -- capability query --config rivun.toml --target <uuid> --cache .rivun/capabilities.jsonl --json
cargo run -p rivun-cli -- capability cache refresh --config rivun.toml --json --strict
cargo run -p rivun-cli -- capability cache verify --path .rivun/capabilities.jsonl
cargo run -p rivun-cli -- capability cache list --path .rivun/capabilities.jsonl --peer <uuid> --json
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
`rivun check-config` rejects any advertised capability without an explicit grant.

Remote query responses can be appended to a local capability cache with
`capability query --cache`. Operators can refresh every configured peer in one
run with `capability cache refresh --config rivun.toml`; it uses
`[capability_cache].path` unless `--path` overrides it, respects local
`[peers.trust]` send permissions, and reports per-peer `ok`, `skipped`, or
`failed` status. The cache is JSONL with `previous_entry_hash` and `entry_hash`,
so `capability cache verify` detects tampering, removed middle entries,
duplicate ids, peer/ad node mismatches, and grants that reference capabilities
not present in the advertisement.

Node configs can require peer routes to be backed by a cached grant:

```toml
[capability_cache]
path = ".rivun/capabilities.jsonl"
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

`rivun check-config` verifies the cache hash chain, finds the latest cached
advertisement for the peer, and requires that advertisement to both include and
grant the requested capability.

## Router

`rivun-router` evaluates `[[routes]]` entries after inbound frames pass signature,
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
cargo run -p rivun-cli -- route explain --config rivun.toml --kind action --subject thermostat.setpoint --json
```

Route targets can be `local_driver`, `peer`, `capability`, `broadcast`, or
`drop`. Peer and broadcast routes create new signed frames from the routing
node. Consensus-protected frames are not forwarded in v1. Peer routes also
respect `[peers.trust]`: `allow_forward=false`, `allow_send=false`,
`status="quarantined"`, or `status="revoked"` prevent forwarding to that
machine during config validation or dispatch.

## Memory

`rivun-memory` is an append-only binary journal with record body hashes,
entry-to-entry hash chaining, disk indexes, tombstones, queries, compaction,
JSONL import/export, and verification.

```bash
cargo run -p rivun-cli -- memory put --dir .rivun/memory --subject note --payload hello
cargo run -p rivun-cli -- memory query --dir .rivun/memory --subject note --json
cargo run -p rivun-cli -- memory verify --dir .rivun/memory
cargo run -p rivun-cli -- memory compact --dir .rivun/memory --out .rivun/memory.compacted
cargo run -p rivun-cli -- memory export-evidence --dir .rivun/memory --receipts logs/receipts
```

Node config can expose memory in local capability advertisements:

```toml
[memory]
dir = ".rivun/memory"
max_record_bytes = 1048576
allow_driver_read = false
allow_driver_write = false
```

Driver memory access remains denied unless host imports are enabled by manifest
or runtime config, the node config enables the corresponding `[memory]` gate,
and explicit capability policy covers the advertised capability. In the ABI v2
foundation, `rivun.memory_write` host calls are appended as `driver` namespace
records in the local binary memory journal with the source node and frame hash.

Newly appended memory entries include `previous_entry_hash` and `entry_hash`.
`rivun memory verify` recalculates body hashes, validates entry hashes, checks the
append-only chain, rejects duplicate entry ids, and rejects tombstones whose
source record is missing. `rivun memory compact` rewrites entries into a fresh
verifiable journal.
`rivun memory export-evidence` emits a bounded JSON evidence bundle with memory
verification counts, entry ids, subjects, content types, body hashes, chain
hashes, optional verified receipt summaries, and limitations. It intentionally
omits memory payload bytes, metadata values, key material, and raw receipt
signatures; preserve the referenced journal directories and re-run `rivun memory verify`
or `rivun receipts verify` to prove the bundle.

