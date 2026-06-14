import assert from "node:assert/strict";
import test from "node:test";

import {
  ControlFrame,
  REGISTRY_BUNDLE_MANIFEST_CONTENT_TYPE,
  REGISTRY_BUNDLE_MANIFEST_REQUEST_SUBJECT,
  ZapStoreClient,
  registryBundleManifestRequestFrame,
  signatureVerificationPlaceholder,
  validateArtifactHash,
  validateRegistryBundleManifestResponse,
} from "../src/index.ts";

const HASH = `blake3:${"0".repeat(64)}`;

test("registry bundle manifest control frame round trips", () => {
  const frame = new ZapStoreClient().registryBundleManifestRequest({ requirePublication: true, requireDrivers: true });
  const encoded = frame.encode();
  const decoded = ControlFrame.decode(encoded);

  assert.equal(decoded.subject, REGISTRY_BUNDLE_MANIFEST_REQUEST_SUBJECT);
  assert.equal(decoded.contentType, REGISTRY_BUNDLE_MANIFEST_CONTENT_TYPE);
  assert.equal(encoded[0], "Z".charCodeAt(0));
  assert.equal(Buffer.from(encoded).readUInt16BE(6), 8);
  assert.deepEqual(decoded.jsonBody(), {
    schema_version: 1,
    require_publication: true,
    require_drivers: true,
  });
});

test("bundle manifest response validation honors required driver metadata", () => {
  assert.throws(
    () =>
      validateRegistryBundleManifestResponse(
        {
          schema_version: 1,
          node_id: "00000000-0000-0000-0000-000000000001",
          manifest: {
            schema_version: 1,
            registry_path: "registry.index.toml",
            registry_hash: HASH,
            entries: [
              {
                action: "echo",
                version: "0.1.0",
                name: "echo-driver",
                abi_version: 1,
                wasm_hash: HASH,
                author_node_id: "00000000-0000-0000-0000-000000000000",
                status: "active",
                manifest_path: "manifests/echo.toml",
                manifest_hash: HASH,
              },
            ],
          },
        },
        { schema_version: 1, require_publication: false, require_drivers: true },
      ),
    /lacks driver metadata/,
  );
});

test("hash and signature helpers are explicit", () => {
  assert.equal(validateArtifactHash(HASH), true);
  assert.equal(validateArtifactHash(`sha256:${"0".repeat(64)}`), false);
  const status = signatureVerificationPlaceholder("registry");
  assert.equal(status.supported, false);
  assert.match(status.reason, /Ed25519/);
});
