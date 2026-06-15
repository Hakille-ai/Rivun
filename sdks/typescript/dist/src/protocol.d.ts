import { type Socket } from "node:dgram";
export declare const MAGIC = "ZENV";
export declare const VERSION = 1;
export declare const HEADER_LEN = 74;
export declare const MAX_SUBJECT_LEN = 512;
export declare const MAX_CONTENT_TYPE_LEN = 128;
export declare const MAX_METADATA_LEN: number;
export declare const MAX_BODY_LEN: number;
export declare const DEFAULT_CONTENT_TYPE = "application/octet-stream";
export declare const REGISTRY_INDEX_CONTENT_TYPE = "application/zap-registry-index+json";
export declare const REGISTRY_BUNDLE_MANIFEST_CONTENT_TYPE = "application/zap-registry-bundle-manifest+json";
export declare const REGISTRY_INDEX_REQUEST_SUBJECT = "zap.registry.index.request";
export declare const REGISTRY_INDEX_RESPONSE_SUBJECT = "zap.registry.index.response";
export declare const REGISTRY_BUNDLE_MANIFEST_REQUEST_SUBJECT = "zap.registry.bundle.manifest.request";
export declare const REGISTRY_BUNDLE_MANIFEST_RESPONSE_SUBJECT = "zap.registry.bundle.manifest.response";
export declare const ZapMessageKind: {
    readonly data: 1;
    readonly event: 2;
    readonly command: 3;
    readonly query: 4;
    readonly response: 5;
    readonly streamChunk: 6;
    readonly action: 7;
    readonly control: 8;
};
export type ZapMessageKindValue = (typeof ZapMessageKind)[keyof typeof ZapMessageKind];
export type ZapEnvelopeOptions = {
    kind: ZapMessageKindValue;
    subject: string;
    contentType: string;
    body?: Uint8Array | string;
    metadata?: Uint8Array | string;
    id?: string;
    correlationId?: string | null;
    causationId?: string | null;
};
export declare class ZapEnvelope {
    kind: ZapMessageKindValue;
    subject: string;
    contentType: string;
    body: Uint8Array;
    metadata: Uint8Array;
    id: string;
    correlationId: string | null;
    causationId: string | null;
    constructor(options: ZapEnvelopeOptions);
    encode(): Uint8Array;
    static decode(input: Uint8Array): ZapEnvelope;
}
export type ControlFrameOptions = {
    subject: string;
    contentType: string;
    body: Uint8Array | string;
    metadata?: Uint8Array | string;
    id?: string;
    correlationId?: string | null;
    causationId?: string | null;
};
export declare class ControlFrame {
    subject: string;
    contentType: string;
    body: Uint8Array;
    metadata: Uint8Array;
    id: string;
    correlationId: string | null;
    causationId: string | null;
    constructor(options: ControlFrameOptions);
    static json(subject: string, contentType: string, payload: unknown): ControlFrame;
    toEnvelope(): ZapEnvelope;
    encode(): Uint8Array;
    jsonBody(): unknown;
    static decode(input: Uint8Array): ControlFrame;
}
export type UdpTarget = {
    host: string;
    port: number;
};
export declare class ZapUdpClient {
    #private;
    constructor(socket?: Socket);
    bind(port?: number, host?: string): Promise<UdpTarget>;
    sendEnvelope(envelope: ZapEnvelope, target: UdpTarget): Promise<number>;
    sendControl(frame: ControlFrame, target: UdpTarget): Promise<number>;
    recvEnvelope(timeoutMs?: number): Promise<ZapEnvelope>;
    requestControl(frame: ControlFrame, target: UdpTarget, timeoutMs?: number): Promise<ControlFrame>;
    close(): void;
}
//# sourceMappingURL=protocol.d.ts.map