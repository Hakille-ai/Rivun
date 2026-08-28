# Operations

## Key Generation

```bash
cargo run -p rivun-cli -- keygen --out .rivun/node.key
```

Keep the generated key file private. Share only `node_id` and `public_key`.

`keygen` refuses to overwrite an existing key file. Use `--force` only when intentionally rotating or replacing a node identity.

## Config Validation

Run this before starting a node:

```bash
cargo run -p rivun-cli -- doctor --config rivun.toml
cargo run -p rivun-cli -- check-config --strict --config rivun.toml
```

For automation:

```bash
cargo run -p rivun-cli -- doctor --config rivun.toml --json --strict
cargo run -p rivun-cli -- check-config --config rivun.toml --json
```

`rivun doctor` is the operator readiness gate. It runs config validation, prints a
score, reports pass/warn/fail checks for production posture, and exits non-zero
with `--strict` unless the node has no readiness warnings. `rivun check-config`
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
- optional signed receipt journal directory.
- optional local RivunStore registry index path.
- optional local memory journal directory.
- optional deterministic route table.

For container deployment, see [Deployment](deployment.md). The production image
runs the same `rivun run --config <path>` daemon command, but expects config and
node state to be mounted into the container.

Relative `key_file` and driver `path` values are resolved from the directory containing the TOML config file. This makes `rivun run --config path/to/node.toml` and `rivun check-config --config path/to/node.toml` independent of the shell's current working directory.

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
cargo run -p rivun-cli -- trust enroll \
  --node-id <peer-node-id> \
  --addr 10.0.0.12:7777 \
  --public-key <peer-public-key> \
  --transport-key <64-hex-chars> \
  --transport-key-epoch 1 \
  --label production
```

Inspect an existing config:

```bash
cargo run -p rivun-cli -- trust inspect --config rivun.toml --json
```

For machine-to-machine onboarding, create a signed invitation from the node
that wants to be trusted, then accept it on the operator side:

```bash
cargo run -p rivun-cli -- peer invite \
  --config rivun.toml \
  --addr 10.0.0.12:7777 \
  --label production \
  --out node-a.invite.json

cargo run -p rivun-cli -- peer accept \
  --invite node-a.invite.json \
  --config rivun.toml \
  --out rivun.with-node-a.toml \
  --json
```

Rotate or revoke configured machine trust material without editing TOML by
hand:

```bash
cargo run -p rivun-cli -- peer rotate \
  --config rivun.toml \
  --node-id <peer-node-id> \
  --out rivun.rotated.toml \
  --json

cargo run -p rivun-cli -- peer revoke \
  --config rivun.toml \
  --node-id <peer-node-id> \
  --out rivun.revoked.toml \
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
= false` blocks `rivun send` and node responses to that peer. `allow_receive =
false` rejects authenticated inbound frames before dispatch. `allow_forward =
false` prevents route targets and broadcasts from forwarding to that peer.

Create and verify a signed manifest (the manifest file is generated by the
first command; see [RivunStore](RivunStore.md#cli) for the full contract):

```bash
cargo run -p rivun-cli -- driver-manifest create --driver examples/wasm-drivers/echo/echo.wat --action echo --author-key .rivun/node.key --out examples/wasm-drivers/echo/echo.manifest.toml
cargo run -p rivun-cli -- driver-manifest verify --driver examples/wasm-drivers/echo/echo.wat --manifest examples/wasm-drivers/echo/echo.manifest.toml
```

Scoped host imports for machine-safe drivers can be declared in signed
manifests:

```bash
cargo run -p rivun-cli -- driver-manifest create \
  --driver machine.wat --action machine.note \
  --author-key .rivun/node.key --out machine.manifest.toml \
  --allow-emit-event --allow-memory-write --max-host-call-bytes 8192
```

For `memory_write`, the receiving node must also configure:

```toml
[memory]
dir = ".rivun/memory"
allow_driver_write = true
```

General `network`, `filesystem`, `clock`, and `environment` permissions remain
rejected by `check-config`.

`check-config --json` includes `signed_driver_count` so deploy scripts can require signed driver provenance.
It also includes `registry_enabled`, `registry_entry_count`, and
`registry_signature_required` when a local RivunStore registry is configured.
Capability, route, and memory automation can inspect `capability_count`,
`route_count`, and `memory_enabled`.

## PACT Operations

PACT records are portable signed action records that use the same key files and
offline verification model as the rest of rivun. They are useful when an operator
needs to carry intent, consent, proof, terms, revocation, and execution status
as protocol evidence.

Create and sign a PACT:

```bash
cargo run -p rivun-cli -- pact create \
  --actor agent.alpha \
  --target driver.valve \
  --intent valve.open \
  --object '{"valve":"v-7"}' \
  --terms '{"max_runtime_ms":5000}' \
  --created-at-micros 1893456000000000 \
  --out pact-unsigned.json

