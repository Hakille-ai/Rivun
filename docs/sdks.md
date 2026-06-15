# ZAP SDKs

The first external SDK distribution lives under `sdks/` and focuses on
protocol-compatible helpers with lightweight local transports:

- `sdks/python`: Python dataclasses for `ZENV` control envelopes, ZapStore
  request/response payloads, and a stdlib UDP client.
- `sdks/typescript`: TypeScript helpers that run in Node, including UDP,
  BLAKE3, Ed25519 verification, typecheck, and declaration build scripts.
- `sdks/go`: Go package for control envelope bytes, UDP transport, canonical
  BLAKE3 hashes, Ed25519 verification, and ZapStore JSON types.
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
existing signature verification methods. Python can compute/verify when its
`crypto` extra is installed. TypeScript uses `@noble/hashes` and
`@noble/ed25519`. Go uses `lukechampine.com/blake3` and the standard Ed25519
package.

## Local Tests

```bash
python -m unittest discover -s sdks/python/tests
npm --prefix sdks/typescript run typecheck
npm --prefix sdks/typescript run build:types
npm --prefix sdks/typescript test
cargo test --manifest-path sdks/rust/Cargo.toml
```

Go tests are included and can be run with:

```bash
go test ./sdks/go/...
```

The SDK workflow installs Python, Node, Go, and Rust toolchains and runs these
checks in CI.
