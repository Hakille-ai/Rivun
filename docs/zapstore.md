# ZapStore Manifests

ZapStore v1 starts as an offline, signed manifest format for WASM action drivers. It does not pretend to be the future global registry yet; it gives nodes a real provenance and integrity check today.

## Manifest Contract

A driver manifest binds:

- one action name;
- one driver artifact hash;
- one WASM ABI version;
- declared host permissions defined by `zap-capability`;
- one author Ed25519 identity;
- one author signature.

The manifest is stored as TOML, but the signature covers a deterministic JSON signing payload with the domain prefix `ZAP-DRIVER-MANIFEST-v1`.

```toml
schema_version = 1
name = "echo-driver"
version = "0.1.0"
action = "echo"
abi_version = 1
wasm_hash = "blake3:..."
author_node_id = "..."
author_public_key = "..."
signature = "..."

[permissions]
network = false
filesystem = false
clock = false
environment = false
```

The node verifies the manifest before startup dispatch:

- schema version is supported;
- ABI version is supported;
- manifest action equals the configured driver action;
- `wasm_hash` matches the local driver bytes;
- author public key derives `author_node_id`;
- author signature is valid.

Current ABI v1 has no general host imports, so drivers that request `network`,
`filesystem`, `clock`, or `environment` are rejected during `check-config` and
daemon startup. Capability discovery can advertise declarations, but it does
not grant host access by itself.

## CLI

Generate a signed manifest:

```bash
cargo run -p zap-cli -- driver-manifest create \
  --driver examples/wasm-drivers/echo/echo.wat \
  --action echo \
  --author-key .zap/node.key \
  --out examples/wasm-drivers/echo/echo.manifest.toml
```

Verify it later:

```bash
cargo run -p zap-cli -- driver-manifest verify \
  --driver examples/wasm-drivers/echo/echo.wat \
  --manifest examples/wasm-drivers/echo/echo.manifest.toml
```

Use it from a node config:

```toml
[[drivers]]
action = "echo"
path = "../wasm-drivers/echo/echo.wat"
manifest = "../wasm-drivers/echo/echo.manifest.toml"
```

`check-config --json` reports `signed_driver_count` so automation can fail policies that require all drivers to be signed.

## Local Registry Index

`registry.index.toml` is a local index over signed manifests. It does not replace
manifest signatures; it adds operator policy such as approved versions and
revocation.

```bash
cargo run -p zap-cli -- registry init --out registry.index.toml
cargo run -p zap-cli -- registry add \
  --registry registry.index.toml \
  --manifest examples/wasm-drivers/echo/echo.manifest.toml
cargo run -p zap-cli -- registry verify \
  --registry registry.index.toml \
  --manifest examples/wasm-drivers/echo/echo.manifest.toml
cargo run -p zap-cli -- registry revoke \
  --registry registry.index.toml \
  --action echo \
  --version 0.1.0 \
  --reason "bad release"
cargo run -p zap-cli -- registry sign \
  --registry registry.index.toml \
  --operator-key .zap/node.key
cargo run -p zap-cli -- registry verify-signature \
  --registry registry.index.toml
cargo run -p zap-cli -- registry list --registry registry.index.toml --json
```

When configured, `zap-node` validates signed driver manifests against the local
registry before startup. Active entries must match name, version, ABI, hash, and
author node id. Revoked entries are rejected.

Registry signatures are optional by default for compatibility with existing
local indexes. Production configs can require an operator signature:

```toml
[registry]
path = "registry.index.toml"
require_signature = true
```

`check-config --json` includes `registry_enabled`, `registry_entry_count`, and
`registry_signature_required` for deployment gates.

Any registry mutation, including `add` and `revoke`, clears the operator
signature. Re-run `registry sign` after reviewing the changed index.

## Driver SDK

`zap-driver-sdk` exposes ABI v1 constants, result packing helpers, and a small
`ZapDriver` trait for Rust driver authors. It is intentionally minimal: the
runtime still enforces the exported WASM ABI and has no host capabilities in
ABI v1.