cargo run -p rivun-cli -- pact sign \
  --input pact-unsigned.json \
  --key .rivun/node.key \
  --out pact-signed.json
```

Verify, revoke, and bundle:

```bash
cargo run -p rivun-cli -- pact verify --input pact-signed.json --json
cargo run -p rivun-cli -- pact revoke \
  --input pact-signed.json \
  --revoked-by ops.lead \
  --reason "operator stop" \
  --key .rivun/node.key \
  --out pact-revoked.json
cargo run -p rivun-cli -- pact bundle export \
  --pact pact-signed.json \
  --out pact-bundle.json
cargo run -p rivun-cli -- pact bundle verify --bundle pact-bundle.json --json
```

Use `rivun pact schema --out pact.schema.json` when another system needs the JSON
shape. Use `rivun fixtures verify --fixtures fixtures --sdk <sdk-path>` to prove
SDK conformance against the shared PACT fixtures.

Create a local registry index and add a signed manifest:

```bash
cargo run -p rivun-cli -- registry init --out registry.index.toml
cargo run -p rivun-cli -- registry add --registry registry.index.toml --manifest examples/wasm-drivers/echo/echo.manifest.toml
cargo run -p rivun-cli -- registry revoke --registry registry.index.toml --action echo --version 0.1.0 --reason "bad release"
cargo run -p rivun-cli -- registry deprecate --registry registry.index.toml --action echo --version 0.1.0 --reason "use 0.2.0"
cargo run -p rivun-cli -- registry migration add --registry registry.index.toml --action echo --version 2.0.0 --from-version-req '^1.0.0' --from-abi-req '=1' --requires-operator-approval --migration-driver echo-migrate@0.1.0
cargo run -p rivun-cli -- registry sign --registry registry.index.toml --operator-key .rivun/node.key
cargo run -p rivun-cli -- registry verify-signature --registry registry.index.toml
cargo run -p rivun-cli -- registry resolve --registry registry.index.toml --action echo --version-req '^0.1.0' --abi-req '>=1,<=2' --json
cargo run -p rivun-cli -- registry pull --config rivun.toml --target <uuid> --out registry.index.toml --operator-public-key <base64-public-key> --json
cargo run -p rivun-cli -- registry mirror --config rivun.toml --out mirrored-registry.index.toml --operator-public-key <base64-public-key> --json
cargo run -p rivun-cli -- registry sign --registry mirrored-registry.index.toml --operator-key .rivun/node.key
cargo run -p rivun-cli -- registry publication create --registry mirrored-registry.index.toml --publisher-key .rivun/node.key --out registry.publication.json --channel stable --json
cargo run -p rivun-cli -- registry publication verify --registry mirrored-registry.index.toml --publication registry.publication.json --json
cargo run -p rivun-cli -- registry plan create --registry mirrored-registry.index.toml --publication registry.publication.json --planner-key .rivun/node.key --out registry.install-plan.json --driver 'echo@^0.1.0' --abi-req '>=1,<=2' --json
cargo run -p rivun-cli -- registry plan verify --registry mirrored-registry.index.toml --plan registry.install-plan.json --planner-public-key <base64-public-key> --json
cargo run -p rivun-cli -- registry bundle export --registry mirrored-registry.index.toml --publication registry.publication.json --out RivunStore-bundle --driver echo@0.1.0=examples/wasm-drivers/echo/echo.wat --json
cargo run -p rivun-cli -- registry bundle pull-manifest --config rivun.toml --target <uuid> --out pulled-RivunStore.bundle.json --require-publication --require-drivers --json
cargo run -p rivun-cli -- registry bundle verify --bundle RivunStore-bundle --require-drivers --json
cargo run -p rivun-cli -- registry bundle import --bundle RivunStore-bundle --out .rivun/imported-RivunStore --require-drivers --json
```

Configure a node to enforce that index:

```toml
[registry]
path = "registry.index.toml"
require_signature = true
bundle_path = "RivunStore-bundle"
```

Set `require_signature = true` for production gates that should fail when the
local registry was not approved by an operator key. Registry mutations clear the
operator signature, so review and re-sign after every `add`, `deprecate`,
`migration add`, or `revoke`.
Use `registry resolve` in installers or CI gates to choose the highest active
driver entry matching an action, `MAJOR.MINOR.PATCH` version requirement, and
optional ABI requirement. Supported version requirements include `*`, exact versions,
`^1.2.3`, `~1.2.3`, and comma-separated comparators such as
`>=1.0.0,<2.0.0`; ABI requirements use integer comparators such as `=1` or
`>=1,<=2`. Deprecated and revoked entries are never selected automatically. Use
`registry deprecate` for migration nudges, `registry migration add` for signed
upgrade instructions, and `registry revoke` for unsafe releases that must be
blocked.
Remote registry pulls use signed control frames plus the nested registry
operator signature. Treat pulled indexes as deployment input: verify the
operator key, then run `check-config --strict` before starting a daemon.
Remote registry mirrors fetch multiple peers and merge compatible entries.
Revoked entries win over active entries for the same driver version; conflicting
hashes, authors, names, or ABI versions fail the merge. Mirrored indexes are
unsigned until reviewed and re-signed.
Registry publication metadata signs the canonical hash of the approved registry
index. Archive `registry.publication.json` with the deployed index so later
audits can prove the exact registry bytes used by a rollout.
Registry install plans sign the exact resolved driver set that a CI job,
machine, or factory installer should trust. `registry plan create` binds each
`action@version-req` request to the selected active version, ABI, manifest path,
WASM hash, registry hash, optional ABI requirement, migration metadata, optional
publication hash, target, and labels. `registry plan verify` rechecks the
planner signature and registry hash before installation.
Registry bundles package the signed registry, publication metadata, copied
manifests, and optional driver artifacts into a safe directory layout.
Verification recomputes every listed hash before import; use `--require-drivers`
for air-gapped or factory deployments that must carry executable artifacts.
Nodes with `registry.bundle_path` can serve the bundle manifest over signed
control frames. Use `registry bundle pull-manifest --require-publication
--require-drivers` to discover a peer's published bundle contract before
fetching files through an external artifact channel or importing a local copy.

Capability discovery is explicit and signed:

```bash
cargo run -p rivun-cli -- capability list --config rivun.toml --json
cargo run -p rivun-cli -- capability inspect-manifest --manifest examples/wasm-drivers/echo/echo.manifest.toml --json
cargo run -p rivun-cli -- capability query --config rivun.toml --target <uuid> --cache .rivun/capabilities.jsonl --json
cargo run -p rivun-cli -- capability cache refresh --config rivun.toml --json --strict
cargo run -p rivun-cli -- capability cache verify --path .rivun/capabilities.jsonl
cargo run -p rivun-cli -- capability cache list --path .rivun/capabilities.jsonl --peer <uuid> --json
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
`capability_cache_enabled`, and `peer_grant_route_count`. `rivun doctor` turns
those counts into readiness checks.

