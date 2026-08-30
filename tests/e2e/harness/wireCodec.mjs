import { blake3, blake3Hex } from './blake3.mjs';
import {
  Keypair,
  PublicKey,
  POA_DIGEST_DOMAIN,
  POA_SIGNATURE_DOMAIN,
  formatUuid,
  parseUuid,
  signatureHint,
  domainMessage,
} from './crypto.mjs';

export const MAGIC_NUMBER = 0x5A41505F; //  ZAP_
export const MAGIC_BYTES = Buffer.from('ZAP_', 'ascii');
export const VERSION = 1;
export const HEADER_LEN = 64;
export const SIGNING_PREFIX_LEN = 56;
export const MAX_PAYLOAD_LEN = 16 * 1024 * 1024; // 16 MiB

export const AUTH_TRAILER_MAGIC = Buffer.from('ZSIG', 'ascii');
export const AUTH_TRAILER_LEN = 72;
export const ED25519_SIGNATURE_LEN = 64;

export const POA_TRAILER_MAGIC = Buffer.from('ZPOA', 'ascii');
export const POA_TRAILER_VERSION = 1;
export const POA_TRAILER_HEADER_LEN = 44;
export const POA_ATTESTATION_LEN = 80;
export const MAX_POA_ATTESTATIONS = 64;

export const Flags = {
  NONE: 0,
  ENCRYPTED: 1 << 0,          // 0x0001
  PRIORITY: 1 << 1,           // 0x0002
  REQUIRES_CONSENSUS: 1 << 2, // 0x0004
  SIGNED: 1 << 3,             // 0x0008
  BROADCAST: 1 << 4,          // 0x0010
};

export class RivunHeader {
  constructor({
    magic = MAGIC_NUMBER,
    version = VERSION,
    flags = Flags.NONE,
    sourceNode = '00000000-0000-0000-0000-000000000000',
    targetNode = '00000000-0000-0000-0000-000000000000',
    timestampMicros = BigInt(Date.now()) * 1000n,
    payloadLen = 0,
    rivunSign = Buffer.alloc(8),
  }) {
    this.magic = magic;
    this.version = version;
    this.flags = flags;
    this.sourceNode = sourceNode;
    this.targetNode = targetNode;
    this.timestampMicros = typeof timestampMicros === 'bigint' ? timestampMicros : BigInt(timestampMicros);
    this.payloadLen = payloadLen;
    this.rivunSign = Buffer.isBuffer(rivunSign) ? rivunSign : Buffer.from(rivunSign);
  }

  encode() {
    const buf = Buffer.alloc(HEADER_LEN);
    buf.writeUInt32BE(this.magic, 0);
    buf.writeUInt16BE(this.version, 4);
    buf.writeUInt16BE(this.flags, 6);

    const srcBuf = parseUuid(this.sourceNode);
    srcBuf.copy(buf, 8, 0, 16);

    const tgtBuf = parseUuid(this.targetNode);
    tgtBuf.copy(buf, 24, 0, 16);

    buf.writeBigUInt64BE(this.timestampMicros, 40);
    buf.writeBigUInt64BE(BigInt(this.payloadLen), 48);

    if (this.rivunSign.length === 8) {
      this.rivunSign.copy(buf, 56, 0, 8);
    }
    return buf;
  }

  static decode(buf) {
    if (buf.length < HEADER_LEN) {
      throw new Error('Header too short: expected 64 bytes, got ' + buf.length);
    }
    const magic = buf.readUInt32BE(0);
    if (magic !== MAGIC_NUMBER) {
      throw new Error('Invalid magic number: 0x' + magic.toString(16).toUpperCase());
    }
    const version = buf.readUInt16BE(4);
    if (version !== VERSION) {
      throw new Error('Unsupported version: ' + version);
    }
    const flags = buf.readUInt16BE(6);
    if ((flags & ~0x001f) !== 0) {
      throw new Error('Unknown flag bits: 0x' + flags.toString(16));
    }
    const sourceNode = formatUuid(buf.subarray(8, 24));
    const targetNode = formatUuid(buf.subarray(24, 40));
    const timestampMicros = buf.readBigUInt64BE(40);
    const payloadLenBig = buf.readBigUInt64BE(48);
    if (payloadLenBig > BigInt(MAX_PAYLOAD_LEN)) {
      throw new Error('Payload length exceeds maximum 16MiB: ' + payloadLenBig);
    }
    const payloadLen = Number(payloadLenBig);
    const rivunSign = Buffer.from(buf.subarray(56, 64));

    return new RivunHeader({
      magic,
      version,
      flags,
      sourceNode,
      targetNode,
      timestampMicros,
      payloadLen,
      rivunSign,
    });
  }
}

