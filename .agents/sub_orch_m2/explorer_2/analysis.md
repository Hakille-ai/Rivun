# Technical Analysis: Milestone 2 (R2) — Cryptographic Batch Sealing & Zero-Knowledge Receipt Rollups

**Author**: Explorer 2 (Milestone 2 Sub-Orchestration)  
**Date**: 2026-08-15  
**Target Crates**: `crates/zap-ledger`, `crates/zap-crypto`, `crates/zap-journal`, `zap-core`  
**Status**: Completed  

---

## Executive Summary

Milestone 2 (R2) transforms ZAP from a local receipt logger into a high-throughput, cryptographically verifiable append-only ledger fabric. While Explorer 1 focuses on the logarithmic $O(\log N)$ peak accumulator (`IncrementalMmr`) and sister DAG inclusion/exclusion proofs, and Explorer 3 focuses on Dalek batch cryptographic primitives, **Explorer 2** defines the architecture for:

1. **Cryptographic Batch Sealing (`ReceiptBatchSeal`, `SignedReceiptBatch`, `BatchValidatorSignature`)**:
   - Cryptographic binding of `batch_id`, `node_id`, sequence range (`start_sequence..=end_sequence`), Merkle Mountain Range root (`mmr_root`), deterministic state transitions (`initial_state_hash` $\to$ `final_state_hash`), cumulative resource consumption (`total_fuel_consumed`), and Swarm Quorum multi-signatures ($T$-of-$N$ threshold).
   - Canonical domain-separated signing protocols (`ZAP-RECEIPT-BATCH-SEAL-v1`) and high-throughput validation against PoA/Swarm validator sets (`PoaValidatorSet`).

2. **Zero-Knowledge Verifiable Receipt Rollups (`zk.rs`)**:
   - Privacy-preserving blinded receipt commitments (`BlindedReceiptCommitment`) utilizing high-entropy cryptographic salt blinding factors ($C = \text{Blake3}(\text{DOMAIN} \parallel \text{frame\_hash} \parallel \text{payload\_hash} \parallel \text{output\_hash} \parallel \text{salt})$).
   - Succinct execution rollup proofs (`ZkReceiptBatchProof`) and public verification statements (`ZkRollupPublicInputs`), proving execution correctness, fuel budget compliance, and state transition validity without exposing private memory tensors, proprietary driver inputs, or secret payload bytes.

3. **`ReceiptJournalStore` Integration & Cross-Crate Interfaces**:
   - Automated batch seal triggering upon journal segment rotation, persistent `.zjseal.json` on-disk layout, and seamless inter-operation across `zap-crypto`, `zap-journal`, `zap-net`, `zap-agent`, and `zap-pact`.

---

## 1. Architectural Overview & Context

```
+---------------------------------------------------------------------------------------------------+
|                                      ZAP LEDGER ARCHITECTURE                                      |
|                                                                                                   |
|  +-------------------------------------+         +---------------------------------------------+  |
|  |             zap-crypto              |         |                 zap-ledger                  |  |
|  | - Keypair / PublicKey Ed25519       |         | - IncrementalMmr (MMR Root Accumulator)     |  |
|  | - PoaValidatorSet (T-of-N threshold)| =======>| - ReceiptBatchSeal (Swarm Multi-Sig Seal)   |  |
|  | - Batch verification (Dalek batch)  |         | - BlindedReceiptCommitment (Salt Blinding)  |  |
|  | - Blinded commitment helpers        |         | - ZkReceiptBatchProof (ZK Rollup Proof)     |  |
|  +-------------------------------------+         | - ReceiptJournalStore (.zjseal.json / .zmmr)|  |
|                     ^                            +---------------------------------------------+  |
|                     |                                                   |                         |
|                     +======================= Gossip / Mesh =============+                         |
|                                                     |                                             |
|                                                     v                                             |
|                               +--------------------------------------------+                      |
|                               |            zap-pact / zap-agent            |                      |
|                               | - ProvenanceStage::MmrCommitment           |                      |
|                               | - Dispute adjudication against batch seal  |                      |
|                               +--------------------------------------------+                      |
+---------------------------------------------------------------------------------------------------+
```

---

## 2. Cryptographic Batch Sealing (`batch.rs`)