Remote capability responses can be cached locally with `capability query
--cache`. For normal operations, prefer `capability cache refresh --config
rivun.toml --strict` so every configured peer is refreshed into
`[capability_cache].path` before deployment gates run. The refresh report
includes per-peer `ok`, `skipped`, and `failed` status and respects local peer
trust policy. The cache is append-only JSONL with entry hash chaining; verify
it before using it for deployment review or routing decisions.

Require cached peer grants before forwarding selected messages:

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
peer = "peer-node-id"
```

`rivun check-config --strict` fails when the cache is missing, corrupt, stale
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
cargo run -p rivun-cli -- route explain --config rivun.toml --kind action --subject echo --json
```

Local memory is an append-only binary journal with body hashes, entry hash
chaining, disk indexes, and tombstones:

```toml
[memory]
dir = ".rivun/memory"
max_record_bytes = 1048576
allow_driver_read = false
allow_driver_write = false
```

Operate on the store:

```bash
cargo run -p rivun-cli -- memory put --dir .rivun/memory --subject note --payload hello
cargo run -p rivun-cli -- memory get --dir .rivun/memory <record-id> --json
cargo run -p rivun-cli -- memory query --dir .rivun/memory --subject note --json
cargo run -p rivun-cli -- memory tombstone --dir .rivun/memory <record-id> --reason "superseded"
cargo run -p rivun-cli -- memory verify --dir .rivun/memory
cargo run -p rivun-cli -- memory prune --dir .rivun/memory --before-created-at-micros 1735689600000000 --out .rivun/memory.pruned
cargo run -p rivun-cli -- memory compact --dir .rivun/memory --out .rivun/memory.compacted
cargo run -p rivun-cli -- memory import-jsonl --in legacy-memory.jsonl --dir .rivun/memory
cargo run -p rivun-cli -- memory export-jsonl --dir .rivun/memory --out memory.archive.jsonl
cargo run -p rivun-cli -- memory export-evidence --dir .rivun/memory --receipts logs/receipts --manifest-out evidence.manifest.json --signing-key .rivun/node.key
```

