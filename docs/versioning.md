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

CLI JSON output is a compatibility surface. Adding optional fields is a minor
release. Removing fields, renaming fields, changing default decisions, or
changing exit semantics requires explicit migration notes and should be treated
as a major compatibility event once ZAP reaches 1.0.

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

Protocol fixture changes must pass the release-readiness gate in
[release.md](release.md). New required fixture fields, stricter parser behavior,
or changed canonical subjects require migration notes even when binary wire
versions do not change.

## Profile Versions

Profiles such as PACT live above the wire format and inside `ZENV` envelopes.
They carry their own `schema_version` while reusing existing ZAP identity,
hashing, signatures, policy, PoA, and receipts.

PACT v1 uses content type `application/zap-pact+json`, subjects
`zap.pact.record`, `zap.pact.verify`, `zap.pact.revoke`, and
`zap.pact.bundle`, plus the signature domain `ZAP-PACT-v1`. Changes to the
canonical signing field list, field order, nested JSON normalization, hash
format, signature domain, or subject/content-type constants are compatibility
events and require migration notes plus fixture updates across official SDKs.

## Domain Packs and SDKs

Domain pack metadata uses `schema_version`. Compatible additions to pack
metadata, policies, schemas, or examples are minor-compatible when older
validators can ignore the new data safely. Required metadata changes, risk-level
semantics, or stricter validation rules require migration notes.

SDKs should follow the workspace version. A release is not ready until shared
fixtures pass across Python, TypeScript, Rust, and Go. Local machines may lack
Go; CI release readiness is authoritative and must run Go conformance before a
stable release.

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
