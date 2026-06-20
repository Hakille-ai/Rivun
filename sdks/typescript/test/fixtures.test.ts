import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import path from "node:path";
import test from "node:test";

import {
  ControlFrame,
  MAGIC,
  REGISTRY_BUNDLE_MANIFEST_CONTENT_TYPE,
  REGISTRY_BUNDLE_MANIFEST_REQUEST_SUBJECT,
  REGISTRY_BUNDLE_MANIFEST_RESPONSE_SUBJECT,
  REGISTRY_INDEX_CONTENT_TYPE,
  REGISTRY_INDEX_REQUEST_SUBJECT,
  REGISTRY_INDEX_RESPONSE_SUBJECT,
  PACT_BUNDLE_SUBJECT,
  PACT_CONTENT_TYPE,
  PACT_RECORD_SUBJECT,
  RECEIPT_REPLICATION_CONTENT_TYPE,
  RECEIPT_REPLICATION_RESPONSE_SUBJECT,
  VERSION,
  ZapEnvelope,
  ZapMessageKind,
  pactHash,
  receiptSigningMessage,
  registryBundleManifestRequestFrame,
  registryIndexRequestFrame,
  validateArtifactHash,
  validateReceiptResponseShape,
  verifyPact,
  verifyPactBundle,
} from "../src/index.ts";

type RegistryBundleManifestRequestFixture = {
  fixture_schema_version: number;
  envelope: {
    magic: string;
    version: number;
    kind_name: string;
    kind_value: number;
    reserved: number;
    id: string;
    correlation_id: string;
    causation_id: string;
    subject: string;
    content_type: string;
    metadata_base64: string;
    body_json: {
      schema_version: number;
      require_publication: boolean;
      require_drivers: boolean;
    };
  };
};

type ControlSubjectsFixture = {
  fixture_schema_version: number;
  envelope: {
    magic: string;
    version: number;
    kind_name: string;
    kind_value: number;
  };
  subjects: Array<{
    subject: string;
    content_type: string;
    purpose: string;
  }>;
};

type AgentIntentFixture = {
  fixture_schema_version: number;
  subject: string;
  content_type: string;
  body_json: {
    type: string;
    payload: {
      schema_version: number;
      intent_id: string;
      session_id: string;
      source_agent: string;
      target_agent?: string;
      kind: string;
      objective: string;
      input: unknown;
      required_capabilities: string[];
      priority: string;
      metadata: Record<string, unknown>;
    };
  };
};

type UnsignedControlFrameFixture = {
  fixture_schema_version: number;
  envelope: {
    magic: string;
    version: number;
    kind_name: string;
    kind_value: number;
    reserved: number;
    id: string;
    correlation_id: string;
    causation_id: string;
    subject: string;
    content_type: string;
    metadata_base64: string;
    body_json: {
      schema_version: number;
      require_signature: boolean;
    };
  };
  security: {
    signed: boolean;
    encrypted: boolean;
    signature_hint_hex: string;
    auth_trailer: null;
    poa_trailer: null;
  };
};

type ReceiptSampleFixture = {
  fixture_schema_version: number;
  subject: string;
  content_type: string;
  body_json: {
    schema_version: number;
    request_id: string;
    truncated: boolean;
    receipts: Array<{
      schema_version: number;
      receipt_id: string;
      node_id: string;
      frame_id: string;
      subject: string;
      content_type: string;
      body_hash: string;
      policy_decision: string;
      outcome: string;
      started_at_unix_micros: number;
      finished_at_unix_micros: number;
      metadata: Record<string, unknown>;
      signer_public_key: string;
      signature: string;
    }>;
  };
};

test("registry bundle manifest fixture matches TypeScript ZapStore helper", async () => {
  const fixture = await loadRootFixture<RegistryBundleManifestRequestFixture>(
    "zenv-control-registry-bundle-manifest-request.json",
  );

  assert.equal(fixture.fixture_schema_version, 1);
  assert.equal(fixture.envelope.magic, MAGIC);
  assert.equal(fixture.envelope.version, VERSION);
  assert.equal(fixture.envelope.kind_name, "control");
  assert.equal(fixture.envelope.kind_value, ZapMessageKind.control);
  assert.equal(fixture.envelope.reserved, 0);
  assert.equal(fixture.envelope.subject, REGISTRY_BUNDLE_MANIFEST_REQUEST_SUBJECT);
  assert.equal(fixture.envelope.content_type, REGISTRY_BUNDLE_MANIFEST_CONTENT_TYPE);
  assert.equal(fixture.envelope.metadata_base64, "");

  const frame = registryBundleManifestRequestFrame({
    requirePublication: fixture.envelope.body_json.require_publication,
    requireDrivers: fixture.envelope.body_json.require_drivers,
  });
  const encoded = frame.encode();
  const decoded = ControlFrame.decode(encoded);

  assert.equal(Buffer.from(encoded).subarray(0, 4).toString("ascii"), fixture.envelope.magic);
  assert.equal(Buffer.from(encoded).readUInt16BE(4), fixture.envelope.version);
  assert.equal(Buffer.from(encoded).readUInt16BE(6), fixture.envelope.kind_value);
  assert.equal(decoded.subject, fixture.envelope.subject);
  assert.equal(decoded.contentType, fixture.envelope.content_type);
  assert.deepEqual(decoded.jsonBody(), fixture.envelope.body_json);
});

