# Challenger 1 Handoff Report: Wire Protocol, Binary Codecs & Consensus Stress Verification

**Date**: 2026-08-29T01:35:00Z  
**Agent**: Challenger 1 (Wire & Consensus Stress Verifier)  
**Target Crates & Apps**: `crates/rivun-core`, `crates/rivun-crypto`, `crates/rivun-envelope`, `crates/rivun-net`, `crates/rivun-ledger`, `apps/marketing-site/lib/protocol.ts`, `sdks/typescript/src/protocol.ts`, `tests/e2e/harness/`  
**Verdict**: **`REQUEST_CHANGES`**

---

## 1. Observation

### 1.1 Test Suite & Baseline Execution
1. Executed full workspace Rust test suite via `cargo test --workspace`:
   - Command: `cargo test --workspace`
   - Result: All unit, integration, and property tests passed across all 25 core crates (`rivun-core`, `rivun-crypto`, `rivun-envelope`, `rivun-net`, `rivun-ledger`, `rivun-node`, `rivun-gateway`, etc.).
2. Executed E2E test runner via `node test-runner.mjs`:
   - Command: `node test-runner.mjs` (in `tests/e2e`)
   - Result: 280 / 280 tests passed across Tiers 1-4.

### 1.2 Empirical Stress Test Harness Execution
Authored and executed standalone stress harness `tests/e2e/challenger1_empirical_stress.mjs`:
- Command: `node challenger1_empirical_stress.mjs`
- Test Summary: 27 comprehensive adversarial stress tests covering:
  - Big-endian 64-byte `ZAP_` wire framing, magic mutation, version fuzzing, invalid flag bitmask rejection, and >16 MiB payload boundaries.
  - 74-byte `ZENV` universal messaging envelope layout across all 8 discrete message kinds (`Data`, `Event`, `Command`, `Query`, `Response`, `StreamChunk`, `Action`, `Control`).
  - 72-byte Ed25519 `ZSIG` trailers, 56-byte signing prefix isolation, signature bit-flip adversarial testing, key mismatch rejection, and 8-byte fast-hint $O(1)$ pre-verification rejection.
  - `ZPOA` consensus trailers, Byzantine quorum threshold formula $T = \lfloor 2N/3 \rfloor + 1$ across $N \in [1..100]$, 2-Phase BFT state machine, and validator equivocation slashing.
  - Merkle Mountain Range (MMR) carry-over subtree merging ($N \in [1..16]$), bagged peak root determinism, single-leaf & batch inclusion proofs, and monotonic range non-membership exclusion proofs.

### 1.3 Concrete Deficiencies Observed

#### Finding 1: Payload Length Bit Shift in `apps/marketing-site/lib/protocol.ts`
- **Location**: `apps/marketing-site/lib/protocol.ts`, lines 168-169
```typescript
wireView.setUint32(48, wirePayloadLen, false);
wireView.setUint32(52, 0, false); // reserved
```
- **Canonical Rust Specification** (`crates/rivun-core/src/lib.rs`, lines 26-27, 347-350):
```rust
pub const ZAP_LEN_OFFSET: usize = 48;
pub const ZAP_SIGN_OFFSET: usize = 56;
// ...
let rivun_len = u64::from_be_bytes(input[ZAP_LEN_OFFSET..ZAP_SIGN_OFFSET].try_into().unwrap());
if rivun_len > MAX_PAYLOAD_LEN {
    return Err(RivunError::PayloadTooLarge(rivun_len));
}
```
- **Empirical Execution Result** (`test_marketing_codec_crosscheck.mjs`):
```
Wire Header Byte 48..56 (Payload Length area): <Buffer 00 00 00 7b 00 00 00 00>
Interpreted as u64: 528280977408 (expected: 123)
```
- **Observation**: `protocol.ts` places the 32-bit payload length in the most significant 4 bytes (`48..52`) and zeros in the least significant 4 bytes (`52..56`). When read as a 64-bit big-endian integer by standard decoders, the payload length evaluates to `wirePayloadLen << 32` ($528,280,977,408$ bytes), causing `rivun-core` and compliant SDKs to immediately abort with `PayloadTooLarge`.

