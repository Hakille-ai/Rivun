import {
  ControlFrame,
  REGISTRY_BUNDLE_MANIFEST_CONTENT_TYPE,
  REGISTRY_BUNDLE_MANIFEST_REQUEST_SUBJECT,
  REGISTRY_INDEX_CONTENT_TYPE,
  REGISTRY_INDEX_REQUEST_SUBJECT,
} from "./protocol.ts";
import * as ed25519 from "@noble/ed25519";
import { blake3 } from "@noble/hashes/blake3";
import { sha512 } from "@noble/hashes/sha512";

ed25519.etc.sha512Sync = (...messages: Uint8Array[]) => sha512(ed25519.etc.concatBytes(...messages));

export const REGISTRY_INDEX_SYNC_SCHEMA_VERSION = 1;
export const REGISTRY_BUNDLE_SCHEMA_VERSION = 1;
export const REGISTRY_INSTALL_PLAN_SCHEMA_VERSION = 1;
export const DRIVER_ABI_VERSION = 1;
export const DRIVER_HASH_PREFIX = "blake3:";
export const RECEIPT_SCHEMA_VERSION = 1;
export const RECEIPT_REPLICATION_SCHEMA_VERSION = 1;
export const RECEIPT_REPLICATION_CONTENT_TYPE = "application/rivun-receipts+json";
export const RECEIPT_REPLICATION_REQUEST_SUBJECT = "rivun.receipts.request";
export const RECEIPT_REPLICATION_RESPONSE_SUBJECT = "rivun.receipts.response";
export const RECEIPT_SIGNATURE_DOMAIN = "rivun-ACTION-RECEIPT-v1";
export const AGENT_CONTENT_TYPE = "application/rivun-agent+json";
export const AGENT_INTENT_SUBJECT = "rivun.agent.intent";
export const AGENT_STATUS_SUBJECT = "rivun.agent.status";
export const AGENT_RESULT_SUBJECT = "rivun.agent.result";
export const PACT_SCHEMA_VERSION = 1;
export const PACT_CONTENT_TYPE = "application/rivun-pact+json";
export const PACT_RECORD_SUBJECT = "rivun.pact.record";
export const PACT_VERIFY_SUBJECT = "rivun.pact.verify";
export const PACT_REVOKE_SUBJECT = "rivun.pact.revoke";
export const PACT_BUNDLE_SUBJECT = "rivun.pact.bundle";
export const PACT_SIGNATURE_DOMAIN = "ZAP-PACT-v1";

export type RivunPactStatus = "draft" | "active" | "expired" | "revoked" | "invalid";

export type RivunPact = {
  schema_version: number;
  pact_id: string;
  actor: string;
  target: string;
  intent: string;
  object?: unknown;
  terms?: unknown;
  consent?: unknown;
  proof?: unknown;
  created_at_micros: number;
  expires_at_micros?: number | null;
  actor_public_key?: string;
  hash?: string;
  signature?: string;
  status?: RivunPactStatus;
};

export type RivunPactBundle = {
  schema_version: number;
  pact: RivunPact;
  verifications?: unknown[];
  revocations?: unknown[];
  metadata?: Record<string, unknown>;
};

export type DriverRegistryStatus = "active" | "deprecated" | "revoked";

export type DriverRegistryMigration = {
  from_version_requirement: string;
  from_abi_requirement?: string;
  requires_operator_approval?: boolean;
  migration_driver_action?: string;
  migration_driver_version?: string;
  notes?: string;
};

export type RegistryIndexRequest = {
  schema_version: number;
  require_signature: boolean;
};

export type RegistryBundleManifestRequest = {
  schema_version: number;
  require_publication: boolean;
  require_drivers: boolean;
};

export type DriverRegistryEntry = {
  name: string;
  version: string;
  action: string;
  abi_version: number;
  wasm_hash: string;
  manifest_path?: string;
  author_node_id: string;
  status?: DriverRegistryStatus;
  revoked_reason?: string;
  deprecated_reason?: string;
  migrations?: DriverRegistryMigration[];
};

