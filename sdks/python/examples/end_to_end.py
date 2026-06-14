from zap_sdk import (
    ControlFrame,
    RegistryBundleManifestRequest,
    RegistryBundleManifestResponse,
    registry_bundle_manifest_request_frame,
)


request = registry_bundle_manifest_request_frame(require_publication=True, require_drivers=True)
wire_payload = request.encode()

parsed = ControlFrame.decode(wire_payload)
body = parsed.json_body()
assert body == RegistryBundleManifestRequest(require_publication=True, require_drivers=True).to_dict()

response = RegistryBundleManifestResponse.from_dict(
    {
        "schema_version": 1,
        "node_id": "00000000-0000-0000-0000-000000000001",
        "unavailable_reason": "example has no network peer",
    }
)
response.verify_shape(RegistryBundleManifestRequest(require_publication=True, require_drivers=True))

print(f"built {parsed.subject} ({len(wire_payload)} bytes)")
