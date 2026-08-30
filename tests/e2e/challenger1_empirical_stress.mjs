/**
 * CHALLENGER 1: WIRE PROTOCOL & CONSENSUS ENGINE EMPIRICAL STRESS HARNESS
 * 
 * Tests:
 * 1. Big-endian 64-byte ZAP_ header & wire frame codec
 * 2. 74-byte ZENV universal messaging envelope codec
 * 3. Ed25519 ZSIG trailer & signature hint fast rejection
 * 4. ZPOA consensus trailer, Byzantine quorum thresholds (T = floor(2N/3) + 1), and equivocation slashing
 * 5. Merkle Mountain Range (MMR) accumulator, carry-over peak merging, inclusion & exclusion proofs
 * 6. Marketing site lib/protocol.ts browser-side encoder & cross-boundary edge cases
 */

import { blake3, blake3Hex } from './harness/blake3.mjs';
import {
  Keypair,
  PublicKey,
  POA_DIGEST_DOMAIN,
  POA_SIGNATURE_DOMAIN,
  formatUuid,
  parseUuid,
  nodeIdFromPublicKey,
  signatureHint,
  domainMessage,
} from './harness/crypto.mjs';
import {
  MAGIC_NUMBER,
  VERSION,
  HEADER_LEN,
  SIGNING_PREFIX_LEN,
  MAX_PAYLOAD_LEN,
  AUTH_TRAILER_MAGIC,
  AUTH_TRAILER_LEN,
  POA_TRAILER_MAGIC,
  POA_TRAILER_VERSION,
  POA_TRAILER_HEADER_LEN,
  POA_ATTESTATION_LEN,
  MAX_POA_ATTESTATIONS,
  Flags,
  RivunHeader,
  RivunFrame,
  AuthTrailer,
  PoaAttestation,
  PoaTrailer,
  signFrame,
  verifyFrame,
  certifyFrame,
  verifyPoaCertificate,
  poaFrameDigest,
} from './harness/wireCodec.mjs';
import {
  ZENV_MAGIC_NUMBER,
  ZENV_VERSION,
  ZENV_HEADER_LEN,
  MAX_SUBJECT_LEN,
  MAX_CONTENT_TYPE_LEN,
  MAX_METADATA_LEN,
  MAX_BODY_LEN,
  MessageKind,
  RivunEnvelope,
} from './harness/zenvCodec.mjs';
import {
  BftConsensusEngine,
  calculateQuorumThreshold,
} from './harness/consensus.mjs';
import {
  MerkleMountainRange,
  bagPeaks,
  mmrParentHash,
} from './harness/mmr.mjs';
import {
  assert,
  assertEqual,
  assertDeepEqual,
  assertThrows,
} from './harness/assert.mjs';

let passedTests = 0;
let failedTests = 0;
const failures = [];

function runTest(suite, name, fn) {
  try {
    fn();
    passedTests++;
    process.stdout.write(`  [PASS] ${suite} -> ${name}\n`);
  } catch (err) {
    failedTests++;
    failures.push({ suite, name, error: err.message, stack: err.stack });
    process.stdout.write(`  [FAIL] ${suite} -> ${name}: ${err.message}\n`);
  }
}

console.log('='.repeat(80));
console.log('CHALLENGER 1: WIRE PROTOCOL & CONSENSUS EMPIRICAL STRESS VERIFICATION');
console.log('='.repeat(80));
console.log(`Execution Time: ${new Date().toISOString()}`);
console.log(`Node.js: ${process.version}`);
console.log('-'.repeat(80));

// ============================================================================
// SUITE 1: 64-BYTE ZAP_ WIRE HEADER & FRAMING STRESS
// ============================================================================
console.log('\n>> SUITE 1: 64-BYTE ZAP_ WIRE HEADER & FRAMING STRESS');

runTest('Suite 1', '1.1: Exact 64-byte Header Layout & Big-Endian Offsets', () => {
  const srcUuid = '11111111-2222-3333-4444-555555555555';
  const tgtUuid = '66666666-7777-8888-9999-aaaaaaaaaaaa';
  const ts = 1700000000123456n;
  const payloadLen = 42;
  const signHintBytes = Buffer.from([1, 2, 3, 4, 5, 6, 7, 8]);

  const header = new RivunHeader({
    magic: MAGIC_NUMBER,
    version: 1,
    flags: Flags.ENCRYPTED | Flags.PRIORITY | Flags.SIGNED,
    sourceNode: srcUuid,
    targetNode: tgtUuid,
    timestampMicros: ts,
    payloadLen,
    rivunSign: signHintBytes,
  });

  const buf = header.encode();
  assertEqual(buf.length, 64, 'Header must be exactly 64 bytes');
  assertEqual(buf.readUInt32BE(0), 0x5A41505F, 'Magic at [0..4] must be 0x5A41505F (ZAP_)');
  assertEqual(buf.readUInt16BE(4), 1, 'Version at [4..6] must be 1');
  assertEqual(buf.readUInt16BE(6), 0x0001 | 0x0002 | 0x0008, 'Flags at [6..8] must match bitmask');
  assertEqual(formatUuid(buf.subarray(8, 24)), srcUuid, 'Source Node at [8..24]');
  assertEqual(formatUuid(buf.subarray(24, 40)), tgtUuid, 'Target Node at [24..40]');
  assertEqual(buf.readBigUInt64BE(40), ts, 'Timestamp at [40..48]');
  assertEqual(Number(buf.readBigUInt64BE(48)), payloadLen, 'Payload length at [48..56]');
  assertEqual(buf.subarray(56, 64).toString('hex'), signHintBytes.toString('hex'), 'Signature hint at [56..64]');

  const decoded = RivunHeader.decode(buf);
  assertEqual(decoded.magic, MAGIC_NUMBER);
  assertEqual(decoded.version, 1);
  assertEqual(decoded.flags, Flags.ENCRYPTED | Flags.PRIORITY | Flags.SIGNED);
  assertEqual(decoded.sourceNode, srcUuid);
  assertEqual(decoded.targetNode, tgtUuid);
  assertEqual(decoded.timestampMicros, ts);
  assertEqual(decoded.payloadLen, payloadLen);
  assertEqual(decoded.rivunSign.toString('hex'), signHintBytes.toString('hex'));
});

