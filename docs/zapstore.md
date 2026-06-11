# ZapStore Manifests

ZapStore v1 starts as an offline, signed manifest format for WASM action drivers. It does not pretend to be the future global registry yet; it gives nodes a real provenance and integrity check today.

## Manifest Contract

A driver manifest binds:

- one action name;
- one driver artifact hash;
- one WASM ABI version;
- declared host permissions;
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

Current ABI v1 has no host imports, so drivers that request `network`, `filesystem`, `clock`, or `environment` are rejected during `check-config` and daemon startup. This keeps capability declarations honest until host APIs are implemented.

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
cargo run -p zap-cli -- registry list --registry registry.index.toml --json
```

When configured, `zap-node` validates signed driver manifests against the local
registry before startup. Active entries must match name, version, ABI, hash, and
author node id. Revoked entries are rejected.

```toml
[registry]
path = "registry.index.toml"
```

`check-config --json` includes `registry_enabled` and `registry_entry_count` for
deployment gates.

## Driver SDK

`zap-driver-sdk` exposes ABI v1 constants, result packing helpers, and a small
`ZapDriver` trait for Rust driver authors. It is intentionally minimal: the
runtime still enforces the exported WASM ABI and has no host capabilities in
ABI v1.
