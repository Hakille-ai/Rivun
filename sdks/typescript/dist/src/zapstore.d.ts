import { ControlFrame } from "./protocol.ts";
export declare const REGISTRY_INDEX_SYNC_SCHEMA_VERSION = 1;
export declare const REGISTRY_BUNDLE_SCHEMA_VERSION = 1;
export declare const REGISTRY_INSTALL_PLAN_SCHEMA_VERSION = 1;
export declare const DRIVER_ABI_VERSION = 1;
export declare const DRIVER_HASH_PREFIX = "blake3:";
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
export declare class ZapStoreClient {
    registryIndexRequest(requireSignature?: boolean): ControlFrame;
    registryBundleManifestRequest(options?: {
        requirePublication?: boolean;
        requireDrivers?: boolean;
    }): ControlFrame;
}
export declare function registryIndexRequestFrame(requireSignature?: boolean): ControlFrame;
export declare function registryBundleManifestRequestFrame(options?: {
    requirePublication?: boolean;
    requireDrivers?: boolean;
}): ControlFrame;
export declare function validateRegistryBundleManifestResponse(response: RegistryBundleManifestResponse, request: RegistryBundleManifestRequest): void;
export declare function validateRegistryBundleManifest(manifest: RegistryBundleManifest): void;
export declare function validateRegistryBundleEntry(entry: RegistryBundleEntry): void;
export declare function validateArtifactHash(value: string): boolean;
export declare function artifactHash(bytes: Uint8Array): string;
export declare function verifyEd25519Signature(message: Uint8Array, signatureBase64: string, publicKeyBase64: string): Promise<boolean>;
export declare function signatureVerificationPlaceholder(kind: string): SignatureVerificationStatus;
//# sourceMappingURL=zapstore.d.ts.map