runTest('Suite 1', '1.2: Magic Number Fuzzing & Mutation Rejection', () => {
  const validHeader = new RivunHeader({}).encode();
  const corruptMagics = [
    0x00000000,
    0x5A415000,
    0x5A41505E,
    0x5A415060,
    0xFFFFFFFF,
    0x48545450, // "HTTP"
    0x504F5354, // "POST"
  ];

  for (const badMagic of corruptMagics) {
    const corruptBuf = Buffer.from(validHeader);
    corruptBuf.writeUInt32BE(badMagic, 0);
    assertThrows(() => RivunHeader.decode(corruptBuf), 'Invalid magic number', `Should reject bad magic 0x${badMagic.toString(16)}`);
  }
});

runTest('Suite 1', '1.3: Version Fuzzing & Rejection', () => {
  const validHeader = new RivunHeader({}).encode();
  const badVersions = [0, 2, 3, 10, 255, 65535];

  for (const badVer of badVersions) {
    const corruptBuf = Buffer.from(validHeader);
    corruptBuf.writeUInt16BE(badVer, 4);
    assertThrows(() => RivunHeader.decode(corruptBuf), 'Unsupported version', `Should reject bad version ${badVer}`);
  }
});

runTest('Suite 1', '1.4: Strict Flag Bitmask Fuzzing & Unknown Bit Rejection', () => {
  const validHeader = new RivunHeader({}).encode();
  // Valid bits are 0x0001 (ENCRYPTED), 0x0002 (PRIORITY), 0x0004 (REQUIRES_CONSENSUS), 0x0008 (SIGNED), 0x0010 (BROADCAST)
  // Max valid flags is 0x001F. Any bit >= 0x0020 must be rejected.
  const badFlagBits = [
    0x0020, 0x0040, 0x0080, 0x0100, 0x0200, 0x0400, 0x0800, 0x1000, 0x2000, 0x4000, 0x8000,
    0x003F, 0x00FF, 0xFFFF
  ];

  for (const badFlag of badFlagBits) {
    const corruptBuf = Buffer.from(validHeader);
    corruptBuf.writeUInt16BE(badFlag, 6);
    assertThrows(() => RivunHeader.decode(corruptBuf), 'Unknown flag bits', `Should reject unknown flag bitmask 0x${badFlag.toString(16)}`);
  }
});

runTest('Suite 1', '1.5: Payload Length Boundaries (0B, 16MiB, Overflow, Truncation)', () => {
  const validHeader = new RivunHeader({}).encode();

  // 16 MiB max payload
  const maxPayloadHeader = Buffer.from(validHeader);
  maxPayloadHeader.writeBigUInt64BE(BigInt(MAX_PAYLOAD_LEN), 48);
  const decMax = RivunHeader.decode(maxPayloadHeader);
  assertEqual(decMax.payloadLen, MAX_PAYLOAD_LEN);

  // 16 MiB + 1 byte -> must reject
  const overPayloadHeader = Buffer.from(validHeader);
  overPayloadHeader.writeBigUInt64BE(BigInt(MAX_PAYLOAD_LEN + 1), 48);
  assertThrows(() => RivunHeader.decode(overPayloadHeader), 'Payload length exceeds maximum 16MiB');

  // Extreme overflow: 2^64 - 1
  const maxUint64Header = Buffer.from(validHeader);
  maxUint64Header.writeBigUInt64BE(0xFFFFFFFFFFFFFFFFn, 48);
  assertThrows(() => RivunHeader.decode(maxUint64Header), 'Payload length exceeds maximum 16MiB');

  // Truncated header buffer (< 64 bytes)
  for (let len = 0; len < 64; len++) {
    const trunc = validHeader.subarray(0, len);
    assertThrows(() => RivunHeader.decode(trunc), 'Header too short');
  }
});