export class AuthTrailer {
  constructor(signature, algorithm = 1, signatureLen = 64) {
    this.magic = AUTH_TRAILER_MAGIC;
    this.algorithm = algorithm;
    this.signatureLen = signatureLen;
    this.signature = Buffer.isBuffer(signature) ? signature : Buffer.from(signature);
  }

  encode() {
    const buf = Buffer.alloc(AUTH_TRAILER_LEN);
    this.magic.copy(buf, 0, 0, 4);
    buf.writeUInt16BE(this.algorithm, 4);
    buf.writeUInt16BE(this.signatureLen, 6);
    this.signature.copy(buf, 8, 0, 64);
    return buf;
  }

  static decode(buf) {
    if (buf.length < AUTH_TRAILER_LEN) {
      throw new Error('Auth trailer too short: expected 72 bytes, got ' + buf.length);
    }
    const magic = buf.subarray(0, 4);
    if (!magic.equals(AUTH_TRAILER_MAGIC)) {
      throw new Error('Invalid auth trailer magic');
    }
    const algorithm = buf.readUInt16BE(4);
    if (algorithm !== 1) {
      throw new Error('Unsupported signature algorithm: ' + algorithm);
    }
    const signatureLen = buf.readUInt16BE(6);
    if (signatureLen !== 64) {
      throw new Error('Invalid signature length: ' + signatureLen);
    }
    const signature = Buffer.from(buf.subarray(8, 72));
    return new AuthTrailer(signature, algorithm, signatureLen);
  }
}

export class PoaAttestation {
  constructor(validatorNode, signature) {
    this.validatorNode = validatorNode;
    this.signature = Buffer.isBuffer(signature) ? signature : Buffer.from(signature);
  }

  encode() {
    const buf = Buffer.alloc(POA_ATTESTATION_LEN);
    parseUuid(this.validatorNode).copy(buf, 0, 0, 16);
    this.signature.copy(buf, 16, 0, 64);
    return buf;
  }

  static decode(buf) {
    if (buf.length < POA_ATTESTATION_LEN) {
      throw new Error('PoA attestation too short: expected 80 bytes, got ' + buf.length);
    }
    const validatorNode = formatUuid(buf.subarray(0, 16));
    const signature = Buffer.from(buf.subarray(16, 80));
    return new PoaAttestation(validatorNode, signature);
  }
}

export class PoaTrailer {
  constructor(threshold, frameDigest, attestations = []) {
    this.magic = POA_TRAILER_MAGIC;
    this.version = POA_TRAILER_VERSION;
    this.threshold = threshold;
    this.attestations = attestations;
    this.frameDigest = Buffer.isBuffer(frameDigest) ? frameDigest : Buffer.from(frameDigest);
  }

  encode() {
    const headerBuf = Buffer.alloc(POA_TRAILER_HEADER_LEN);
    this.magic.copy(headerBuf, 0, 0, 4);
    headerBuf.writeUInt16BE(this.version, 4);
    headerBuf.writeUInt16BE(this.threshold, 6);
    headerBuf.writeUInt16BE(this.attestations.length, 8);
    headerBuf.writeUInt16BE(0, 10); // reserved
    this.frameDigest.copy(headerBuf, 12, 0, 32);

    const attBufs = this.attestations.map((a) => a.encode());
    return Buffer.concat([headerBuf, ...attBufs]);
  }

  static decode(buf) {
    if (buf.length < POA_TRAILER_HEADER_LEN) {
      throw new Error('PoA trailer header too short: expected at least 44 bytes, got ' + buf.length);
    }
    const magic = buf.subarray(0, 4);
    if (!magic.equals(POA_TRAILER_MAGIC)) {
      throw new Error('Invalid PoA trailer magic');
    }
    const version = buf.readUInt16BE(4);
    if (version !== POA_TRAILER_VERSION) {
      throw new Error('Unsupported PoA trailer version: ' + version);
    }
    const threshold = buf.readUInt16BE(6);
    if (threshold === 0) {
      throw new Error('Invalid PoA threshold: 0');
    }
    const count = buf.readUInt16BE(8);
    if (count > MAX_POA_ATTESTATIONS) {
      throw new Error('PoA attestation count exceeds maximum: ' + count);
    }
    const expectedLen = POA_TRAILER_HEADER_LEN + count * POA_ATTESTATION_LEN;
    if (buf.length < expectedLen) {
      throw new Error('PoA trailer length mismatch: expected ' + expectedLen + ' bytes, got ' + buf.length);
    }
    const frameDigest = Buffer.from(buf.subarray(12, 44));
    const attestations = [];
    for (let i = 0; i < count; i++) {
      const offset = POA_TRAILER_HEADER_LEN + i * POA_ATTESTATION_LEN;
      attestations.push(PoaAttestation.decode(buf.subarray(offset, offset + POA_ATTESTATION_LEN)));
    }
    return new PoaTrailer(threshold, frameDigest, attestations);
  }
}

