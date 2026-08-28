import { type Socket } from "node:dgram";
export declare const MAGIC = "ZENV";
export declare const VERSION = 1;
export declare const HEADER_LEN = 74;
export declare const MAX_SUBJECT_LEN = 512;
export declare const MAX_CONTENT_TYPE_LEN = 128;
export declare const MAX_METADATA_LEN: number;
export declare const MAX_BODY_LEN: number;
export declare const DEFAULT_CONTENT_TYPE = "application/octet-stream";
export declare const REGISTRY_INDEX_CONTENT_TYPE = "application/rivun-registry-index+json";
export declare const REGISTRY_BUNDLE_MANIFEST_CONTENT_TYPE = "application/rivun-registry-bundle-manifest+json";
export declare const REGISTRY_INDEX_REQUEST_SUBJECT = "rivun.registry.index.request";
export declare const REGISTRY_INDEX_RESPONSE_SUBJECT = "rivun.registry.index.response";
export declare const REGISTRY_BUNDLE_MANIFEST_REQUEST_SUBJECT = "rivun.registry.bundle.manifest.request";
export declare const REGISTRY_BUNDLE_MANIFEST_RESPONSE_SUBJECT = "rivun.registry.bundle.manifest.response";
export declare const RivunMessageKind: {
    readonly data: 1;
    readonly event: 2;
    readonly command: 3;
    readonly query: 4;
    readonly response: 5;
    readonly streamChunk: 6;
    readonly action: 7;
    readonly control: 8;
};
export type RivunMessageKindValue = (typeof RivunMessageKind)[keyof typeof RivunMessageKind];
export type RivunEnvelopeOptions = {
    kind: RivunMessageKindValue;
    subject: string;
    contentType: string;
    body?: Uint8Array | string;
    metadata?: Uint8Array | string;
    id?: string;
    correlationId?: string | null;
    causationId?: string | null;
};
export declare class RivunEnvelope {
    kind: RivunMessageKindValue;
    subject: string;
    contentType: string;
    body: Uint8Array;
    metadata: Uint8Array;
    id: string;
    correlationId: string | null;
    causationId: string | null;
    constructor(options: RivunEnvelopeOptions);
    encode(): Uint8Array;
    static decode(input: Uint8Array): RivunEnvelope;
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
    toEnvelope(): RivunEnvelope;
    encode(): Uint8Array;
    jsonBody(): unknown;
    static decode(input: Uint8Array): ControlFrame;
}
export type UdpTarget = {
    host: string;
    port: number;
};
export declare class RivunUdpClient {
    #private;
    constructor(socket?: Socket);
    bind(port?: number, host?: string): Promise<UdpTarget>;
    sendEnvelope(envelope: RivunEnvelope, target: UdpTarget): Promise<number>;
    sendControl(frame: ControlFrame, target: UdpTarget): Promise<number>;
    recvEnvelope(timeoutMs?: number): Promise<RivunEnvelope>;
    requestControl(frame: ControlFrame, target: UdpTarget, timeoutMs?: number): Promise<ControlFrame>;
    close(): void;
}
//# sourceMappingURL=protocol.d.ts.map