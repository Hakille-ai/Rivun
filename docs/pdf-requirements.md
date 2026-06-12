# PDF Requirements Trace

This file tracks the technical specification PDF against the current repository. It keeps the project honest: implemented means backed by code and tests, foundation means a real interface exists but the global production network is not complete yet.

| PDF area | Current status | Evidence |
| --- | --- | --- |
| 64-byte ZAP-Wire header | Implemented | `zap-core` parser, encoder, golden vector, property tests |
| Zero-copy header parsing | Implemented for header | `ZapHeader::parse(&[u8])` is allocation-free |
| Ed25519 authenticity | Implemented | `zap-crypto` full signature trailer plus 8-byte hint |
| Encrypted P2P UDP transport | Implemented for static peers | `zap-net` ChaCha20-Poly1305 datagrams, static peer table, tests |
| Peer trust policy | Foundation implemented | `zap-node` validates per-peer send/receive/forward/PoA gates, expiry, and transport key rotation age; `zap trust enroll/inspect` expose operator workflows |
| Noise Protocol Framework | Foundation | `zap-net::noise` derives transport material; full live handshake routing remains future work |
| Reflex action routing | Implemented locally | `zap-node` validates, dispatches action envelopes, and runs registered WASM drivers |
| Deterministic routing | Foundation implemented | `zap-router`, `[[routes]]`, `zap route explain`, local dispatch fallback, and peer forwarding tests |
| Capability discovery | Foundation implemented | `zap-capability`, signed `zap.capability.query`/`response`, cache refresh, local CLI inspection, and node response tests |
| Local memory | Foundation implemented | `zap-memory` JSONL records, BLAKE3 hashes, tombstones, pruning, verification, and CLI commands |
| WASM sandbox execution | Implemented | `zap-runtime` ABI validation, fuel, memory, output, timeout, permissions |
| Broadcast target zero | Implemented | broadcast frames use UUID nil internally while UDP envelopes target concrete peers |
| Cognitive interpreter | Externalized by product decision | Models and gateways emit typed `ZENV` messages; ZAP enforces `[message_policy]`, signatures, routing, PoA, and sandboxing |
| Proof-of-Action | Foundation implemented | `ZPOA` trailer, validator signatures, threshold verification, daemon enforcement, and static configured validator networking exist; dynamic discovery remains future work |
| Financial layer | Out of scope by product decision | Signed action receipts, offline verification, retention filtering, and archive merge provide technical auditability only |
| ZapStore driver registry | Foundation implemented | `zap-store` signed manifests, signed local registry approval, CLI create/verify/sign/revoke, and daemon config enforcement exist; package publishing remains future work |
| SDKs for major languages | Planned | Rust crates exist; external language SDKs still to implement |

Next high-impact PDF features:

1. Add package distribution and remote index publishing for ZapStore.
2. Expose SDK-friendly schemas for typed messages, frames, manifests, receipts, routes, capabilities, and memory.
3. Add remote receipt replication tooling for operators.