test("control subject fixture contains SDK registry subjects and is frame-compatible", async () => {
  const fixture = await loadRootFixture<ControlSubjectsFixture>("control-subjects-v1.json");

  assert.equal(fixture.fixture_schema_version, 1);
  assert.equal(fixture.envelope.magic, MAGIC);
  assert.equal(fixture.envelope.version, VERSION);
  assert.equal(fixture.envelope.kind_name, "control");
  assert.equal(fixture.envelope.kind_value, ZapMessageKind.control);

  const subjects = new Map(fixture.subjects.map((entry) => [entry.subject, entry.content_type]));
  assert.equal(subjects.get(REGISTRY_INDEX_REQUEST_SUBJECT), REGISTRY_INDEX_CONTENT_TYPE);
  assert.equal(subjects.get(REGISTRY_INDEX_RESPONSE_SUBJECT), REGISTRY_INDEX_CONTENT_TYPE);
  assert.equal(subjects.get(REGISTRY_BUNDLE_MANIFEST_REQUEST_SUBJECT), REGISTRY_BUNDLE_MANIFEST_CONTENT_TYPE);
  assert.equal(subjects.get(REGISTRY_BUNDLE_MANIFEST_RESPONSE_SUBJECT), REGISTRY_BUNDLE_MANIFEST_CONTENT_TYPE);
  assert.equal(subjects.get(PACT_BUNDLE_SUBJECT), PACT_CONTENT_TYPE);

  for (const entry of fixture.subjects) {
    const frame = ControlFrame.json(entry.subject, entry.content_type, { schema_version: 1 });
    const decoded = ControlFrame.decode(frame.encode());

    assert.equal(decoded.subject, entry.subject);
    assert.equal(decoded.contentType, entry.content_type);
    assert.deepEqual(decoded.jsonBody(), { schema_version: 1 });
  }
});

test("agent intent fixture can be carried by TypeScript protocol envelopes", async () => {
  const fixture = await loadRootFixture<AgentIntentFixture>("agent-intent-message-v1.json");

  assert.equal(fixture.fixture_schema_version, 1);
  assert.equal(fixture.subject, "zap.agent.intent");
  assert.equal(fixture.content_type, "application/zap-agent+json");
  assert.equal(fixture.body_json.type, "intent");
  assert.equal(fixture.body_json.payload.schema_version, 1);
  assert.equal(fixture.body_json.payload.kind, "act");
  assert.deepEqual(fixture.body_json.payload.required_capabilities, ["driver.execute:valve.open"]);

  const envelope = new ZapEnvelope({
    kind: ZapMessageKind.action,
    subject: fixture.subject,
    contentType: fixture.content_type,
    body: JSON.stringify(fixture.body_json),
  });
  const decoded = ZapEnvelope.decode(envelope.encode());

  assert.equal(decoded.kind, ZapMessageKind.action);
  assert.equal(decoded.subject, fixture.subject);
  assert.equal(decoded.contentType, fixture.content_type);
  assert.deepEqual(JSON.parse(Buffer.from(decoded.body).toString("utf8")), fixture.body_json);
});

