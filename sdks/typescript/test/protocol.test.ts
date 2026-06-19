import assert from "node:assert/strict";
import { createSocket } from "node:dgram";
import test from "node:test";

import {
  AGENT_CONTENT_TYPE,
  AGENT_INTENT_SUBJECT,
  ControlFrame,
  REGISTRY_BUNDLE_MANIFEST_CONTENT_TYPE,
  REGISTRY_BUNDLE_MANIFEST_REQUEST_SUBJECT,
  RECEIPT_REPLICATION_CONTENT_TYPE,
  RECEIPT_REPLICATION_RESPONSE_SUBJECT,
  RECEIPT_SIGNATURE_DOMAIN,
  ZapStoreClient,
  ZapUdpClient,
  artifactHash,
  registryBundleManifestRequestFrame,
  receiptBodyHash,
  receiptSigningMessage,
  signatureVerificationPlaceholder,
  validateArtifactHash,
  validateReceiptResponseShape,
  validateReceiptShape,
  validateRegistryBundleManifestResponse,
  zapDomainMessage,
} from "../src/index.ts";
import type { DriverRegistryEntry, RegistryInstallPlanEntry, RegistryInstallPlanRequest } from "../src/index.ts";

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
  assert.match(artifactHash(Buffer.from("driver")), /^blake3:[0-9a-f]{64}$/);
  const status = signatureVerificationPlaceholder("registry");
  assert.equal(status.supported, false);
  assert.match(status.reason, /Ed25519/);
});

test("receipt crypto helpers validate shape and build exact messages", () => {
  const receipt = {
    schema_version: 1,
    receipt_id: "dddddddd-dddd-4ddd-8ddd-dddddddddddd",
    node_id: "11111111-1111-4111-8111-111111111111",
    frame_id: "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb",
    subject: "zap.registry.index.request",
    content_type: "application/zap-registry-index+json",
    body_hash: HASH,
    policy_decision: "allow",
    outcome: "accepted",
    started_at_unix_micros: 1893456000000000,
    finished_at_unix_micros: 1893456000000100,
    signer_public_key: "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
    signature: "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
  };

  validateReceiptShape(receipt);
  validateReceiptResponseShape({
    schema_version: 1,
    request_id: "cccccccc-cccc-4ccc-8ccc-cccccccccccc",
    truncated: false,
    receipts: [receipt],
  });
  const message = Buffer.from(receiptSigningMessage(receipt)).toString("utf8");
  assert.equal(message.startsWith(`${RECEIPT_SIGNATURE_DOMAIN}{"receipt":`), true);
  assert.match(message, /"signer_node_id":"11111111-1111-4111-8111-111111111111"/);
  assert.doesNotMatch(message, /"signature"/);
  assert.deepEqual(zapDomainMessage("ZAP-TEST-v1", Buffer.from("payload")), Buffer.from("ZAP-TEST-v1\0payload"));
  assert.match(receiptBodyHash(Buffer.from("receipt-body")), /^blake3:[0-9a-f]{64}$/);
  assert.equal(RECEIPT_REPLICATION_RESPONSE_SUBJECT, "zap.receipts.response");
  assert.equal(RECEIPT_REPLICATION_CONTENT_TYPE, "application/zap-receipts+json");
  assert.equal(AGENT_INTENT_SUBJECT, "zap.agent.intent");
  assert.equal(AGENT_CONTENT_TYPE, "application/zap-agent+json");
});

test("install plan types carry ABI requirements and migrations", () => {
  const request: RegistryInstallPlanRequest = {
    action: "echo",
    requirement: "^2.0.0",
    abi_requirement: ">=2,<4",
  };
  const entry: RegistryInstallPlanEntry = {
    action: "echo",
    requirement: "^2.0.0",
    requested_abi_requirement: ">=2,<4",
    selected_version: "2.1.0",
    name: "echo-driver",
    abi_version: 2,
    wasm_hash: HASH,
    author_node_id: "00000000-0000-0000-0000-000000000002",
    migrations: [
      {
        from_version_requirement: "<2.0.0",
        from_abi_requirement: ">=1,<=2",
        requires_operator_approval: true,
        migration_driver_action: "echo.migrate",
        migration_driver_version: "1.0.0",
      },
    ],
  };
  const registryEntry: DriverRegistryEntry = {
    name: "echo-driver",
    version: "2.1.0",
    action: "echo",
    abi_version: 2,
    wasm_hash: HASH,
    author_node_id: "00000000-0000-0000-0000-000000000002",
    migrations: entry.migrations,
  };

  assert.equal(request.abi_requirement, ">=2,<4");
  assert.equal(entry.migrations?.[0]?.migration_driver_action, "echo.migrate");
  assert.equal(registryEntry.migrations?.[0]?.from_abi_requirement, ">=1,<=2");
});

test("UDP client sends and receives control envelopes", async () => {
  const server = createSocket("udp4");
  await new Promise<void>((resolve) => server.bind(0, "127.0.0.1", resolve));
  const address = server.address();
  assert.notEqual(typeof address, "string");
  const target = address as { address: string; port: number };
  server.once("message", (message, remote) => {
    server.send(message, remote.port, remote.address);
  });

  const client = new ZapUdpClient();
  await client.bind();
  const response = await client.requestControl(
    registryBundleManifestRequestFrame({ requireDrivers: true }),
    { host: target.address, port: target.port },
  );
  client.close();
  server.close();

  assert.equal(response.subject, REGISTRY_BUNDLE_MANIFEST_REQUEST_SUBJECT);
  assert.equal((response.jsonBody() as { require_drivers: boolean }).require_drivers, true);
});