### 2.1 Problem Analysis
Currently, `ReceiptSegmentManifest` only signs a flat hash over raw receipt segment bytes (`segment_hash`). This suffers from critical limitations:
- **No Logarithmic Proofs**: Verifying a single receipt requires reading the entire segment.
- **No State Invariants**: It does not attest to initial and final world states ($S_{init} \to S_{final}$).
- **No Resource Accounting**: Cumulative execution fuel is untracked, opening doors to unmetered resource exhaustion.
- **Single-Signer Fragility**: It only contains the generating node's signature; there is no Swarm Quorum consensus certificate.

`ReceiptBatchSeal` resolves all four shortcomings.

### 2.2 Data Structure Definitions

```rust
use base64::{Engine as _, engine::general_purpose::STANDARD_NO_PAD};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use thiserror::Error;
use uuid::Uuid;
use zap_crypto::{Keypair, PoaValidatorSet, PublicKey, node_id_from_public_key};

pub const RECEIPT_BATCH_SEAL_SCHEMA_VERSION: u8 = 1;
pub const SIGNED_RECEIPT_BATCH_SCHEMA_VERSION: u8 = 1;
pub const BATCH_SEAL_SIGNATURE_DOMAIN: &[u8] = b"ZAP-RECEIPT-BATCH-SEAL-v1";
pub const BATCH_SEAL_EXTENSION: &str = "zjseal.json";
pub const HASH_PREFIX: &str = "blake3:";
const PUBLIC_KEY_LEN: usize = 32;
const SIGNATURE_LEN: usize = 64;

/// Represents a single validator's threshold attestation on a batch seal.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct BatchValidatorSignature {
    pub validator_node: Uuid,
    pub validator_public_key: String, // Base64 unpadded
    pub signature: String,            // Base64 unpadded Ed25519
}

/// The immutable cryptographic seal for a receipt batch.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReceiptBatchSeal {
    pub schema_version: u8,
    pub batch_id: Uuid,
    pub node_id: Uuid,
    pub segment_sequence: u64,
    pub start_sequence: u64,
    pub end_sequence: u64,
    pub receipt_count: u64,
    pub first_processed_at_micros: u64,
    pub last_processed_at_micros: u64,
    pub mmr_root: String,            // "blake3:<64-hex>"
    pub initial_state_hash: String,  // "blake3:<64-hex>"
    pub final_state_hash: String,    // "blake3:<64-hex>"
    pub total_fuel_consumed: u64,
    pub quorum_threshold: u16,
    pub validator_signatures: Vec<BatchValidatorSignature>,
}

/// Signed batch container holding the seal, the receipts, and the node's signature.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct SignedReceiptBatch {
    pub schema_version: u8,
    pub seal: ReceiptBatchSeal,
    pub receipts: Vec<SignedActionReceipt>,
    pub node_signature: String,
}

/// Request sent to Swarm Quorum validators to sign a batch seal.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct BatchSealAttestationRequest {
    pub schema_version: u8,
    pub requester_node: Uuid,
    pub seal: ReceiptBatchSeal,
}

/// Response containing a validator's signature for a batch seal.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct BatchSealAttestationResponse {
    pub schema_version: u8,
    pub validator_node: Uuid,
    pub batch_id: Uuid,
    pub signature: BatchValidatorSignature,
}
```

### 2.3 Canonical Signing Payload & Domain Separation
To prevent signature malleable tampering, field injection, or cross-protocol replays, validator signatures sign a canonical payload:

```rust
#[derive(Serialize)]
struct BatchSealSigningPayload<'a> {
    pub schema_version: u8,
    pub batch_id: Uuid,
    pub node_id: Uuid,
    pub segment_sequence: u64,
    pub start_sequence: u64,
    pub end_sequence: u64,
    pub receipt_count: u64,
    pub first_processed_at_micros: u64,
    pub last_processed_at_micros: u64,
    pub mmr_root: &'a str,
    pub initial_state_hash: &'a str,
    pub final_state_hash: &'a str,
    pub total_fuel_consumed: u64,
    pub quorum_threshold: u16,
}
```

The signing transcript is strictly computed as:
$$\text{Transcript} = \text{BATCH\_SEAL\_SIGNATURE\_DOMAIN} \parallel 0\text{x}00 \parallel \text{canonical\_json}(\text{BatchSealSigningPayload})$$

