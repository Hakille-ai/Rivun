# ZAP: Universal Low-Latency Protocol

ZAP is a Rust implementation of a compact, signed, encrypted, low-latency
protocol for moving typed messages between nodes. Actions are one important use
case, but the protocol is not limited to action dispatch: it can carry data,
events, commands, queries, responses, stream chunks, actions, and control
messages.

ZAP is protocol infrastructure. It is independent of AI models, LLM providers,
and application runtimes. Receipts and Proof-of-Action support auditability;
they are not billing, settlement, rewards, or financial rails.

## Project Status

ZAP is pre-1.0 and under active development. The codebase is structured like a
production system from the start: strict binary parsing, cryptographic
verification, encrypted transport, sandboxed WASM execution, CLI tooling,
tests, benchmarks, Docker packaging, and operator documentation.

Compatibility is taken seriously even before 1.0. See
[Versioning](docs/versioning.md) before changing public APIs, CLI behavior, or
wire formats.

## Architecture

- **Wire**: the fixed 64-byte `ZAP_` frame header plus optional auth and PoA
  trailers.
- **Envelope**: the `ZENV` payload format with universal kind, subject, content
  type, metadata bytes, and body bytes.
- **Transport**: encrypted UDP datagrams, static peer addressing, nonce replay
  checks, and Noise helper primitives.
- **Node**: daemon policy, peer verification, replay protection, receipts, and
  dispatch.
- **Runtime**: Wasmtime execution with ABI checks, fuel, memory, time, output,
  and permission limits.
- **Adapters**: CLI, SDKs, bridges, model runtimes, devices, and application
  connectors.

## Crates

- `zap-core`: ZAP-Wire frames, flags, auth trailers, PoA trailers, parsing and
  encoding.
- `zap-envelope`: universal `ZENV` payload envelopes.
- `zap-crypto`: node identity, key files, Ed25519 signing, verification, and
  PoA certificates.
- `zap-net`: encrypted UDP endpoint, peer table, nonce replay checks, and Noise
  helper.
- `zap-runtime`: sandboxed WASM driver execution.
- `zap-driver-sdk`: minimal ABI helpers for driver authors.
- `zap-node`: daemon config, verification, receipts, and action dispatch.
- `zap-cli`: operator commands.
- `zap-intent`: deterministic local intent compiler.
- `zap-ledger`: signed receipt records.
- `zap-store`: signed WASM driver manifests.

## Quickstart

```bash
cargo test --workspace --all-targets
cargo run -p zap-cli -- keygen --out .zap/node.key
cargo run -p zap-cli -- compile-intent "Ajuster la temperature a 20" --explain
cargo run -p zap-cli -- bench parse --iterations 100000
```

`zap keygen` refuses to overwrite an existing key unless `--force` is provided.

## Local Node Flow

Generate one key per node, copy each node's `node_id`, `public_key`, and a
shared 32-byte `transport_key` into TOML configs based on
`examples/configs/node-a.toml` and `examples/configs/node-b.toml`, validate both
configs, then run:

```bash
cargo run -p zap-cli -- check-config --config examples/configs/node-a.toml
cargo run -p zap-cli -- check-config --config examples/configs/node-b.toml
cargo run -p zap-cli -- run --config examples/configs/node-a.toml
cargo run -p zap-cli -- send --config examples/configs/node-b.toml --target <node-a-uuid> --action echo --payload hello
cargo run -p zap-cli -- send --config examples/configs/node-b.toml --target <node-a-uuid> --kind event --subject sensor.temperature --payload '{"c":21.5}' --content-type application/json
```

`zap send` binds to the `bind` address in its config so the receiver can enforce
static peer addresses. Do not run `zap run` and `zap send` from the same config
at the same time.

Relative `key_file`, driver `path`, receipt, and manifest paths are resolved
from the directory containing the TOML config.

## Docker

Build the production-style image:

```bash
docker build -t zap:local .
```

Generate a container node key and start the compose template:

```bash
mkdir -p .zap/container
docker compose run --rm node keygen --out /var/lib/zap/node.key
docker compose up --build
```

The container runs as a non-root user, exposes UDP `7000`, uses a read-only root
filesystem in Compose, and stores node state under `/var/lib/zap`.

See [Deployment](docs/deployment.md) for production notes.

## Common Commands

Validate a config:

```bash
cargo run -p zap-cli -- check-config --strict --config zap.toml
cargo run -p zap-cli -- check-config --config zap.toml --json
```

Send a universal action envelope to a WASM driver:

```bash
cargo run -p zap-cli -- send --config zap.toml --target <uuid> --action echo --payload hello
cargo run -p zap-cli -- send --config zap.toml --target <uuid> --action upload --payload-file payload.bin --binary-payload
```

Send a universal event envelope:

```bash
cargo run -p zap-cli -- send --config zap.toml --target <uuid> --kind event --subject sensor.temperature --payload '{"c":21.5}' --content-type application/json --metadata '{"source":"sim"}'
```

Create and verify a signed driver manifest:

```bash
cargo run -p zap-cli -- driver-manifest create --driver examples/wasm-drivers/echo/echo.wat --action echo --author-key .zap/node.key --out examples/wasm-drivers/echo/echo.manifest.toml
cargo run -p zap-cli -- driver-manifest verify --driver examples/wasm-drivers/echo/echo.wat --manifest examples/wasm-drivers/echo/echo.manifest.toml
```

Create and verify a local ZapStore registry index:

```bash
cargo run -p zap-cli -- registry init --out registry.index.toml
cargo run -p zap-cli -- registry add --registry registry.index.toml --manifest examples/wasm-drivers/echo/echo.manifest.toml
cargo run -p zap-cli -- registry verify --registry registry.index.toml --manifest examples/wasm-drivers/echo/echo.manifest.toml
cargo run -p zap-cli -- registry revoke --registry registry.index.toml --action echo --version 0.1.0 --reason "bad release"
cargo run -p zap-cli -- registry sign --registry registry.index.toml --operator-key .zap/node.key
cargo run -p zap-cli -- registry verify-signature --registry registry.index.toml
```

Apply an intent policy before sending or inspecting a plan:

```bash
cargo run -p zap-cli -- compile-intent "Ajuster la temperature a 20" --policy policy.json --explain
cargo run -p zap-cli -- send --config zap.toml --target <uuid> --intent "Ajuster la temperature a 20" --policy policy.json
```

Create and sign a portable PoA attestation request:

```bash
cargo run -p zap-cli -- send --config zap.toml --target <uuid> --intent "declencher arret urgence robot" --poa-network
cargo run -p zap-cli -- send --config zap.toml --target <uuid> --intent "declencher arret urgence robot" --poa-network --poa-timeout-ms 5000
cargo run -p zap-cli -- poa request --frame critical-frame.bin --requester-key .zap/node.key --threshold 1 > poa-request.json
cargo run -p zap-cli -- poa attest --request poa-request.json --validator-key .zap/validator.key > poa-response.json
```

Verify a signed receipt log:

```bash
cargo run -p zap-cli -- receipts verify --path logs/actions.jsonl
cargo run -p zap-cli -- receipts prune --path logs/actions.jsonl --before-processed-at-micros 1735689600000000 --out logs/actions.retained.jsonl
cargo run -p zap-cli -- receipts merge logs/node-a.jsonl logs/node-b.jsonl --out logs/receipts.archive.jsonl
```

Inspect a saved frame and verify it with a public key:

```bash
cargo run -p zap-cli -- inspect frame.bin --verify-with-public-key <base64-public-key>
```

## Development

Required checks:

```bash
cargo ci-fmt
cargo ci-test
cargo ci-smoke
cargo ci-bench-smoke
cargo ci-clippy
```

These aliases are defined in [.cargo/config.toml](.cargo/config.toml) and mirror
the GitHub Actions workflow. `cargo ci-smoke` launches the real `zap` binary,
sends an action through a local node, and verifies the signed receipt log.
`cargo ci-bench-smoke` compiles and runs the Criterion benchmark harnesses in
test mode.

Full performance runs are handled by GitHub Actions on Linux:

```bash
cargo ci-bench-full
cargo ci-bench-compare --base target/bench-results/base.json --head target/bench-results/head.json
```

Pull requests compare the base commit and head commit on the same runner and
fail when critical benchmark regressions exceed the thresholds in
[tools/bench-thresholds.toml](tools/bench-thresholds.toml). Pushes to `main`
publish the benchmark history to GitHub Pages:
[ZAP Benchmarks](https://hakille-ai.github.io/ZAP/).

The repository includes GitHub Actions CI for Linux, Windows, clippy, tests, and
Docker build validation, plus a separate performance workflow for benchmark
gates and Pages publishing. Use `zap check-config --strict` for production
readiness gates where validation warnings should fail deployment.

## Security

Please report vulnerabilities privately. See [SECURITY.md](SECURITY.md) and
[Security Model](docs/security.md).

Important defaults:

- Ed25519 identity and full-frame signatures;
- `ZAP_SIGN` is only an 8-byte hint;
- encrypted UDP datagrams use authenticated encryption;
- inbound nonce and frame replay checks are enabled by default;
- WASM drivers have no host capabilities unless explicit future APIs grant
  them;
- frames marked `REQUIRES_CONSENSUS` require PoA certificates before dispatch.

## Contributing

Contributions are welcome when they preserve the protocol's safety boundaries.
Start with [CONTRIBUTING.md](CONTRIBUTING.md), [GOVERNANCE.md](GOVERNANCE.md),
and [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md).

## Documentation

- [Protocol](docs/protocol.md)
- [Security Model](docs/security.md)
- [Deployment](docs/deployment.md)
- [Operations](docs/operations.md)
- [Runtime](docs/runtime.md)
- [ZapStore](docs/zapstore.md)
- [Intent Compiler](docs/intent.md)
- [Signed Receipts](docs/receipts.md)
- [Versioning](docs/versioning.md)
- [Release Process](docs/release.md)
- [Roadmap](docs/roadmap.md)
- [PDF Requirements Trace](docs/pdf-requirements.md)

## License

Licensed under the Apache License, Version 2.0. See [LICENSE](LICENSE) and
[NOTICE](NOTICE).