export type DriverRegistry = {
  schema_version: number;
  generated_by?: string;
  operator_node_id?: string;
  operator_public_key?: string;
  signature?: string;
  entries: DriverRegistryEntry[];
};

export type RegistryIndexResponse = {
  schema_version: number;
  node_id: string;
  registry?: DriverRegistry;
  unavailable_reason?: string;
};

export type RegistryBundleEntry = {
  action: string;
  version: string;
  name: string;
  abi_version: number;
  wasm_hash: string;
  author_node_id: string;
  status: DriverRegistryStatus;
  manifest_path?: string;
  manifest_hash?: string;
  driver_path?: string;
  driver_hash?: string;
};

export type RegistryBundleManifest = {
  schema_version: number;
  generated_by?: string;
  registry_path: string;
  registry_hash: string;
  publication_path?: string;
  publication_hash?: string;
  entries: RegistryBundleEntry[];
};

export type RegistryBundleManifestResponse = {
  schema_version: number;
  node_id: string;
  manifest?: RegistryBundleManifest;
  unavailable_reason?: string;
};

export type RegistryInstallPlanRequest = {
  action: string;
  requirement: string;
  abi_version?: number;
  abi_requirement?: string;
};

export type RegistryInstallPlanEntry = {
  action: string;
  requirement: string;
  requested_abi_version?: number;
  requested_abi_requirement?: string;
  selected_version: string;
  name: string;
  abi_version: number;
  wasm_hash: string;
  manifest_path?: string;
  author_node_id: string;
  migrations?: DriverRegistryMigration[];
};

export type RegistryInstallPlan = {
  schema_version: number;
  registry_hash: string;
  registry_entries: number;
  registry_operator_node_id?: string;
  publication_hash?: string;
  requested_at_micros: number;
  target?: string;
  labels: string[];
  entries: RegistryInstallPlanEntry[];
  planner_node_id: string;
  planner_public_key: string;
  signature: string;
};

export type SignatureVerificationStatus = {
  supported: boolean;
  reason: string;
};

export type ReceiptSample = {
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
  metadata?: Record<string, unknown>;
  signer_public_key: string;
  signature: string;
};

export type ReceiptReplicationResponseBody = {
  schema_version: number;
  request_id: string;
  truncated: boolean;
  receipts: ReceiptSample[];
};

export class RivunStoreClient {
  registryIndexRequest(requireSignature = false): ControlFrame {
    return registryIndexRequestFrame(requireSignature);
  }

  registryBundleManifestRequest(options: {
    requirePublication?: boolean;
    requireDrivers?: boolean;
  } = {}): ControlFrame {
    return registryBundleManifestRequestFrame(options);
  }
}

export function registryIndexRequestFrame(requireSignature = false): ControlFrame {
  const request: RegistryIndexRequest = {
    schema_version: REGISTRY_INDEX_SYNC_SCHEMA_VERSION,
    require_signature: requireSignature,
  };
  return ControlFrame.json(REGISTRY_INDEX_REQUEST_SUBJECT, REGISTRY_INDEX_CONTENT_TYPE, request);
}

export function registryBundleManifestRequestFrame(options: {
  requirePublication?: boolean;
  requireDrivers?: boolean;
} = {}): ControlFrame {
  const request: RegistryBundleManifestRequest = {
    schema_version: REGISTRY_BUNDLE_SCHEMA_VERSION,
    require_publication: options.requirePublication ?? false,
    require_drivers: options.requireDrivers ?? false,
  };
  return ControlFrame.json(REGISTRY_BUNDLE_MANIFEST_REQUEST_SUBJECT, REGISTRY_BUNDLE_MANIFEST_CONTENT_TYPE, request);
}