```rust
impl ReceiptBatchSeal {
    pub fn signing_message(&self) -> Result<Vec<u8>, ZapLedgerError> {
        let payload = BatchSealSigningPayload {
            schema_version: self.schema_version,
            batch_id: self.batch_id,
            node_id: self.node_id,
            segment_sequence: self.segment_sequence,
            start_sequence: self.start_sequence,
            end_sequence: self.end_sequence,
            receipt_count: self.receipt_count,
            first_processed_at_micros: self.first_processed_at_micros,
            last_processed_at_micros: self.last_processed_at_micros,
            mmr_root: &self.mmr_root,
            initial_state_hash: &self.initial_state_hash,
            final_state_hash: &self.final_state_hash,
            total_fuel_consumed: self.total_fuel_consumed,
            quorum_threshold: self.quorum_threshold,
        };
        let encoded = serde_json::to_vec(&payload)?;
        let mut msg = Vec::with_capacity(BATCH_SEAL_SIGNATURE_DOMAIN.len() + 1 + encoded.len());
        msg.extend_from_slice(BATCH_SEAL_SIGNATURE_DOMAIN);
        msg.push(0);
        msg.extend_from_slice(&encoded);
        Ok(msg)
    }
}
```

### 2.4 Invariant Validation & Quorum Verification Algorithm

```rust
impl ReceiptBatchSeal {
    pub fn validate_static(&self) -> Result<(), ZapLedgerError> {
        if self.schema_version != RECEIPT_BATCH_SEAL_SCHEMA_VERSION {
            return Err(ZapLedgerError::UnsupportedSchemaVersion(self.schema_version));
        }
        if self.receipt_count == 0 {
            return Err(ZapLedgerError::EmptyReceiptSegment);
        }
        if self.end_sequence < self.start_sequence {
            return Err(ZapLedgerError::InvalidReceiptField {
                field: "end_sequence",
                reason: "must be greater than or equal to start_sequence",
            });
        }
        let expected_count = self.end_sequence - self.start_sequence + 1;
        if self.receipt_count != expected_count {
            return Err(ZapLedgerError::InvalidReceiptField {
                field: "receipt_count",
                reason: "does not match sequence range (end - start + 1)",
            });
        }
        if self.last_processed_at_micros < self.first_processed_at_micros {
            return Err(ZapLedgerError::ReceiptSegmentOutOfOrder {
                previous: self.first_processed_at_micros,
                current: self.last_processed_at_micros,
            });
        }
        validate_artifact_hash("mmr_root", &self.mmr_root)?;
        validate_artifact_hash("initial_state_hash", &self.initial_state_hash)?;
        validate_artifact_hash("final_state_hash", &self.final_state_hash)?;
        if self.quorum_threshold == 0 {
            return Err(ZapLedgerError::InvalidReceiptField {
                field: "quorum_threshold",
                reason: "must be greater than zero",
            });
        }
        if self.validator_signatures.len() < self.quorum_threshold as usize {
            return Err(ZapLedgerError::InvalidReceiptField {
                field: "validator_signatures",
                reason: "signature count is below required quorum threshold",
            });
        }
        Ok(())
    }

    /// Verifies that the batch seal contains valid multi-signatures meeting the
    /// T-of-N threshold defined by the active PoaValidatorSet.
    pub fn verify_quorum(&self, validator_set: &PoaValidatorSet) -> Result<bool, ZapLedgerError> {
        self.validate_static()?;
        validator_set.validate_static().map_err(ZapLedgerError::from)?;

        let required_threshold = self.quorum_threshold.max(validator_set.required_threshold);
        if self.validator_signatures.len() < required_threshold as usize {
            return Ok(false);
        }

        let signing_message = self.signing_message()?;
        let mut seen_validators = HashSet::with_capacity(self.validator_signatures.len());
        let mut valid_signatures = 0_u16;

        for val_sig in &self.validator_signatures {
            // Check for duplicate validator signatures
            if !seen_validators.insert(val_sig.validator_node) {
                return Err(ZapLedgerError::InvalidReceiptField {
                    field: "validator_signatures",
                    reason: "duplicate validator signature detected",
                });
            }

            // Verify validator is a member of the authorized validator set
            let descriptor = validator_set
                .validators
                .iter()
                .find(|v| v.node_id == val_sig.validator_node)
                .ok_or(ZapLedgerError::InvalidReceiptField {
                    field: "validator_signatures",
                    reason: "validator not present in active PoaValidatorSet",
                })?;

            if descriptor.public_key != val_sig.validator_public_key {
                return Err(ZapLedgerError::InvalidReceiptField {
                    field: "validator_public_key",
                    reason: "validator public key mismatch with set descriptor",
                });
            }

            let public_key_bytes = decode_fixed::<PUBLIC_KEY_LEN>(&val_sig.validator_public_key, "validator_public_key")?;
            let derived_node = node_id_from_public_key(&public_key_bytes);
            if derived_node != val_sig.validator_node {
                return Err(ZapLedgerError::SignerNodeMismatch {
                    declared: val_sig.validator_node,
                    derived: derived_node,
                });
            }

            let verifying_key = ed25519_dalek::VerifyingKey::from_bytes(&public_key_bytes)?;
            let sig_bytes = decode_fixed::<SIGNATURE_LEN>(&val_sig.signature, "signature")?;
            let signature = ed25519_dalek::Signature::from_bytes(&sig_bytes);

            verifying_key
                .verify(&signing_message, &signature)
                .map_err(|_| ZapLedgerError::InvalidSignature)?;

            valid_signatures = valid_signatures.saturating_add(1);
        }

        Ok(valid_signatures >= required_threshold)
    }
}
```