Verification recalculates every body hash, validates the entry hash chain,
rejects duplicate entry ids, and rejects tombstones whose source record is
missing. Compaction writes a fresh verifiable journal.

`rivun send` is a one-shot peer process. It validates the config, binds to the
config `bind` address, sends one signed frame, and exits. This is deliberate:
receivers reject datagrams whose source address does not match the configured
peer address.

rivun can carry raw bytes or universal `ZENV` envelopes. `rivun-node` also accepts the older JSON action envelope for compatibility, but new CLI sends use `ZENV`.

For daemon driver execution, send a universal action envelope:

```bash
cargo run -p rivun-cli -- send --config rivun.toml --target <uuid> --action echo --payload hello
```

For a universal event envelope:

```bash
cargo run -p rivun-cli -- send --config rivun.toml --target <uuid> --kind event --subject sensor.temperature --payload '{"c":21.5}' --content-type application/json
```

Inline metadata is accepted as bytes, so JSON and plain text are both valid:

```bash
cargo run -p rivun-cli -- send --config rivun.toml --target <uuid> --kind event --subject sensor.temperature --payload '{"c":21.5}' --content-type application/json --metadata '{"source":"sim"}'
```

Agents, models, and operator tools should send typed `ZENV` messages directly:

```bash
cargo run -p rivun-cli -- send --config rivun.toml --target <uuid> \
  --kind action --subject thermostat.setpoint \
  --payload '{"temperature_c":20}' --content-type application/json
```

Critical actions, such as `safety.emergency_stop`, should be protected by
receiver-side `[message_policy]` and sent with `REQUIRES_CONSENSUS` plus a PoA
certificate:

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
subject = "telemetry.*"
decision = "allow"
reason = "telemetry is read-only"
```

If `default_decision` is omitted, receivers keep the historical `allow`
behavior for unmatched messages. Set it to `deny` to fail closed.

```bash
cargo run -p rivun-cli -- send --config rivun.toml --target <uuid> \
  --kind action --subject safety.emergency_stop \
  --payload '{"reason":"operator_request"}' --content-type application/json \
  --requires-consensus --poa-validator-key .rivun/validator.key
```

Or request attestations from configured validator peers:

```bash
cargo run -p rivun-cli -- send --config rivun.toml --target <uuid> \
  --kind action --subject safety.emergency_stop \
  --payload '{"reason":"operator_request"}' --content-type application/json \
  --requires-consensus --poa-network
cargo run -p rivun-cli -- send --config rivun.toml --target <uuid> \
  --kind action --subject safety.emergency_stop \
  --payload '{"reason":"operator_request"}' --content-type application/json \
  --requires-consensus --poa-network --poa-timeout-ms 5000
```

Each `[poa]` validator used by `--poa-network` must also be configured as a
peer so `rivun send` can reach it over encrypted UDP. Validator nodes answer
signed `poa.attestation_request` control envelopes with signed
`poa.attestation_response` envelopes. `--poa-timeout-ms` controls how long the
sender waits for enough validator responses; the default is 2000 ms.

For offline review and future validator workflows, create and sign portable PoA
attestation JSON:

```bash
cargo run -p rivun-cli -- poa request --frame critical-frame.bin --requester-key .rivun/node.key --threshold 1 > poa-request.json
cargo run -p rivun-cli -- poa attest --request poa-request.json --validator-key .rivun/validator.key > poa-response.json
```

For versioned validator-set distribution, sign the set with an authority key,
verify it, then apply it to a node config:

```bash
cargo run -p rivun-cli -- poa validator-set create \
  --authority-key .rivun/operator.key \
  --epoch 4 \
  --threshold 2 \
  --validator <validator-a-node-id>=<validator-a-public-key> \
  --validator <validator-b-node-id>=<validator-b-public-key> \
  --label production \
  --out poa-validators.v4.json

cargo run -p rivun-cli -- poa validator-set verify \
  --path poa-validators.v4.json \
  --authority-public-key <operator-public-key> \
  --json

cargo run -p rivun-cli -- poa validator-set pull \
  --config rivun.toml \
  --target <peer-node-id> \
  --authority-public-key <operator-public-key> \
  --min-epoch 4 \
  --out poa-validators.v4.json \
  --json

cargo run -p rivun-cli -- poa validator-set apply \
  --config rivun.toml \
  --set poa-validators.v4.json \
  --authority-public-key <operator-public-key> \
  --out rivun.with-poa-set.toml \
  --json