export function validateRegistryBundleManifestResponse(
  response: RegistryBundleManifestResponse,
  request: RegistryBundleManifestRequest,
): void {
  if (response.schema_version !== REGISTRY_BUNDLE_SCHEMA_VERSION) {
    throw new Error(`unsupported registry bundle schema version ${response.schema_version}`);
  }
  if (request.schema_version !== REGISTRY_BUNDLE_SCHEMA_VERSION) {
    throw new Error(`unsupported registry bundle request schema version ${request.schema_version}`);
  }
  if (!response.manifest) return;
  validateRegistryBundleManifest(response.manifest);
  if (request.require_publication && (!response.manifest.publication_path || !response.manifest.publication_hash)) {
    throw new Error("registry bundle publication path/hash metadata is incomplete");
  }
  if (request.require_drivers) {
    for (const entry of response.manifest.entries) {
      if (!entry.driver_path || !entry.driver_hash) {
        throw new Error(`registry bundle entry ${entry.action}@${entry.version} lacks driver metadata`);
      }
    }
  }
}

export function validateRegistryBundleManifest(manifest: RegistryBundleManifest): void {
  if (manifest.schema_version !== REGISTRY_BUNDLE_SCHEMA_VERSION) {
    throw new Error(`unsupported registry bundle schema version ${manifest.schema_version}`);
  }
  validateRelativePath(manifest.registry_path);
  if (!validateArtifactHash(manifest.registry_hash)) throw new Error(`invalid registry hash ${manifest.registry_hash}`);
  if (Boolean(manifest.publication_path) !== Boolean(manifest.publication_hash)) {
    throw new Error("registry bundle publication path/hash metadata is incomplete");
  }
  if (manifest.publication_path) validateRelativePath(manifest.publication_path);
  if (manifest.publication_hash && !validateArtifactHash(manifest.publication_hash)) {
    throw new Error(`invalid publication hash ${manifest.publication_hash}`);
  }
  const seen = new Set<string>();
  for (const entry of manifest.entries) {
    const key = `${entry.action}@${entry.version}`;
    if (seen.has(key)) throw new Error(`duplicate registry bundle entry ${key}`);
    seen.add(key);
    validateRegistryBundleEntry(entry);
  }
}

export function validateRegistryBundleEntry(entry: RegistryBundleEntry): void {
  if (entry.action.trim() === "") throw new Error("driver action must not be empty");
  if (!validateArtifactHash(entry.wasm_hash)) throw new Error(`invalid wasm hash ${entry.wasm_hash}`);
  if (Boolean(entry.manifest_path) !== Boolean(entry.manifest_hash)) {
    throw new Error(`registry bundle entry ${entry.action}@${entry.version} has incomplete manifest metadata`);
  }
  if (entry.manifest_path) validateRelativePath(entry.manifest_path);
  if (entry.manifest_hash && !validateArtifactHash(entry.manifest_hash)) {
    throw new Error(`invalid manifest hash ${entry.manifest_hash}`);
  }
  if (Boolean(entry.driver_path) !== Boolean(entry.driver_hash)) {
    throw new Error(`registry bundle entry ${entry.action}@${entry.version} has incomplete driver metadata`);
  }
  if (entry.driver_path) validateRelativePath(entry.driver_path);
  if (entry.driver_hash) {
    if (!validateArtifactHash(entry.driver_hash)) throw new Error(`invalid driver hash ${entry.driver_hash}`);
    if (entry.driver_hash !== entry.wasm_hash) {
      throw new Error(`driver hash does not match wasm hash for ${entry.action}@${entry.version}`);
    }
  }
}

export function validateArtifactHash(value: string): boolean {
  return /^blake3:[0-9a-f]{64}$/.test(value);
}

export function receiptBodyHash(bytes: Uint8Array): string {
  return artifactHash(bytes);
}

export function artifactHash(bytes: Uint8Array): string {
  return `${DRIVER_HASH_PREFIX}${Buffer.from(blake3(bytes)).toString("hex")}`;
}

export function pactCanonicalSigningBytes(pact: RivunPact): Uint8Array {
  validatePactShape(pact);
  const payload = {
    pact_id: pact.pact_id,
    actor: pact.actor,
    target: pact.target,
    intent: pact.intent,
    object: normalizeJsonValue(pact.object ?? null),
    terms: normalizeJsonValue(pact.terms ?? null),
    consent: normalizeJsonValue(pact.consent ?? null),
    proof: normalizeJsonValue(pact.proof ?? null),
    created_at_micros: pact.created_at_micros,
    expires_at_micros: pact.expires_at_micros ?? null,
  };
  return Buffer.from(JSON.stringify(payload), "utf8");
}