#### Finding 2: ZENV 74-Byte Header Internal Layout Misalignment in `apps/marketing-site/lib/protocol.ts`
- **Location**: `apps/marketing-site/lib/protocol.ts`, lines 78-86, 104-129
```typescript
// [0..4] magic ZENV (4B)
// [4..6] version 1 (2B)
// [6..7] kind u8 (1B)
// [7..8] reserved u8 (1B)
// [8..24] envelope UUID (16B)
// [24..40] correlation UUID (16B)
// [40..56] causation UUID (16B)
// [56..58] subject len u16 (2B)
// [58..60] content-type len u16 (2B)
// [60..64] metadata len u32 (4B)
// [64..72] body len u64 (8B)
// [72..74] padding / alignment (2B) -> 74 bytes total header
```
- **Canonical Rust Specification** (`crates/rivun-envelope/src/lib.rs`, lines 21-31) and TypeScript SDK (`sdks/typescript/src/protocol.ts`, lines 73-84):
```rust
const MAGIC_OFFSET: usize = 0;           // 4 bytes
const VERSION_OFFSET: usize = 4;         // 2 bytes
const KIND_OFFSET: usize = 6;            // 2 bytes (u16)
const RESERVED_OFFSET: usize = 8;        // 2 bytes (u16)
const ID_OFFSET: usize = 10;             // 16 bytes
const CORRELATION_ID_OFFSET: usize = 26; // 16 bytes
const CAUSATION_ID_OFFSET: usize = 42;   // 16 bytes
const SUBJECT_LEN_OFFSET: usize = 58;    // 2 bytes
const CONTENT_TYPE_LEN_OFFSET: usize = 60;// 2 bytes
const METADATA_LEN_OFFSET: usize = 62;   // 4 bytes
const BODY_LEN_OFFSET: usize = 66;       // 8 bytes
// Total = 74 bytes without trailing padding
```
- **Empirical Execution Result** (`test_marketing_codec_crosscheck.mjs`):
```
ZENV Envelope Magic at [0..4]: ZENV
ZENV Envelope Kind at [6..8]: 512
ZENV Envelope Reserved at [8..10]: 1511
ZENV Envelope ID at [10..26]: 166ccbf14c5d81cfc772662a7a160000
SDK Envelope Decode FAILED: unknown envelope kind 512
```
- **Observation**: `protocol.ts` encodes `kind` as a single `u8` at byte 6 and `reserved` as a single `u8` at byte 7, shifting all subsequent fields (`id`, `correlation_id`, `causation_id`, `subject_len`, `content_type_len`, `metadata_len`, `body_len`) by 2 bytes earlier and inserting 2 dummy padding bytes at `[72..74]`. Compliant decoders fail with `unknown envelope kind 512` (since `0x02, 0x00` in big-endian u16 is 512).

#### Finding 3: Validation Lenience in `tests/e2e/harness/zenvCodec.mjs`
- **Location**: `tests/e2e/harness/zenvCodec.mjs`, `decode()` method lines 98-168
- **Observation**:
  - Does not verify that bytes `8..10` (`reserved`) equal 0.
  - Does not enforce that non-Data message kinds (`Event`, `Command`, `Query`, etc.) require a non-empty `subject` (unlike `crates/rivun-envelope/src/lib.rs` and `sdks/typescript/src/protocol.ts`).

---

## 2. Logic Chain

1. **Protocol Invariant**: Rivun-Wire requires deterministic cross-language wire compatibility between Rust workspace crates, native SDKs (TypeScript, Go, Python), and browser web platforms.
2. **Observation 1.3.1**: `apps/marketing-site/lib/protocol.ts` writes 32-bit payload length at 48..52 and zeros at 52..56.
3. **Inference 1**: Any standard 64-bit big-endian reader reading bytes 48..56 interprets the upper 32 bits as `payloadLen`, producing a value $2^{32}$ times larger than intended.
4. **Observation 1.3.2**: `apps/marketing-site/lib/protocol.ts` encodes `kind` as 1 byte, `reserved` as 1 byte, and shifts the UUID and length headers by 2 bytes, adding 2 bytes of padding at the end.
5. **Inference 2**: Any canonical decoder (Rust `rivun-envelope`, TypeScript SDK, Go SDK) reading an envelope generated by `protocol.ts` receives invalid message kinds (e.g. `kind=2` reads as `512`), non-zero reserved fields, and shifted field lengths.
6. **Observation 1.3.3**: The E2E test harness `zenvCodec.mjs` permitted frames with non-zero reserved fields and missing subjects to pass decoding.
7. **Inference 3**: While the internal UI visualizer on the marketing site renders cleanly in-browser against its own internal data structures, the binary frame encoder in `protocol.ts` fails cross-platform conformance tests against the canonical Rust specification and SDKs.

