import { ControlFrame } from "./protocol.ts";
export declare const REGISTRY_INDEX_SYNC_SCHEMA_VERSION = 1;
export declare const REGISTRY_BUNDLE_SCHEMA_VERSION = 1;
export declare const REGISTRY_INSTALL_PLAN_SCHEMA_VERSION = 1;
export declare const DRIVER_ABI_VERSION = 1;
export declare const DRIVER_HASH_PREFIX = "blake3:";
export declare const RECEIPT_SCHEMA_VERSION = 1;
export declare const RECEIPT_REPLICATION_SCHEMA_VERSION = 1;
export declare const RECEIPT_REPLICATION_CONTENT_TYPE = "application/zap-receipts+json";
export declare const RECEIPT_REPLICATION_REQUEST_SUBJECT = "zap.receipts.request";
export declare const RECEIPT_REPLICATION_RESPONSE_SUBJECT = "zap.receipts.response";
export declare const RECEIPT_SIGNATURE_DOMAIN = "ZAP-ACTION-RECEIPT-v1";
export declare const AGENT_CONTENT_TYPE = "application/zap-agent+json";
export declare const AGENT_INTENT_SUBJECT = "zap.agent.intent";
export declare const AGENT_STATUS_SUBJECT = "zap.agent.status";
export declare const AGENT_RESULT_SUBJECT = "zap.agent.result";
export declare const PACT_SCHEMA_VERSION = 1;
export declare const PACT_CONTENT_TYPE = "application/zap-pact+json";
export declare const PACT_RECORD_SUBJECT = "zap.pact.record";
export declare const PACT_VERIFY_SUBJECT = "zap.pact.verify";
export declare const PACT_REVOKE_SUBJECT = "zap.pact.revoke";
export declare const PACT_BUNDLE_SUBJECT = "zap.pact.bundle";
export declare const PACT_SIGNATURE_DOMAIN = "ZAP-PACT-v1";
export type ZapPactStatus = "draft" | "active" | "expired" | "revoked" | "invalid";
export type ZapPact = {
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
    status?: ZapPactStatus;
};
export type ZapPactBundle = {
    schema_version: number;
    pact: ZapPact;
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
export declare function receiptBodyHash(bytes: Uint8Array): string;
export declare function artifactHash(bytes: Uint8Array): string;
export declare function pactCanonicalSigningBytes(pact: ZapPact): Uint8Array;
export declare function pactHash(pact: ZapPact): string;
export declare function validatePactShape(pact: ZapPact): void;
export declare function verifyPact(pact: ZapPact, nowMicros?: number): Promise<boolean>;
export declare function verifyPactBundle(bundle: ZapPactBundle, nowMicros?: number): Promise<boolean>;
export declare function zapDomainMessage(domain: Uint8Array | string, message: Uint8Array): Uint8Array;
export declare function receiptSigningMessage(receipt: ReceiptSample | Record<string, unknown>): Uint8Array;
export declare function validateReceiptShape(receipt: ReceiptSample | Record<string, unknown>): void;
export declare function validateReceiptResponseShape(response: ReceiptReplicationResponseBody | Record<string, unknown>): void;
export declare function verifyEd25519Signature(message: Uint8Array, signatureBase64: string, publicKeyBase64: string): Promise<boolean>;
export declare function signatureVerificationPlaceholder(kind: string): SignatureVerificationStatus;
//# sourceMappingURL=zapstore.d.ts.map