export function pactHash(pact: RivunPact): string {
  return artifactHash(pactCanonicalSigningBytes(pact));
}

export function validatePactShape(pact: RivunPact): void {
  if (pact.schema_version !== PACT_SCHEMA_VERSION) {
    throw new Error(`unsupported PACT schema version ${String(pact.schema_version)}`);
  }
  validateUuid(pact.pact_id, "pact_id");
  for (const field of ["actor", "target", "intent"] as const) {
    if (typeof pact[field] !== "string" || pact[field].trim() === "") {
      throw new Error(`PACT ${field} must be a non-empty string`);
    }
  }
  if (!Number.isSafeInteger(pact.created_at_micros) || pact.created_at_micros < 0) {
    throw new Error("PACT created_at_micros must be a non-negative integer");
  }
  if (
    pact.expires_at_micros !== undefined &&
    pact.expires_at_micros !== null &&
    (!Number.isSafeInteger(pact.expires_at_micros) || pact.expires_at_micros <= pact.created_at_micros)
  ) {
    throw new Error("PACT expires_at_micros must be greater than created_at_micros");
  }
  if (pact.hash !== undefined && !validateArtifactHash(pact.hash)) {
    throw new Error(`invalid PACT hash ${pact.hash}`);
  }
}

export async function verifyPact(pact: RivunPact, nowMicros?: number): Promise<boolean> {
  validatePactShape(pact);
  if (pact.status === "revoked") return false;
  if (pact.expires_at_micros !== undefined && pact.expires_at_micros !== null && nowMicros !== undefined) {
    if (nowMicros > pact.expires_at_micros) return false;
  }
  if (!pact.hash || pact.hash !== pactHash(pact)) return false;
  if (!pact.signature || !pact.actor_public_key) return false;
  return verifyEd25519Signature(
    rivunDomainMessage(PACT_SIGNATURE_DOMAIN, pactCanonicalSigningBytes(pact)),
    pact.signature,
    pact.actor_public_key,
  );
}

export async function verifyPactBundle(bundle: RivunPactBundle, nowMicros?: number): Promise<boolean> {
  if (bundle.schema_version !== PACT_SCHEMA_VERSION) {
    throw new Error(`unsupported PACT bundle schema version ${String(bundle.schema_version)}`);
  }
  if (Array.isArray(bundle.revocations) && bundle.revocations.length > 0) return false;
  return verifyPact(bundle.pact, nowMicros);
}

export function rivunDomainMessage(domain: Uint8Array | string, message: Uint8Array): Uint8Array {
  const domainBytes = typeof domain === "string" ? Buffer.from(domain, "utf8") : Buffer.from(domain);
  return Buffer.concat([domainBytes, Buffer.from([0]), Buffer.from(message)]);
}

export function receiptSigningMessage(receipt: ReceiptSample | Record<string, unknown>): Uint8Array {
  const record = receipt as Record<string, unknown>;
  const signerPublicKey = requiredString(record, "signer_public_key");
  const signerNodeId = optionalString(record, "signer_node_id") ?? optionalString(record, "node_id");
  if (!signerNodeId) throw new Error("receipt signer_node_id or node_id is required");

  const unsignedReceipt =
    typeof record.receipt === "object" && record.receipt !== null
      ? record.receipt
      : Object.fromEntries(
          Object.entries(record).filter(([key]) => key !== "signature" && key !== "signer_public_key"),
        );
  const payload = {
    receipt: unsignedReceipt,
    signer_node_id: signerNodeId,
    signer_public_key: signerPublicKey,
  };
  return Buffer.concat([Buffer.from(RECEIPT_SIGNATURE_DOMAIN, "utf8"), Buffer.from(JSON.stringify(payload), "utf8")]);
}