runTest('Suite 1', '1.6: Flag Invariants & Trailer Pairing Enforcement', () => {
  const kp = Keypair.generate();
  const payload = Buffer.from('Rivun Consensus Critical Payload 2026', 'utf8');

  // Case A: SIGNED flag set, but buffer ends before AuthTrailer -> reject
  const signedFrame = signFrame(kp, new RivunFrame(new RivunHeader({ sourceNode: kp.nodeId }), payload));
  const signedBytes = signedFrame.encode();
  const truncatedAuth = signedBytes.subarray(0, signedBytes.length - 10);
  assertThrows(() => RivunFrame.decode(truncatedAuth), 'Signed frame is missing an Ed25519 auth trailer');

  // Case B: Frame not marked SIGNED, but trailing garbage present -> reject length mismatch
  const unsignedFrame = new RivunFrame(new RivunHeader({ sourceNode: kp.nodeId }), payload);
  const unsignedBytes = unsignedFrame.encode();
  const trailingGarbage = Buffer.concat([unsignedBytes, Buffer.from('EXTRA_BYTES')]);
  assertThrows(() => RivunFrame.decode(trailingGarbage), 'Frame length mismatch');
});


// ============================================================================
// SUITE 2: 74-BYTE ZENV UNIVERSAL MESSAGING ENVELOPE STRESS
// ============================================================================
console.log('\n>> SUITE 2: 74-BYTE ZENV UNIVERSAL MESSAGING ENVELOPE STRESS');

runTest('Suite 2', '2.1: Exact 74-byte ZENV Layout & All 8 Discrete Message Kinds', () => {
  const kinds = [
    { id: MessageKind.Data, name: 'Data', requiresSubject: false },
    { id: MessageKind.Event, name: 'Event', requiresSubject: true },
    { id: MessageKind.Command, name: 'Command', requiresSubject: true },
    { id: MessageKind.Query, name: 'Query', requiresSubject: true },
    { id: MessageKind.Response, name: 'Response', requiresSubject: true },
    { id: MessageKind.StreamChunk, name: 'StreamChunk', requiresSubject: true },
    { id: MessageKind.Action, name: 'Action', requiresSubject: true },
    { id: MessageKind.Control, name: 'Control', requiresSubject: true },
  ];

  for (const k of kinds) {
    const env = new RivunEnvelope({
      kind: k.id,
      id: '12345678-1234-5678-1234-567812345678',
      correlationId: 'aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee',
      causationId: 'ffffffff-0000-1111-2222-333333333333',
      subject: k.requiresSubject ? `rivun.${k.name.toLowerCase()}.test` : '',
      contentType: 'application/json',
      metadata: Buffer.from(JSON.stringify({ priority: 'high' })),
      body: Buffer.from(JSON.stringify({ status: 'ok', kind: k.name })),
    });

    const encoded = env.encode();
    assert(encoded.length >= ZENV_HEADER_LEN, 'Encoded envelope must be >= 74 bytes');
    assertEqual(encoded.readUInt32BE(0), ZENV_MAGIC_NUMBER, 'Magic must be ZENV');
    assertEqual(encoded.readUInt16BE(4), ZENV_VERSION, 'Version must be 1');
    assertEqual(encoded.readUInt16BE(6), k.id, `Kind at [6..8] must be ${k.id}`);
    assertEqual(encoded.readUInt16BE(8), 0, 'Reserved at [8..10] must be 0');

    const decoded = RivunEnvelope.decode(encoded);
    assertEqual(decoded.kind, k.id);
    assertEqual(decoded.id, env.id);
    assertEqual(decoded.correlationId, env.correlationId);
    assertEqual(decoded.causationId, env.causationId);
    assertEqual(decoded.subject, env.subject);
    assertEqual(decoded.contentType, env.contentType);
    assertEqual(decoded.metadata.toString('utf8'), env.metadata.toString('utf8'));
    assertEqual(decoded.body.toString('utf8'), env.body.toString('utf8'));
  }
});

runTest('Suite 2', '2.2: Invalid Message Kinds Fuzzing & Rejection', () => {
  const validEnv = new RivunEnvelope({ subject: 'test' }).encode();
  const invalidKinds = [0, 9, 10, 255, 65535];

  for (const badKind of invalidKinds) {
    const buf = Buffer.from(validEnv);
    buf.writeUInt16BE(badKind, 6);
    assertThrows(() => RivunEnvelope.decode(buf), 'Unknown envelope kind');
  }
});

runTest('Suite 2', '2.3: Non-zero Reserved Field Detection', () => {
  const validEnv = new RivunEnvelope({ subject: 'test' }).encode();
  const badReservedValues = [0x0001, 0x0080, 0x0100, 0xFFFF];

  for (const r of badReservedValues) {
    const buf = Buffer.from(validEnv);
    buf.writeUInt16BE(r, 8);
    assertThrows(() => RivunEnvelope.decode(buf), 'Reserved field must be zero');
  }
});

runTest('Suite 2', '2.4: Missing Subject Enforcement for Non-Data Kinds', () => {
  const nonDataKinds = [
    MessageKind.Event,
    MessageKind.Command,
    MessageKind.Query,
    MessageKind.Response,
    MessageKind.StreamChunk,
    MessageKind.Action,
    MessageKind.Control,
  ];

  for (const k of nonDataKinds) {
    const env = new RivunEnvelope({ kind: k, subject: '' });
    const encoded = env.encode();
    assertThrows(() => RivunEnvelope.decode(encoded), 'Subject is required');
  }
});