test("unsigned control frame fixture round-trips without security trailers", async () => {
  const fixture = await loadRootFixture<UnsignedControlFrameFixture>(
    "protocol/zenv-unsigned-control-frame-v1.json",
  );

  assert.equal(fixture.fixture_schema_version, 1);
  assert.equal(fixture.envelope.magic, MAGIC);
  assert.equal(fixture.envelope.version, VERSION);
  assert.equal(fixture.envelope.kind_name, "control");
  assert.equal(fixture.envelope.kind_value, ZapMessageKind.control);
  assert.equal(fixture.envelope.subject, REGISTRY_INDEX_REQUEST_SUBJECT);
  assert.equal(fixture.envelope.content_type, REGISTRY_INDEX_CONTENT_TYPE);

  const frame = registryIndexRequestFrame(fixture.envelope.body_json.require_signature);
  const deterministic = new ControlFrame({
    subject: frame.subject,
    contentType: frame.contentType,
    body: frame.body,
    id: fixture.envelope.id,
  });
  const decoded = ZapEnvelope.decode(deterministic.encode());

  assert.equal(decoded.kind, ZapMessageKind.control);
  assert.equal(decoded.id, fixture.envelope.id);
  assert.equal(decoded.correlationId, null);
  assert.equal(decoded.causationId, null);
  assert.equal(decoded.subject, fixture.envelope.subject);
  assert.equal(decoded.contentType, fixture.envelope.content_type);
  assert.deepEqual(JSON.parse(Buffer.from(decoded.body).toString("utf8")), fixture.envelope.body_json);
  assert.equal(fixture.security.signed, false);
  assert.equal(fixture.security.encrypted, false);
  assert.equal(fixture.security.signature_hint_hex, "0000000000000000");
  assert.equal(fixture.security.auth_trailer, null);
  assert.equal(fixture.security.poa_trailer, null);
});

test("receipt sample fixture can be carried in a control envelope", async () => {
  const fixture = await loadRootFixture<ReceiptSampleFixture>("protocol/receipt-sample-v1.json");
  const receipt = fixture.body_json.receipts[0];

  assert.equal(fixture.fixture_schema_version, 1);
  assert.equal(fixture.subject, RECEIPT_REPLICATION_RESPONSE_SUBJECT);
  assert.equal(fixture.content_type, RECEIPT_REPLICATION_CONTENT_TYPE);
  assert.equal(fixture.body_json.schema_version, 1);
  assert.equal(fixture.body_json.truncated, false);
  validateReceiptResponseShape(fixture.body_json);
  assert.equal(receipt.schema_version, 1);
  assert.equal(receipt.frame_id, "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb");
  assert.equal(receipt.subject, REGISTRY_INDEX_REQUEST_SUBJECT);
  assert.equal(receipt.content_type, REGISTRY_INDEX_CONTENT_TYPE);
  assert.equal(receipt.policy_decision, "allow");
  assert.equal(receipt.outcome, "accepted");
  assert.equal(validateArtifactHash(receipt.body_hash), true);
  assert.equal(receipt.finished_at_unix_micros >= receipt.started_at_unix_micros, true);
  assert.equal(Buffer.from(receiptSigningMessage(receipt)).includes(Buffer.from('"signature"')), false);

  const frame = ControlFrame.json(fixture.subject, fixture.content_type, fixture.body_json);
  const decoded = ControlFrame.decode(frame.encode());

  assert.equal(decoded.subject, fixture.subject);
  assert.equal(decoded.contentType, fixture.content_type);
  assert.deepEqual(decoded.jsonBody(), fixture.body_json);
});

test("PACT fixtures reproduce canonical hash and verify in TypeScript", async () => {
  const record = await loadRootFixture<any>("pact-record-v1.json");
  const bundle = await loadRootFixture<any>("pact-bundle-v1.json");

  assert.equal(record.subject, PACT_RECORD_SUBJECT);
  assert.equal(record.content_type, PACT_CONTENT_TYPE);
  assert.equal(pactHash(record.body_json), record.body_json.hash);
  assert.equal(await verifyPact(record.body_json, 1893457000000000), true);
  assert.equal(bundle.subject, PACT_BUNDLE_SUBJECT);
  assert.equal(await verifyPactBundle(bundle.body_json, 1893457000000000), true);
});

test("security protocol fixtures cover signed, PoA, capability, and datagram shapes", async () => {
  const signed = await loadRootFixture<any>("protocol/signed-control-frame-v1.json");
  const poa = await loadRootFixture<any>("protocol/poa-control-frame-v1.json");
  const capability = await loadRootFixture<any>("protocol/capability-response-v1.json");
  const datagram = await loadRootFixture<any>("protocol/encrypted-datagram-v1.json");

  assert.equal(signed.security.signed, true);
  assert.equal(signed.security.auth_trailer.algorithm, "ed25519");
  assert.equal(poa.security.signed, true);
  assert.equal(poa.security.poa_trailer.threshold, 1);
  assert.equal(capability.subject, "zap.capability.response");
  assert.equal(capability.content_type, "application/zap-capability+json");
  assert.equal(capability.body_json.capabilities.includes("driver.execute:echo"), true);
  assert.equal(datagram.cipher, "ChaCha20-Poly1305");
  assert.equal(datagram.nonce_hex.length, 24);
});

async function loadRootFixture<T>(name: string): Promise<T> {
  const fixturePath = path.join(import.meta.dirname, "..", "..", "..", "fixtures", name);
  return JSON.parse(await readFile(fixturePath, "utf8")) as T;
}
