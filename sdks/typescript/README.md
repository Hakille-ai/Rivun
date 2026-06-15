# ZAP TypeScript SDK

TypeScript helpers for ZAP `ZENV` control envelopes, local UDP transport, and
ZapStore registry payloads.

This package prepares protocol-compatible bytes and JSON for a ZAP transport,
CLI bridge, browser worker, or test fixture. In Node it also includes a small
`ZapUdpClient` for loopback/dev peer integration.

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
`artifactHash()` computes canonical BLAKE3 values through `@noble/hashes`.

`verifyEd25519Signature()` verifies registry/publication signatures through
`@noble/ed25519`.

## Test

Node 24 can run the TypeScript tests with built-in type stripping:

```bash
node --test --experimental-strip-types sdks/typescript/test/*.test.ts
npm run typecheck
npm run build:types
```
