# Operations

## Key Generation

```bash
cargo run -p zap-cli -- keygen --out .zap/node.key
```

Keep the generated key file private. Share only `node_id` and `public_key`.

`keygen` refuses to overwrite an existing key file. Use `--force` only when intentionally rotating or replacing a node identity.

## Config Validation

Run this before starting a node:

```bash
cargo run -p zap-cli -- doctor --config zap.toml
cargo run -p zap-cli -- check-config --strict --config zap.toml
```

For automation:

```bash
cargo run -p zap-cli -- doctor --config zap.toml --json --strict
cargo run -p zap-cli -- check-config --config zap.toml --json
```

`zap doctor` is the operator readiness gate. It runs config validation, prints a
score, reports pass/warn/fail checks for production posture, and exits non-zero
with `--strict` unless the node has no readiness warnings. `zap check-config`
remains the lower-level structural validator.

The validator and doctor checks cover:

- local bind address syntax;
- key file readability and parseability;
- peer address syntax and duplicates;
- peer `public_key` derives the configured `node_id`;
- transport key length and nonzero value;
- peer trust status, send/receive/forward permissions, optional expiry, and
  optional transport-key rotation age;
- runtime/security limits are nonzero where required;
- duplicate driver actions;
- WASM/WAT driver files compile and expose ABI v1 before daemon startup;
- signed driver manifests match the configured action, local driver hash, ABI version, and author signature when `manifest` is configured;
- memory paths do not overlap key files, receipts, registries, drivers, or manifests;
- route targets reference configured peers or drivers, and capability routes do
  not silently point at non-executable v1 capabilities without a warning.
- peer routes with `requires_peer_grant` are backed by the latest verified
  cached advertisement for the target peer.
- capability grants reference capabilities actually advertised by the node, and
  optional policy can require every advertised capability to have a grant.

During daemon startup, configured drivers are compiled, ABI-validated, and kept in memory. Updating a driver file requires a daemon restart.

## Config

Node configs are TOML files with:

- local bind address;
- local key file path;
- static peer list;
- optional peer trust policy;
- optional registered WASM drivers;
- runtime limits.
- anti-replay policy.
- optional signed receipt log path.
- optional local ZapStore registry index path.
- optional local memory JSONL path.
- optional deterministic route table.

For container deployment, see [Deployment](deployment.md). The production image
runs the same `zap run --config <path>` daemon command, but expects config and
node state to be mounted into the container.

Relative `key_file` and driver `path` values are resolved from the directory containing the TOML config file. This makes `zap run --config path/to/node.toml` and `zap check-config --config path/to/node.toml` independent of the shell's current working directory.

Relative driver `manifest` paths are resolved the same way:

```toml
[[drivers]]
action = "echo"
path = "../wasm-drivers/echo/echo.wat"
manifest = "../wasm-drivers/echo/echo.manifest.toml"
```

## Peer Trust and Enrollment

Static peers now carry an explicit local trust contract. Generate a verified
TOML enrollment block before adding a new machine:

```bash
cargo run -p zap-cli -- trust enroll \
  --node-id <peer-node-id> \
  --addr 10.0.0.12:7777 \
  --public-key <peer-public-key> \
  --transport-key <64-hex-chars> \
  --transport-key-epoch 1 \
  --label production
```

Inspect an existing config:

```bash
cargo run -p zap-cli -- trust inspect --config zap.toml --json
```

For machine-to-machine onboarding, create a signed invitation from the node
that wants to be trusted, then accept it on the operator side:

```bash
cargo run -p zap-cli -- peer invite \
  --config zap.toml \
  --addr 10.0.0.12:7777 \
  --label production \
  --out node-a.invite.json

cargo run -p zap-cli -- peer accept \
  --invite node-a.invite.json \
  --config zap.toml \
  --out zap.with-node-a.toml \
  --json
```

Rotate or revoke configured machine trust material without editing TOML by
hand:

```bash
cargo run -p zap-cli -- peer rotate \
  --config zap.toml \
  --node-id <peer-node-id> \
  --out zap.rotated.toml \
  --json

cargo run -p zap-cli -- peer revoke \
  --config zap.toml \
  --node-id <peer-node-id> \
  --out zap.revoked.toml \
  --json
```

Invitations are signed by the inviting node over a domain-separated payload.
`peer accept` verifies the signature, node id, transport key, expiry, and trust
labels before emitting a peer block or updated config. `peer revoke` marks the
peer `revoked` and disables send, receive, forward, and PoA-attestation gates.

Each peer can restrict machine communication:

```toml
[trust]
require_peer_expiry = true
max_transport_key_age_micros = 2592000000000

[[peers]]
node_id = "..."
addr = "10.0.0.12:7777"
public_key = "..."
transport_key = "..."
transport_key_epoch = 3
transport_key_rotated_at_micros = 1760000000000000

[peers.trust]
status = "trusted"
allow_send = true
allow_receive = true
allow_forward = false
allow_poa_attestation = true
expires_at_micros = 1765000000000000
labels = ["production", "edge"]
```

`status = "revoked"` fails config validation. `status = "quarantined"` keeps
the peer in the file for operator review but disables transport use. `allow_send
= false` blocks `zap send` and node responses to that peer. `allow_receive =
false` rejects authenticated inbound frames before dispatch. `allow_forward =
false` prevents route targets and broadcasts from forwarding to that peer.

Create and verify a signed manifest:

```bash
cargo run -p zap-cli -- driver-manifest create --driver examples/wasm-drivers/echo/echo.wat --action echo --author-key .zap/node.key --out examples/wasm-drivers/echo/echo.manifest.toml
cargo run -p zap-cli -- driver-manifest verify --driver examples/wasm-drivers/echo/echo.wat --manifest examples/wasm-drivers/echo/echo.manifest.toml
```

Scoped host imports for machine-safe drivers can be declared in signed
manifests:

```bash
cargo run -p zap-cli -- driver-manifest create \
  --driver machine.wat --action machine.note \
  --author-key .zap/node.key --out machine.manifest.toml \
  --allow-emit-event --allow-memory-write --max-host-call-bytes 8192
```

For `memory_write`, the receiving node must also configure:

```toml
[memory]
path = ".zap/memory.jsonl"
allow_driver_write = true
```

General `network`, `filesystem`, `clock`, and `environment` permissions remain
rejected by `check-config`.

`check-config --json` includes `signed_driver_count` so deploy scripts can require signed driver provenance.
It also includes `registry_enabled`, `registry_entry_count`, and
`registry_signature_required` when a local ZapStore registry is configured.
Capability, route, and memory automation can inspect `capability_count`,
`route_count`, and `memory_enabled`.

Create a local registry index and add a signed manifest:

```bash
cargo run -p zap-cli -- registry init --out registry.index.toml
cargo run -p zap-cli -- registry add --registry registry.index.toml --manifest examples/wasm-drivers/echo/echo.manifest.toml
cargo run -p zap-cli -- registry revoke --registry registry.index.toml --action echo --version 0.1.0 --reason "bad release"
cargo run -p zap-cli -- registry sign --registry registry.index.toml --operator-key .zap/node.key
cargo run -p zap-cli -- registry verify-signature --registry registry.index.toml
```

Configure a node to enforce that index:

```toml
[registry]
path = "registry.index.toml"
require_signature = true
```

Set `require_signature = true` for production gates that should fail when the
local registry was not approved by an operator key. Registry mutations clear the
operator signature, so review and re-sign after every `add` or `revoke`.

Capability discovery is explicit and signed:

```bash
cargo run -p zap-cli -- capability list --config zap.toml --json
cargo run -p zap-cli -- capability inspect-manifest --manifest examples/wasm-drivers/echo/echo.manifest.toml --json
cargo run -p zap-cli -- capability query --config zap.toml --target <uuid> --cache .zap/capabilities.jsonl --json
cargo run -p zap-cli -- capability cache refresh --config zap.toml --json --strict
cargo run -p zap-cli -- capability cache verify --path .zap/capabilities.jsonl
cargo run -p zap-cli -- capability cache list --path .zap/capabilities.jsonl --peer <uuid> --json
```

Discovery is descriptive only. A discovered capability does not grant a driver
host access and does not bypass manifests, registry policy, PoA, or route
validation.

Attach explicit policy grants to local advertisements when a deployment needs
machine-checkable capability provenance:

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

`check-config --json` reports `capability_grant_count`,
`capability_requirement_count`, `ungranted_capability_count`,
`capability_cache_enabled`, and `peer_grant_route_count`. `zap doctor` turns
those counts into readiness checks.

Remote capability responses can be cached locally with `capability query
--cache`. For normal operations, prefer `capability cache refresh --config
zap.toml --strict` so every configured peer is refreshed into
`[capability_cache].path` before deployment gates run. The refresh report
includes per-peer `ok`, `skipped`, and `failed` status and respects local peer
trust policy. The cache is append-only JSONL with entry hash chaining; verify
it before using it for deployment review or routing decisions.

