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
  VERSION,
  ZapEnvelope,
  ZapMessageKind,
  registryBundleManifestRequestFrame,
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

async function loadRootFixture<T>(name: string): Promise<T> {
  const fixturePath = path.join(import.meta.dirname, "..", "..", "..", "fixtures", name);
  return JSON.parse(await readFile(fixturePath, "utf8")) as T;
}
