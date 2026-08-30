import {
  ByteSegment,
  EncodedFrameResult,
  HexDumpLine,
  MESSAGE_KINDS,
  MessageKindName,
  ProtocolFlags,
} from "./types";
import {
  bytesToHex,
  bytesToUuid,
  deriveNodeIdFromPublicKey,
  deriveSignHint,
  fastHash32,
  generatePoaAttestations,
  hexToBytes,
  signFramePrefixAndPayload,
} from "./crypto";

// Magic constants
export const WIRE_MAGIC = 0x5a41505f; // "ZAP_"
export const ZENV_MAGIC = 0x5a454e56; // "ZENV"
export const ZSIG_MAGIC = 0x5a534947; // "ZSIG"
export const ZPOA_MAGIC = 0x5a504f41; // "ZPOA"

export interface FrameEncodeOptions {
  kind: MessageKindName;
  subject: string;
  contentType: string;
  payloadJson: string;
  flags: ProtocolFlags;
  sourceUuidStr?: string;
  targetUuidStr?: string;
  timestampMicros?: bigint;
  poaThreshold?: number;
  poaAttestationCount?: number;
}

export function encodeRivunFrame(options: FrameEncodeOptions): EncodedFrameResult {
  const enc = new TextEncoder();
  const subjectBytes = enc.encode(options.subject);
  const contentTypeBytes = enc.encode(options.contentType);
  const metadataBytes = new Uint8Array(0); // Optional metadata
  const bodyBytes = enc.encode(options.payloadJson);

  // Derive source / target UUIDs
  let sourceUuidStr = options.sourceUuidStr;
  if (!sourceUuidStr) {
    const demoPub = fastHash32("Rivun-DEMO-SENDER-KEY", "sender-alpha");
    sourceUuidStr = deriveNodeIdFromPublicKey(demoPub);
  }
  const sourceBytes = hexToBytes(sourceUuidStr.replace(/-/g, ""));

  let targetUuidStr = options.targetUuidStr;
  if (!targetUuidStr || options.flags.broadcast) {
    targetUuidStr = options.flags.broadcast
      ? "00000000-0000-0000-0000-000000000000"
      : "11112222-3333-4444-5555-666677778888";
  }
  const targetBytes = hexToBytes(targetUuidStr.replace(/-/g, ""));

  // Calculate flags bitmask
  let flagsBitmask = 0;
  if (options.flags.encrypted) flagsBitmask |= 0x0001;
  if (options.flags.priority) flagsBitmask |= 0x0002;
  if (options.flags.requiresConsensus) flagsBitmask |= 0x0004;
  if (options.flags.signed) flagsBitmask |= 0x0008;
  if (options.flags.broadcast) flagsBitmask |= 0x0010;

  const timestampMicros =
    options.timestampMicros ?? BigInt(Date.now()) * 1000n + 450n;

  // 1. Construct ZENV Envelope Buffer
  // 74-byte header:
  // [0..4] magic ZENV (4B)
  // [4..6] version 1 (2B)
  // [6..8] kind u16 (2B)
  // [8..10] reserved u16 (2B)
  // [10..26] envelope UUID (16B)
  // [26..42] correlation UUID (16B)
  // [42..58] causation UUID (16B)
  // [58..60] subject len u16 (2B)
  // [60..62] content-type len u16 (2B)
  // [62..66] metadata len u32 (4B)
  // [66..74] body len u64 (8B)
  // Total = 74 bytes
  const zenvHeaderLen = 74;
  const zenvTotalLen =
    zenvHeaderLen +
    subjectBytes.length +
    contentTypeBytes.length +
    metadataBytes.length +
    bodyBytes.length;

  const zenvBuffer = new Uint8Array(zenvTotalLen);
  const zenvView = new DataView(zenvBuffer.buffer);

  // Magic 0x5A454E56 ("ZENV")
  zenvView.setUint32(0, ZENV_MAGIC, false);
  // Version 1
  zenvView.setUint16(4, 1, false);
  // Kind
  const kindInfo = MESSAGE_KINDS[options.kind] || MESSAGE_KINDS.data;
  zenvView.setUint16(6, kindInfo.id, false);
  zenvView.setUint16(8, 0, false); // reserved (2B)

  // Envelope UUID (deterministic based on subject & timestamp)
  const envUuidBytes = fastHash32(
    "Rivun-ENV-UUID",
    `${options.subject}-${timestampMicros}`
  ).slice(0, 16);
  envUuidBytes[6] = (envUuidBytes[6] & 0x0f) | 0x40; // v4
  envUuidBytes[8] = (envUuidBytes[8] & 0x3f) | 0x80;
  zenvBuffer.set(envUuidBytes, 10);

  // Correlation UUID (zeros if not chained)
  const corrBytes = new Uint8Array(16);
  zenvBuffer.set(corrBytes, 26);

  // Causation UUID (zeros if not chained)
  const causBytes = new Uint8Array(16);
  zenvBuffer.set(causBytes, 42);

  // Lengths
  zenvView.setUint16(58, subjectBytes.length, false);
  zenvView.setUint16(60, contentTypeBytes.length, false);
  zenvView.setUint32(62, metadataBytes.length, false);
  zenvView.setBigUint64(66, BigInt(bodyBytes.length), false);

  // Copy payloads into envelope
  let zenvOffset = 74;
  zenvBuffer.set(subjectBytes, zenvOffset);
  zenvOffset += subjectBytes.length;

  zenvBuffer.set(contentTypeBytes, zenvOffset);
  zenvOffset += contentTypeBytes.length;

  zenvBuffer.set(metadataBytes, zenvOffset);
  zenvOffset += metadataBytes.length;

  zenvBuffer.set(bodyBytes, zenvOffset);
  zenvOffset += bodyBytes.length;

  // The envelope is the wire payload
  const wirePayload = zenvBuffer;
  const wirePayloadLen = wirePayload.length;

  // 2. Build 64-byte Rivun Wire Header
  // [0..4] magic 0x5A41505F (4B)
  // [4..6] version 0x0001 (2B)
  // [6..8] flags (2B)
  // [8..24] source UUID (16B)
  // [24..40] target UUID (16B)
  // [40..48] timestamp micros (8B)
  // [48..56] payload len u64 (8B)
  // [56..64] signature hint (8B)
  const wireHeader = new Uint8Array(64);
  const wireView = new DataView(wireHeader.buffer);

  wireView.setUint32(0, WIRE_MAGIC, false);
  wireView.setUint16(4, 1, false);
  wireView.setUint16(6, flagsBitmask, false);
  wireHeader.set(sourceBytes.slice(0, 16), 8);
  wireHeader.set(targetBytes.slice(0, 16), 24);
  wireView.setBigUint64(40, timestampMicros, false);
  wireView.setBigUint64(48, BigInt(wirePayloadLen), false);

  // 3. Compute Trailing Signatures
  let authTrailer = new Uint8Array(0);
  let poaTrailer = new Uint8Array(0);

  const signingPrefix56 = wireHeader.slice(0, 56);
  const fullSignature = signFramePrefixAndPayload(signingPrefix56, wirePayload);
  const signHint = deriveSignHint(fullSignature);
  wireHeader.set(signHint, 56);

  if (options.flags.signed) {
    // 72-byte AuthTrailer:
    // [0..4] magic 0x5A534947 ("ZSIG")
    // [4..6] algorithm 0x0001 (Ed25519)
    // [6..8] sig_len 64
    // [8..72] signature 64B
    authTrailer = new Uint8Array(72);
    const authView = new DataView(authTrailer.buffer);
    authView.setUint32(0, ZSIG_MAGIC, false);
    authView.setUint16(4, 1, false); // Ed25519
    authView.setUint16(6, 64, false); // length
    authTrailer.set(fullSignature, 8);
  }

  if (options.flags.requiresConsensus) {
    // Proof of Action Trailer:
    // [0..4] magic 0x5A504F41 ("ZPOA")
    // [4..6] version 1
    // [6..8] threshold T
    // [8..10] attestation count K
    // [10..12] reserved
    // [12..44] 32-byte frame digest
    // [44..44+80*K] K * (16B validator node UUID + 64B signature)
    const threshold = options.poaThreshold ?? 2;
    const attCount = options.poaAttestationCount ?? 3;
    const frameDigest = fastHash32("Rivun-POA-DIGEST-v1", wirePayload);

    const poaLen = 44 + 80 * attCount;
    poaTrailer = new Uint8Array(poaLen);
    const poaView = new DataView(poaTrailer.buffer);

    poaView.setUint32(0, ZPOA_MAGIC, false);
    poaView.setUint16(4, 1, false);
    poaView.setUint16(6, threshold, false);
    poaView.setUint16(8, attCount, false);
    poaView.setUint16(10, 0, false);
    poaTrailer.set(frameDigest, 12);

    const attestations = generatePoaAttestations(frameDigest, attCount);
    let attOffset = 44;
    for (const att of attestations) {
      poaTrailer.set(att.validatorNodeBytes.slice(0, 16), attOffset);
      poaTrailer.set(att.signatureBytes.slice(0, 64), attOffset + 16);
      attOffset += 80;
    }
  }

  // 4. Assemble Final Contiguous Frame
  const totalSize =
    wireHeader.length +
    wirePayload.length +
    authTrailer.length +
    poaTrailer.length;
  const rawBytes = new Uint8Array(totalSize);

  let currentOffset = 0;
  rawBytes.set(wireHeader, currentOffset);
  currentOffset += wireHeader.length;

  rawBytes.set(wirePayload, currentOffset);
  currentOffset += wirePayload.length;

  if (authTrailer.length > 0) {
    rawBytes.set(authTrailer, currentOffset);
    currentOffset += authTrailer.length;
  }

  if (poaTrailer.length > 0) {
    rawBytes.set(poaTrailer, currentOffset);
    currentOffset += poaTrailer.length;
  }

  // 5. Build Byte Segment Metadata
  const segments: ByteSegment[] = [];

  // Wire Header segments
  segments.push({
    name: "Magic Number (ZAP_)",
    category: "magic",
    offset: 0,
    length: 4,
    hex: bytesToHex(wireHeader.slice(0, 4)),
    description: "Protocol Identifier (0x5A41505F)",
    parsedValue: "ZAP_",
    colorClass: "text-[#5B8CFF] bg-[#5B8CFF]/15 border-[#5B8CFF]/30",
  });

  segments.push({
    name: "Protocol Version",
    category: "version",
    offset: 4,
    length: 2,
    hex: bytesToHex(wireHeader.slice(4, 6)),
    description: "Rivun Wire Version (v1)",
    parsedValue: "0x0001",
    colorClass: "text-blue-300 bg-blue-500/10 border-blue-500/25",
  });

  segments.push({
    name: "Header Flags",
    category: "flags",
    offset: 6,
    length: 2,
    hex: bytesToHex(wireHeader.slice(6, 8)),
    description: `Bitmask (0x${flagsBitmask.toString(16).padStart(4, "0")})`,
    parsedValue: Object.entries(options.flags)
      .filter(([, v]) => v)
      .map(([k]) => k.toUpperCase())
      .join(" | ") || "NONE",
    colorClass: "text-amber-400 bg-amber-500/10 border-amber-500/25",
  });

  segments.push({
    name: "Source Node UUID",
    category: "source",
    offset: 8,
    length: 16,
    hex: bytesToHex(wireHeader.slice(8, 24)),
    description: "Sender UUIDv8 Identity",
    parsedValue: sourceUuidStr,
    colorClass: "text-cyan-400 bg-cyan-500/10 border-cyan-500/25",
  });

  segments.push({
    name: "Target Node UUID",
    category: "target",
    offset: 24,
    length: 16,
    hex: bytesToHex(wireHeader.slice(24, 40)),
    description: options.flags.broadcast ? "Broadcast All (Nil UUID)" : "Target Peer UUID",
    parsedValue: targetUuidStr,
    colorClass: "text-indigo-400 bg-indigo-500/10 border-indigo-500/25",
  });

  segments.push({
    name: "Timestamp Micros",
    category: "timestamp",
    offset: 40,
    length: 8,
    hex: bytesToHex(wireHeader.slice(40, 48)),
    description: "Big-endian Unix Microseconds",
    parsedValue: `${timestampMicros} µs`,
    colorClass: "text-teal-400 bg-teal-500/10 border-teal-500/25",
  });

  segments.push({
    name: "Payload Length",
    category: "length",
    offset: 48,
    length: 8,
    hex: bytesToHex(wireHeader.slice(48, 56)),
    description: "Wire Payload byte count (u64)",
    parsedValue: `${wirePayloadLen} bytes`,
    colorClass: "text-emerald-400 bg-emerald-500/10 border-emerald-500/25",
  });

  segments.push({
    name: "Signature Fast-Hint",
    category: "hint",
    offset: 56,
    length: 8,
    hex: bytesToHex(wireHeader.slice(56, 64)),
    description: "BLAKE3 fast rejection hash prefix",
    parsedValue: `0x${bytesToHex(signHint)}`,
    colorClass: "text-rose-400 bg-rose-500/10 border-rose-500/25",
  });

  // Envelope Header (offset 64..138)
  segments.push({
    name: "ZENV Envelope Header",
    category: "envelope",
    offset: 64,
    length: 74,
    hex: bytesToHex(rawBytes.slice(64, 138)),
    description: `Magic 'ZENV', Kind: ${kindInfo.label}, UUID: ${bytesToUuid(envUuidBytes)}`,
    parsedValue: `Kind=${kindInfo.name}, SubLen=${subjectBytes.length}, BodyLen=${bodyBytes.length}`,
    colorClass: "text-purple-400 bg-purple-500/10 border-purple-500/25",
  });

  let payloadOffsetCursor = 138;

  // Subject Bytes
  segments.push({
    name: "Subject Field",
    category: "subject",
    offset: payloadOffsetCursor,
    length: subjectBytes.length,
    hex: bytesToHex(rawBytes.slice(payloadOffsetCursor, payloadOffsetCursor + subjectBytes.length)),
    description: "UTF-8 Routing Topic / Subject",
    parsedValue: options.subject,
    colorClass: "text-yellow-300 bg-yellow-500/10 border-yellow-500/25",
  });
  payloadOffsetCursor += subjectBytes.length;

  // Content-Type Bytes
  segments.push({
    name: "Content-Type Field",
    category: "content_type",
    offset: payloadOffsetCursor,
    length: contentTypeBytes.length,
    hex: bytesToHex(rawBytes.slice(payloadOffsetCursor, payloadOffsetCursor + contentTypeBytes.length)),
    description: "MIME / Sub-protocol Type",
    parsedValue: options.contentType,
    colorClass: "text-sky-300 bg-sky-500/10 border-sky-500/25",
  });
  payloadOffsetCursor += contentTypeBytes.length;

  // Body Bytes
  segments.push({
    name: "Message Body (Payload)",
    category: "payload",
    offset: payloadOffsetCursor,
    length: bodyBytes.length,
    hex: bytesToHex(rawBytes.slice(payloadOffsetCursor, payloadOffsetCursor + bodyBytes.length)),
    description: "Data / Command / Action JSON Body",
    parsedValue: options.payloadJson.slice(0, 48) + (options.payloadJson.length > 48 ? "..." : ""),
    colorClass: "text-white bg-white/10 border-white/20",
  });
  payloadOffsetCursor += bodyBytes.length;

  // Auth Trailer
  if (authTrailer.length > 0) {
    segments.push({
      name: "Auth Trailer (ZSIG)",
      category: "auth_trailer",
      offset: payloadOffsetCursor,
      length: 72,
      hex: bytesToHex(rawBytes.slice(payloadOffsetCursor, payloadOffsetCursor + 72)),
      description: "Ed25519 64-Byte Digital Signature (Magic: ZSIG)",
      parsedValue: `Ed25519 Sig: ${bytesToHex(fullSignature).slice(0, 16)}...`,
      colorClass: "text-emerald-400 bg-emerald-500/15 border-emerald-500/35",
    });
    payloadOffsetCursor += 72;
  }

  // PoA Trailer
  if (poaTrailer.length > 0) {
    segments.push({
      name: "Proof-of-Action (ZPOA)",
      category: "poa_trailer",
      offset: payloadOffsetCursor,
      length: poaTrailer.length,
      hex: bytesToHex(rawBytes.slice(payloadOffsetCursor, payloadOffsetCursor + poaTrailer.length)),
      description: `BFT Consensus Attestation Quorum (T=${options.poaThreshold ?? 2}, K=${options.poaAttestationCount ?? 3})`,
      parsedValue: `${options.poaAttestationCount ?? 3} Validator Signatures`,
      colorClass: "text-amber-300 bg-amber-500/15 border-amber-500/35",
    });
  }

  // 6. Generate Hex Dump Lines (16 bytes per line)
  const hexDumpLines: HexDumpLine[] = [];
  const bytesPerLine = 16;
  const numLines = Math.ceil(rawBytes.length / bytesPerLine);

  for (let lineIdx = 0; lineIdx < numLines; lineIdx++) {
    const lineStart = lineIdx * bytesPerLine;
    const lineEnd = Math.min(lineStart + bytesPerLine, rawBytes.length);
    const lineBytes = rawBytes.slice(lineStart, lineEnd);

    const hexBytesArray = [];
    let asciiStr = "";

    for (let i = 0; i < lineBytes.length; i++) {
      const globalOffset = lineStart + i;
      const b = lineBytes[i];
      const byteHex = b.toString(16).padStart(2, "0").toUpperCase();

      // Find matching segment
      let segmentIndex = -1;
      let colorClass = "text-gray-400";
      for (let sIdx = 0; sIdx < segments.length; sIdx++) {
        const seg = segments[sIdx];
        if (globalOffset >= seg.offset && globalOffset < seg.offset + seg.length) {
          segmentIndex = sIdx;
          colorClass = seg.colorClass;
          break;
        }
      }

      hexBytesArray.push({
        byteHex,
        globalOffset,
        segmentIndex,
        colorClass,
      });

      // ASCII printable char
      if (b >= 32 && b <= 126) {
        asciiStr += String.fromCharCode(b);
      } else {
        asciiStr += ".";
      }
    }

    hexDumpLines.push({
      offset: lineStart,
      offsetHex: lineStart.toString(16).padStart(4, "0").toUpperCase(),
      hexBytes: hexBytesArray,
      ascii: asciiStr,
    });
  }

  const blake3Digest = bytesToHex(fastHash32("Rivun-FRAME-DIGEST-v1", rawBytes));

  return {
    rawBytes,
    totalSize,
    wireHeaderSize: 64,
    envelopeHeaderSize: 74,
    payloadSize: wirePayloadLen,
    authTrailerSize: authTrailer.length,
    poaTrailerSize: poaTrailer.length,
    segments,
    hexDumpLines,
    blake3Digest,
    signatureHint: bytesToHex(signHint),
    sourceUuid: sourceUuidStr,
    targetUuid: targetUuidStr,
    timestampMicros,
  };
}