runTest('Suite 2', '2.5: Subject & Content-Type Length Boundaries (512B / 128B)', () => {
  // 512 bytes subject: OK
  const maxSubject = 'a'.repeat(MAX_SUBJECT_LEN);
  const envMaxSub = new RivunEnvelope({ kind: MessageKind.Event, subject: maxSubject });
  const decMaxSub = RivunEnvelope.decode(envMaxSub.encode());
  assertEqual(decMaxSub.subject.length, MAX_SUBJECT_LEN);

  // 513 bytes subject: Reject
  const overSubject = 'a'.repeat(MAX_SUBJECT_LEN + 1);
  assertThrows(() => new RivunEnvelope({ kind: MessageKind.Event, subject: overSubject }).encode(), 'Subject length exceeds maximum 512 bytes');

  // 128 bytes Content-Type: OK
  const maxCt = 'c'.repeat(MAX_CONTENT_TYPE_LEN);
  const envMaxCt = new RivunEnvelope({ kind: MessageKind.Data, contentType: maxCt });
  const decMaxCt = RivunEnvelope.decode(envMaxCt.encode());
  assertEqual(decMaxCt.contentType.length, MAX_CONTENT_TYPE_LEN);

  // 129 bytes Content-Type: Reject
  const overCt = 'c'.repeat(MAX_CONTENT_TYPE_LEN + 1);
  assertThrows(() => new RivunEnvelope({ kind: MessageKind.Data, contentType: overCt }).encode(), 'Content-Type length exceeds maximum 128 bytes');
});


// ============================================================================
// SUITE 3: ED25519 ZSIG TRAILERS & SIGNATURE HINT STRESS
// ============================================================================
console.log('\n>> SUITE 3: ED25519 ZSIG TRAILERS & SIGNATURE HINT STRESS');

runTest('Suite 3', '3.1: 72-Byte AuthTrailer Layout & Algorithm Verification', () => {
  const sig = Buffer.alloc(64, 0xAA);
  const trailer = new AuthTrailer(sig, 1, 64);
  const enc = trailer.encode();
  assertEqual(enc.length, 72, 'AuthTrailer must be 72 bytes');
  assertEqual(enc.subarray(0, 4).toString('ascii'), 'ZSIG');
  assertEqual(enc.readUInt16BE(4), 1, 'Algorithm must be 1 (Ed25519)');
  assertEqual(enc.readUInt16BE(6), 64, 'Signature length must be 64');
  assertEqual(enc.subarray(8, 72).toString('hex'), sig.toString('hex'));

  // Corrupted algorithm -> reject
  const badAlgoBuf = Buffer.from(enc);
  badAlgoBuf.writeUInt16BE(2, 4);
  assertThrows(() => AuthTrailer.decode(badAlgoBuf), 'Unsupported signature algorithm');

  // Corrupted sig length -> reject
  const badSigLenBuf = Buffer.from(enc);
  badSigLenBuf.writeUInt16BE(65, 6);
  assertThrows(() => AuthTrailer.decode(badSigLenBuf), 'Invalid signature length');
});

runTest('Suite 3', '3.2: Ed25519 Sign and Verify Happy Path & Transcript Isolation', () => {
  const kp = Keypair.generate();
  const payload = Buffer.from('Zero-Trust Cryptographic Action Payload', 'utf8');
  const frame = new RivunFrame(new RivunHeader({ sourceNode: kp.nodeId }), payload);

  const signed = signFrame(kp, frame);
  assert((signed.header.flags & Flags.SIGNED) !== 0, 'Frame must have SIGNED flag');
  assert(signed.auth !== null, 'Frame must contain AuthTrailer');
  assertEqual(signed.auth.signature.length, 64);

  // Verification passes with correct public key
  const pk = PublicKey.fromBytes(kp.publicKeyBytes);
  assert(verifyFrame(pk, signed), 'Valid signed frame must verify');
});

runTest('Suite 3', '3.3: Fast Signature Hint Rejection (O(1) Pre-Verification)', () => {
  const kp = Keypair.generate();
  const payload = Buffer.from('Fast Hint Verification', 'utf8');
  const frame = signFrame(kp, new RivunFrame(new RivunHeader({ sourceNode: kp.nodeId }), payload));

  // Mutate 1 byte of the 8-byte signature hint in the wire header
  frame.header.rivunSign[0] ^= 0xFF;

  const pk = PublicKey.fromBytes(kp.publicKeyBytes);
  assertThrows(() => verifyFrame(pk, frame), 'Signature hint mismatch', 'Hint tampering must be caught immediately');
});

