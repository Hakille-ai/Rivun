# Changelog

All notable changes to rivun are documented here.

This project follows Semantic Versioning for crates and documented CLI behavior.
The @@@@rivun_HEADER@@WIRE@@ protocol has its own explicit version field and compatibility rules
described in [docs/versioning.md](docs/versioning.md).

## Unreleased

- **Rivun Cloud SaaS Platform**: Multi-tenant Axum 0.8 REST & Server-Sent Events (SSE) server (`crates/rivun-cloud-api`) supporting orgs, team RBAC, scoped tokens, receipt indexing, domain packs, and dashboard meta-audit trail.
- **Edge Cloud Bridge**: Background daemon (`crates/rivun-cloud-bridge`) for automatic telemetry streaming, batch receipt pushing, incident capturing, and zero-trust atomic policy apply.
- **Rivun Control Workstation**: Local operator workstation & secure Ed25519 key vault (`apps/rivun-control`) enabling offline human-in-the-loop inspection and signing of staged policy bundles.
- **Enterprise Dark Dashboard**: Next.js 16 / React 19 / Tailwind dark-mode UI (`apps/rivun-dashboard`) with live SSE ticker, 7-point Doctor diagnostic badges, interactive 7-stage causal provenance graphs, visual rule builder, and air-gapped CLI verification modal.
- Removed the in-protocol `rivun-intent` crate and CLI intent commands in favor
  of external typed-message agent gateways.
- Added receiver-side `[message_policy]` rules and `rivun send
  --requires-consensus` for deterministic allow/deny/require-PoA enforcement.
- Added production Docker packaging with a non-root runtime image.
- Added open-source project files: license, security policy, contribution guide,
  code of conduct, governance notes, and release/versioning docs.
- Clarified that rivun is a universal protocol layer, not a model runtime or
  financial rail.

## 0.1.0

- Initial Rust workspace with strict @@@@rivun_HEADER@@WIRE@@ v1 frame parsing and encoding.
- Added universal `ZENV` envelopes.
- Added Ed25519 frame signatures and Proof-of-Action trailers.
- Added encrypted UDP transport, static peer configuration, and replay checks.
- Added Wasmtime driver sandboxing and signed driver manifests.
- Added CLI, daemon, tests, and benchmarks.
