# Rivun Rust SDK

External Rust SDK for Rivun control envelopes and RivunStore payloads.

Unlike the dependency-free Python/TypeScript/Go SDKs, this crate reuses the
canonical Rivun crates through path dependencies. It can therefore compute BLAKE3
artifact hashes and call the existing RivunStore signature verification routines.

## Build a registry bundle manifest request

```rust
use rivun_sdk::{registry_bundle_manifest_request_frame, ControlFrame};

let frame = registry_bundle_manifest_request_frame(true, true)?;
let payload = frame.encode();
let parsed = ControlFrame::decode(&payload)?;

assert_eq!(parsed.subject(), "rivun.registry.bundle.manifest.request");
# Ok::<(), Box<dyn std::error::Error>>(())
```

## Shared fixtures and conformance

The Rust SDK reuses the canonical local Rivun crates through path dependencies,
so its conformance coverage comes from those crates plus SDK round-trip tests.
It currently covers ZENV control frame construction, RivunStore request/response
types, canonical artifact hashing, and signature verification through the
reference implementation.

The repository-level `fixtures/` directory is the shared source of readable
SDK examples. Rust root fixture tests are not mirrored in this SDK yet; when
adding one, keep the fixture JSON deterministic and add a test under
`sdks/rust/src/lib.rs` or a dedicated integration test that checks the fixture
schema version, subject, media type, and decoded body against the canonical
types.

## Test

```bash
cargo test --manifest-path sdks/rust/Cargo.toml
```