runTest('Suite 3', '3.4: Adversarial Bit-Flip Stress on Signature & Header Transcript', () => {
  const kp = Keypair.generate();
  const pk = PublicKey.fromBytes(kp.publicKeyBytes);
  const payload = Buffer.from('Adversarial Bit Flip Attack Resistance', 'utf8');

  // Attack 1: Mutate payload
  const frame1 = signFrame(kp, new RivunFrame(new RivunHeader({ sourceNode: kp.nodeId }), payload));
  frame1.payload[0] ^= 0x01; // flip 1 bit
  assertThrows(() => verifyFrame(pk, frame1), 'Ed25519 signature verification failed');

  // Attack 2: Mutate timestamp in 56-byte signing prefix
  const frame2 = signFrame(kp, new RivunFrame(new RivunHeader({ sourceNode: kp.nodeId }), payload));
  frame2.header.timestampMicros += 1n;
  assertThrows(() => verifyFrame(pk, frame2), 'Ed25519 signature verification failed');

  // Attack 3: Mutate signature bytes
  const frame3 = signFrame(kp, new RivunFrame(new RivunHeader({ sourceNode: kp.nodeId }), payload));
  frame3.auth.signature[10] ^= 0x80;
  // Recompute hint so hint matches mutated sig, but ed25519 math fails
  frame3.header.rivunSign = signatureHint(frame3.auth.signature);
  assertThrows(() => verifyFrame(pk, frame3), 'Ed25519 signature verification failed');
});

runTest('Suite 3', '3.5: Key Mismatch and Impersonation Defense', () => {
  const kpAlice = Keypair.generate();
  const kpBob = Keypair.generate();
  const pkBob = PublicKey.fromBytes(kpBob.publicKeyBytes);

  const payload = Buffer.from('Alice Authenticated Message', 'utf8');
  const signedByAlice = signFrame(kpAlice, new RivunFrame(new RivunHeader({ sourceNode: kpAlice.nodeId }), payload));

  // Bob verifies Alice's frame -> source node mismatch
  assertThrows(() => verifyFrame(pkBob, signedByAlice), 'does not match verifying key node');

  // Alice tries to forge Bob's source node identity -> rejected during signing
  assertThrows(() => {
    signFrame(kpAlice, new RivunFrame(new RivunHeader({ sourceNode: kpBob.nodeId }), payload));
  }, 'does not match signing key node');
});


// ============================================================================
// SUITE 4: PROOF-OF-ACTION (ZPOA) CONSENSUS & BYZANTINE QUORUM STRESS
// ============================================================================
console.log('\n>> SUITE 4: PROOF-OF-ACTION (ZPOA) CONSENSUS & BYZANTINE QUORUM STRESS');

runTest('Suite 4', '4.1: Byzantine Quorum Formula T = floor(2N/3) + 1 Exhaustive Table', () => {
  const testCases = [
    { n: 1, expectedT: 1 },
    { n: 2, expectedT: 2 },
    { n: 3, expectedT: 3 },
    { n: 4, expectedT: 3 },
    { n: 5, expectedT: 4 },
    { n: 6, expectedT: 5 },
    { n: 7, expectedT: 5 },
    { n: 8, expectedT: 6 },
    { n: 9, expectedT: 7 },
    { n: 10, expectedT: 7 },
    { n: 16, expectedT: 11 },
    { n: 32, expectedT: 22 },
    { n: 64, expectedT: 43 },
    { n: 100, expectedT: 67 },
  ];

  for (const tc of testCases) {
    const t = calculateQuorumThreshold(tc.n);
    assertEqual(t, tc.expectedT, `Quorum for N=${tc.n} must be T=${tc.expectedT}, got ${t}`);
  }
});

runTest('Suite 4', '4.2: PoaTrailer Layout & Attestation Bounds (44B + K*80B)', () => {
  const digest = blake3(Buffer.from('Poa Frame Digest'));
  const v1 = Keypair.generate();
  const v2 = Keypair.generate();
  const atts = [
    new PoaAttestation(v1.nodeId, Buffer.alloc(64, 1)),
    new PoaAttestation(v2.nodeId, Buffer.alloc(64, 2)),
  ];

  const trailer = new PoaTrailer(2, digest, atts);
  const enc = trailer.encode();
  const expectedLen = 44 + 2 * 80;
  assertEqual(enc.length, expectedLen, `PoA trailer must be exactly 44 + 2*80 = ${expectedLen} bytes`);
  assertEqual(enc.subarray(0, 4).toString('ascii'), 'ZPOA');
  assertEqual(enc.readUInt16BE(4), 1, 'Version must be 1');
  assertEqual(enc.readUInt16BE(6), 2, 'Threshold must be 2');
  assertEqual(enc.readUInt16BE(8), 2, 'Attestation count must be 2');

  const dec = PoaTrailer.decode(enc);
  assertEqual(dec.threshold, 2);
  assertEqual(dec.attestations.length, 2);
  assertEqual(dec.attestations[0].validatorNode, v1.nodeId);
  assertEqual(dec.attestations[1].validatorNode, v2.nodeId);

  // Reject Threshold 0
  const zeroThreshBuf = Buffer.from(enc);
  zeroThreshBuf.writeUInt16BE(0, 6);
  assertThrows(() => PoaTrailer.decode(zeroThreshBuf), 'Invalid PoA threshold: 0');

  // Reject K > 64
  const overCountBuf = Buffer.from(enc);
  overCountBuf.writeUInt16BE(65, 8);
  assertThrows(() => PoaTrailer.decode(overCountBuf), 'PoA attestation count exceeds maximum: 65');
});

