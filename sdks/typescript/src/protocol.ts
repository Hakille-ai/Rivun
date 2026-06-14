import { randomUUID } from "node:crypto";

export const MAGIC = "ZENV";
export const VERSION = 1;
export const HEADER_LEN = 74;
export const MAX_SUBJECT_LEN = 512;
export const MAX_CONTENT_TYPE_LEN = 128;
export const MAX_METADATA_LEN = 64 * 1024;
export const MAX_BODY_LEN = 16 * 1024 * 1024;
export const DEFAULT_CONTENT_TYPE = "application/octet-stream";

export const REGISTRY_INDEX_CONTENT_TYPE = "application/zap-registry-index+json";
export const REGISTRY_BUNDLE_MANIFEST_CONTENT_TYPE = "application/zap-registry-bundle-manifest+json";
export const REGISTRY_INDEX_REQUEST_SUBJECT = "zap.registry.index.request";
export const REGISTRY_INDEX_RESPONSE_SUBJECT = "zap.registry.index.response";
export const REGISTRY_BUNDLE_MANIFEST_REQUEST_SUBJECT = "zap.registry.bundle.manifest.request";
export const REGISTRY_BUNDLE_MANIFEST_RESPONSE_SUBJECT = "zap.registry.bundle.manifest.response";

export const ZapMessageKind = {
  data: 1,
  event: 2,
  command: 3,
  query: 4,
  response: 5,
  streamChunk: 6,
  action: 7,
  control: 8,
} as const;

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

export class ZapEnvelope {
  kind: ZapMessageKindValue;
  subject: string;
  contentType: string;
  body: Uint8Array;
  metadata: Uint8Array;
  id: string;
  correlationId: string | null;
  causationId: string | null;

  constructor(options: ZapEnvelopeOptions) {
    this.kind = options.kind;
    this.subject = options.subject;
    this.contentType = options.contentType;
    this.body = toBytes(options.body ?? new Uint8Array());
    this.metadata = toBytes(options.metadata ?? new Uint8Array());
    this.id = options.id ?? randomUUID();
    this.correlationId = options.correlationId ?? null;
    this.causationId = options.causationId ?? null;
    validateParts(this.kind, this.subject, this.contentType, this.metadata.byteLength, this.body.byteLength);
  }

  encode(): Uint8Array {
    const subject = utf8(this.subject);
    const contentType = utf8(this.contentType);
    const header = Buffer.alloc(HEADER_LEN);
    header.write(MAGIC, 0, "ascii");
    header.writeUInt16BE(VERSION, 4);
    header.writeUInt16BE(this.kind, 6);
    header.writeUInt16BE(0, 8);
    uuidToBytes(this.id).copy(header, 10);
    uuidToBytes(this.correlationId).copy(header, 26);
    uuidToBytes(this.causationId).copy(header, 42);
    header.writeUInt16BE(subject.byteLength, 58);
    header.writeUInt16BE(contentType.byteLength, 60);
    header.writeUInt32BE(this.metadata.byteLength, 62);
    header.writeBigUInt64BE(BigInt(this.body.byteLength), 66);
    return Buffer.concat([header, subject, contentType, Buffer.from(this.metadata), Buffer.from(this.body)]);
  }

  static decode(input: Uint8Array): ZapEnvelope {
    const bytes = Buffer.from(input);
    if (bytes.byteLength < HEADER_LEN) {
      throw new Error(`envelope too short: expected at least ${HEADER_LEN}, got ${bytes.byteLength}`);
    }
    if (bytes.subarray(0, 4).toString("ascii") !== MAGIC) {
      throw new Error("invalid envelope magic");
    }
    const version = bytes.readUInt16BE(4);
    if (version !== VERSION) {
      throw new Error(`unsupported envelope version ${version}`);
    }
    const kind = bytes.readUInt16BE(6) as ZapMessageKindValue;
    if (!isKnownKind(kind)) {
      throw new Error(`unknown envelope kind ${kind}`);
    }
    const reserved = bytes.readUInt16BE(8);
    if (reserved !== 0) {
      throw new Error(`reserved envelope field must be zero, got ${reserved}`);
    }
    const id = bytesToUuid(bytes.subarray(10, 26));
    const correlationId = optionalUuid(bytes.subarray(26, 42));
    const causationId = optionalUuid(bytes.subarray(42, 58));
    const subjectLen = bytes.readUInt16BE(58);
    const contentTypeLen = bytes.readUInt16BE(60);
    const metadataLen = bytes.readUInt32BE(62);
    const bodyLen = Number(bytes.readBigUInt64BE(66));
    validateLengths(kind, subjectLen, contentTypeLen, metadataLen, bodyLen);
    const expected = HEADER_LEN + subjectLen + contentTypeLen + metadataLen + bodyLen;
    if (bytes.byteLength !== expected) {
      throw new Error(`envelope length mismatch: expected ${expected}, got ${bytes.byteLength}`);
    }
    const subjectStart = HEADER_LEN;
    const contentTypeStart = subjectStart + subjectLen;
    const metadataStart = contentTypeStart + contentTypeLen;
    const bodyStart = metadataStart + metadataLen;
    return new ZapEnvelope({
      kind,
      id,
      correlationId,
      causationId,
      subject: bytes.subarray(subjectStart, contentTypeStart).toString("utf8"),
      contentType: bytes.subarray(contentTypeStart, metadataStart).toString("utf8"),
      metadata: bytes.subarray(metadataStart, bodyStart),
      body: bytes.subarray(bodyStart),
    });
  }
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

export class ControlFrame {
  subject: string;
  contentType: string;
  body: Uint8Array;
  metadata: Uint8Array;
  id: string;
  correlationId: string | null;
  causationId: string | null;