export function validateReceiptShape(receipt: ReceiptSample | Record<string, unknown>): void {
  const record = receipt as Record<string, unknown>;
  if (record.schema_version !== RECEIPT_SCHEMA_VERSION) {
    throw new Error(`unsupported receipt schema version ${String(record.schema_version)}`);
  }
  for (const field of [
    "receipt_id",
    "node_id",
    "frame_id",
    "subject",
    "content_type",
    "body_hash",
    "policy_decision",
    "outcome",
    "signer_public_key",
    "signature",
  ]) {
    requiredString(record, field);
  }
  validateUuid(requiredString(record, "receipt_id"), "receipt_id");
  validateUuid(requiredString(record, "node_id"), "node_id");
  validateUuid(requiredString(record, "frame_id"), "frame_id");
  const bodyHash = requiredString(record, "body_hash");
  if (!validateArtifactHash(bodyHash)) throw new Error(`invalid receipt body hash ${bodyHash}`);
  const started = requiredNumber(record, "started_at_unix_micros");
  const finished = requiredNumber(record, "finished_at_unix_micros");
  if (finished < started) throw new Error("receipt finished_at_unix_micros is before started_at_unix_micros");
}

export function validateReceiptResponseShape(response: ReceiptReplicationResponseBody | Record<string, unknown>): void {
  const record = response as Record<string, unknown>;
  if (record.schema_version !== RECEIPT_REPLICATION_SCHEMA_VERSION) {
    throw new Error(`unsupported receipt replication schema version ${String(record.schema_version)}`);
  }
  validateUuid(requiredString(record, "request_id"), "request_id");
  if (typeof record.truncated !== "boolean") throw new Error("receipt response truncated must be a boolean");
  if (!Array.isArray(record.receipts)) throw new Error("receipt response receipts must be a list");
  for (const receipt of record.receipts) {
    if (typeof receipt !== "object" || receipt === null) throw new Error("receipt response entries must be objects");
    validateReceiptShape(receipt as Record<string, unknown>);
  }
}

export async function verifyEd25519Signature(
  message: Uint8Array,
  signatureBase64: string,
  publicKeyBase64: string,
): Promise<boolean> {
  return ed25519.verify(decodeBase64NoPad(signatureBase64), message, decodeBase64NoPad(publicKeyBase64));
}

export function signatureVerificationPlaceholder(kind: string): SignatureVerificationStatus {
  return {
    supported: false,
    reason: `${kind} signatures are Ed25519 signatures over Rivun domain-separated payloads. Build the exact canonical message and call verifyEd25519Signature(), or use rivun-cli/Rust for canonical registry verification.`,
  };
}

function validateRelativePath(path: string): void {
  if (path.length === 0 || path.startsWith("/") || path.startsWith("\\")) {
    throw new Error(`bundle path ${path} is not a safe relative path`);
  }
  const parts = path.replaceAll("\\", "/").split("/");
  if (parts.some((part) => part === "" || part === "." || part === "..")) {
    throw new Error(`bundle path ${path} is not a safe relative path`);
  }
}

function normalizeJsonValue(value: unknown): unknown {
  if (Array.isArray(value)) return value.map((item) => normalizeJsonValue(item));
  if (value !== null && typeof value === "object") {
    return Object.fromEntries(
      Object.entries(value as Record<string, unknown>)
        .sort(([left], [right]) => left.localeCompare(right))
        .map(([key, nested]) => [key, normalizeJsonValue(nested)]),
    );
  }
  return value;
}

function decodeBase64NoPad(value: string): Uint8Array {
  const padding = (4 - (value.length % 4)) % 4;
  return Buffer.from(`${value}${"=".repeat(padding)}`, "base64");
}

function requiredString(data: Record<string, unknown>, key: string): string {
  const value = data[key];
  if (typeof value !== "string" || value.length === 0) throw new Error(`${key} is required`);
  return value;
}

function optionalString(data: Record<string, unknown>, key: string): string | undefined {
  const value = data[key];
  return typeof value === "string" && value.length > 0 ? value : undefined;
}

function requiredNumber(data: Record<string, unknown>, key: string): number {
  const value = data[key];
  if (typeof value !== "number") throw new Error(`${key} is required`);
  return value;
}

function validateUuid(value: string, field: string): void {
  if (!/^[0-9a-f]{8}-[0-9a-f]{4}-[1-5][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i.test(value)) {
    throw new Error(`${field} is not a valid UUID`);
  }
}
