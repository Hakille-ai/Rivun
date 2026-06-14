# ZAP TypeScript SDK

Dependency-free TypeScript helpers for ZAP `ZENV` control envelopes and
ZapStore registry payloads.

This package does not open sockets. It prepares protocol-compatible bytes and
JSON for a ZAP transport, CLI bridge, browser worker, or test fixture.

## Build a registry bundle manifest request

```ts
import { registryBundleManifestRequestFrame } from "./src/index.ts";

const frame = registryBundleManifestRequestFrame({
  requirePublication: true,
  requireDrivers: true,
});

const payload = frame.encode();
console.log(frame.subject, payload.byteLength);
```

## Parse a control frame

```ts
import { ControlFrame } from "./src/index.ts";

const parsed = ControlFrame.decode(payload);
console.log(parsed.jsonBody());
```

## Integrity helpers

`validateArtifactHash()` checks the canonical `blake3:<64 hex chars>` shape.
Node's standard crypto module does not currently provide BLAKE3, so
`artifactHash()` throws an explicit error instead of producing an incompatible
hash. Use `zap-cli`, the Rust SDK, or a caller-provided BLAKE3 implementation
for canonical checksum production.

Signature verification is represented by `signatureVerificationPlaceholder()`
until the package grows a vetted Ed25519 backend.

## Test

Node 24 can run the TypeScript tests with built-in type stripping:

```bash
node --test --experimental-strip-types sdks/typescript/test/*.test.ts
```
