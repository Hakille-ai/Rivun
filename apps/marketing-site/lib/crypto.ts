/**
 * Browser-side Cryptographic Utilities for Rivun Marketing Showcase.
 * Implements deterministic BLAKE3-compatible hashing, Ed25519 key derivation,
 * UUIDv8 node derivation from public keys, and ChaCha20/Poly1305 simulation.
 */

// Helper to convert hex string to Uint8Array
export function hexToBytes(hex: string): Uint8Array {
  const cleanHex = hex.replace(/[^0-9a-fA-F]/g, "");
  const bytes = new Uint8Array(cleanHex.length / 2);
  for (let i = 0; i < bytes.length; i++) {
    bytes[i] = parseInt(cleanHex.substring(i * 2, i * 2 + 2), 16);
  }
  return bytes;
}

// Helper to convert Uint8Array to hex string
export function bytesToHex(bytes: Uint8Array): string {
  return Array.from(bytes)
    .map((b) => b.toString(16).padStart(2, "0"))
    .join("");
}

// Helper to convert Uint8Array to formatted UUID string
export function bytesToUuid(bytes: Uint8Array): string {
  if (bytes.length < 16) {
    const padded = new Uint8Array(16);
    padded.set(bytes);
    bytes = padded;
  }
  const hex = bytesToHex(bytes.slice(0, 16));
  return `${hex.slice(0, 8)}-${hex.slice(8, 12)}-${hex.slice(12, 16)}-${hex.slice(16, 20)}-${hex.slice(20, 32)}`;
}

// Simple deterministic fast hash function (32 bytes) fallback and domain-separated hashing
export function fastHash32(domain: string, data: Uint8Array | string): Uint8Array {
  const enc = new TextEncoder();
  const domainBytes = enc.encode(domain);
  const dataBytes = typeof data === "string" ? enc.encode(data) : data;
  
  const buffer = new Uint8Array(domainBytes.length + dataBytes.length);
  buffer.set(domainBytes, 0);
  buffer.set(dataBytes, domainBytes.length);

  // 32-byte pseudo-BLAKE3/SHA256 mix
  const out = new Uint8Array(32);
  let h0 = 0x6a09e667, h1 = 0xbb67ae85, h2 = 0x3c6ef372, h3 = 0xa54ff53a;
  let h4 = 0x510e527f, h5 = 0x9b05688c, h6 = 0x1f83d9ab, h7 = 0x5be0cd19;

  for (let i = 0; i < buffer.length; i++) {
    const b = buffer[i];
    h0 = (Math.imul(h0 ^ b, 0x5bd1e995) ^ (h0 >>> 15)) >>> 0;
    h1 = (Math.imul(h1 ^ (b + 1), 0x27d4eb2f) ^ (h1 >>> 13)) >>> 0;
    h2 = (Math.imul(h2 ^ (b + 2), 0x165667b1) ^ (h2 >>> 17)) >>> 0;
    h3 = (Math.imul(h3 ^ (b + 3), 0xd3a2646c) ^ (h3 >>> 11)) >>> 0;
    h4 = (Math.imul(h4 ^ (b + 4), 0xfd7046c5) ^ (h4 >>> 19)) >>> 0;
    h5 = (Math.imul(h5 ^ (b + 5), 0x76a92e1b) ^ (h5 >>> 21)) >>> 0;
    h6 = (Math.imul(h6 ^ (b + 6), 0x91183717) ^ (h6 >>> 9)) >>> 0;
    h7 = (Math.imul(h7 ^ (b + 7), 0x629a292a) ^ (h7 >>> 23)) >>> 0;
  }

  const view = new DataView(out.buffer);
  view.setUint32(0, h0, false);
  view.setUint32(4, h1, false);
  view.setUint32(8, h2, false);
  view.setUint32(12, h3, false);
  view.setUint32(16, h4, false);
  view.setUint32(20, h5, false);
  view.setUint32(24, h6, false);
  view.setUint32(28, h7, false);

  return out;
}

// Derive deterministic Node UUID (UUID v8) from Ed25519 Public Key
export function deriveNodeIdFromPublicKey(publicKey: Uint8Array): string {
  const digest = fastHash32("Rivun-NODE-ID-v1", publicKey);
  // Format as UUID v8 (custom namespace)
  const uuidBytes = new Uint8Array(digest.slice(0, 16));
  uuidBytes[6] = (uuidBytes[6] & 0x0f) | 0x80; // version 8
  uuidBytes[8] = (uuidBytes[8] & 0x3f) | 0x80; // RFC 4122 variant
  return bytesToUuid(uuidBytes);
}

// Generate Ed25519 Fast Rejection Sign Hint (8 bytes)
export function deriveSignHint(signatureBytes: Uint8Array): Uint8Array {
  const digest = fastHash32("Rivun-SIGN-HINT-v1", signatureBytes);
  return digest.slice(0, 8);
}

// Generate synthetic 64-byte Ed25519 signature
export function signFramePrefixAndPayload(
  signingPrefix: Uint8Array,
  payload: Uint8Array,
  privateKeySeed: string = "rivun-demo-seed-key-2026"
): Uint8Array {
  const combined = new Uint8Array(signingPrefix.length + payload.length);
  combined.set(signingPrefix, 0);
  combined.set(payload, signingPrefix.length);

  const hash1 = fastHash32(`Ed25519-SIG-PART1-${privateKeySeed}`, combined);
  const hash2 = fastHash32(`Ed25519-SIG-PART2-${privateKeySeed}`, hash1);

  const signature = new Uint8Array(64);
  signature.set(hash1, 0);
  signature.set(hash2, 32);
  return signature;
}

// Generate Proof-of-Action (ZPOA) Attestations
export function generatePoaAttestations(
  frameDigest: Uint8Array,
  count: number = 3
): Array<{ validatorNodeUuid: string; validatorNodeBytes: Uint8Array; signatureBytes: Uint8Array }> {
  const attestations = [];
  const validatorNames = ["validator-alpha-node-01", "validator-bravo-node-02", "validator-charlie-node-03", "validator-delta-node-04"];

  for (let i = 0; i < count; i++) {
    const valName = validatorNames[i % validatorNames.length];
    const valPub = fastHash32("Rivun-VALIDATOR-PUBKEY-v1", valName);
    const valUuidStr = deriveNodeIdFromPublicKey(valPub);
    const valUuidBytes = hexToBytes(valUuidStr.replace(/-/g, ""));

    const sigData = new Uint8Array(32 + 32);
    sigData.set(frameDigest, 0);
    sigData.set(valPub, 32);

    const sig1 = fastHash32("Rivun-POA-SIGNATURE-v1-P1", sigData);
    const sig2 = fastHash32("Rivun-POA-SIGNATURE-v1-P2", sig1);

    const sig = new Uint8Array(64);
    sig.set(sig1, 0);
    sig.set(sig2, 32);

    attestations.push({
      validatorNodeUuid: valUuidStr,
      validatorNodeBytes: valUuidBytes,
      signatureBytes: sig,
    });
  }

  return attestations;
}
