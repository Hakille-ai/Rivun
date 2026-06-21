# PDF Requirements Trace

This file tracks the technical specification PDF against the current repository. It keeps the project honest: implemented means backed by code and tests, foundation means a real interface exists but the global production network is not complete yet.

| PDF area | Current status | Evidence |
| --- | --- | --- |
| 64-byte ZAP-Wire header | Implemented | `zap-core` parser, encoder, golden vector, property tests |
| Zero-copy header parsing | Implemented for header | `ZapHeader::parse(&[u8])` is allocation-free |
| Ed25519 authenticity | Implemented | `zap-crypto` full signature trailer plus 8-byte hint |
| Encrypted P2P UDP transport | Implemented for static peers | `zap-net` ChaCha20-Poly1305 datagrams, static peer table, tests |
| Peer trust policy | Foundation implemented | `zap-node` validates per-peer send/receive/forward/PoA gates, expiry, and transport key rotation age; `zap trust enroll/inspect` plus signed `zap peer invite/accept/rotate/revoke` expose operator workflows |
| Noise Protocol Framework | Foundation | `zap-net::noise` derives transport material; full live handshake routing remains future work |
| Reflex action routing | Implemented locally | `zap-node` validates, dispatches action envelopes, and runs registered WASM drivers |
| Deterministic routing | Foundation implemented | `zap-router`, `[[routes]]`, `zap route explain`, local dispatch fallback, and peer forwarding tests |
| Capability discovery | Foundation implemented | `zap-capability`, signed `zap.capability.query`/`response`, cache refresh, local CLI inspection, and node response tests |
| Local memory | Foundation implemented | `zap-memory` binary journal records, BLAKE3 hashes, indexes, tombstones, compaction, JSONL import/export, verification, and CLI commands |
| WASM sandbox execution | Implemented | `zap-runtime` ABI validation, fuel, memory, output, timeout, permissions |
| Broadcast target zero | Implemented | broadcast frames use UUID nil internally while UDP envelopes target concrete peers |
| Cognitive interpreter | Externalized by product decision | Models and gateways emit typed `ZENV` messages; ZAP enforces `[message_policy]`, signatures, routing, PoA, and sandboxing |
| Proof-of-Action | Foundation implemented | `ZPOA` trailer, validator signatures, threshold verification, daemon enforcement, static configured validator networking, signed versioned validator-set files, and peer pull propagation exist; dynamic discovery remains future work |
| Financial layer | Out of scope by product decision | Signed action receipts, offline verification, remote pull, journal compaction, and JSONL export provide technical auditability only |
| ZapStore driver registry | Foundation implemented | `zap-store` signed manifests, signed local registry approval, semantic version requirement resolution, ABI requirement ranges, migration metadata, deprecation and revocation states, signed install plans, CLI create/verify/sign/revoke/deprecate/migration/resolve/pull/mirror/publication/plan/bundle, daemon config enforcement, signed peer index fetch, remote bundle manifest discovery, strict merge conflict detection, revocation-priority mirroring, signed publication metadata, and offline bundle export/verify/import exist; remote artifact transfer services remain future work |
| SDKs for major languages | Foundation implemented | Rust crates plus external Python, TypeScript, Go, and Rust SDK directories with protocol/ZapStore helpers and examples now exist; publishable package pipelines remain future work |

Next high-impact PDF features:

1. Add fleet rollout automation and remote bundle artifact transfer services for ZapStore.
2. Expose SDK-friendly schemas for typed messages, frames, manifests, receipts, routes, capabilities, and memory.
3. Add dynamic validator discovery, automated validator-set rollout, and quorum policy hardening.