```

Applied configs use `poa.validator_set` and `poa.validator_set_authority`.
`rivun-node` verifies the signed set at validation/startup, rejects invalid or
expired sets, and uses `max(poa.required_threshold, set.required_threshold)` as
the effective threshold. `validator-set pull` uses signed `ZENV` control
messages, verifies the peer response, verifies the nested validator-set
signature, and writes the received JSON unchanged.

For binary action payloads, use a file and mark the payload as opaque:

```bash
cargo run -p rivun-cli -- send --config rivun.toml --target <uuid> --action upload --payload-file payload.bin --binary-payload
```

Omit `--action` only when you intentionally want to send raw bytes that a `ZapEndpoint` peer, not `ZapNode` action dispatch, will consume directly.

Omit `--action` but include `--kind` when you want rivun to wrap the payload in a universal envelope. If `--content-type` is omitted, inline `--payload` defaults to `text/plain` and `--payload-file` defaults to `application/octet-stream`.

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

Signed receipts are optional and stored in an append-only binary journal:

```toml
[receipts]
dir = "logs/receipts"
```

Receipts are audit records signed by the processing node. They are not financial records.

Verify a receipt journal after a test run, pull peer receipts for an audit
window, or export a JSONL archive:

```bash
cargo run -p rivun-cli -- receipts verify --dir logs/receipts
cargo run -p rivun-cli -- receipts pull --config rivun.toml --target <peer-node-id> --out-dir logs/peer-receipts --json
cargo run -p rivun-cli -- receipts export-jsonl --dir logs/receipts --out logs/receipts.archive.jsonl
cargo run -p rivun-cli -- receipts compact --dir logs/receipts --out logs/receipts.compacted
```

## Agent, Discovery, and Pack Operations

### Agent protocol messages

The CLI builds validated agent protocol JSON without sending it. Use these for
fixtures, operator handoffs, or payloads that a later step wraps in `ZENV`:

```bash
cargo run -p rivun-cli -- agent session --owner-agent ops.lead --out session.json
cargo run -p rivun-cli -- agent intent --source-agent ops.lead --target-agent executor.safety --objective "open valve" --input '{"valve":"v-7"}' --capability driver.execute:valve.open --out intent.json
cargo run -p rivun-cli -- agent status --session-id <uuid> --agent-id executor.safety --status running --progress-per-mille 500
cargo run -p rivun-cli -- agent result --session-id <uuid> --intent-id <uuid> --agent-id executor.safety --status completed --outputs '{"ok":true}'
cargo run -p rivun-cli -- agent delegate --session-id <uuid> --to-agent agent.beta --objective "scoped work" --out delegation.json
cargo run -p rivun-cli -- agent negotiate --session-id <uuid> --required-capability driver.execute:valve.open --out negotiation.json
```

Validate and export schemas:

```bash
cargo run -p rivun-cli -- agent validate --input intent.json --subject rivun.agent.intent --json
cargo run -p rivun-cli -- agent schema --out agent.schema.json
```

See [Agent Protocol](agent-protocol.md) for the contracts.

### Discovery

```bash
# Send a signed service advertisement to a peer
cargo run -p rivun-cli -- discovery announce --config rivun.toml --target <uuid> \
  --service echo=driver.execute:echo --label production --json

# Query a peer for signed services, peers, and learned announcements
cargo run -p rivun-cli -- discovery query --config rivun.toml --target <uuid> \
  --capability driver.execute:echo --json
```

See [Discovery](discovery.md).

### Domain packs

```bash
# Scaffold, build, sign, verify, install, and audit a pack
cargo run -p rivun-cli -- pack init --dir my-pack --id rivun-pack-example --name "Example"
cargo run -p rivun-cli -- pack build --pack my-pack --out my-pack.zpack --json
cargo run -p rivun-cli -- pack sign --bundle my-pack.zpack --key .rivun/node.key --json
cargo run -p rivun-cli -- pack verify --bundle my-pack.zpack --signature my-pack.zpack.sig --public-key <base64-public-key> --json
cargo run -p rivun-cli -- pack install --bundle my-pack.zpack --store-dir .rivun/packs --trusted-key <base64-public-key> --json
cargo run -p rivun-cli -- pack audit --pack my-pack --max-risk medium --json