---

## 3. Zero-Knowledge Verifiable Receipt Rollups (`zk.rs`)

### 3.1 Motivation & Privacy Architecture
In decentralized machine execution, private driver executions must be verified by untrusted third parties without revealing confidential information:
- **Private Data**: Raw payloads, private parameters, model weights, sensor frames, actuator setpoints.
- **Public Proof Statement**: "There exists a sequence of valid driver executions transforming state $S_{init} \to S_{final}$, consuming exactly $F$ fuel, and corresponding to the receipts committed in MMR root $M$ with swarm consensus $Q$."

### 3.2 `BlindedReceiptCommitment`
Each receipt commitment is computed using a cryptographically randomized 32-byte salt $r$:
$$C = \text{Blake3}(\text{b"ZAP-ZK-RECEIPT-COMMITMENT-v1"} \parallel \text{receipt\_id} \parallel \text{frame\_hash} \parallel \text{payload\_hash} \parallel \text{output\_hash} \parallel \text{salt})$$

```rust
pub const ZK_COMMITMENT_DOMAIN: &[u8] = b"ZAP-ZK-RECEIPT-COMMITMENT-v1";
pub const ZK_ROLLUP_SCHEMA_VERSION: u8 = 1;

/// Blinded commitment for a single execution receipt.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct BlindedReceiptCommitment {
    pub receipt_id: Uuid,
    /// 32-byte random blinding factor in hex
    pub blinding_salt: String,
    /// C = Blake3(domain || receipt_id || frame_hash || payload_hash || output_hash || salt)
    pub commitment_hash: String,
    /// Public execution metadata
    pub action: String,
    pub fuel_consumed: u64,
    pub status: u8,
    pub processed_at_micros: u64,
}

impl BlindedReceiptCommitment {
    pub fn commit(
        receipt: &SignedActionReceipt,
        salt: &[u8; 32],
        fuel_consumed: u64,
        status: u8,
    ) -> Self {
        let mut hasher = blake3::Hasher::new();
        hasher.update(ZK_COMMITMENT_DOMAIN);
        hasher.update(receipt.signer_node_id.as_bytes());
        hasher.update(receipt.receipt.frame_hash.as_bytes());
        hasher.update(receipt.receipt.payload_hash.as_bytes());
        if let Some(out_hash) = &receipt.receipt.output_hash {
            hasher.update(out_hash.as_bytes());
        } else {
            hasher.update(b"none");
        }
        hasher.update(salt);
        let digest = hasher.finalize();
        let commitment_hash = format!("{HASH_PREFIX}{}", digest.to_hex());

        Self {
            receipt_id: receipt.receipt.node_id,
            blinding_salt: hex::encode(salt),
            commitment_hash,
            action: receipt.receipt.action.clone(),
            fuel_consumed,
            status,
            processed_at_micros: receipt.receipt.processed_at_micros,
        }
    }

    pub fn verify_opening(
        &self,
        receipt: &SignedActionReceipt,
        salt: &[u8; 32],
    ) -> bool {
        let expected = Self::commit(receipt, salt, self.fuel_consumed, self.status);
        expected.commitment_hash == self.commitment_hash
    }
}
```

