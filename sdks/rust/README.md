# ZAP Rust SDK

External Rust SDK for ZAP control envelopes and ZapStore payloads.

Unlike the dependency-free Python/TypeScript/Go SDKs, this crate reuses the
canonical ZAP crates through path dependencies. It can therefore compute BLAKE3
artifact hashes and call the existing ZapStore signature verification routines.

## Build a registry bundle manifest request

```rust
use zap_sdk::{registry_bundle_manifest_request_frame, ControlFrame};

let frame = registry_bundle_manifest_request_frame(true, true)?;
let payload = frame.encode();
let parsed = ControlFrame::decode(&payload)?;

assert_eq!(parsed.subject(), "zap.registry.bundle.manifest.request");
# Ok::<(), Box<dyn std::error::Error>>(())
```

## Test

```bash
cargo test --manifest-path sdks/rust/Cargo.toml
```
