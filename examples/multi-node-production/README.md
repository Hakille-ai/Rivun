# Production-Style Multi-Node Example

This example shows the operator sequence for a small two-node ZAP deployment
with signed driver provenance, receipt journals, capability cache review, and
observability assets.

The TOML files are templates. Generate real node keys before running them.

## 1. Generate Node Keys

```bash
cargo run -p zap-cli -- keygen --out .zap/node-a.key
cargo run -p zap-cli -- keygen --out .zap/node-b.key
```

Copy each generated `node_id` and `public_key` into:

- `examples/multi-node-production/node-a.toml`;
- `examples/multi-node-production/node-b.toml`.

Use a unique 32-byte transport key per peer relationship.

## 2. Sign Driver Manifests

```bash
cargo run -p zap-cli -- driver-manifest create \
  --driver examples/wasm-drivers/echo/echo.wat \
  --action echo \
  --author-key .zap/node-a.key \
  --out examples/multi-node-production/manifests/echo.manifest.toml

cargo run -p zap-cli -- driver-manifest create \
  --driver examples/wasm-drivers/thermostat/thermostat.wat \
  --action thermostat.setpoint \
  --author-key .zap/node-b.key \
  --out examples/multi-node-production/manifests/thermostat.manifest.toml
```

## 3. Prepare Registry Bundle

```bash
cargo run -p zap-cli -- registry init --out examples/multi-node-production/registry.index.toml
cargo run -p zap-cli -- registry add \
  --registry examples/multi-node-production/registry.index.toml \
  --manifest examples/multi-node-production/manifests/echo.manifest.toml
cargo run -p zap-cli -- registry add \
  --registry examples/multi-node-production/registry.index.toml \
  --manifest examples/multi-node-production/manifests/thermostat.manifest.toml
cargo run -p zap-cli -- registry sign \
  --registry examples/multi-node-production/registry.index.toml \
  --operator-key .zap/node-a.key
```

Create publication metadata and an install plan:

```bash
cargo run -p zap-cli -- registry publication create \
  --registry examples/multi-node-production/registry.index.toml \
  --publisher-key .zap/node-a.key \
  --out examples/multi-node-production/registry.publication.json \
  --channel stable \
  --json

cargo run -p zap-cli -- registry plan create \
  --registry examples/multi-node-production/registry.index.toml \
  --publication examples/multi-node-production/registry.publication.json \
  --planner-key .zap/node-a.key \
  --out examples/multi-node-production/registry.install-plan.json \
  --driver 'echo@^0.1.0' \
  --driver 'thermostat.setpoint@^0.1.0' \
  --json
```

## 4. Validate Before Start

```bash
cargo run -p zap-cli -- doctor --config examples/multi-node-production/node-a.toml --strict --json
cargo run -p zap-cli -- doctor --config examples/multi-node-production/node-b.toml --strict --json
cargo run -p zap-cli -- capability cache refresh --config examples/multi-node-production/node-a.toml --strict --json
```

## 5. Run and Observe

Run node A and node B in separate terminals:

```bash
RUST_LOG=info cargo run -p zap-cli -- run --config examples/multi-node-production/node-a.toml
RUST_LOG=info cargo run -p zap-cli -- run --config examples/multi-node-production/node-b.toml
```

In another terminal, send a typed action:

```bash
cargo run -p zap-cli -- send \
  --config examples/multi-node-production/node-a.toml \
  --target <node-b-uuid> \
  --kind action \
  --subject thermostat.setpoint \
  --payload '{"temperature_c":20}' \
  --content-type application/json
```

Use the assets in `crates/zap-ops/config` for production-style monitoring:

- import `grafana/zap-production-dashboard.json`;
- load `prometheus/zap-rules.yml`;
- run the collector config in `otel/collector.yml`.