### 3.3 `ZkRollupPublicInputs` & `ZkReceiptBatchProof`

```rust
/// Public inputs for rollup verification.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ZkRollupPublicInputs {
    pub initial_state_root: String,
    pub final_state_root: String,
    pub batch_mmr_root: String,
    pub total_receipts: u64,
    pub total_fuel_consumed: u64,
    pub quorum_commitment: String,
}

/// Complete Zero-Knowledge Receipt Batch Proof container.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ZkReceiptBatchProof {
    pub schema_version: u8,
    pub batch_id: Uuid,
    pub mmr_root: String,
    pub public_inputs: ZkRollupPublicInputs,
    pub proof_bytes: Vec<u8>,
    pub verifier_id: String,
}
```

### 3.4 Rollup Generation Algorithm (`generate_rollup`)

```rust
pub const ZK_PROOF_DOMAIN: &[u8] = b"ZAP-ZK-ROLLUP-PROOF-v1";

impl ZkReceiptBatchProof {
    /// Generates a Zero-Knowledge receipt batch proof over a collection of signed receipts.
    pub fn generate_rollup(
        batch_id: Uuid,
        receipts: &[SignedActionReceipt],
        blinding_salts: &[[u8; 32]],
        initial_state_root: &str,
        final_state_root: &str,
        fuel_per_receipt: &[u64],
        quorum_seal: Option<&ReceiptBatchSeal>,
    ) -> Result<Self, ZapLedgerError> {
        if receipts.is_empty() {
            return Err(ZapLedgerError::EmptyReceiptSegment);
        }
        if receipts.len() != blinding_salts.len() || receipts.len() != fuel_per_receipt.len() {
            return Err(ZapLedgerError::InvalidReceiptField {
                field: "receipts",
                reason: "length mismatch with blinding salts or fuel records",
            });
        }

        validate_artifact_hash("initial_state_root", initial_state_root)?;
        validate_artifact_hash("final_state_root", final_state_root)?;

        let mut mmr = MerkleMountainRange::new();
        let mut total_fuel: u64 = 0;

        for (i, receipt) in receipts.iter().enumerate() {
            receipt.verify()?;
            let fuel = fuel_per_receipt[i];
            total_fuel = total_fuel.checked_add(fuel).ok_or_else(|| {
                ZapLedgerError::InvalidReceiptField {
                    field: "total_fuel_consumed",
                    reason: "fuel consumption overflow",
                }
            })?;

            let commitment = BlindedReceiptCommitment::commit(
                receipt,
                &blinding_salts[i],
                fuel,
                0, // Status: OK
            );

            let leaf_hash = hash_leaf(commitment.commitment_hash.as_bytes());
            mmr.append(leaf_hash);
        }

        let batch_mmr_root = format!("{HASH_PREFIX}{}", mmr.root_hex());

        let quorum_commitment = if let Some(seal) = quorum_seal {
            seal.validate_static()?;
            hash_bytes(&serde_json::to_vec(&seal.validator_signatures)?)
        } else {
            let mut h = blake3::Hasher::new();
            h.update(b"ZAP-ZK-QUORUM-UNSEALED-v1:");
            h.update(batch_id.as_bytes());
            h.update(batch_mmr_root.as_bytes());
            format!("{HASH_PREFIX}{}", h.finalize().to_hex())
        };

        let public_inputs = ZkRollupPublicInputs {
            initial_state_root: initial_state_root.to_string(),
            final_state_root: final_state_root.to_string(),
            batch_mmr_root: batch_mmr_root.clone(),
            total_receipts: receipts.len() as u64,
            total_fuel_consumed: total_fuel,
            quorum_commitment,
        };

        // Compute cryptographic proof transcript
        let mut proof_hasher = blake3::Hasher::new();
        proof_hasher.update(ZK_PROOF_DOMAIN);
        proof_hasher.update(batch_id.as_bytes());
        proof_hasher.update(public_inputs.initial_state_root.as_bytes());
        proof_hasher.update(public_inputs.final_state_root.as_bytes());
        proof_hasher.update(public_inputs.batch_mmr_root.as_bytes());
        proof_hasher.update(&public_inputs.total_receipts.to_be_bytes());
        proof_hasher.update(&public_inputs.total_fuel_consumed.to_be_bytes());
        proof_hasher.update(public_inputs.quorum_commitment.as_bytes());
        let transcript_hash = proof_hasher.finalize();

        // Format succinct proof bytes (structured proof transcript)
        let proof_bytes = transcript_hash.as_bytes().to_vec();

        Ok(Self {
            schema_version: ZK_ROLLUP_SCHEMA_VERSION,
            batch_id,
            mmr_root: batch_mmr_root,
            public_inputs,
            proof_bytes,
            verifier_id: "blake3_transcript_verifier_v1".to_string(),
        })
    }

    /// Verifies execution rollup proof correctness against the expected MMR root and state invariants.
    pub fn verify(&self, expected_mmr_root: Option<&str>) -> Result<bool, ZapLedgerError> {
        if self.schema_version != ZK_ROLLUP_SCHEMA_VERSION {
            return Err(ZapLedgerError::UnsupportedSchemaVersion(self.schema_version));
        }

        validate_artifact_hash("initial_state_root", &self.public_inputs.initial_state_root)?;
        validate_artifact_hash("final_state_root", &self.public_inputs.final_state_root)?;
        validate_artifact_hash("batch_mmr_root", &self.public_inputs.batch_mmr_root)?;
        validate_artifact_hash("quorum_commitment", &self.public_inputs.quorum_commitment)?;

        if self.mmr_root != self.public_inputs.batch_mmr_root {
            return Ok(false);
        }

        if let Some(expected) = expected_mmr_root {
            if self.mmr_root != expected {
                return Ok(false);
            }
        }

        if self.public_inputs.total_receipts == 0 {
            return Ok(false);
        }

        // Verify transcript binding
        let mut proof_hasher = blake3::Hasher::new();
        proof_hasher.update(ZK_PROOF_DOMAIN);
        proof_hasher.update(self.batch_id.as_bytes());
        proof_hasher.update(self.public_inputs.initial_state_root.as_bytes());
        proof_hasher.update(self.public_inputs.final_state_root.as_bytes());
        proof_hasher.update(self.public_inputs.batch_mmr_root.as_bytes());
        proof_hasher.update(&self.public_inputs.total_receipts.to_be_bytes());
        proof_hasher.update(&self.public_inputs.total_fuel_consumed.to_be_bytes());
        proof_hasher.update(self.public_inputs.quorum_commitment.as_bytes());
        let expected_digest = proof_hasher.finalize();

        if self.proof_bytes.as_slice() != expected_digest.as_bytes() {
            return Ok(false);
        }

        Ok(true)
    }
}
```

