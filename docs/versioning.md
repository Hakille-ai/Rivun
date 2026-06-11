# Versioning

ZAP has three compatibility surfaces: Rust crates, CLI behavior, and wire
protocol data. They are related but not identical.

## Crates and CLI

Rust crates and documented CLI behavior follow Semantic Versioning:

- patch releases fix bugs and security issues without intentional breakage;
- minor releases add compatible features;
- major releases may break public APIs or documented CLI behavior.

ZAP is currently pre-1.0, so API movement is allowed, but compatibility changes
still need changelog entries and migration notes.

## Protocol Versions

ZAP-Wire has an explicit `VERSION` field in the 64-byte frame header. `ZENV`
has its own `version` field. These versions change only when parsers cannot
accept older data safely and unambiguously.

Rules:

- never reinterpret existing bytes silently;
- reject unknown required flags or nonzero reserved fields;
- add golden vectors for stable binary layouts;
- keep downgrade behavior explicit;
- document migration paths before changing the default emitter.

## MSRV and Toolchain

The minimum supported Rust version is declared in `Cargo.toml`. The local
[rust-toolchain.toml](../rust-toolchain.toml) tracks stable Rust so contributors
do not get pinned to a partially installed point release.

MSRV bumps are allowed before 1.0, but they must be called out in the changelog.

## Deprecation

Deprecated APIs should remain for at least one minor release when practical.
Security fixes may remove or reject unsafe behavior immediately.

Current example: `zap-node` accepts the legacy JSON action envelope for
compatibility, while new CLI sends emit universal `ZENV` envelopes.
