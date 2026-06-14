import json
import unittest
from uuid import UUID

from zap_sdk import (
    REGISTRY_BUNDLE_MANIFEST_CONTENT_TYPE,
    REGISTRY_BUNDLE_MANIFEST_REQUEST_SUBJECT,
    ControlFrame,
    RegistryBundleEntry,
    RegistryBundleManifest,
    RegistryBundleManifestRequest,
    RegistryBundleManifestResponse,
    ZapStoreClient,
    registry_bundle_manifest_request_frame,
    validate_artifact_hash,
    verify_signature_placeholder,
)

HASH = "blake3:" + "0" * 64


class ProtocolTests(unittest.TestCase):
    def test_registry_bundle_manifest_request_control_frame_round_trips(self):
        frame = ZapStoreClient().registry_bundle_manifest_request(require_publication=True, require_drivers=True)

        encoded = frame.encode()
        decoded = ControlFrame.decode(encoded)

        self.assertEqual(decoded.subject, REGISTRY_BUNDLE_MANIFEST_REQUEST_SUBJECT)
        self.assertEqual(decoded.content_type, REGISTRY_BUNDLE_MANIFEST_CONTENT_TYPE)
        self.assertEqual(decoded.json_body()["schema_version"], 1)
        self.assertTrue(decoded.json_body()["require_publication"])
        self.assertTrue(decoded.json_body()["require_drivers"])
        self.assertEqual(encoded[0:4], b"ZENV")
        self.assertEqual(int.from_bytes(encoded[6:8], "big"), 8)

    def test_bundle_manifest_response_shape_enforces_requested_driver_hashes(self):
        manifest = RegistryBundleManifest(
            schema_version=1,
            registry_path="registry.index.toml",
            registry_hash=HASH,
            entries=[
                RegistryBundleEntry(
                    action="echo",
                    version="0.1.0",
                    name="echo-driver",
                    abi_version=1,
                    wasm_hash=HASH,
                    author_node_id=UUID(int=0),
                    manifest_path="manifests/echo.toml",
                    manifest_hash=HASH,
                )
            ],
        )
        response = RegistryBundleManifestResponse(
            schema_version=1,
            node_id=UUID(int=1),
            manifest=manifest,
        )

        with self.assertRaisesRegex(ValueError, "lacks driver metadata"):
            response.verify_shape(RegistryBundleManifestRequest(require_drivers=True))

    def test_json_body_uses_protocol_field_names(self):
        frame = registry_bundle_manifest_request_frame(require_publication=False, require_drivers=True)

        self.assertEqual(
            json.loads(frame.body.decode("utf-8")),
            {"schema_version": 1, "require_publication": False, "require_drivers": True},
        )

    def test_hash_and_signature_helpers_are_explicit(self):
        self.assertTrue(validate_artifact_hash(HASH))
        self.assertFalse(validate_artifact_hash("sha256:" + "0" * 64))
        status = verify_signature_placeholder("registry")
        self.assertFalse(status.supported)
        self.assertIn("Ed25519", status.reason)


if __name__ == "__main__":
    unittest.main()
