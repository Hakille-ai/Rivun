# PDF Requirements Trace

This file tracks the technical specification PDF against the current repository. It keeps the project honest: implemented means backed by code and tests, foundation means a real interface exists but the global production network is not complete yet.

| PDF area | Current status | Evidence |
| --- | --- | --- |
| 64-byte ZAP-Wire header | Implemented | `zap-core` parser, encoder, golden vector, property tests |
| Zero-copy header parsing | Implemented for header | `ZapHeader::parse(&[u8])` is allocation-free |
| Ed25519 authenticity | Implemented | `zap-crypto` full signature trailer plus 8-byte hint |
| Encrypted P2P UDP transport | Implemented for static peers | `zap-net` ChaCha20-Poly1305 datagrams, static peer table, tests |
| Noise Protocol Framework | Foundation | `zap-net::noise` derives transport material; full live handshake routing remains future work |
| Reflex action routing | Implemented locally | `zap-node` validates, dispatches action envelopes, and runs registered WASM drivers |
| WASM sandbox execution | Implemented | `zap-runtime` ABI validation, fuel, memory, output, timeout, permissions |
| Broadcast target zero | Implemented | broadcast frames use UUID nil internally while UDP envelopes target concrete peers |
| Cognitive interpreter | Foundation implemented | `zap-intent`, `zap compile-intent`, and `zap send --intent` |
| Proof-of-Action | Foundation implemented | `ZPOA` trailer, validator signatures, threshold verification, daemon enforcement, and static configured validator networking exist; dynamic discovery remains future work |
| Financial layer | Out of scope by product decision | Signed action receipts provide technical auditability only |
| ZapStore driver registry | Foundation implemented | `zap-store` signed manifests, signed local registry approval, CLI create/verify/sign, and daemon config enforcement exist; package publishing remains future work |
| SDKs for major languages | Planned | Rust crates exist; external language SDKs still to implement |

Next high-impact PDF features:

1. Add package distribution and remote index publishing for ZapStore.
2. Add networked validator quorum and discovery.
3. Expose SDK-friendly schemas for intents, frames, manifests, and receipts.
4. Add receipt replication and retention tooling for operators.
