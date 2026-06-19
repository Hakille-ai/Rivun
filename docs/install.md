# Install ZAP

ZAP is currently installed from source. The repository contains the Rust
workspace, the `zap-cli` operator binary, example node configs, Docker files,
and SDKs.

## Requirements

- Rust stable with support for Rust 1.93 or newer.
- Git.
- Docker, optional, for container builds and Compose deployment.

The repository includes `rust-toolchain.toml`, so `rustup` will select the
configured stable toolchain automatically when you run Cargo commands from the
repo.

## Clone and Build

```bash
git clone https://github.com/Hakille-ai/ZAP.git
cd ZAP

cargo test --workspace --all-targets
cargo build --release -p zap-cli
```

The compiled binary is written to:

- `target/release/zap` on Linux and macOS.
- `target/release/zap.exe` on Windows.

You can run the binary directly from `target/release`, copy it onto your `PATH`,
or use Cargo while developing:

```bash
cargo run -p zap-cli -- --help
cargo run -p zap-cli -- doctor --help
```

## Local Developer Setup

Generate node identity keys before running the included two-node example:

```bash
cargo run -p zap-cli -- keygen --out .zap/node-a.key
cargo run -p zap-cli -- keygen --out .zap/node-b.key
```

`keygen` prints the generated `node_id` and `public_key`. Copy each node's
public key into the peer entry of the other node config before attempting a
signed send. The checked-in files under `examples/configs/` are useful
templates, but their peer keys must match the keys on disk.

Validate the example configs:

```bash
cargo run -p zap-cli -- check-config --config examples/configs/node-a.toml
cargo run -p zap-cli -- check-config --config examples/configs/node-b.toml

cargo run -p zap-cli -- doctor --config examples/configs/node-a.toml
```

Start a node in one terminal:

```bash
cargo run -p zap-cli -- run --config examples/configs/node-a.toml
```

Send from the other node in a second terminal:

```bash
cargo run -p zap-cli -- send --config examples/configs/node-b.toml \
  --target <node-a-uuid> \
  --action echo \
  --payload "Hello ZAP"
```

Do not run `zap run` and `zap send` from the same config at the same time. The
sender binds to the `bind` address in its config so peers can enforce static
peer addresses.

## Docker

Build the local container image:

```bash
docker build -t zap:local .
```

Run the Compose setup:

```bash
mkdir -p .zap/container
docker compose run --rm node keygen --out /var/lib/zap/node.key
docker compose up --build
```

See [Deployment](deployment.md) for production hardening and runtime topology
notes.

## Next Steps

- Follow [Getting Started](getting-started.md) for the two-node walkthrough.
- Use [Operations](operations.md) for CLI diagnostics, receipts, and monitoring.
- Read [Security](security.md) before using ZAP with real peers or machines.
