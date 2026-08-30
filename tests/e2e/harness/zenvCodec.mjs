import { formatUuid, parseUuid } from './crypto.mjs';

export const ZENV_MAGIC_NUMBER = 0x5A454E56; //  ZENV
export const ZENV_MAGIC_BYTES = Buffer.from('ZENV', 'ascii');
export const ZENV_VERSION = 1;
export const ZENV_HEADER_LEN = 74;

export const MAX_SUBJECT_LEN = 512;
export const MAX_CONTENT_TYPE_LEN = 128;
export const MAX_METADATA_LEN = 64 * 1024;
export const MAX_BODY_LEN = 16 * 1024 * 1024;

export const MessageKind = {
  Data: 1,
  Event: 2,
  Command: 3,
  Query: 4,
  Response: 5,
  StreamChunk: 6,
  Action: 7,
  Control: 8,
};

export const MessageKindName = {
  1: 'data',
  2: 'event',
  3: 'command',
  4: 'query',
  5: 'response',
  6: 'stream_chunk',
  7: 'action',
  8: 'control',
};

export class RivunEnvelope {
  constructor({
    magic = ZENV_MAGIC_NUMBER,
    version = ZENV_VERSION,
    kind = MessageKind.Data,
    id = '00000000-0000-0000-0000-000000000001',
    correlationId = '00000000-0000-0000-0000-000000000000',
    causationId = '00000000-0000-0000-0000-000000000000',
    subject = '',
    contentType = 'application/octet-stream',
    metadata = Buffer.alloc(0),
    body = Buffer.alloc(0),
  }) {
    this.magic = magic;
    this.version = version;
    this.kind = typeof kind === 'string' ? MessageKind[kind] || 1 : kind;
    this.id = id;
    this.correlationId = correlationId;
    this.causationId = causationId;
    this.subject = subject;
    this.contentType = contentType;
    this.metadata = Buffer.isBuffer(metadata)
      ? metadata
      : Buffer.from(typeof metadata === 'object' ? JSON.stringify(metadata) : String(metadata), 'utf8');
    this.body = Buffer.isBuffer(body)
      ? body
      : Buffer.from(typeof body === 'object' ? JSON.stringify(body) : String(body), 'utf8');
  }

  encode() {
    const subjectBuf = Buffer.from(this.subject, 'utf8');
    if (subjectBuf.length > MAX_SUBJECT_LEN) {
      throw new Error('Subject length exceeds maximum 512 bytes: ' + subjectBuf.length);
    }
    const ctBuf = Buffer.from(this.contentType, 'ascii');
    if (ctBuf.length > MAX_CONTENT_TYPE_LEN) {
      throw new Error('Content-Type length exceeds maximum 128 bytes: ' + ctBuf.length);
    }
    if (this.metadata.length > MAX_METADATA_LEN) {
      throw new Error('Metadata length exceeds maximum 64KiB: ' + this.metadata.length);
    }
    if (this.body.length > MAX_BODY_LEN) {
      throw new Error('Body length exceeds maximum 16MiB: ' + this.body.length);
    }

    const headerBuf = Buffer.alloc(ZENV_HEADER_LEN);
    headerBuf.writeUInt32BE(this.magic, 0);
    headerBuf.writeUInt16BE(this.version, 4);
    headerBuf.writeUInt16BE(this.kind, 6);
    headerBuf.writeUInt16BE(0, 8); // reserved

    parseUuid(this.id).copy(headerBuf, 10, 0, 16);
    parseUuid(this.correlationId).copy(headerBuf, 26, 0, 16);
    parseUuid(this.causationId).copy(headerBuf, 42, 0, 16);

    headerBuf.writeUInt16BE(subjectBuf.length, 58);
    headerBuf.writeUInt16BE(ctBuf.length, 60);
    headerBuf.writeUInt32BE(this.metadata.length, 62);
    headerBuf.writeBigUInt64BE(BigInt(this.body.length), 66);

    return Buffer.concat([headerBuf, subjectBuf, ctBuf, this.metadata, this.body]);
  }

  static decode(buf) {
    if (buf.length < ZENV_HEADER_LEN) {
      throw new Error('Envelope too short: expected at least 74 bytes, got ' + buf.length);
    }
    const magic = buf.readUInt32BE(0);
    if (magic !== ZENV_MAGIC_NUMBER) {
      throw new Error('Invalid ZENV magic number: 0x' + magic.toString(16).toUpperCase());
    }
    const version = buf.readUInt16BE(4);
    if (version !== ZENV_VERSION) {
      throw new Error('Unsupported ZENV version: ' + version);
    }
    const kind = buf.readUInt16BE(6);
    if (kind < 1 || kind > 8) {
      throw new Error(`Unknown envelope kind (unsupported message kind): ${kind}`);
    }
    const reserved = buf.readUInt16BE(8);
    if (reserved !== 0) {
      throw new Error(`Reserved field must be zero, got ${reserved}`);
    }

    const id = formatUuid(buf.subarray(10, 26));
    const correlationId = formatUuid(buf.subarray(26, 42));
    const causationId = formatUuid(buf.subarray(42, 58));

    const subjectLen = buf.readUInt16BE(58);
    const ctLen = buf.readUInt16BE(60);
    const metaLen = buf.readUInt32BE(62);
    const bodyLenBig = buf.readBigUInt64BE(66);

    if (kind !== MessageKind.Data && subjectLen === 0) {
      throw new Error(`Subject is required for kind ${kind}`);
    }
    if (subjectLen > MAX_SUBJECT_LEN) {
      throw new Error('Subject length exceeds maximum 512 bytes');
    }
    if (ctLen > MAX_CONTENT_TYPE_LEN) {
      throw new Error('Content-Type length exceeds maximum 128 bytes');
    }
    if (metaLen > MAX_METADATA_LEN) {
      throw new Error('Metadata length exceeds maximum 64KiB');
    }
    if (bodyLenBig > BigInt(MAX_BODY_LEN)) {
      throw new Error('Body length exceeds maximum 16MiB');
    }

    const bodyLen = Number(bodyLenBig);
    const totalExpected = ZENV_HEADER_LEN + subjectLen + ctLen + metaLen + bodyLen;
    if (buf.length < totalExpected) {
      throw new Error('Envelope truncated: expected ' + totalExpected + ' bytes, got ' + buf.length);
    }

    let offset = ZENV_HEADER_LEN;
    const subject = buf.subarray(offset, offset + subjectLen).toString('utf8');
    offset += subjectLen;

    const contentType = buf.subarray(offset, offset + ctLen).toString('ascii');
    offset += ctLen;

    const metadata = Buffer.from(buf.subarray(offset, offset + metaLen));
    offset += metaLen;

    const body = Buffer.from(buf.subarray(offset, offset + bodyLen));

    return new RivunEnvelope({
      magic,
      version,
      kind,
      id,
      correlationId,
      causationId,
      subject,
      contentType,
      metadata,
      body,
    });
  }
}
