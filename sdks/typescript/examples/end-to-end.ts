import {
  ControlFrame,
  REGISTRY_BUNDLE_MANIFEST_REQUEST_SUBJECT,
  registryBundleManifestRequestFrame,
  validateRegistryBundleManifestResponse,
} from "../src/index.ts";

const frame = registryBundleManifestRequestFrame({
  requirePublication: true,
  requireDrivers: true,
});
const encoded = frame.encode();
const parsed = ControlFrame.decode(encoded);

if (parsed.subject !== REGISTRY_BUNDLE_MANIFEST_REQUEST_SUBJECT) {
  throw new Error(`unexpected subject ${parsed.subject}`);
}

validateRegistryBundleManifestResponse(
  {
    schema_version: 1,
    node_id: "00000000-0000-0000-0000-000000000001",
    unavailable_reason: "example has no network peer",
  },
  parsed.jsonBody() as { schema_version: number; require_publication: boolean; require_drivers: boolean },
);

console.log(`built ${parsed.subject} (${encoded.byteLength} bytes)`);
