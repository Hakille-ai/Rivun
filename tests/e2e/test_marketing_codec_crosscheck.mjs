/**
 * Empirical cross-codec validator:
 * Compares marketing-site protocol.ts frame generation against
 * TypeScript SDK (sdks/typescript/src/protocol.ts) and E2E Harness.
 */

import { encodeRivunFrame } from '../../apps/marketing-site/lib/protocol.ts';
import { RivunEnvelope as SdkEnvelope } from '../../sdks/typescript/src/protocol.ts';
import { RivunFrame as HarnessFrame, RivunHeader as HarnessHeader } from './harness/wireCodec.mjs';
import { RivunEnvelope as HarnessEnvelope } from './harness/zenvCodec.mjs';

console.log('Testing Marketing Site Frame Encoder Cross-Compatibility...');

const encoded = encodeRivunFrame({
  kind: 'event',
  subject: 'rivun.test.event',
  contentType: 'application/json',
  payloadJson: JSON.stringify({ hello: 'world' }),
  flags: {
    encrypted: false,
    priority: true,
    requiresConsensus: false,
    signed: true,
    broadcast: false,
  },
  sourceUuidStr: '11111111-2222-3333-4444-555555555555',
  targetUuidStr: '66666666-7777-8888-9999-aaaaaaaaaaaa',
});

console.log(`Raw Frame Bytes: ${encoded.rawBytes.length} bytes`);
console.log(`Wire Header Size: ${encoded.wireHeaderSize} bytes`);
console.log(`Payload (ZENV) Size: ${encoded.payloadSize} bytes`);
console.log(`Auth Trailer Size: ${encoded.authTrailerSize} bytes`);

// Test 1: Wire Header decoding with Harness
const wireHeaderBuf = Buffer.from(encoded.rawBytes.subarray(0, 64));
console.log('\nWire Header Byte 48..56 (Payload Length area):', wireHeaderBuf.subarray(48, 56));
const u64PayloadLen = wireHeaderBuf.readBigUInt64BE(48);
console.log(`Interpreted as u64: ${u64PayloadLen} (expected: ${encoded.payloadSize})`);

// Test 2: ZENV Envelope extraction
const zenvBuf = Buffer.from(encoded.rawBytes.subarray(64, 64 + encoded.payloadSize));
console.log('\nZENV Envelope Magic at [0..4]:', zenvBuf.subarray(0, 4).toString('ascii'));
console.log('ZENV Envelope Kind at [6..8]:', zenvBuf.readUInt16BE(6));
console.log('ZENV Envelope Reserved at [8..10]:', zenvBuf.readUInt16BE(8));
console.log('ZENV Envelope ID at [10..26]:', zenvBuf.subarray(10, 26).toString('hex'));

try {
  const sdkDecoded = SdkEnvelope.decode(new Uint8Array(zenvBuf));
  console.log('SDK Envelope Decode: SUCCESS', sdkDecoded);
} catch (e) {
  console.log('SDK Envelope Decode FAILED:', e.message);
}