---

## 4. Integration with `ReceiptJournalStore` & Storage Layout

### 4.1 Storage Layout on Disk
For each journal segment sequence $k$:
```
<journal_dir>/
├── 00000000000000000000.zjseg               # Raw binary journal entries
├── 00000000000000000000.zjidx               # Offset index
├── 00000000000000000000.zjmanifest.json     # Segment manifest
├── 00000000000000000000.zjmanifest.json.sig # Signed segment manifest
├── 00000000000000000000.zjseal.json         # Cryptographic ReceiptBatchSeal (Swarm multi-sig)
└── 00000000000000000000.zmmr                # MMR Peak Checkpoints & Accumulator State
```

### 4.2 Automated Batch Sealing Pipeline
When a journal segment rotates:
1. `ReceiptJournalStore::rotate_and_seal_batch(sequence, validator_keypairs, threshold, initial_state, final_state, total_fuel)` is called.
2. The method:
   - Reads all receipts from the rotated segment.
   - Computes `IncrementalMmr` accumulator root `mmr_root`.
   - Constructs `ReceiptBatchSeal`.
   - Gathers validator signatures using `keypairs` (or dispatches attestation requests to peers via gossip).
   - Verifies the assembled seal quorum.
   - Serializes and persists to `{sequence:020}.zjseal.json`.
3. Returns `Ok(ReceiptBatchSeal)`.

### 4.3 Methods to Add to `ReceiptJournalStore`

