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
cargo run -p zap-cli -- check-config --strict --config zap.toml
```

For automation:

```bash
cargo run -p zap-cli -- check-config --config zap.toml --json
```

The validator checks:

- local bind address syntax;
- key file readability and parseability;
- peer address syntax and duplicates;
- peer `public_key` derives the configured `node_id`;
- transport key length and nonzero value;
- runtime/security limits are nonzero where required;
- duplicate driver actions;
- WASM/WAT driver files compile and expose ABI v1 before daemon startup;
- signed driver manifests match the configured action, local driver hash, ABI version, and author signature when `manifest` is configured.

During daemon startup, configured drivers are compiled, ABI-validated, and kept in memory. Updating a driver file requires a daemon restart.

## Config

Node configs are TOML files with:

- local bind address;
- local key file path;
- static peer list;
- optional registered WASM drivers;
- runtime limits.
- anti-replay policy.
- optional signed receipt log path.
- optional local ZapStore registry index path.

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

Create and verify a signed manifest:

```bash
cargo run -p zap-cli -- driver-manifest create --driver examples/wasm-drivers/echo/echo.wat --action echo --author-key .zap/node.key --out examples/wasm-drivers/echo/echo.manifest.toml
cargo run -p zap-cli -- driver-manifest verify --driver examples/wasm-drivers/echo/echo.wat --manifest examples/wasm-drivers/echo/echo.manifest.toml
```

`check-config --json` includes `signed_driver_count` so deploy scripts can require signed driver provenance.
It also includes `registry_enabled`, `registry_entry_count`, and
`registry_signature_required` when a local ZapStore registry is configured.

Create a local registry index and add a signed manifest:

```bash
cargo run -p zap-cli -- registry init --out registry.index.toml
cargo run -p zap-cli -- registry add --registry registry.index.toml --manifest examples/wasm-drivers/echo/echo.manifest.toml
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
local registry was not approved by an operator key.

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

For PDF Phase 2-style intent compilation, inspect the plan first, then send:

```bash
cargo run -p zap-cli -- compile-intent "Ajuster la temperature a 20" --explain
cargo run -p zap-cli -- send --config zap.toml --target <uuid> --intent "Ajuster la temperature a 20"
```

Apply an optional JSON policy before sending:

```bash
cargo run -p zap-cli -- send --config zap.toml --target <uuid> --intent "Ajuster la temperature a 20" --policy policy.json
```

Critical intent steps, such as `safety.emergency_stop`, set `REQUIRES_CONSENSUS` and need a local PoA certificate:

```bash
cargo run -p zap-cli -- send --config zap.toml --target <uuid> --intent "declencher arret urgence robot" --poa-validator-key .zap/validator.key
```

Or request attestations from configured validator peers:

```bash
cargo run -p zap-cli -- send --config zap.toml --target <uuid> --intent "declencher arret urgence robot" --poa-network
cargo run -p zap-cli -- send --config zap.toml --target <uuid> --intent "declencher arret urgence robot" --poa-network --poa-timeout-ms 5000
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

Signed receipts are optional and append-only JSONL:

```toml
[receipts]
path = "logs/actions.jsonl"
```

Receipts are audit records signed by the processing node. They are not financial records.

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