# Validate the bundled preview packs
cargo run -p rivun-cli -- pack list --root examples/domain-packs --json
cargo run -p rivun-cli -- pack validate --pack examples/domain-packs/agentic-dev --json
```

See [Domain Packs](domain-packs.md).

## Gateway Operations

Start the AI Agent Gateway (MCP over stdio and/or HTTP REST/SSE/WebSocket):

```bash
# HTTP gateway on 127.0.0.1:8080 with an auth token and receipt journal
cargo run -p rivun-cli -- gateway start \
  --config rivun.toml \
  --http-bind 127.0.0.1:8080 \
  --auth-token <token> \
  --journal-dir logs/receipts \
  --memory-dir .rivun/memory

# Add an MCP server over stdio for a local agent runtime
cargo run -p rivun-cli -- gateway start --mcp-stdio

# Check gateway status
cargo run -p rivun-cli -- gateway status --addr http://127.0.0.1:8080 --json
```

Verify a provenance chain digest and receipt journals with provenance:

```bash
cargo run -p rivun-cli -- provenance verify --chain chain.json --key .rivun/node.key --json
cargo run -p rivun-cli -- receipts verify --dir logs/receipts --provenance
```

See [Gateway](gateway.md) for transports, MCP tools, resources, prompts, and
the provenance chain reference.

### Fleet health

```bash
cargo run -p rivun-cli -- fleet doctor --config rivun.toml --json
cargo run -p rivun-cli -- fleet doctor --config rivun.toml --strict --json
```

See [Telemetry](telemetry.md).

### Cluster simulation and swarm tests

```bash
# In-memory 3-node cluster
cargo run -p rivun-cli -- cluster up --nodes 3 --duration-secs 5 --json
cargo run -p rivun-cli -- cluster status --nodes 3 --json

# High-throughput gossip consensus benchmark
cargo run -p rivun-cli -- swarm bench --nodes 4 --rate 1000 --duration-secs 3