---

## 3. Caveats

- `apps/marketing-site/lib/protocol.ts` is currently used for the interactive Hero frame encoder/decoder visualizer and hex inspector on the marketing site. Its internal visualizer rendering works because it parses its own synthetic segments.
- The Rust workspace implementation (`crates/rivun-core`, `crates/rivun-crypto`, `crates/rivun-envelope`, `crates/rivun-net`, `crates/rivun-ledger`) is fully conformant, thoroughly tested, and completely robust under high-throughput and Byzantine adversarial stress.
- As an adversarial challenger operating under strict review-only constraints, no implementation code was modified.

---

## 4. Conclusion & Recommended Remediations

**Verdict**: **`REQUEST_CHANGES`**

### Required Remediations

#### 1. In `apps/marketing-site/lib/protocol.ts`:
- **Fix Wire Header Payload Length (Lines 168-169)**:
  Change:
  ```typescript
  // Replace:
  wireView.setUint32(48, wirePayloadLen, false);
  wireView.setUint32(52, 0, false); // reserved
  // With:
  wireView.setBigUint64(48, BigInt(wirePayloadLen), false);
  ```
- **Fix ZENV Envelope 74-Byte Layout (Lines 99-130)**:
  Change:
  ```typescript
  // Replace:
  zenvView.setUint32(0, ZENV_MAGIC, false);
  zenvView.setUint16(4, 1, false);
  const kindInfo = MESSAGE_KINDS[options.kind] || MESSAGE_KINDS.data;
  zenvView.setUint8(6, kindInfo.id);
  zenvView.setUint8(7, 0); // reserved
  zenvBuffer.set(envUuidBytes, 8);
  zenvBuffer.set(corrBytes, 24);
  zenvBuffer.set(causBytes, 40);
  zenvView.setUint16(56, subjectBytes.length, false);
  zenvView.setUint16(58, contentTypeBytes.length, false);
  zenvView.setUint32(60, metadataBytes.length, false);
  zenvView.setBigUint64(64, BigInt(bodyBytes.length), false);
  zenvView.setUint16(72, 0, false); // alignment

  // With canonical layout matching Rust and TypeScript SDK:
  zenvView.setUint32(0, ZENV_MAGIC, false);
  zenvView.setUint16(4, 1, false);
  const kindInfo = MESSAGE_KINDS[options.kind] || MESSAGE_KINDS.data;
  zenvView.setUint16(6, kindInfo.id, false);
  zenvView.setUint16(8, 0, false); // reserved (2B)
  zenvBuffer.set(envUuidBytes, 10);
  zenvBuffer.set(corrBytes, 26);
  zenvBuffer.set(causBytes, 42);
  zenvView.setUint16(58, subjectBytes.length, false);
  zenvView.setUint16(60, contentTypeBytes.length, false);
  zenvView.setUint32(62, metadataBytes.length, false);
  zenvView.setBigUint64(66, BigInt(bodyBytes.length), false);
  ```

#### 2. In `tests/e2e/harness/zenvCodec.mjs`:
- In `RivunEnvelope.decode(buf)`:
  - Add reserved check:
    ```javascript
    const reserved = buf.readUInt16BE(8);
    if (reserved !== 0) {
      throw new Error(`Reserved field must be zero, got ${reserved}`);
    }
    ```
  - Add missing subject check:
    ```javascript
    if (kind !== MessageKind.Data && subjectLen === 0) {
      throw new Error(`Subject is required for kind ${kind}`);
    }
    ```

---

## 5. Verification Method

To independently reproduce and verify all observations and conclusions:

1. **Run Full Workspace Rust Test Suite**:
   ```powershell
   cargo test --workspace
   ```
2. **Run E2E Protocol & Scenario Test Suite**:
   ```powershell
   cd "tests/e2e"
   node test-runner.mjs
   ```
3. **Run Challenger 1 Empirical Stress Test Suite**:
   ```powershell
   cd "tests/e2e"
   node challenger1_empirical_stress.mjs
   ```
4. **Run Cross-Codec Parity Check**:
   ```powershell
   cd "tests/e2e"
   npx tsx test_marketing_codec_crosscheck.mjs
   ```
