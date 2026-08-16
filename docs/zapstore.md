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

Generate a signed manifest. The manifest file is written by this command; it
is not part of the source checkout:

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
cargo run -p zap-cli -- registry deprecate \
  --registry registry.index.toml \
  --action echo \
  --version 0.1.0 \
  --reason "use 0.2.0"
cargo run -p zap-cli -- registry migration add \
  --registry registry.index.toml \
  --action echo \
  --version 2.0.0 \
  --from-version-req '^1.0.0' \
  --from-abi-req '=1' \
  --requires-operator-approval \
  --migration-driver echo-migrate@0.1.0
cargo run -p zap-cli -- registry sign \
  --registry registry.index.toml \
  --operator-key .zap/node.key
cargo run -p zap-cli -- registry verify-signature \
  --registry registry.index.toml
cargo run -p zap-cli -- registry list --registry registry.index.toml --json
cargo run -p zap-cli -- registry resolve \
  --registry registry.index.toml \
  --action echo \
  --version-req '^0.1.0' \
  --abi-req '>=1,<=2' \
  --json
cargo run -p zap-cli -- registry pull \
  --config zap.toml \
  --target <peer-node-id> \
  --out registry.index.toml \
  --operator-public-key <base64-public-key> \
  --json
cargo run -p zap-cli -- registry mirror \
  --config zap.toml \
  --out mirrored-registry.index.toml \
  --operator-public-key <base64-public-key> \
  --json
cargo run -p zap-cli -- registry sign \
  --registry mirrored-registry.index.toml \
  --operator-key .zap/node.key
cargo run -p zap-cli -- registry publication create \
  --registry mirrored-registry.index.toml \
  --publisher-key .zap/node.key \
  --out registry.publication.json \
  --channel stable
cargo run -p zap-cli -- registry publication verify \
  --registry mirrored-registry.index.toml \
  --publication registry.publication.json
cargo run -p zap-cli -- registry plan create \
  --registry mirrored-registry.index.toml \
  --publication registry.publication.json \
  --planner-key .zap/node.key \
  --out registry.install-plan.json \
  --driver 'echo@^0.1.0' \
  --abi-req '>=1,<=2' \
  --json
cargo run -p zap-cli -- registry plan verify \
  --registry mirrored-registry.index.toml \
  --plan registry.install-plan.json \
  --planner-public-key <base64-public-key>
cargo run -p zap-cli -- registry bundle export \
  --registry mirrored-registry.index.toml \
  --publication registry.publication.json \
  --out zapstore-bundle \
  --driver echo@0.1.0=examples/wasm-drivers/echo/echo.wat \
  --json
cargo run -p zap-cli -- registry bundle pull-manifest \
  --config zap.toml \
  --target <peer-node-id> \
  --out pulled-zapstore.bundle.json \
  --require-publication \
  --require-drivers \
  --json
cargo run -p zap-cli -- registry bundle verify \
  --bundle zapstore-bundle \
  --require-drivers
cargo run -p zap-cli -- registry bundle import \
  --bundle zapstore-bundle \
  --out .zap/imported-zapstore
```

When configured, `zap-node` validates signed driver manifests against the local
registry before startup. Active entries must match name, version, ABI, hash, and
author node id. Revoked entries are rejected. Deprecated entries remain
verifiable for existing pinned deployments but are skipped by automatic
`registry resolve` selection so new installs migrate forward.

Registry signatures are optional by default for compatibility with existing
local indexes. Production configs can require an operator signature:

```toml
[registry]
path = "registry.index.toml"
require_signature = true
bundle_path = "zapstore-bundle"
```

`check-config --json` includes `registry_enabled`, `registry_entry_count`, and
`registry_signature_required` for deployment gates. It also reports
`registry_bundle_enabled` when a `bundle_path` publishes a local
`zapstore.bundle.json`.

Any registry mutation, including `add`, `deprecate`, `migration add`, and
`revoke`, clears the operator signature. Re-run `registry sign` after reviewing
the changed index.

`zap registry resolve` selects the highest active entry for an action that
matches a semantic version requirement and optional ABI requirement. Version
requirements support `*`, exact `1.2.3` or `=1.2.3`, caret ranges such as
`^1.2.3`, tilde ranges such as `~1.2.3`, and comma-separated comparators such
as `>=1.0.0,<2.0.0`. ABI requirements use the same comparator grammar over
integer ABI versions, for example `=1` or `>=1,<=2`. The older
`--abi-version` flag remains an exact ABI shortcut. Resolution ignores revoked
and deprecated entries and requires `MAJOR.MINOR.PATCH` versions so automated
installers and machine agents do not guess between incompatible driver releases.

Registry entries can also carry migration metadata. `zap registry migration add`
records the source version requirement, optional source ABI requirement,
operator-approval requirement, optional migration driver, and notes on the
target entry. The metadata is covered by the registry operator signature and is
copied into install plans, so rollout tools can see when a selected package
requires an explicit migration step.

Nodes with `[registry].path` respond to `zap.registry.index.request` control
messages with their current registry index. Use `zap registry pull` from another
configured peer to fetch that index over the signed ZAP transport. Passing
`--operator-public-key` implies `--require-signature` and rejects indexes that
were not approved by the expected operator key.

Use `zap registry mirror` to fetch every send-allowed peer, or repeat `--peer`
to select specific peers, and merge the returned indexes into one local file.
The merge is intentionally strict: matching action/version entries must agree
on name, ABI, artifact hash, and author; revoked entries override active
entries for the same driver version. The merged file is unsigned, so review it
and run `registry sign` before using it in production configs.

Publication metadata is a signed JSON statement over the canonical BLAKE3 hash
of a signed registry index. `registry publication create` refuses unsigned
registry indexes, records the registry operator node, entry count, publication
timestamp, channel, and labels, then signs that statement with the publisher key.
`registry publication verify` recomputes the registry hash and verifies the
publisher signature, giving release pipelines an immutable approval artifact to
archive next to the registry file.

Install plans are signed JSON deployment intents over a signed registry hash.
`registry plan create` takes one or more `--driver action@version-req` requests,
resolves each request to the highest active compatible entry, records the
selected version/hash/manifest/ABI metadata, optional ABI requirement, migration
metadata, optionally binds a publication hash, and signs the plan with the
planner key. `registry plan verify` rechecks the planner signature, registry
operator signature, registry hash, and every selected entry before a CI job or
machine installer trusts the plan.

Registry bundles are filesystem directories with `zapstore.bundle.json`,
`registry.index.toml`, optional `registry.publication.json`, copied driver
manifests, and optional driver artifacts. `registry bundle export` verifies the
signed registry, publication metadata, manifest signatures, and supplied driver
hashes before writing the bundle. `registry bundle verify` repeats those checks
offline and can require that every entry carries a driver artifact. `registry
bundle import` verifies first, then copies only the listed safe relative bundle
paths into the destination directory.

Nodes with `[registry].bundle_path` answer
`zap.registry.bundle.manifest.request` control messages with the bundle
manifest at `bundle_path/zapstore.bundle.json`. `zap registry bundle
pull-manifest` retrieves that manifest over signed ZAP transport and can require
publication metadata and driver artifact hashes. This is the manifest-first
distribution layer: use it for discovery and preflight, then verify/import the
actual bundle directory with the existing checksum gates.

## Driver SDK

`zap-driver-sdk` exposes ABI v1 constants, result packing helpers, and a small
`ZapDriver` trait for Rust driver authors. It is intentionally minimal: the
runtime still enforces the exported WASM ABI and has no host capabilities in
ABI v1.