  constructor(options: ControlFrameOptions) {
    this.subject = options.subject;
    this.contentType = options.contentType;
    this.body = toBytes(options.body);
    this.metadata = toBytes(options.metadata ?? new Uint8Array());
    this.id = options.id ?? randomUUID();
    this.correlationId = options.correlationId ?? null;
    this.causationId = options.causationId ?? null;
  }

  static json(subject: string, contentType: string, payload: unknown): ControlFrame {
    return new ControlFrame({
      subject,
      contentType,
      body: JSON.stringify(payload),
    });
  }

  toEnvelope(): ZapEnvelope {
    return new ZapEnvelope({
      kind: ZapMessageKind.control,
      subject: this.subject,
      contentType: this.contentType,
      body: this.body,
      metadata: this.metadata,
      id: this.id,
      correlationId: this.correlationId,
      causationId: this.causationId,
    });
  }

  encode(): Uint8Array {
    return this.toEnvelope().encode();
  }

  jsonBody(): unknown {
    return JSON.parse(Buffer.from(this.body).toString("utf8"));
  }

  static decode(input: Uint8Array): ControlFrame {
    const envelope = ZapEnvelope.decode(input);
    if (envelope.kind !== ZapMessageKind.control) {
      throw new Error(`expected control envelope, got kind ${envelope.kind}`);
    }
    return new ControlFrame({
      subject: envelope.subject,
      contentType: envelope.contentType,
      body: envelope.body,
      metadata: envelope.metadata,
      id: envelope.id,
      correlationId: envelope.correlationId,
      causationId: envelope.causationId,
    });
  }
}

function validateParts(
  kind: ZapMessageKindValue,
  subject: string,
  contentType: string,
  metadataLen: number,
  bodyLen: number,
): void {
  validateLengths(kind, utf8(subject).byteLength, utf8(contentType).byteLength, metadataLen, bodyLen);
}

function validateLengths(
  kind: ZapMessageKindValue,
  subjectLen: number,
  contentTypeLen: number,
  metadataLen: number,
  bodyLen: number,
): void {
  if (subjectLen > MAX_SUBJECT_LEN) throw new Error(`subject length exceeds maximum ${MAX_SUBJECT_LEN}`);
  if (contentTypeLen > MAX_CONTENT_TYPE_LEN) {
    throw new Error(`content_type length exceeds maximum ${MAX_CONTENT_TYPE_LEN}`);
  }
  if (metadataLen > MAX_METADATA_LEN) throw new Error(`metadata length exceeds maximum ${MAX_METADATA_LEN}`);
  if (bodyLen > MAX_BODY_LEN) throw new Error(`body length exceeds maximum ${MAX_BODY_LEN}`);
  if (kind !== ZapMessageKind.data && subjectLen === 0) throw new Error(`subject is required for kind ${kind}`);
}

function isKnownKind(kind: number): kind is ZapMessageKindValue {
  return kind >= ZapMessageKind.data && kind <= ZapMessageKind.control;
}

function utf8(value: string): Buffer {
  return Buffer.from(value, "utf8");
}

function toBytes(value: Uint8Array | string): Uint8Array {
  return typeof value === "string" ? utf8(value) : value;
}

function uuidToBytes(value: string | null): Buffer {
  if (value === null) return Buffer.alloc(16);
  return Buffer.from(value.replaceAll("-", ""), "hex");
}

function bytesToUuid(bytes: Uint8Array): string {
  const hex = Buffer.from(bytes).toString("hex");
  return `${hex.slice(0, 8)}-${hex.slice(8, 12)}-${hex.slice(12, 16)}-${hex.slice(16, 20)}-${hex.slice(20)}`;
}

function optionalUuid(bytes: Uint8Array): string | null {
  if (Buffer.from(bytes).equals(Buffer.alloc(16))) return null;
  return bytesToUuid(bytes);
}
