# Getting Started with ZAP

This guide walks you through setting up ZAP, compiling the workspace, running a local two-node cluster, sending typed messages, and executing a sandboxed WASM driver.

For a shorter source-install checklist, start with [Install ZAP](install.md).

---

## Prerequisites

ZAP requires the following tools:
- **Rust 1.93+** (edition 2024) — Install via [rustup.rs](https://rustup.rs).
- **Git** — To clone and manage the repository.
- **Docker** (optional) — For containerized deployments.

---

## 1. Installation & Compilation

Clone the repository and build the ZAP workspace in release mode:

```bash
# Clone the repository
git clone https://github.com/Hakille-ai/ZAP.git
cd ZAP

# Run unit tests to verify the toolchain
cargo test --workspace --all-targets

# Compile the CLI tool
cargo build --release -p zap-cli
```

The compiled binary is available at `target/release/zap` (or `target/release/zap.exe` on Windows).

---

## 2. Generating Node Identity Keys

Each ZAP node requires an Ed25519 identity keypair to sign messages and authenticate itself.

Create a workspace directory and generate a node key file:

```bash
mkdir -p .zap

# Generate key for Node A
cargo run -p zap-cli -- keygen --out .zap/node-a.key

# Generate key for Node B
cargo run -p zap-cli -- keygen --out .zap/node-b.key
```

> **Security Note:** The `keygen` command restricts file permissions (read/write only by owner) by default on Unix systems and will refuse to overwrite an existing key file unless you pass the `--force` flag.

---

## 3. Configuring the Two-Node Cluster

To demonstrate local message dispatch, we will configure two nodes:
- **Node A** (Receiver / Daemon): Runs on port `7000`.
- **Node B** (Sender): Runs on port `7001`.

Create their respective TOML configuration files under `examples/configs/` (or check the pre-configured files):

### Node A Config (`examples/configs/node-a.toml`)
```toml
node_id = "a0000000-0000-0000-0000-00000000000a"
bind = "127.0.0.1:7000"
key_file = "../../.zap/node-a.key"

[security]
enforce_signatures = true
enforce_replay_protection = true

[[peers]]
node_id = "b0000000-0000-0000-0000-00000000000b"
addr = "127.0.0.1:7001"
public_key = "<Insert Node B Public Key here (Base64)>"
transport_key = "0102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f20"
```

### Node B Config (`examples/configs/node-b.toml`)
```toml
node_id = "b0000000-0000-0000-0000-00000000000b"
bind = "127.0.0.1:7001"
key_file = "../../.zap/node-b.key"

[security]
enforce_signatures = true
enforce_replay_protection = true

[[peers]]
node_id = "a0000000-0000-0000-0000-00000000000a"
addr = "127.0.0.1:7000"
public_key = "<Insert Node A Public Key here (Base64)>"
transport_key = "0102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f20"
```

> **Note:** The `public_key` field must contain the base64-encoded verifying key generated inside your key files. You can find this inside `.zap/node-a.key` and `.zap/node-b.key`. The `transport_key` is a shared 32-byte hex string.

---

## 4. Running Node Diagnostics (`doctor`)

Before starting the daemons, verify that your configuration is syntactically and structurally correct:

```bash
cargo run -p zap-cli -- doctor --config examples/configs/node-a.toml
```

The output gives a readiness score (0-100) and details any missing components or security risks.

---

## 5. Exchanging Messages

Start Node A in one terminal:

```bash
cargo run -p zap-cli -- run --config examples/configs/node-a.toml
```

In another terminal, send an `echo` action from Node B to Node A:

```bash
cargo run -p zap-cli -- send --config examples/configs/node-b.toml \
  --target a0000000-0000-0000-0000-00000000000a \
  --action echo --payload "Hello ZAP"
```

You should see Node A receive the frame, decrypt it, verify the signature, log the execution receipt, and run the echo handler!

---

## 6. Sending Typed Agent Actions

ZAP expects agents, models, or operator tools to produce strict typed messages.
For example, send a JSON action envelope directly:

```bash
cargo run -p zap-cli -- send --config examples/configs/node-b.toml \
  --target a0000000-0000-0000-0000-00000000000a \
  --kind action --subject thermostat.setpoint \
  --payload '{"temperature_c":22}' --content-type application/json
```

The system builds a ZENV envelope, signs it, sends it over UDP, verifies it on
the receiver, checks message policy, and triggers the target action.