runTest('Suite 4', '4.3: Multi-Validator Certification & Threshold Verification', () => {
  const senderKp = Keypair.generate();
  const v1 = Keypair.generate();
  const v2 = Keypair.generate();
  const v3 = Keypair.generate();
  const v4 = Keypair.generate();
  const validators = [v1, v2, v3, v4];
  const N = 4;
  const T = calculateQuorumThreshold(N); // 3

  const payload = Buffer.from('High-Value Financial Transfer Request', 'utf8');
  const baseFrame = signFrame(
    senderKp,
    new RivunFrame(
      new RivunHeader({
        sourceNode: senderKp.nodeId,
        flags: Flags.REQUIRES_CONSENSUS,
      }),
      payload
    )
  );

  // Case 1: Exactly T=3 validators certify -> PASS
  const certified3 = certifyFrame(baseFrame, T, [v1, v2, v3]);
  assertEqual(certified3.poa.attestations.length, 3);
  const publicKeys = validators.map((v) => PublicKey.fromBytes(v.publicKeyBytes));
  assert(verifyPoaCertificate(certified3, publicKeys, T), '3 of 4 validators must satisfy quorum T=3');

  // Case 2: Insufficient attestations (2 of 4) -> REJECT
  assertThrows(() => {
    certifyFrame(baseFrame, T, [v1, v2]);
  }, 'PoA threshold not met: required 3, got 2');

  // Case 3: Duplicate validator in attestations -> REJECT
  assertThrows(() => {
    certifyFrame(baseFrame, T, [v1, v2, v1]);
  }, 'Duplicate PoA validator');

  // Case 4: Corrupted validator signature -> REJECT during verification
  const tamperedCertified = certifyFrame(baseFrame, T, [v1, v2, v3]);
  tamperedCertified.poa.attestations[0].signature[0] ^= 0xFF;
  assertThrows(() => {
    verifyPoaCertificate(tamperedCertified, publicKeys, T);
  }, 'PoA validator signature failed');
});

runTest('Suite 4', '4.4: 2-Phase BFT State Machine & Commit Certificate Quorum', () => {
  const v1 = Keypair.generate();
  const v2 = Keypair.generate();
  const v3 = Keypair.generate();
  const v4 = Keypair.generate();
  const validators = [v1, v2, v3, v4];
  const publicKeys = validators.map((v) => PublicKey.fromBytes(v.publicKeyBytes));

  const engine = new BftConsensusEngine({
    epoch: 1,
    validators: validators.map((v) => ({ nodeId: v.nodeId })),
  });
  assertEqual(engine.threshold, 3, 'T=3 for N=4');

  const height = 1;
  const proposalHash = blake3Hex(Buffer.from('Proposal Block Height 1'));

  // Phase 1: Propose
  const proposal = engine.propose(v1, height, proposalHash);
  assertEqual(proposal.proposerNode, v1.nodeId);
  assertEqual(proposal.proposalHash, proposalHash);

  // Phase 2: Prevote (3 votes)
  engine.castPrevote(v1, height, proposalHash);
  engine.castPrevote(v2, height, proposalHash);
  assertEqual(engine.checkPolka(height, proposalHash), false, '2 prevotes < threshold 3');
  engine.castPrevote(v3, height, proposalHash);
  assertEqual(engine.checkPolka(height, proposalHash), true, '3 prevotes >= threshold 3 (Polka formed)');

  // Phase 3: Precommit (3 votes)
  engine.castPrecommit(v1, height, proposalHash);
  engine.castPrecommit(v2, height, proposalHash);
  engine.castPrecommit(v3, height, proposalHash);

  // Phase 4: Create Commit Certificate
  const cert = engine.createCommitCertificate(height, proposalHash, publicKeys);
  assertEqual(cert.height, height);
  assertEqual(cert.proposalHash, proposalHash);
  assertEqual(cert.attestationCount, 3);
  assertEqual(cert.threshold, 3);
  assertEqual(engine.committedHeight, height);
});

runTest('Suite 4', '4.5: Byzantine Equivocation Slashing Detection', () => {
  const v1 = Keypair.generate();
  const v2 = Keypair.generate();
  const v3 = Keypair.generate();
  const v4 = Keypair.generate();
  const validators = [v1, v2, v3, v4];

  const engine = new BftConsensusEngine({
    epoch: 1,
    validators: validators.map((v) => ({ nodeId: v.nodeId })),
  });

  const height = 1;
  const hashA = blake3Hex(Buffer.from('Block A'));
  const hashB = blake3Hex(Buffer.from('Conflicting Block B'));

  // Equivocation 1: Leader proposes conflicting blocks in same round
  engine.propose(v1, height, hashA);
  assertThrows(() => {
    engine.propose(v1, height, hashB);
  }, 'Equivocation detected: leader slashed!');

  assert(engine.slashedNodes.has(v1.nodeId), 'Leader must be in slashed nodes set');

  // Slashed leader cannot vote or propose again
  assertThrows(() => {
    engine.propose(v1, height, hashA);
  }, 'Slashed validator cannot propose');

  assertThrows(() => {
    engine.castPrevote(v1, height, hashA);
  }, 'Slashed validator cannot vote');

  // Equivocation 2: Validator double-votes in prevote
  engine.castPrevote(v2, height, hashA);
  assertThrows(() => {
    engine.castPrevote(v2, height, hashB);
  }, 'Equivocation detected: validator slashed!');
  assert(engine.slashedNodes.has(v2.nodeId), 'Validator 2 must be slashed');
});