# Byzantine partition chaos test
cargo run -p rivun-cli -- swarm partition-test --nodes 5 --partition-fraction 0.4 --json
```

See [Swarm](swarm.md) and [Network](network.md).

## Incident Runbooks

Use these runbooks when a production node enters degraded or critical state.
Before changing traffic, preserve the current config, receipt journal, registry
index, capability cache, validator-set file, driver manifest, and the last
operator command output. Do not prune or rewrite evidence while an incident is
open.

Capture a bounded local snapshot before remediation when the node host is still
reachable:

```bash
cargo run -p rivun-cli -- incident snapshot --config /etc/rivun/rivun.toml --out incidents/$(date +%Y%m%d-%H%M%S)-snapshot.json
```

The snapshot embeds `doctor` output, redacted config readiness counts, memory
verification summaries, receipt summaries, and capability-cache verification
when those paths are configured or passed explicitly. It omits key material,
transport keys, raw memory payloads, memory metadata, raw receipt signatures,
and live packet captures; archive the referenced source files separately.

Embedding services can expose the node health surface directly from
`ZapNode::health_snapshot()`, `ZapNode::health_json()`, or
`ZapNode::healthz_text()`. Daemons can expose the same contract by setting
`[observability].http_bind`; `rivun run` then serves `/metrics`, `/healthz`, and
`/healthz.json` on that TCP address. Treat `critical` as a traffic-freeze signal
and use the named check to choose the runbook below. Treat `degraded` as an
operator investigation signal unless the same check is rising across the fleet.

### Policy Default Allow In Production

Trigger:

- `rivun doctor --strict` reports a policy readiness failure;
- `[message_policy].default_decision` is missing or set to `allow` on a
  production receiver;
- a high-risk action matched no explicit rule and was accepted.

Immediate checks:

```bash
cargo run -p rivun-cli -- doctor --config /etc/rivun/rivun.toml --strict --json
cargo run -p rivun-cli -- check-config --config /etc/rivun/rivun.toml --json
cargo run -p rivun-cli -- policy evaluate --policy /etc/rivun/policy.toml --kind action --subject safety.emergency_stop --requires-consensus --strict --json
```

Containment:

- freeze new driver installs, route changes, and automated action rollout;
- set `default_decision = "deny"` in the production receiver config;
- add explicit `allow`, `deny`, `require_poa`, or approval rules for known
  subjects before restart;
- restart only after `doctor --strict` and a targeted `policy evaluate` pass.

Rollback criteria:

- rollback to the last config whose policy section is fail-closed and whose
  receipt journal shows expected denials for unknown subjects;
- keep the permissive config as incident evidence, not as a fallback.

Preserve:

- old and new `rivun.toml`;
- `doctor --json` and `policy evaluate --json` output;
- receipt records for accepted unmatched actions;
- operator approval notes for any temporary exception.

### Registry Signature Invalid

Trigger:

- `@@rivun_HEADER@@registry_signature_valid` is `0`;
- `registry verify-signature` fails;
- a pull, mirror, add, deprecate, migration, or revoke command cleared the
  operator signature.

Immediate checks:

```bash
cargo run -p rivun-cli -- registry verify-signature --registry /var/lib/rivun/registry.index.toml
cargo run -p rivun-cli -- registry publication verify --registry /var/lib/rivun/registry.index.toml --publication /var/lib/rivun/registry.publication.json --json
cargo run -p rivun-cli -- registry bundle verify --bundle /var/lib/rivun/RivunStore-bundle --require-drivers --json
```

Containment:

- block new driver installs and automatic registry resolution;
- keep running drivers pinned to already verified manifests;
- compare the invalid index with the last signed publication hash;
- re-sign only after operator review of every registry entry and copied driver
  artifact.

Rollback criteria:

- restore the last registry index whose operator signature, publication, bundle
  hashes, and driver manifests all verify;
- revoke a bad release instead of silently replacing it when there is evidence
  it reached a node.

Preserve:

- invalid `registry.index.toml`;
- publication JSON, bundle manifest, and copied driver artifacts;
- `registry verify-signature`, `publication verify`, and `bundle verify`
  output;
- release notes or operator approval for the registry mutation.

### Stale Capability Cache

Trigger:

- `@@rivun_HEADER@@capability_cache_age_seconds` exceeds the deployment policy;
- `check-config --strict` reports stale or missing peer grants;
- a route with `requires_peer_grant` cannot prove the target advertisement.

Immediate checks:

```bash
cargo run -p rivun-cli -- capability cache verify --path /var/lib/rivun/capabilities.jsonl
cargo run -p rivun-cli -- capability cache list --path /var/lib/rivun/capabilities.jsonl --json
cargo run -p rivun-cli -- capability cache refresh --config /etc/rivun/rivun.toml --strict --json
cargo run -p rivun-cli -- check-config --config /etc/rivun/rivun.toml --strict --json
```

Containment:

- disable or hold routes that depend on `requires_peer_grant`;
- do not infer grants from old advertisements;
- refresh from trusted peers only and investigate peers that fail refresh.

Rollback criteria:

- route traffic only after the cache verifies, each required peer advertises the
  expected capability, and `check-config --strict` passes;
- rollback route changes if a target peer no longer grants the required
  capability.

Preserve:

- stale cache JSONL;
- refreshed cache JSONL;
- per-peer refresh report;
- route explain output for affected subjects.

### Receipt Verification Failure

Trigger:

- `@@rivun_HEADER@@receipt_log_verify_failures_total` increases;
- `receipts verify` fails on the local or pulled receipt journal;
- audit records are missing, duplicated, or hash-chain verification fails.

Immediate checks:

```bash
cargo run -p rivun-cli -- receipts verify --dir /var/lib/rivun/receipts
cargo run -p rivun-cli -- receipts pull --config /etc/rivun/rivun.toml --target <peer-node-id> --out-dir /var/lib/rivun/incidents/peer-receipts --json
cargo run -p rivun-cli -- receipts export-jsonl --dir /var/lib/rivun/receipts --out /var/lib/rivun/incidents/receipts-local.jsonl
cargo run -p rivun-cli -- receipts export-jsonl --dir /var/lib/rivun/incidents/peer-receipts --out /var/lib/rivun/incidents/receipts-peer.jsonl
```

Containment:

- stop receipt archival compaction;
- copy the failing journal directory to incident storage before daemon restart;
- if the node must continue processing, cut over to a new receipt directory only
  after preserving the broken directory.

Rollback criteria:

- rollback to the last deployment whose receipt journal verifies from the first
  retained record through the current audit window;
- pause automated actions if receipts cannot prove policy, PoA, and driver
  outcomes for high-risk messages.

Preserve:

- failing receipt journal directory byte-for-byte;
- pulled peer receipts for the same time window;
- storage and host incident logs;
- command output from every `receipts verify`, `pull`, and `merge` command.

### PoA Attestation Failure

Trigger:

- `@@rivun_HEADER@@poa_attestation_failures_total` increases;
- critical actions time out with `--poa-network`;
- validator-set verification, threshold, or attestation signatures fail.

Immediate checks:

```bash
cargo run -p rivun-cli -- poa validator-set verify --path /etc/rivun/poa-validators.json --authority-public-key <operator-public-key> --json
cargo run -p rivun-cli -- trust inspect --config /etc/rivun/rivun.toml --json
cargo run -p rivun-cli -- doctor --config /etc/rivun/rivun.toml --strict --json
```

For an offline frame review:

```bash
cargo run -p rivun-cli -- poa request --frame critical-frame.bin --requester-key .rivun/node.key --threshold 1 > poa-request.json
cargo run -p rivun-cli -- poa attest --request poa-request.json --validator-key .rivun/validator.key > poa-response.json
```

Containment:

- stop retry loops for critical actions until validator reachability and set
  epoch are understood;
- verify validator peer trust allows PoA attestation;
- check clock skew before rotating validators.

Rollback criteria:

- use the last signed validator set whose epoch, authority signature, threshold,
  and peer trust all verify;
- do not lower the effective threshold as a rollback unless an approved incident
  exception exists.

Preserve:

- failing validator-set JSON;
- PoA request and response JSON;
- `doctor --json` and `trust inspect --json` output;
- receipt records for denied or timed-out critical actions.

### Driver Runtime Errors

Trigger:

- `@@rivun_HEADER@@driver_execution_errors_total` increases;
- driver execution latency exceeds the runtime budget;
- a WASM/WAT driver fails ABI validation, fuel, memory, timeout, or output
  limits.

Immediate checks:

```bash
cargo run -p rivun-cli -- driver-manifest verify --driver /var/lib/rivun/drivers/<driver>.wasm --manifest /var/lib/rivun/manifests/<driver>.manifest.toml
cargo run -p rivun-cli -- registry resolve --registry /var/lib/rivun/registry.index.toml --action <action> --version-req '^1.0.0' --abi-req '>=1,<=2' --json
cargo run -p rivun-cli -- route explain --config /etc/rivun/rivun.toml --kind action --subject <action> --json
cargo run -p rivun-cli -- check-config --config /etc/rivun/rivun.toml --strict --json
```

Containment:

- stop routing new traffic to the affected action when errors are persistent;
- revoke unsafe driver versions or deprecate noisy versions with migration
  guidance;
- keep unrelated actions running only if route and receipt evidence show the
  blast radius is isolated.

Rollback criteria:

- restore the last active registry entry whose manifest verifies and whose
  receipts show successful execution under the same runtime limits;
- if the driver requires host imports, rollback to the last manifest with the
  approved scoped host permissions.

Preserve:

- driver artifact, manifest, registry entry, and install plan;
- route explain output;
- runtime limit configuration;
- receipts and logs for failed executions.

### Replay Spikes

Trigger:

- `@@rivun_HEADER@@frames_rejected_total` rises with replay or nonce-related rejection
  reasons;
- peers report duplicate sends after restart or network retry storms;
- clock skew or transport-key epoch changes line up with rejection spikes.

Immediate checks:

```bash
cargo run -p rivun-cli -- doctor --config /etc/rivun/rivun.toml --strict --json
cargo run -p rivun-cli -- trust inspect --config /etc/rivun/rivun.toml --json
cargo run -p rivun-cli -- check-config --config /etc/rivun/rivun.toml --json
```

Containment:

- freeze peer key rotation and topology changes until the spike source is
  identified;
- quarantine peers that replay frames after their transport key epoch changed;
- preserve sender logs before increasing replay cache capacity.

Rollback criteria:

- rollback a peer trust or transport-key change if replay rejections began at
  the same epoch and stop after restoring the previous trusted material;
- keep the stricter replay policy unless it is proven to reject fresh traffic.

Preserve:

- sender and receiver logs for the spike window;
- peer trust config before and after the change;
- `doctor`, `trust inspect`, and `check-config` JSON;
- representative rejected frames if available through safe packet capture.

## Logging

Use `RUST_LOG`:

```bash
RUST_LOG=info cargo run -p rivun-cli -- run --config rivun.toml
```

## Inspecting Frames

```bash
cargo run -p rivun-cli -- inspect frame.bin
cargo run -p rivun-cli -- inspect frame.bin --verify-with-public-key <base64-public-key>
```

The inspect command decodes raw rivun frames, not encrypted UDP datagrams. When the frame payload is a `ZENV` envelope, inspect also prints:

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
cargo run -p rivun-cli -- keygen --out .rivun/node-a.key
cargo run -p rivun-cli -- keygen --out .rivun/node-b.key
```

2. Copy `node_id` and `public_key` values into `examples/configs/node-a.toml` and `examples/configs/node-b.toml`.

3. Run node A:

```bash
cargo run -p rivun-cli -- run --config examples/configs/node-a.toml
```

4. In another terminal, send from node B:

```bash
cargo run -p rivun-cli -- send --config examples/configs/node-b.toml --target <node-a-uuid> --action echo --payload hello
```