Require cached peer grants before forwarding selected messages:

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
peer = "peer-node-id"
```

`zap check-config --strict` fails when the cache is missing, corrupt, stale
according to `max_age_micros`, missing the peer advertisement, or when the
latest advertisement does not grant the required capability.

Routes can forward, broadcast, drop, or dispatch messages deterministically:

```toml
[[routes]]
name = "echo-local"

[routes.match]
kind = "action"
subject = "echo"

[routes.target]
local_driver = "echo"
```

Explain a route before deployment:

```bash
cargo run -p zap-cli -- route explain --config zap.toml --kind action --subject echo --json
```

Local memory is append-only JSONL with body hashes, entry hash chaining, and
tombstones:

```toml
[memory]
path = ".zap/memory.jsonl"
max_record_bytes = 1048576
allow_driver_read = false
allow_driver_write = false
```

Operate on the store:

```bash
cargo run -p zap-cli -- memory put --path .zap/memory.jsonl --subject note --payload hello
cargo run -p zap-cli -- memory query --path .zap/memory.jsonl --subject note --json
cargo run -p zap-cli -- memory verify --path .zap/memory.jsonl
```

Verification recalculates every body hash, validates the entry hash chain,
rejects duplicate entry ids, and rejects tombstones whose source record is
missing. Pruning writes a fresh verifiable chain for retained entries and drops
tombstones whose source record was pruned.

`zap send` is a one-shot peer process. It validates the config, binds to the
config `bind` address, sends one signed frame, and exits. This is deliberate:
receivers reject datagrams whose source address does not match the configured
peer address.

ZAP can carry raw bytes or universal `ZENV` envelopes. `zap-node` also accepts the older JSON action envelope for compatibility, but new CLI sends use `ZENV`.

For daemon driver execution, send a universal action envelope:

```bash
cargo run -p zap-cli -- send --config zap.toml --target <uuid> --action echo --payload hello
```

For a universal event envelope:

```bash
cargo run -p zap-cli -- send --config zap.toml --target <uuid> --kind event --subject sensor.temperature --payload '{"c":21.5}' --content-type application/json
```

Inline metadata is accepted as bytes, so JSON and plain text are both valid:

```bash
cargo run -p zap-cli -- send --config zap.toml --target <uuid> --kind event --subject sensor.temperature --payload '{"c":21.5}' --content-type application/json --metadata '{"source":"sim"}'
```

Agents, models, and operator tools should send typed `ZENV` messages directly:

```bash
cargo run -p zap-cli -- send --config zap.toml --target <uuid> \
  --kind action --subject thermostat.setpoint \
  --payload '{"temperature_c":20}' --content-type application/json
```

Critical actions, such as `safety.emergency_stop`, should be protected by
receiver-side `[message_policy]` and sent with `REQUIRES_CONSENSUS` plus a PoA
certificate:

```bash
cargo run -p zap-cli -- send --config zap.toml --target <uuid> \
  --kind action --subject safety.emergency_stop \
  --payload '{"reason":"operator_request"}' --content-type application/json \
  --requires-consensus --poa-validator-key .zap/validator.key
```

Or request attestations from configured validator peers:

```bash
cargo run -p zap-cli -- send --config zap.toml --target <uuid> \
  --kind action --subject safety.emergency_stop \
  --payload '{"reason":"operator_request"}' --content-type application/json \
  --requires-consensus --poa-network
cargo run -p zap-cli -- send --config zap.toml --target <uuid> \
  --kind action --subject safety.emergency_stop \
  --payload '{"reason":"operator_request"}' --content-type application/json \
  --requires-consensus --poa-network --poa-timeout-ms 5000
```

Each `[poa]` validator used by `--poa-network` must also be configured as a
peer so `zap send` can reach it over encrypted UDP. Validator nodes answer
signed `poa.attestation_request` control envelopes with signed
`poa.attestation_response` envelopes. `--poa-timeout-ms` controls how long the
sender waits for enough validator responses; the default is 2000 ms.

For offline review and future validator workflows, create and sign portable PoA
attestation JSON:

```bash
cargo run -p zap-cli -- poa request --frame critical-frame.bin --requester-key .zap/node.key --threshold 1 > poa-request.json
cargo run -p zap-cli -- poa attest --request poa-request.json --validator-key .zap/validator.key > poa-response.json
```

For versioned validator-set distribution, sign the set with an authority key,
verify it, then apply it to a node config:

```bash
cargo run -p zap-cli -- poa validator-set create \
  --authority-key .zap/operator.key \
  --epoch 4 \
  --threshold 2 \
  --validator <validator-a-node-id>=<validator-a-public-key> \
  --validator <validator-b-node-id>=<validator-b-public-key> \
  --label production \
  --out poa-validators.v4.json