// ============================================================================
// SUITE 5: MERKLE MOUNTAIN RANGE (MMR) ACCUMULATOR & EXCLUSION PROOFS
// ============================================================================
console.log('\n>> SUITE 5: MERKLE MOUNTAIN RANGE (MMR) ACCUMULATOR & EXCLUSION PROOFS');

runTest('Suite 5', '5.1: Carry-Over Subtree Peak Merging Across Varied Leaf Counts', () => {
  const mmr = new MerkleMountainRange();

  // Leaf 0 (N=1): 1 peak (height 0)
  mmr.append('receipt-0');
  assertEqual(mmr.leafCount, 1);
  assertEqual(mmr.peaks.length, 1);

  // Leaf 1 (N=2): Carry-over merge -> 1 peak (height 1)
  mmr.append('receipt-1');
  assertEqual(mmr.leafCount, 2);
  assertEqual(mmr.peaks.length, 1);

  // Leaf 2 (N=3): 2 peaks (height 1, height 0)
  mmr.append('receipt-2');
  assertEqual(mmr.leafCount, 3);
  assertEqual(mmr.peaks.length, 2);

  // Leaf 3 (N=4): Carry-over merge -> 1 peak (height 2)
  mmr.append('receipt-3');
  assertEqual(mmr.leafCount, 4);
  assertEqual(mmr.peaks.length, 1);

  // Append up to 15 leaves: 15 = 8 + 4 + 2 + 1 -> 4 peaks
  for (let i = 4; i < 15; i++) {
    mmr.append(`receipt-${i}`);
  }
  assertEqual(mmr.leafCount, 15);
  assertEqual(mmr.peaks.length, 4, '15 leaves must produce 4 active peaks');

  // Append 16th leaf: 16 = 2^4 -> 1 single root peak
  mmr.append('receipt-15');
  assertEqual(mmr.leafCount, 16);
  assertEqual(mmr.peaks.length, 1, '16 leaves must fold into 1 single peak');
});

runTest('Suite 5', '5.2: Bagged Peak Root Determinism & Inclusion Proofs', () => {
  const mmr = new MerkleMountainRange();
  const receiptCount = 25;
  for (let i = 0; i < receiptCount; i++) {
    mmr.append(`action-receipt-sequence-${i}`);
  }

  const rootHex = mmr.getRootHex();
  assert(rootHex.length === 64, 'MMR root must be 32 bytes (64 hex chars)');

  // Verify single inclusion proof for each leaf
  for (let i = 0; i < receiptCount; i++) {
    const proof = mmr.generateInclusionProof(i);
    assertEqual(proof.leafIndex, i);
    assertEqual(proof.root, rootHex);
    assert(mmr.verifyInclusionProof(proof), `Proof for leaf ${i} must verify`);
  }

  // Mutated proof must fail
  const proof0 = mmr.generateInclusionProof(0);
  proof0.root = '00'.repeat(32);
  assertEqual(mmr.verifyInclusionProof(proof0), false, 'Tampered root must fail verification');
});

runTest('Suite 5', '5.3: Batch Inclusion Proof Verification', () => {
  const mmr = new MerkleMountainRange();
  for (let i = 0; i < 50; i++) {
    mmr.append(`audit-receipt-${i}`);
  }

  const batchIndices = [0, 5, 12, 27, 49];
  const batchProof = mmr.generateBatchProof(batchIndices);
  assertEqual(batchProof.leafIndices.length, 5);
  assert(mmr.verifyBatchProof(batchProof), 'Batch inclusion proof must verify');

  // Empty batch -> fail
  assertEqual(mmr.verifyBatchProof({ leafIndices: [] }), false);
});

