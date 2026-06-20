import json
import socket
import threading
import unittest
from pathlib import Path
from uuid import UUID

from zap_sdk import (
    AGENT_CONTENT_TYPE,
    AGENT_INTENT_SUBJECT,
    MissingCryptoBackend,
    PACT_BUNDLE_SUBJECT,
    PACT_CONTENT_TYPE,
    PACT_RECORD_SUBJECT,
    REGISTRY_BUNDLE_MANIFEST_CONTENT_TYPE,
    REGISTRY_BUNDLE_MANIFEST_REQUEST_SUBJECT,
    RECEIPT_REPLICATION_CONTENT_TYPE,
    RECEIPT_REPLICATION_RESPONSE_SUBJECT,
    RECEIPT_SIGNATURE_DOMAIN,
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
    ZapEnvelope,
    ZapMessageKind,
    ZapUdpClient,
    pact_hash,
    registry_index_request_frame,
    registry_bundle_manifest_request_frame,
    receipt_body_hash,
    receipt_signing_message,
    validate_artifact_hash,
    validate_pact_shape,
    validate_receipt_response_shape,
    validate_receipt_shape,
    verify_signature_placeholder,
    verify_pact,
    verify_pact_bundle,
    zap_domain_message,
)

REPO_ROOT = Path(__file__).resolve().parents[3]
FIXTURES = REPO_ROOT / "fixtures"
HASH = "blake3:" + "0" * 64