export class RivunFrame {
  constructor(header, payload = Buffer.alloc(0), auth = null, poa = null) {
    this.header = header instanceof RivunHeader ? header : new RivunHeader(header);
    this.payload = Buffer.isBuffer(payload) ? payload : Buffer.from(payload);
    this.auth = auth;
    this.poa = poa;
    this.header.payloadLen = this.payload.length;
  }

  signingTranscript() {
    const headerBuf = this.header.encode();
    const prefix = headerBuf.subarray(0, SIGNING_PREFIX_LEN);
    return Buffer.concat([prefix, this.payload]);
  }

  encodeWithoutPoa() {
    const parts = [this.header.encode(), this.payload];
    if (this.auth) {
      parts.push(this.auth.encode());
    }
    return Buffer.concat(parts);
  }

  encode() {
    const parts = [this.header.encode(), this.payload];
    if (this.auth) {
      parts.push(this.auth.encode());
    }
    if (this.poa) {
      parts.push(this.poa.encode());
    }
    return Buffer.concat(parts);
  }

  static decode(buf) {
    if (buf.length < HEADER_LEN) {
      throw new Error('Frame too short: expected at least 64 bytes, got ' + buf.length);
    }
    const header = RivunHeader.decode(buf.subarray(0, HEADER_LEN));
    const totalPayloadLen = header.payloadLen;
    const payloadEnd = HEADER_LEN + totalPayloadLen;
    if (buf.length < payloadEnd) {
      throw new Error('Frame length mismatch: expected at least ' + payloadEnd + ' bytes, got ' + buf.length);
    }
    const payload = Buffer.from(buf.subarray(HEADER_LEN, payloadEnd));
    let offset = payloadEnd;

    let auth = null;
    if ((header.flags & Flags.SIGNED) !== 0) {
      if (buf.length < offset + AUTH_TRAILER_LEN) {
        throw new Error('Signed frame is missing an Ed25519 auth trailer');
      }
      auth = AuthTrailer.decode(buf.subarray(offset, offset + AUTH_TRAILER_LEN));
      offset += AUTH_TRAILER_LEN;
    }

    let poa = null;
    if ((header.flags & Flags.REQUIRES_CONSENSUS) !== 0 && offset < buf.length) {
      poa = PoaTrailer.decode(buf.subarray(offset));
      offset += POA_TRAILER_HEADER_LEN + poa.attestations.length * POA_ATTESTATION_LEN;
    }

    if (offset !== buf.length) {
      throw new Error('Frame length mismatch: trailing bytes remaining (' + (buf.length - offset) + ' bytes)');
    }

    return new RivunFrame(header, payload, auth, poa);
  }
}

export function signFrame(keypair, frame) {
  if (frame.header.sourceNode !== keypair.nodeId) {
    throw new Error('Frame source node ' + frame.header.sourceNode + ' does not match signing key node ' + keypair.nodeId);
  }
  const f = new RivunFrame(
    new RivunHeader({
      magic: frame.header.magic,
      version: frame.header.version,
      flags: frame.header.flags | Flags.SIGNED,
      sourceNode: frame.header.sourceNode,
      targetNode: frame.header.targetNode,
      timestampMicros: frame.header.timestampMicros,
      payloadLen: frame.payload.length,
      rivunSign: Buffer.alloc(8),
    }),
    frame.payload,
    null,
    null
  );

  const transcript = f.signingTranscript();
  const sig = keypair.sign(transcript);
  const hint = signatureHint(sig);
  f.header.rivunSign = hint;
  f.auth = new AuthTrailer(sig);
  return f;
}

