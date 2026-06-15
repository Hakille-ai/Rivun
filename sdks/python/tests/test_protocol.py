import json
import socket
import threading
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
    DriverRegistryEntry,
    DriverRegistryMigration,
    RegistryInstallPlanEntry,
    RegistryInstallPlanRequest,
    ZapStoreClient,
    ZapUdpClient,
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

    def test_install_plan_types_carry_abi_requirements_and_migrations(self):
        migration = DriverRegistryMigration(
            from_version_requirement="<2.0.0",
            from_abi_requirement=">=1,<=2",
            requires_operator_approval=True,
            migration_driver_action="echo.migrate",
            migration_driver_version="1.0.0",
            notes="requires device drain",
        )
        request = RegistryInstallPlanRequest(
            action="echo",
            requirement="^2.0.0",
            abi_requirement=">=2,<4",
        )
        entry = RegistryInstallPlanEntry(
            action="echo",
            requirement="^2.0.0",
            requested_abi_requirement=">=2,<4",
            selected_version="2.1.0",
            name="echo-driver",
            abi_version=2,
            wasm_hash=HASH,
            author_node_id=UUID(int=2),
            migrations=[migration],
        )
        registry_entry = DriverRegistryEntry(
            name="echo-driver",
            version="2.1.0",
            action="echo",
            abi_version=2,
            wasm_hash=HASH,
            author_node_id=UUID(int=2),
            migrations=[migration],
        )

        self.assertEqual(request.to_dict()["abi_requirement"], ">=2,<4")
        self.assertEqual(entry.migrations[0].migration_driver_action, "echo.migrate")
        self.assertEqual(registry_entry.migrations[0].from_abi_requirement, ">=1,<=2")

    def test_udp_client_sends_control_envelopes(self):
        server = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
        server.bind(("127.0.0.1", 0))
        received: list[bytes] = []

        def serve() -> None:
            payload, addr = server.recvfrom(65535)
            received.append(payload)
            server.sendto(payload, addr)

        thread = threading.Thread(target=serve)
        thread.start()
        with ZapUdpClient(timeout=2.0) as client:
            response = client.request_control(
                registry_bundle_manifest_request_frame(require_drivers=True),
                server.getsockname(),
            )
        thread.join(timeout=2.0)
        server.close()

        self.assertEqual(response.subject, REGISTRY_BUNDLE_MANIFEST_REQUEST_SUBJECT)
        self.assertTrue(response.json_body()["require_drivers"])
        self.assertTrue(received)


if __name__ == "__main__":
    unittest.main()