def load_fixture(name: str) -> dict:
    with (FIXTURES / name).open(encoding="utf-8") as handle:
        return json.load(handle)


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

    def test_registry_bundle_manifest_request_matches_root_fixture_contract(self):
        fixture = load_fixture("zenv-control-registry-bundle-manifest-request.json")
        envelope = fixture["envelope"]

        frame = registry_bundle_manifest_request_frame(
            require_publication=envelope["body_json"]["require_publication"],
            require_drivers=envelope["body_json"]["require_drivers"],
        )
        decoded = ControlFrame.decode(frame.encode())

        self.assertEqual(fixture["fixture_schema_version"], 1)
        self.assertEqual(envelope["magic"], "ZENV")
        self.assertEqual(envelope["version"], 1)
        self.assertEqual(envelope["kind_name"], ZapMessageKind.CONTROL.protocol_name)
        self.assertEqual(envelope["kind_value"], int(ZapMessageKind.CONTROL))
        self.assertEqual(decoded.subject, envelope["subject"])
        self.assertEqual(decoded.content_type, envelope["content_type"])
        self.assertEqual(decoded.json_body(), envelope["body_json"])
        self.assertEqual(
            RegistryBundleManifestRequest(
                require_publication=True,
                require_drivers=True,
            ).to_dict(),
            envelope["body_json"],
        )

    def test_agent_intent_fixture_is_recognized_as_json_envelope_contract(self):
        fixture = load_fixture("agent-intent-message-v1.json")
        body = fixture["body_json"]
        payload = body["payload"]

        frame = ControlFrame.json(fixture["subject"], fixture["content_type"], body)
        decoded = ControlFrame.decode(frame.encode())
        decoded_payload = decoded.json_body()["payload"]

        self.assertEqual(fixture["fixture_schema_version"], 1)
        self.assertEqual(decoded.subject, "zap.agent.intent")
        self.assertEqual(decoded.content_type, "application/zap-agent+json")
        self.assertEqual(decoded.json_body()["type"], "intent")
        self.assertEqual(decoded_payload["schema_version"], 1)
        self.assertEqual(UUID(decoded_payload["intent_id"]), UUID(payload["intent_id"]))
        self.assertEqual(UUID(decoded_payload["session_id"]), UUID(payload["session_id"]))
        self.assertEqual(decoded_payload["source_agent"], "planner.main")
        self.assertEqual(decoded_payload["target_agent"], "executor.safety")
        self.assertEqual(decoded_payload["required_capabilities"], ["driver.execute:valve.open"])

    def test_unsigned_control_frame_fixture_round_trips_without_trailers(self):
        fixture = load_fixture("protocol/zenv-unsigned-control-frame-v1.json")
        envelope = fixture["envelope"]
        security = fixture["security"]

        frame = registry_index_request_frame(
            require_signature=envelope["body_json"]["require_signature"],
        )
        deterministic = ControlFrame(
            subject=frame.subject,
            content_type=frame.content_type,
            body=frame.body,
            id=UUID(envelope["id"]),
        )
        decoded = ZapEnvelope.decode(deterministic.encode())

        self.assertEqual(fixture["fixture_schema_version"], 1)
        self.assertEqual(decoded.kind, ZapMessageKind.CONTROL)
        self.assertEqual(decoded.id, UUID(envelope["id"]))
        self.assertIsNone(decoded.correlation_id)
        self.assertIsNone(decoded.causation_id)
        self.assertEqual(decoded.subject, envelope["subject"])
        self.assertEqual(decoded.content_type, envelope["content_type"])
        self.assertEqual(json.loads(decoded.body.decode("utf-8")), envelope["body_json"])
        self.assertFalse(security["signed"])
        self.assertFalse(security["encrypted"])
        self.assertEqual(security["signature_hint_hex"], "0000000000000000")
        self.assertIsNone(security["auth_trailer"])
        self.assertIsNone(security["poa_trailer"])

    def test_receipt_sample_fixture_has_stable_response_body(self):
        fixture = load_fixture("protocol/receipt-sample-v1.json")
        body = fixture["body_json"]
        receipt = body["receipts"][0]

        frame = ControlFrame.json(fixture["subject"], fixture["content_type"], body)
        decoded = ControlFrame.decode(frame.encode())

        self.assertEqual(decoded.subject, "zap.receipts.response")
        self.assertEqual(decoded.content_type, "application/zap-receipts+json")
        self.assertEqual(decoded.json_body(), body)
        self.assertEqual(body["schema_version"], 1)
        self.assertFalse(body["truncated"])
        self.assertEqual(receipt["schema_version"], 1)
        self.assertEqual(UUID(receipt["frame_id"]), UUID("bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb"))
        self.assertEqual(receipt["policy_decision"], "allow")
        self.assertEqual(receipt["outcome"], "accepted")
        self.assertTrue(validate_artifact_hash(receipt["body_hash"]))
        self.assertGreaterEqual(receipt["finished_at_unix_micros"], receipt["started_at_unix_micros"])

    def test_receipt_helpers_validate_fixture_and_build_signing_message(self):
        fixture = load_fixture("protocol/receipt-sample-v1.json")
        body = fixture["body_json"]
        receipt = body["receipts"][0]

        validate_receipt_response_shape(body)
        validate_receipt_shape(receipt)

        message = receipt_signing_message(receipt)
        self.assertTrue(message.startswith(RECEIPT_SIGNATURE_DOMAIN + b'{"receipt":'))
        self.assertIn(b'"signer_node_id":"11111111-1111-4111-8111-111111111111"', message)
        self.assertNotIn(b'"signature"', message)
        self.assertEqual(
            zap_domain_message(b"ZAP-TEST-v1", b"payload"),
            b"ZAP-TEST-v1\x00payload",
        )
        try:
            self.assertRegex(receipt_body_hash(b"receipt-body"), r"^blake3:[0-9a-f]{64}$")
        except RuntimeError as exc:
            self.assertIn("BLAKE3", str(exc))
        self.assertEqual(fixture["subject"], RECEIPT_REPLICATION_RESPONSE_SUBJECT)
        self.assertEqual(fixture["content_type"], RECEIPT_REPLICATION_CONTENT_TYPE)
        self.assertEqual(AGENT_INTENT_SUBJECT, "zap.agent.intent")
        self.assertEqual(AGENT_CONTENT_TYPE, "application/zap-agent+json")

    def test_pact_fixtures_reproduce_hash_and_verify(self):
        record = load_fixture("pact-record-v1.json")
        bundle = load_fixture("pact-bundle-v1.json")
        pact = record["body_json"]

        self.assertEqual(record["subject"], PACT_RECORD_SUBJECT)
        self.assertEqual(record["content_type"], PACT_CONTENT_TYPE)
        self.assertEqual(bundle["subject"], PACT_BUNDLE_SUBJECT)
        validate_pact_shape(pact)
        try:
            self.assertEqual(pact_hash(pact), pact["hash"])
            self.assertTrue(verify_pact(pact, 1893457000000000))
            self.assertTrue(verify_pact_bundle(bundle["body_json"], 1893457000000000))
        except MissingCryptoBackend as exc:
            self.skipTest(str(exc))

    def test_receipt_response_shape_rejects_invalid_body_hash(self):
        fixture = load_fixture("protocol/receipt-sample-v1.json")
        body = dict(fixture["body_json"])
        receipt = dict(body["receipts"][0])
        receipt["body_hash"] = "sha256:" + "0" * 64
        body["receipts"] = [receipt]

        with self.assertRaisesRegex(ValueError, "invalid receipt body hash"):
            validate_receipt_response_shape(body)

    def test_security_protocol_fixtures_cover_signed_poa_capability_and_datagram_shapes(self):
        signed = load_fixture("protocol/signed-control-frame-v1.json")
        poa = load_fixture("protocol/poa-control-frame-v1.json")
        capability = load_fixture("protocol/capability-response-v1.json")
        datagram = load_fixture("protocol/encrypted-datagram-v1.json")

        self.assertTrue(signed["security"]["signed"])
        self.assertEqual(signed["security"]["auth_trailer"]["algorithm"], "ed25519")
        self.assertTrue(poa["security"]["signed"])
        self.assertEqual(poa["security"]["poa_trailer"]["threshold"], 1)
        self.assertEqual(capability["subject"], "zap.capability.response")
        self.assertIn("driver.execute:echo", capability["body_json"]["capabilities"])
        self.assertEqual(datagram["cipher"], "ChaCha20-Poly1305")
        self.assertEqual(len(datagram["nonce_hex"]), 24)

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
