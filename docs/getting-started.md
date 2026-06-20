# Getting Started with ZAP

This five-minute path gets a fresh checkout to a verified local ZAP toolchain.
It focuses on deterministic checks first, then points to the two-node smoke
test for live dispatch.

For install variants and Docker packaging, see [Install ZAP](install.md).

## Prerequisites

- Rust 1.93+ with the toolchain from `rust-toolchain.toml`
- Git
- Node.js 24 if you want to validate the website or TypeScript SDK
- Go 1.23 if you want to validate the Go SDK
- Docker only if you want to build the container image

## Five-Minute Source Check

Clone and build the CLI:

```bash
git clone https://github.com/Hakille-ai/ZAP.git
cd ZAP
cargo build --locked -p zap-cli
```

Expected result:

```text
Finished dev [unoptimized + debuginfo] target(s)
```

Verify the stable protocol fixtures:

```bash
cargo run --locked -p zap-cli -- fixtures verify --fixtures fixtures
```

Expected result:

```text
fixtures=fixtures files=14 valid=true
```

Validate the bundled domain packs:

```bash
cargo run --locked -p zap-cli -- pack list --root examples/domain-packs
```

Expected result:

```text
root=examples/domain-packs packs=7 valid=true
```

Run the quick CLI smoke test:

```bash
cargo ci-smoke
```

Expected result:

```text
test result: ok
```

## Config Checks

The example configs are useful for operator validation, but they reference local
state under `.zap/`, which is intentionally ignored by Git. In a prepared local
checkout with matching sample keys, validate them with:

```bash
cargo run --locked -p zap-cli -- check-config --config examples/configs/node-a.toml
cargo run --locked -p zap-cli -- check-config --config examples/configs/node-b.toml
```

Expected result in a prepared checkout:

```text
valid=true
```

In a fresh clone, use `cargo ci-smoke` first because it creates isolated test
keys and configs automatically. Use `doctor` when you want the production
readiness score and warnings for a prepared config:

```bash
cargo run --locked -p zap-cli -- doctor --config examples/configs/node-a.toml
```

`doctor --strict` is intentionally stronger than syntax validation. It can fail
preview configs that are safe for examples but incomplete for production.

## Two-Node Dispatch

For a live local dispatch, prefer the smoke test while developing:

```bash
cargo ci-smoke
```

It launches a node, sends an action, and verifies the resulting receipt without
requiring manual key or peer editing.

To run the nodes yourself, use the checked configs as templates:

```bash
cargo run --locked -p zap-cli -- run --config examples/configs/node-a.toml
```

In another terminal:

```bash
cargo run --locked -p zap-cli -- send --config examples/configs/node-b.toml \
  --target <node-a-uuid> \
  --action echo \
  --payload "Hello ZAP"
```

Do not run `zap run` and `zap send` from the same config at the same time:
`zap send` binds to the config `bind` address so the receiver can enforce static
peer addresses.

## SDK Conformance

Run the shared fixture verifier against an SDK path:

```bash
cargo run --locked -p zap-cli -- fixtures verify --fixtures fixtures --sdk sdks/typescript --json
cargo run --locked -p zap-cli -- fixtures verify --fixtures fixtures --sdk sdks/python --json
cargo run --locked -p zap-cli -- fixtures verify --fixtures fixtures --sdk sdks/go --json
cargo run --locked -p zap-cli -- fixtures verify --fixtures fixtures --sdk sdks/rust --json
```

Then run each SDK's native test command:

```bash
PYTHONPATH=sdks/python/src python -m unittest discover -s sdks/python/tests
npm --prefix sdks/typescript run typecheck
npm --prefix sdks/typescript run build:types
npm --prefix sdks/typescript test
go test ./sdks/go/...
cargo test --manifest-path sdks/rust/Cargo.toml
```

## When to Use ZAP

Use ZAP when you need typed messages, cryptographic node identity, deterministic
policy, sandboxed execution, replay protection, Proof-of-Action gates, or signed
receipts around distributed actions.

Do not use ZAP as a general database, a natural-language agent planner, a
financial ledger, a replacement for every broker/RPC stack, or a way to bypass
the identity, policy, PoA, grant, and receipt model.
