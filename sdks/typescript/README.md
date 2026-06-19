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

## Shared fixtures and conformance

The TypeScript test suite reads the shared protocol fixtures from the
repository-level `fixtures/` directory. It currently asserts:

- `zenv-control-registry-bundle-manifest-request.json` matches the TypeScript
  ZapStore request helper and round-trips through `ControlFrame`.
- `control-subjects-v1.json` includes the SDK registry subjects and uses frame
  compatible media types.
- `agent-intent-message-v1.json` can be carried as an
  `application/zap-agent+json` control envelope.

To add a fixture, create a small deterministic JSON file in `fixtures/`, then
add or extend a test in `sdks/typescript/test/fixtures.test.ts` that checks the
schema version, subject, media type, encoded header fields, and stable body
fields.

## Test

Node 24 can run the TypeScript tests with built-in type stripping:

```bash
npm --prefix sdks/typescript test
npm --prefix sdks/typescript run typecheck
npm --prefix sdks/typescript run build:types
```