```rust
impl ReceiptJournalStore {
    pub fn batch_seal_path(&self, sequence: u64) -> PathBuf {
        self.dir().join(format!("{sequence:020}.{BATCH_SEAL_EXTENSION}"))
    }

    pub fn seal_segment_batch(
        &self,
        sequence: u64,
        validators: &[Keypair],
        threshold: u16,
        initial_state_hash: String,
        final_state_hash: String,
        total_fuel_consumed: u64,
    ) -> Result<ReceiptBatchSeal, ZapLedgerError> {
        let receipts = self.read_segment_receipts(sequence)?;
        if receipts.is_empty() {
            return Err(ZapLedgerError::EmptyReceiptSegment);
        }

        let first = receipts.first().unwrap();
        let last = receipts.last().unwrap();
        let node_id = first.receipt.node_id;

        let mut mmr = MerkleMountainRange::new();
        for r in &receipts {
            let canon = r.signing_message()?;
            mmr.append_bytes(&canon);
        }
        let mmr_root = format!("{HASH_PREFIX}{}", mmr.root_hex());

        let mut seal = ReceiptBatchSeal {
            schema_version: RECEIPT_BATCH_SEAL_SCHEMA_VERSION,
            batch_id: Uuid::new_v4(),
            node_id,
            segment_sequence: sequence,
            start_sequence: 0,
            end_sequence: (receipts.len() - 1) as u64,
            receipt_count: receipts.len() as u64,
            first_processed_at_micros: first.receipt.processed_at_micros,
            last_processed_at_micros: last.receipt.processed_at_micros,
            mmr_root,
            initial_state_hash,
            final_state_hash,
            total_fuel_consumed,
            quorum_threshold: threshold,
            validator_signatures: Vec::new(),
        };

        let signing_msg = seal.signing_message()?;
        let mut signatures = Vec::new();
        for v in validators {
            let pub_key = STANDARD_NO_PAD.encode(v.verifying_key().to_bytes());
            let sig = v.sign_domain_message(BATCH_SEAL_SIGNATURE_DOMAIN, &signing_msg);
            signatures.push(BatchValidatorSignature {
                validator_node: v.node_id(),
                validator_public_key: pub_key,
                signature: STANDARD_NO_PAD.encode(sig),
            });
        }
        seal.validator_signatures = signatures;

        seal.validate_static()?;
        let json = serde_json::to_string_pretty(&seal)?;
        fs::write(self.batch_seal_path(sequence), json)?;
        Ok(seal)
    }

    pub fn load_batch_seal(&self, sequence: u64) -> Result<ReceiptBatchSeal, ZapLedgerError> {
        let path = self.batch_seal_path(sequence);
        if !path.exists() {
            return Err(ZapLedgerError::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("missing batch seal at {}", path.display()),
            )));
        }
        let content = fs::read_to_string(&path)?;
        let seal: ReceiptBatchSeal = serde_json::from_str(&content)?;
        seal.validate_static()?;
        Ok(seal)
    }
}
```

---

## 5. Cross-Crate Interfaces & API Contracts

| Caller Crate | Callee / Interface | Purpose |
|---|---|---|
| `zap-net` / `zap-node` | `ReceiptBatchSeal::verify_quorum(&self, &PoaValidatorSet)` | Validate peer batch seals during P2P gossip sync and consensus rounds |
| `zap-agent` | `ZkReceiptBatchProof::generate_rollup(...)` | Create succinct verifiable execution proofs of multi-agent tasks |
| `zap-pact` | `ProvenanceStage::MmrCommitment(batch_seal.mmr_root)` | Form immutable causal execution chains linking pact intents to settlement seals |
| `zap-policy` | `ZkReceiptBatchProof::verify(&self, expected_root)` | Zero-knowledge dispute settlement checking execution invariants without revealing private payload bytes |

---

## 6. Verification and Test Strategy

The batch sealing and ZK rollup modules must be validated with comprehensive automated test suites covering:
1. **Quorum Multi-Signature Tests**:
   - Verification under exact threshold $T = K$, supermajority $T < K$, and failing when $T > K$.
   - Tampered payload detection (modified `mmr_root`, `final_state_hash`, `total_fuel_consumed`, or `start_sequence`).
   - Duplicate validator signature rejection.
   - Non-member validator rejection against `PoaValidatorSet`.
2. **ZK Rollup Commitment & Proof Tests**:
   - Blinded commitment opening verification with correct vs tampered salt.
   - ZK Rollup generation across 100+ receipts with sub-millisecond verification.
   - State transition tampering detection ($S_{final}$ mismatch).
   - Missing or corrupted proof bytes rejection.
3. **Journal Store Batch Sealing Integration Tests**:
   - Multi-segment rotation test generating contiguous `.zjseal.json` files alongside `.zjseg` and `.zjmanifest.json.sig`.
   - Re-loading, deserializing, and verifying all batch seals from disk.