export function verifyFrame(publicKey, frame) {
  if (frame.header.sourceNode !== publicKey.nodeId) {
    throw new Error('Frame source node ' + frame.header.sourceNode + ' does not match verifying key node ' + publicKey.nodeId);
  }
  if ((frame.header.flags & Flags.SIGNED) === 0 || !frame.auth) {
    throw new Error('Frame is not signed or missing auth trailer');
  }
  const hint = signatureHint(frame.auth.signature);
  if (!hint.equals(frame.header.rivunSign)) {
    throw new Error('Signature hint mismatch');
  }
  const transcript = frame.signingTranscript();
  if (!publicKey.verify(transcript, frame.auth.signature)) {
    throw new Error('Ed25519 signature verification failed');
  }
  return true;
}

export function poaFrameDigest(frame) {
  const enc = frame.encodeWithoutPoa();
  return blake3(Buffer.concat([POA_DIGEST_DOMAIN, enc]));
}

export function certifyFrame(frame, threshold, validatorKeypairs) {
  if ((frame.header.flags & Flags.SIGNED) === 0) {
    throw new Error('Frame must be signed before PoA certification');
  }
  if ((frame.header.flags & Flags.REQUIRES_CONSENSUS) === 0) {
    throw new Error('Frame must be marked REQUIRES_CONSENSUS for PoA certification');
  }
  if (validatorKeypairs.length < threshold) {
    throw new Error('PoA threshold not met: required ' + threshold + ', got ' + validatorKeypairs.length);
  }
  const digest = poaFrameDigest(frame);
  const msg = domainMessage(POA_SIGNATURE_DOMAIN, digest);

  const seen = new Set();
  const attestations = [];
  for (const val of validatorKeypairs) {
    if (seen.has(val.nodeId)) {
      throw new Error('Duplicate PoA validator: ' + val.nodeId);
    }
    seen.add(val.nodeId);
    const sig = val.sign(msg);
    attestations.push(new PoaAttestation(val.nodeId, sig));
  }

  const certified = new RivunFrame(frame.header, frame.payload, frame.auth, new PoaTrailer(threshold, digest, attestations));
  return certified;
}

export function verifyPoaCertificate(frame, validators, requiredThreshold) {
  if (!frame.poa) {
    throw new Error('Frame is missing a Proof-of-Action certificate');
  }
  if (frame.poa.threshold < requiredThreshold) {
    throw new Error('PoA threshold not met: required ' + requiredThreshold + ', got ' + frame.poa.threshold);
  }
  const digest = poaFrameDigest(frame);
  if (!digest.equals(frame.poa.frameDigest)) {
    throw new Error('Proof-of-Action frame digest mismatch');
  }
  const msg = domainMessage(POA_SIGNATURE_DOMAIN, digest);
  const seen = new Set();
  let validCount = 0;

  const validatorMap = new Map();
  for (const v of validators) {
    validatorMap.set(v.nodeId, v);
  }

  for (const att of frame.poa.attestations) {
    if (seen.has(att.validatorNode)) {
      throw new Error('Duplicate PoA validator in certificate: ' + att.validatorNode);
    }
    seen.add(att.validatorNode);
    const pk = validatorMap.get(att.validatorNode);
    if (!pk) {
      throw new Error('Unknown PoA validator: ' + att.validatorNode);
    }
    if (!pk.verify(msg, att.signature)) {
      throw new Error('PoA validator signature failed for ' + att.validatorNode);
    }
    validCount++;
  }

  const effectiveThreshold = Math.max(frame.poa.threshold, requiredThreshold);
  if (validCount < effectiveThreshold) {
    throw new Error('PoA threshold not met: required ' + effectiveThreshold + ', actual ' + validCount);
  }
  return true;
}

export function inspectFrameHex(frame) {
  const enc = frame.encode();
  const hex = enc.toString('hex');
  const sections = [
    { name: 'Wire Header', start: 0, length: 64, color: '#38bdf8' },
    { name: 'Payload', start: 64, length: frame.payload.length, color: '#a855f7' },
  ];
  let offset = 64 + frame.payload.length;
  if (frame.auth) {
    sections.push({ name: 'Auth Trailer (ZSIG)', start: offset, length: 72, color: '#22c55e' });
    offset += 72;
  }
  if (frame.poa) {
    const poaLen = 44 + frame.poa.attestations.length * 80;
    sections.push({ name: 'PoA Trailer (ZPOA)', start: offset, length: poaLen, color: '#f59e0b' });
  }
  return { totalBytes: enc.length, hex, sections };
}