cargo run -p zap-cli -- poa validator-set verify \
  --path poa-validators.v4.json \
  --authority-public-key <operator-public-key> \
  --json

cargo run -p zap-cli -- poa validator-set pull \
  --config zap.toml \
  --target <peer-node-id> \
  --authority-public-key <operator-public-key> \
  --min-epoch 4 \
  --out poa-validators.v4.json \
  --json

cargo run -p zap-cli -- poa validator-set apply \
  --config zap.toml \
  --set poa-validators.v4.json \
  --authority-public-key <operator-public-key> \
  --out zap.with-poa-set.toml \
  --json
```

Applied configs use `poa.validator_set` and `poa.validator_set_authority`.
`zap-node` verifies the signed set at validation/startup, rejects invalid or
expired sets, and uses `max(poa.required_threshold, set.required_threshold)` as
the effective threshold. `validator-set pull` uses signed `ZENV` control
messages, verifies the peer response, verifies the nested validator-set
signature, and writes the received JSON unchanged.

For binary action payloads, use a file and mark the payload as opaque:

```bash
cargo run -p zap-cli -- send --config zap.toml --target <uuid> --action upload --payload-file payload.bin --binary-payload
```

Omit `--action` only when you intentionally want to send raw bytes that a `ZapEndpoint` peer, not `ZapNode` action dispatch, will consume directly.

Omit `--action` but include `--kind` when you want ZAP to wrap the payload in a universal envelope. If `--content-type` is omitted, inline `--payload` defaults to `text/plain` and `--payload-file` defaults to `application/octet-stream`.

Transport keys must be 32-byte hex strings. For local development:

```powershell
-join ((1..32) | ForEach-Object { "42" })
```

Default security policy allows five minutes of clock skew, remembers the last 4096 accepted frame fingerprints, and keeps a per-peer cache of recent transport nonces:

```toml
[security]
max_clock_skew_micros = 300000000
replay_cache_capacity = 4096
```

PoA validators are configured separately:

```toml
[poa]
required_threshold = 1

[[poa.validators]]
node_id = "validator-node-id"
public_key = "validator-public-key"
```

Or with a signed validator set:

```toml
[poa]
required_threshold = 2
validator_set = "poa-validators.v4.json"
validator_set_authority = "operator-public-key"
```

Signed receipts are optional and append-only JSONL:

```toml
[receipts]
path = "logs/actions.jsonl"
```

Receipts are audit records signed by the processing node. They are not financial records.

Verify a receipt log after a test run, pull peer receipts for an audit window,
or archive multiple logs:

```bash
cargo run -p zap-cli -- receipts verify --path logs/actions.jsonl
cargo run -p zap-cli -- receipts pull --config zap.toml --target <peer-node-id> --out logs/peer-actions.jsonl --json
cargo run -p zap-cli -- receipts prune --path logs/actions.jsonl --before-processed-at-micros 1735689600000000 --out logs/actions.retained.jsonl
cargo run -p zap-cli -- receipts merge logs/node-a.jsonl logs/node-b.jsonl --out logs/receipts.archive.jsonl
```

## Logging

Use `RUST_LOG`:

```bash
RUST_LOG=info cargo run -p zap-cli -- run --config zap.toml
```

## Inspecting Frames

```bash
cargo run -p zap-cli -- inspect frame.bin
cargo run -p zap-cli -- inspect frame.bin --verify-with-public-key <base64-public-key>
```

The inspect command decodes raw ZAP frames, not encrypted UDP datagrams. When the frame payload is a `ZENV` envelope, inspect also prints:

- `envelope_kind`;
- `subject`;
- `content_type`;
- `metadata_len`;
- `body_len`.

Use `--verify-with-public-key` for offline audit when you only need signature
verification. `--verify-with-key` remains available for local key files, but it
is not required to verify a frame.

## Local Two-Node Smoke Test

1. Generate two keys:

```bash
cargo run -p zap-cli -- keygen --out .zap/node-a.key
cargo run -p zap-cli -- keygen --out .zap/node-b.key
```

2. Copy `node_id` and `public_key` values into `examples/configs/node-a.toml` and `examples/configs/node-b.toml`.

3. Run node A:

```bash
cargo run -p zap-cli -- run --config examples/configs/node-a.toml
```

4. In another terminal, send from node B:

```bash
cargo run -p zap-cli -- send --config examples/configs/node-b.toml --target <node-a-uuid> --action echo --payload hello
```
