# Changelog

All notable changes to ZAP are documented here.

This project follows Semantic Versioning for crates and documented CLI behavior.
The ZAP-Wire protocol has its own explicit version field and compatibility rules
described in [docs/versioning.md](docs/versioning.md).

## Unreleased

- Added production Docker packaging with a non-root runtime image.
- Added open-source project files: license, security policy, contribution guide,
  code of conduct, governance notes, and release/versioning docs.
- Clarified that ZAP is a universal protocol layer, not a model runtime or
  financial rail.

## 0.1.0

- Initial Rust workspace with strict ZAP-Wire v1 frame parsing and encoding.
- Added universal `ZENV` envelopes.
- Added Ed25519 frame signatures and Proof-of-Action trailers.
- Added encrypted UDP transport, static peer configuration, and replay checks.
- Added Wasmtime driver sandboxing and signed driver manifests.
- Added CLI, daemon, deterministic intent compiler, tests, and benchmarks.
