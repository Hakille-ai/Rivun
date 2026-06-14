import {
  ControlFrame,
  REGISTRY_BUNDLE_MANIFEST_CONTENT_TYPE,
  REGISTRY_BUNDLE_MANIFEST_REQUEST_SUBJECT,
  REGISTRY_INDEX_CONTENT_TYPE,
  REGISTRY_INDEX_REQUEST_SUBJECT,
} from "./protocol.ts";

export const REGISTRY_INDEX_SYNC_SCHEMA_VERSION = 1;
export const REGISTRY_BUNDLE_SCHEMA_VERSION = 1;
export const REGISTRY_INSTALL_PLAN_SCHEMA_VERSION = 1;
export const DRIVER_ABI_VERSION = 1;
export const DRIVER_HASH_PREFIX = "blake3:";

export type DriverRegistryStatus = "active" | "deprecated" | "revoked";

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
};

export type RegistryInstallPlanEntry = {
  action: string;
  requirement: string;
  requested_abi_version?: number;
  selected_version: string;
  name: string;
  abi_version: number;
  wasm_hash: string;
  manifest_path?: string;
  author_node_id: string;
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

export class ZapStoreClient {
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

export function artifactHash(_bytes: Uint8Array): never {
  throw new Error(
    "canonical ZAP artifact hashes use BLAKE3; Node standard crypto does not expose BLAKE3. Use zap-cli, the Rust SDK, or a caller-provided BLAKE3 backend.",
  );
}

export function signatureVerificationPlaceholder(kind: string): SignatureVerificationStatus {
  return {
    supported: false,
    reason: `${kind} signatures are Ed25519 signatures over ZAP domain-separated payloads. This dependency-free TypeScript SDK does not verify them yet; use zap-cli or the Rust SDK.`,
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