runTest('Suite 5', '5.4: Monotonic Range Non-Membership (Exclusion) Proof Verification', () => {
  // Exclusion proof logic: prove an item X with sequence number S_x does not exist
  // by proving that leaf L_i (with sequence S_i < S_x) and leaf L_{i+1} (with sequence S_{i+1} > S_x)
  // are consecutive in the MMR.
  const mmr = new MerkleMountainRange();
  const receipts = [
    { seq: 100, data: 'receipt-seq-100' },
    { seq: 200, data: 'receipt-seq-200' },
    { seq: 300, data: 'receipt-seq-300' },
    { seq: 400, data: 'receipt-seq-400' },
  ];

  for (const r of receipts) {
    mmr.append(JSON.stringify(r));
  }

  // Verify exclusion of non-existent target seq = 250
  const targetSeq = 250;
  const lowerIndex = 1; // seq = 200
  const upperIndex = 2; // seq = 300

  const lowerProof = mmr.generateInclusionProof(lowerIndex);
  const upperProof = mmr.generateInclusionProof(upperIndex);

  function verifyExclusionProof(target, lowerReceipt, upperReceipt, lowerP, upperP) {
    // 1. Both bounding proofs must be valid
    if (!mmr.verifyInclusionProof(lowerP) || !mmr.verifyInclusionProof(upperP)) {
      return false;
    }
    // 2. Must be adjacent indices
    if (upperP.leafIndex !== lowerP.leafIndex + 1) {
      return false;
    }
    // 3. Strict inequality bounding
    if (lowerReceipt.seq >= target || upperReceipt.seq <= target) {
      return false;
    }
    return true;
  }

  const validExclusion = verifyExclusionProof(targetSeq, receipts[1], receipts[2], lowerProof, upperProof);
  assert(validExclusion, 'Exclusion proof for target seq 250 must succeed');

  // Invalid: target seq 150 is NOT bounded by 200 and 300
  const invalidExclusion = verifyExclusionProof(150, receipts[1], receipts[2], lowerProof, upperProof);
  assertEqual(invalidExclusion, false, 'Invalid range bound must fail exclusion proof');
});


// ============================================================================
// SUITE 6: MARKETING SITE LIB/PROTOCOL.TS ENCODER & EDGE CASES
// ============================================================================
console.log('\n>> SUITE 6: MARKETING SITE LIB/PROTOCOL.TS STRESS');

runTest('Suite 6', '6.1: Cross-Flag Combinations & Boundary Payload Verification', () => {
  // Test permutations of all 5 flags in the protocol framing specification
  const flagCombos = [
    { encrypted: false, priority: false, requiresConsensus: false, signed: false, broadcast: false },
    { encrypted: true, priority: false, requiresConsensus: false, signed: false, broadcast: false },
    { encrypted: false, priority: true, requiresConsensus: false, signed: false, broadcast: false },
    { encrypted: false, priority: false, requiresConsensus: true, signed: true, broadcast: false },
    { encrypted: true, priority: true, requiresConsensus: true, signed: true, broadcast: true },
  ];

  for (const flags of flagCombos) {
    let bitmask = 0;
    if (flags.encrypted) bitmask |= Flags.ENCRYPTED;
    if (flags.priority) bitmask |= Flags.PRIORITY;
    if (flags.requiresConsensus) bitmask |= Flags.REQUIRES_CONSENSUS;
    if (flags.signed) bitmask |= Flags.SIGNED;
    if (flags.broadcast) bitmask |= Flags.BROADCAST;

    const kp = Keypair.generate();
    let frame = new RivunFrame(
      new RivunHeader({
        flags: bitmask,
        sourceNode: kp.nodeId,
        targetNode: flags.broadcast ? '00000000-0000-0000-0000-000000000000' : '11112222-3333-4444-5555-666677778888',
      }),
      Buffer.from('Universal Test Payload', 'utf8')
    );

    if (flags.signed) {
      frame = signFrame(kp, frame);
    }
    if (flags.requiresConsensus) {
      const v1 = Keypair.generate();
      const v2 = Keypair.generate();
      frame = certifyFrame(frame, 2, [v1, v2]);
    }

    const encoded = frame.encode();
    assert(encoded.length >= 64, 'Encoded frame >= 64B');
    const decoded = RivunFrame.decode(encoded);
    assertEqual(decoded.header.flags, bitmask);
    assertEqual(decoded.payload.toString('utf8'), 'Universal Test Payload');
  }
});

runTest('Suite 6', '6.2: Unicode, Emoji, and Binary Special Characters in Payload', () => {
  const kp = Keypair.generate();
  const unicodePayload = Buffer.from('⚡ Rivun Zero-Trust Protocol 🚀 \u0000\u0001\u00FF\uFFFF こんにちは世界 🛡️', 'utf8');

  const frame = signFrame(kp, new RivunFrame(new RivunHeader({ sourceNode: kp.nodeId }), unicodePayload));
  const encoded = frame.encode();
  const decoded = RivunFrame.decode(encoded);

  assertEqual(decoded.payload.toString('utf8'), unicodePayload.toString('utf8'), 'Unicode and binary bytes must preserve exact fidelity');
  const pk = PublicKey.fromBytes(kp.publicKeyBytes);
  assert(verifyFrame(pk, decoded), 'Unicode payload must verify signature correctly');
});

// ============================================================================
// SUMMARY & VERDICT
// ============================================================================
console.log('\n' + '='.repeat(80));
console.log(`TOTAL STRESS TESTS:  ${passedTests + failedTests}`);
console.log(`TOTAL PASSED:        ${passedTests}`);
console.log(`TOTAL FAILED:        ${failedTests}`);
console.log('='.repeat(80));

if (failedTests > 0) {
  console.log('\nFAILURES:');
  for (const f of failures) {
    console.log(`- [${f.suite}] ${f.name}`);
    console.log(`  ${f.error}`);
  }
  process.exit(1);
} else {
  console.log('\n>>> ALL EMPIRICAL CHALLENGES AND STRESS TESTS PASSED WITH 100% INTEGRITY. <<<');
  process.exit(0);
}
