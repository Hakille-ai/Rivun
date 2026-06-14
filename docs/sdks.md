# ZAP SDKs

The first external SDK distribution lives under `sdks/` and focuses on
protocol-compatible, network-free helpers:

- `sdks/python`: Python dataclasses for `ZENV` control envelopes and ZapStore
  request/response payloads.
- `sdks/typescript`: TypeScript helpers that run in Node with no runtime
  dependencies.
- `sdks/go`: Go standard-library package for control envelope bytes and
  ZapStore JSON types.
- `sdks/rust`: Rust SDK crate that wraps the canonical local ZAP crates through
  path dependencies.

## Common Surface

Each SDK can build and parse ZAP control envelopes for current ZapStore control
subjects:

- `zap.registry.index.request`
- `zap.registry.index.response`
- `zap.registry.bundle.manifest.request`
- `zap.registry.bundle.manifest.response`

Each SDK also includes base ZapStore types for registry index entries, bundle
manifests, bundle entries, install plan requests, install plans, and install
plan entries.

## Integrity Helpers

ZapStore artifact hashes are canonical `blake3:<64 hex chars>` values.

The Rust SDK reuses `zap-store` and can compute canonical BLAKE3 hashes and run
existing signature verification methods. Python, TypeScript, and Go validate the
canonical hash shape without pretending to verify unavailable crypto; their
checksum/signature helpers return or raise explicit unsupported-backend results
when the language standard library cannot produce the exact ZAP proof.

## Local Tests

```bash
python -m unittest discover -s sdks/python/tests
node --test --experimental-strip-types sdks/typescript/test/*.test.ts
cargo test --manifest-path sdks/rust/Cargo.toml
```

Go tests are included and can be run with:

```bash
go test ./sdks/go/...
```

The current Windows worker image used for this pass did not have `go` or `tsc`
installed, so Go compile verification and TypeScript declaration builds require
a toolchain-equipped environment.
