//! Cryptographic Provenance Chain Engine
//!
//! Enforces non-repudiable causal linkage across all phases of AI agent execution:
//! $H_{\text{intent}} \to H_{\text{negotiation}} \to H_{\text{policy}} \to H_{\text{driver}} \to H_{\text{poa}} \to H_{\text{receipt}} \to H_{\text{root}}$
//! signed with the node's Ed25519 identity key.

use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use uuid::Uuid;
use zap_core::now_micros;
use zap_crypto::{Keypair, PublicKey, ZapCryptoError};

use crate::{AgentIntent, Result, Validate, ZapAgentError};

pub const PROVENANCE_SCHEMA_VERSION: u8 = 1;
pub const PROVENANCE_SIGNATURE_DOMAIN: &[u8] = b"ZAP-PROVENANCE-CHAIN-v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProvenanceStage {
    Intent,
    Negotiation,
    Policy,
    Consensus,
    Driver,
    Poa,
    Receipt,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProvenanceStep {
    pub stage: ProvenanceStage,
    pub step_hash: String,
    pub previous_hash: Option<String>,
    pub input_data_hash: String,
    pub timestamp_micros: u64,
    #[serde(default)]
    pub metadata: BTreeMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProvenanceChainDigest {
    pub schema_version: u8,
    pub chain_id: Uuid,
    pub session_id: Uuid,
    pub intent_id: Uuid,
    pub steps: Vec<ProvenanceStep>,
    pub root_hash: String,
    pub node_id: Uuid,
    pub signature: String,
    pub created_at_micros: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProvenanceVerificationReport {
    pub valid: bool,
    pub chain_id: Uuid,
    pub root_hash: String,
    pub node_id: Uuid,
    pub verified_steps: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failed_stage: Option<ProvenanceStage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failure_reason: Option<String>,
}

pub struct ProvenanceChainBuilder {
    chain_id: Uuid,
    session_id: Uuid,
    intent_id: Uuid,
    intent_step: Option<ProvenanceStep>,
    negotiation_step: Option<ProvenanceStep>,
    policy_step: Option<ProvenanceStep>,
    consensus_step: Option<ProvenanceStep>,
    driver_step: Option<ProvenanceStep>,
    poa_step: Option<ProvenanceStep>,
    receipt_step: Option<ProvenanceStep>,
}

impl ProvenanceChainBuilder {
    #[must_use]
    pub fn new(session_id: Uuid, intent_id: Uuid) -> Self {
        Self {
            chain_id: Uuid::new_v4(),
            session_id,
            intent_id,
            intent_step: None,
            negotiation_step: None,
            policy_step: None,
            consensus_step: None,
            driver_step: None,
            poa_step: None,
            receipt_step: None,
        }
    }

    pub fn with_intent(mut self, intent: &AgentIntent) -> Result<Self> {
        intent.validate()?;
        let canonical_json = serde_json::to_vec(intent)?;
        let mut hasher = Sha256::new();
        hasher.update(&canonical_json);
        let input_hash = hex::encode(hasher.finalize());

        let step_hash = input_hash.clone();

        let mut metadata = BTreeMap::new();
        metadata.insert(
            "source_agent".to_string(),
            serde_json::Value::String(intent.source_agent.to_string()),
        );
        metadata.insert(
            "objective".to_string(),
            serde_json::Value::String(intent.objective.clone()),
        );

        self.intent_step = Some(ProvenanceStep {
            stage: ProvenanceStage::Intent,
            step_hash,
            previous_hash: None,
            input_data_hash: input_hash,
            timestamp_micros: now_micros().unwrap_or(0),
            metadata,
        });

        Ok(self)
    }

    pub fn with_intent_hash(
        mut self,
        intent_hash: &str,
        metadata: BTreeMap<String, serde_json::Value>,
    ) -> Self {
        self.intent_step = Some(ProvenanceStep {
            stage: ProvenanceStage::Intent,
            step_hash: intent_hash.to_string(),
            previous_hash: None,
            input_data_hash: intent_hash.to_string(),
            timestamp_micros: now_micros().unwrap_or(0),
            metadata,
        });
        self
    }

    pub fn with_negotiation(
        mut self,
        negotiation_data: &serde_json::Value,
        metadata: BTreeMap<String, serde_json::Value>,
    ) -> Result<Self> {
        let prev = self
            .intent_step
            .as_ref()
            .ok_or(ZapAgentError::MissingStep(ProvenanceStage::Intent))?;

        let canonical_json = serde_json::to_vec(negotiation_data)?;
        let mut data_hasher = Sha256::new();
        data_hasher.update(&canonical_json);
        let input_hash = hex::encode(data_hasher.finalize());

        let mut step_hasher = Sha256::new();
        step_hasher.update(prev.step_hash.as_bytes());
        step_hasher.update(b":");
        step_hasher.update(input_hash.as_bytes());
        let step_hash = hex::encode(step_hasher.finalize());

        self.negotiation_step = Some(ProvenanceStep {
            stage: ProvenanceStage::Negotiation,
            step_hash,
            previous_hash: Some(prev.step_hash.clone()),
            input_data_hash: input_hash,
            timestamp_micros: now_micros().unwrap_or(0),
            metadata,
        });

        Ok(self)
    }

    pub fn with_policy(
        mut self,
        policy_digest: &str,
        decision: &str,
        metadata: BTreeMap<String, serde_json::Value>,
    ) -> Result<Self> {
        let prev_hash = if let Some(neg) = &self.negotiation_step {
            neg.step_hash.clone()
        } else if let Some(intent) = &self.intent_step {
            intent.step_hash.clone()
        } else {
            return Err(ZapAgentError::MissingStep(ProvenanceStage::Intent));
        };

        let mut data_hasher = Sha256::new();
        data_hasher.update(policy_digest.as_bytes());
        data_hasher.update(b":");
        data_hasher.update(decision.as_bytes());
        let input_hash = hex::encode(data_hasher.finalize());

        let mut step_hasher = Sha256::new();
        step_hasher.update(prev_hash.as_bytes());
        step_hasher.update(b":");
        step_hasher.update(input_hash.as_bytes());
        let step_hash = hex::encode(step_hasher.finalize());

        let mut meta = metadata;
        meta.insert(
            "decision".to_string(),
            serde_json::Value::String(decision.to_string()),
        );
        meta.insert(
            "policy_digest".to_string(),
            serde_json::Value::String(policy_digest.to_string()),
        );

        self.policy_step = Some(ProvenanceStep {
            stage: ProvenanceStage::Policy,
            step_hash,
            previous_hash: Some(prev_hash),
            input_data_hash: input_hash,
            timestamp_micros: now_micros().unwrap_or(0),
            metadata: meta,
        });

        Ok(self)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn with_consensus(
        mut self,
        certificate_hash: &str,
        epoch: u64,
        round: u64,
        threshold: u16,
        total_validators: u16,
        signer_bitmask: &[u8],
        signatures_count: usize,
        metadata: BTreeMap<String, serde_json::Value>,
    ) -> Result<Self> {
        let prev_hash = if let Some(pol) = &self.policy_step {
            pol.step_hash.clone()
        } else if let Some(neg) = &self.negotiation_step {
            neg.step_hash.clone()
        } else if let Some(intent) = &self.intent_step {
            intent.step_hash.clone()
        } else {
            return Err(ZapAgentError::MissingStep(ProvenanceStage::Intent));
        };

        let mut data_hasher = Sha256::new();
        data_hasher.update(certificate_hash.as_bytes());
        data_hasher.update(b":");
        data_hasher.update(epoch.to_be_bytes());
        data_hasher.update(b":");
        data_hasher.update(round.to_be_bytes());
        data_hasher.update(b":");
        data_hasher.update(threshold.to_be_bytes());
        data_hasher.update(b":");
        data_hasher.update(total_validators.to_be_bytes());
        data_hasher.update(b":");
        data_hasher.update(signer_bitmask);
        let input_hash = hex::encode(data_hasher.finalize());

        let mut step_hasher = Sha256::new();
        step_hasher.update(prev_hash.as_bytes());
        step_hasher.update(b":");
        step_hasher.update(input_hash.as_bytes());
        let step_hash = hex::encode(step_hasher.finalize());

        let mut meta = metadata;
        meta.insert(
            "certificate_hash".to_string(),
            serde_json::Value::String(certificate_hash.to_string()),
        );
        meta.insert("epoch".to_string(), serde_json::Value::Number(epoch.into()));
        meta.insert("round".to_string(), serde_json::Value::Number(round.into()));
        meta.insert(
            "threshold".to_string(),
            serde_json::Value::Number(threshold.into()),
        );
        meta.insert(
            "total_validators".to_string(),
            serde_json::Value::Number(total_validators.into()),
        );
        meta.insert(
            "signer_bitmask".to_string(),
            serde_json::Value::String(hex::encode(signer_bitmask)),
        );
        meta.insert(
            "signatures_count".to_string(),
            serde_json::Value::Number(signatures_count.into()),
        );

        self.consensus_step = Some(ProvenanceStep {
            stage: ProvenanceStage::Consensus,
            step_hash,
            previous_hash: Some(prev_hash),
            input_data_hash: input_hash,
            timestamp_micros: now_micros().unwrap_or(0),
            metadata: meta,
        });

        Ok(self)
    }

    pub fn with_driver(
        mut self,
        driver_id: &str,
        input_hash: &str,
        output_hash: &str,
        metadata: BTreeMap<String, serde_json::Value>,
    ) -> Result<Self> {
        let prev_hash = if let Some(consensus) = &self.consensus_step {
            consensus.step_hash.clone()
        } else if let Some(pol) = &self.policy_step {
            pol.step_hash.clone()
        } else if let Some(neg) = &self.negotiation_step {
            neg.step_hash.clone()
        } else if let Some(intent) = &self.intent_step {
            intent.step_hash.clone()
        } else {
            return Err(ZapAgentError::MissingStep(ProvenanceStage::Intent));
        };

        let mut data_hasher = Sha256::new();
        data_hasher.update(driver_id.as_bytes());
        data_hasher.update(b":");
        data_hasher.update(input_hash.as_bytes());
        data_hasher.update(b":");
        data_hasher.update(output_hash.as_bytes());
        let data_hash = hex::encode(data_hasher.finalize());

        let mut step_hasher = Sha256::new();
        step_hasher.update(prev_hash.as_bytes());
        step_hasher.update(b":");
        step_hasher.update(data_hash.as_bytes());
        let step_hash = hex::encode(step_hasher.finalize());

        let mut meta = metadata;
        meta.insert(
            "driver_id".to_string(),
            serde_json::Value::String(driver_id.to_string()),
        );

        self.driver_step = Some(ProvenanceStep {
            stage: ProvenanceStage::Driver,
            step_hash,
            previous_hash: Some(prev_hash),
            input_data_hash: data_hash,
            timestamp_micros: now_micros().unwrap_or(0),
            metadata: meta,
        });

        Ok(self)
    }

    pub fn with_poa(
        mut self,
        poa_signatures: &[String],
        metadata: BTreeMap<String, serde_json::Value>,
    ) -> Result<Self> {
        let prev = if let Some(driver) = &self.driver_step {
            driver
        } else if let Some(consensus) = &self.consensus_step {
            consensus
        } else if let Some(pol) = &self.policy_step {
            pol
        } else if let Some(neg) = &self.negotiation_step {
            neg
        } else if let Some(intent) = &self.intent_step {
            intent
        } else {
            return Err(ZapAgentError::MissingStep(ProvenanceStage::Intent));
        };

        let mut data_hasher = Sha256::new();
        for sig in poa_signatures {
            data_hasher.update(sig.as_bytes());
            data_hasher.update(b";");
        }
        let data_hash = hex::encode(data_hasher.finalize());

        let mut step_hasher = Sha256::new();
        step_hasher.update(prev.step_hash.as_bytes());
        step_hasher.update(b":");
        step_hasher.update(data_hash.as_bytes());
        let step_hash = hex::encode(step_hasher.finalize());

        let mut meta = metadata;
        meta.insert(
            "poa_attestation_count".to_string(),
            serde_json::Value::Number(poa_signatures.len().into()),
        );

        self.poa_step = Some(ProvenanceStep {
            stage: ProvenanceStage::Poa,
            step_hash,
            previous_hash: Some(prev.step_hash.clone()),
            input_data_hash: data_hash,
            timestamp_micros: now_micros().unwrap_or(0),
            metadata: meta,
        });

        Ok(self)
    }

    pub fn with_receipt(
        mut self,
        receipt_id: &str,
        processed_at_micros: u64,
        metadata: BTreeMap<String, serde_json::Value>,
    ) -> Result<Self> {
        let prev_hash = if let Some(poa) = &self.poa_step {
            poa.step_hash.clone()
        } else if let Some(driver) = &self.driver_step {
            driver.step_hash.clone()
        } else if let Some(consensus) = &self.consensus_step {
            consensus.step_hash.clone()
        } else if let Some(pol) = &self.policy_step {
            pol.step_hash.clone()
        } else if let Some(neg) = &self.negotiation_step {
            neg.step_hash.clone()
        } else if let Some(intent) = &self.intent_step {
            intent.step_hash.clone()
        } else {
            return Err(ZapAgentError::MissingStep(ProvenanceStage::Intent));
        };

        let mut data_hasher = Sha256::new();
        data_hasher.update(receipt_id.as_bytes());
        data_hasher.update(b":");
        data_hasher.update(processed_at_micros.to_be_bytes());
        let data_hash = hex::encode(data_hasher.finalize());

        let mut step_hasher = Sha256::new();
        step_hasher.update(prev_hash.as_bytes());
        step_hasher.update(b":");
        step_hasher.update(data_hash.as_bytes());
        let step_hash = hex::encode(step_hasher.finalize());

        let mut meta = metadata;
        meta.insert(
            "receipt_id".to_string(),
            serde_json::Value::String(receipt_id.to_string()),
        );

        self.receipt_step = Some(ProvenanceStep {
            stage: ProvenanceStage::Receipt,
            step_hash,
            previous_hash: Some(prev_hash),
            input_data_hash: data_hash,
            timestamp_micros: now_micros().unwrap_or(0),
            metadata: meta,
        });

        Ok(self)
    }

    pub fn build_and_sign(self, keypair: &Keypair) -> Result<ProvenanceChainDigest> {
        let mut steps = Vec::new();
        if let Some(s) = self.intent_step {
            steps.push(s);
        } else {
            return Err(ZapAgentError::MissingStep(ProvenanceStage::Intent));
        }
        if let Some(s) = self.negotiation_step {
            steps.push(s);
        }
        if let Some(s) = self.policy_step {
            steps.push(s);
        }
        if let Some(s) = self.consensus_step {
            steps.push(s);
        }
        if let Some(s) = self.driver_step {
            steps.push(s);
        }
        if let Some(s) = self.poa_step {
            steps.push(s);
        }
        if let Some(s) = self.receipt_step {
            steps.push(s);
        }

        let root_hash = compute_root_hash(&steps);

        let signing_key = SigningKey::from_bytes(&keypair.secret_bytes());
        let mut transcript = Vec::new();
        transcript.extend_from_slice(PROVENANCE_SIGNATURE_DOMAIN);
        transcript.push(0);
        transcript.extend_from_slice(root_hash.as_bytes());
        let sig: Signature = signing_key.sign(&transcript);
        let signature = hex::encode(sig.to_bytes());

        Ok(ProvenanceChainDigest {
            schema_version: PROVENANCE_SCHEMA_VERSION,
            chain_id: self.chain_id,
            session_id: self.session_id,
            intent_id: self.intent_id,
            steps,
            root_hash,
            node_id: keypair.node_id(),
            signature,
            created_at_micros: now_micros().unwrap_or(0),
        })
    }
}

pub fn compute_root_hash(steps: &[ProvenanceStep]) -> String {
    let mut hasher = Sha256::new();
    for step in steps {
        hasher.update(format!("{:?}", step.stage).to_lowercase().as_bytes());
        hasher.update(b":");
        hasher.update(step.step_hash.as_bytes());
        hasher.update(b";");
    }
    hex::encode(hasher.finalize())
}

impl ProvenanceChainDigest {
    pub fn stage_step(&self, stage: ProvenanceStage) -> Option<&ProvenanceStep> {
        self.steps.iter().find(|s| s.stage == stage)
    }

    pub fn verify_step(&self, stage: ProvenanceStage) -> Result<()> {
        let (idx, step) = self
            .steps
            .iter()
            .enumerate()
            .find(|(_, s)| s.stage == stage)
            .ok_or(ZapAgentError::MissingStep(stage))?;

        if idx == 0 {
            if step.stage != ProvenanceStage::Intent {
                return Err(ZapAgentError::StepVerificationFailed {
                    stage,
                    expected: "stage == Intent".to_string(),
                    actual: format!("{:?}", step.stage),
                });
            }
            if step.previous_hash.is_some() {
                return Err(ZapAgentError::StepVerificationFailed {
                    stage,
                    expected: "previous_hash == None".to_string(),
                    actual: format!("{:?}", step.previous_hash),
                });
            }
            if step.step_hash != step.input_data_hash {
                return Err(ZapAgentError::StepVerificationFailed {
                    stage,
                    expected: step.input_data_hash.clone(),
                    actual: step.step_hash.clone(),
                });
            }
        } else {
            let prev_step = &self.steps[idx - 1];
            let declared_prev =
                step.previous_hash
                    .as_ref()
                    .ok_or(ZapAgentError::StepVerificationFailed {
                        stage,
                        expected: format!("previous_hash == {}", prev_step.step_hash),
                        actual: "None".to_string(),
                    })?;

            if declared_prev != &prev_step.step_hash {
                return Err(ZapAgentError::StepVerificationFailed {
                    stage,
                    expected: prev_step.step_hash.clone(),
                    actual: declared_prev.clone(),
                });
            }

            let mut step_hasher = Sha256::new();
            step_hasher.update(declared_prev.as_bytes());
            step_hasher.update(b":");
            step_hasher.update(step.input_data_hash.as_bytes());
            let computed_hash = hex::encode(step_hasher.finalize());

            if computed_hash != step.step_hash {
                return Err(ZapAgentError::StepVerificationFailed {
                    stage,
                    expected: computed_hash,
                    actual: step.step_hash.clone(),
                });
            }
        }

        Ok(())
    }

    pub fn verify_chain(&self, public_key: &PublicKey) -> Result<ProvenanceVerificationReport> {
        self.verify(public_key)
    }

    pub fn verify(&self, public_key: &PublicKey) -> Result<ProvenanceVerificationReport> {
        if self.schema_version != PROVENANCE_SCHEMA_VERSION {
            return Ok(ProvenanceVerificationReport {
                valid: false,
                chain_id: self.chain_id,
                root_hash: self.root_hash.clone(),
                node_id: self.node_id,
                verified_steps: 0,
                failed_stage: None,
                failure_reason: Some(format!(
                    "Unsupported schema version: {}",
                    self.schema_version
                )),
            });
        }

        if self.steps.is_empty() {
            return Ok(ProvenanceVerificationReport {
                valid: false,
                chain_id: self.chain_id,
                root_hash: self.root_hash.clone(),
                node_id: self.node_id,
                verified_steps: 0,
                failed_stage: None,
                failure_reason: Some("Provenance chain contains no steps".to_string()),
            });
        }

        let mut verified_count = 0;
        let mut last_hash: Option<String> = None;

        for (idx, step) in self.steps.iter().enumerate() {
            if idx == 0 {
                if step.stage != ProvenanceStage::Intent {
                    return Ok(ProvenanceVerificationReport {
                        valid: false,
                        chain_id: self.chain_id,
                        root_hash: self.root_hash.clone(),
                        node_id: self.node_id,
                        verified_steps: verified_count,
                        failed_stage: Some(step.stage),
                        failure_reason: Some("First step must be Intent stage".to_string()),
                    });
                }
                if step.previous_hash.is_some() {
                    return Ok(ProvenanceVerificationReport {
                        valid: false,
                        chain_id: self.chain_id,
                        root_hash: self.root_hash.clone(),
                        node_id: self.node_id,
                        verified_steps: verified_count,
                        failed_stage: Some(step.stage),
                        failure_reason: Some("First step must not have previous_hash".to_string()),
                    });
                }
                if step.step_hash != step.input_data_hash {
                    return Ok(ProvenanceVerificationReport {
                        valid: false,
                        chain_id: self.chain_id,
                        root_hash: self.root_hash.clone(),
                        node_id: self.node_id,
                        verified_steps: verified_count,
                        failed_stage: Some(step.stage),
                        failure_reason: Some(format!(
                            "Intent step_hash mismatch: expected {}, got {}",
                            step.input_data_hash, step.step_hash
                        )),
                    });
                }
            } else {
                let prev = match &step.previous_hash {
                    Some(p) => p,
                    None => {
                        return Ok(ProvenanceVerificationReport {
                            valid: false,
                            chain_id: self.chain_id,
                            root_hash: self.root_hash.clone(),
                            node_id: self.node_id,
                            verified_steps: verified_count,
                            failed_stage: Some(step.stage),
                            failure_reason: Some(format!(
                                "Step {:?} is missing previous_hash link",
                                step.stage
                            )),
                        });
                    }
                };

                if let Some(expected_prev) = &last_hash
                    && prev != expected_prev
                {
                    return Ok(ProvenanceVerificationReport {
                        valid: false,
                        chain_id: self.chain_id,
                        root_hash: self.root_hash.clone(),
                        node_id: self.node_id,
                        verified_steps: verified_count,
                        failed_stage: Some(step.stage),
                        failure_reason: Some(format!(
                            "Causal break at stage {:?}: previous_hash {} != prior step_hash {}",
                            step.stage, prev, expected_prev
                        )),
                    });
                }

                // Verify transition hash: SHA256(prev:input_data_hash)
                let mut step_hasher = Sha256::new();
                step_hasher.update(prev.as_bytes());
                step_hasher.update(b":");
                step_hasher.update(step.input_data_hash.as_bytes());
                let computed_hash = hex::encode(step_hasher.finalize());

                if computed_hash != step.step_hash {
                    return Ok(ProvenanceVerificationReport {
                        valid: false,
                        chain_id: self.chain_id,
                        root_hash: self.root_hash.clone(),
                        node_id: self.node_id,
                        verified_steps: verified_count,
                        failed_stage: Some(step.stage),
                        failure_reason: Some(format!(
                            "Stage {:?} hash corrupted: computed {}, declared {}",
                            step.stage, computed_hash, step.step_hash
                        )),
                    });
                }
            }

            last_hash = Some(step.step_hash.clone());
            verified_count += 1;
        }

        // Verify root hash calculation
        let computed_root = compute_root_hash(&self.steps);
        if computed_root != self.root_hash {
            return Ok(ProvenanceVerificationReport {
                valid: false,
                chain_id: self.chain_id,
                root_hash: self.root_hash.clone(),
                node_id: self.node_id,
                verified_steps: verified_count,
                failed_stage: None,
                failure_reason: Some(format!(
                    "Merkle root mismatch: computed {}, declared {}",
                    computed_root, self.root_hash
                )),
            });
        }

        // Verify node ID matches public key
        if public_key.node_id() != self.node_id {
            return Ok(ProvenanceVerificationReport {
                valid: false,
                chain_id: self.chain_id,
                root_hash: self.root_hash.clone(),
                node_id: self.node_id,
                verified_steps: verified_count,
                failed_stage: None,
                failure_reason: Some(format!(
                    "Signer node ID mismatch: key derived {}, chain declared {}",
                    public_key.node_id(),
                    self.node_id
                )),
            });
        }

        // Verify Ed25519 signature
        let sig_bytes = match hex::decode(&self.signature) {
            Ok(bytes) if bytes.len() == 64 => bytes,
            _ => {
                return Ok(ProvenanceVerificationReport {
                    valid: false,
                    chain_id: self.chain_id,
                    root_hash: self.root_hash.clone(),
                    node_id: self.node_id,
                    verified_steps: verified_count,
                    failed_stage: None,
                    failure_reason: Some("Invalid signature format/length".to_string()),
                });
            }
        };

        let mut sig_arr = [0u8; 64];
        sig_arr.copy_from_slice(&sig_bytes);
        let signature = Signature::from_bytes(&sig_arr);

        let verifying_key = match VerifyingKey::from_bytes(&public_key.to_bytes()) {
            Ok(vk) => vk,
            Err(_) => {
                return Err(ZapCryptoError::InvalidKeyLength {
                    kind: "public_key",
                    expected: 32,
                    actual: 32,
                }
                .into());
            }
        };

        let mut transcript = Vec::new();
        transcript.extend_from_slice(PROVENANCE_SIGNATURE_DOMAIN);
        transcript.push(0);
        transcript.extend_from_slice(self.root_hash.as_bytes());

        if verifying_key.verify(&transcript, &signature).is_err() {
            return Ok(ProvenanceVerificationReport {
                valid: false,
                chain_id: self.chain_id,
                root_hash: self.root_hash.clone(),
                node_id: self.node_id,
                verified_steps: verified_count,
                failed_stage: None,
                failure_reason: Some("Ed25519 signature verification failed".to_string()),
            });
        }

        Ok(ProvenanceVerificationReport {
            valid: true,
            chain_id: self.chain_id,
            root_hash: self.root_hash.clone(),
            node_id: self.node_id,
            verified_steps: verified_count,
            failed_stage: None,
            failure_reason: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AgentId, IntentKind};

    #[test]
    fn test_full_provenance_chain_generation_and_verification() {
        let keypair = Keypair::generate();
        let session_id = Uuid::new_v4();
        let intent_id = Uuid::new_v4();

        let mut intent = AgentIntent::new(
            session_id,
            AgentId::new("agent_1").unwrap(),
            IntentKind::Act,
            "Transfer asset",
        );
        intent.intent_id = intent_id;

        let chain = ProvenanceChainBuilder::new(session_id, intent_id)
            .with_intent(&intent)
            .unwrap()
            .with_negotiation(
                &serde_json::json!({"negotiated_capability": "driver.execute:asset_transfer"}),
                BTreeMap::new(),
            )
            .unwrap()
            .with_policy("policy_digest_sha256", "ALLOW", BTreeMap::new())
            .unwrap()
            .with_driver("asset_driver_v1", "in_hash", "out_hash", BTreeMap::new())
            .unwrap()
            .with_poa(
                &["sig_val1".to_string(), "sig_val2".to_string()],
                BTreeMap::new(),
            )
            .unwrap()
            .with_receipt("receipt_42", 1_700_000_000, BTreeMap::new())
            .unwrap()
            .build_and_sign(&keypair)
            .unwrap();

        let report = chain.verify(&keypair.verifying_key()).unwrap();
        assert!(report.valid);
        assert_eq!(report.verified_steps, 6);
        assert!(report.failure_reason.is_none());

        assert!(chain.verify_step(ProvenanceStage::Intent).is_ok());
        assert!(chain.verify_step(ProvenanceStage::Negotiation).is_ok());
        assert!(chain.verify_step(ProvenanceStage::Policy).is_ok());
        assert!(chain.verify_step(ProvenanceStage::Driver).is_ok());
        assert!(chain.verify_step(ProvenanceStage::Poa).is_ok());
        assert!(chain.verify_step(ProvenanceStage::Receipt).is_ok());
    }

    #[test]
    fn test_tampered_step_fails_verification() {
        let keypair = Keypair::generate();
        let session_id = Uuid::new_v4();
        let intent_id = Uuid::new_v4();

        let mut intent = AgentIntent::new(
            session_id,
            AgentId::new("agent_1").unwrap(),
            IntentKind::Act,
            "Transfer asset",
        );
        intent.intent_id = intent_id;

        let mut chain = ProvenanceChainBuilder::new(session_id, intent_id)
            .with_intent(&intent)
            .unwrap()
            .with_policy("policy_digest", "ALLOW", BTreeMap::new())
            .unwrap()
            .with_receipt("receipt_42", 1_700_000_000, BTreeMap::new())
            .unwrap()
            .build_and_sign(&keypair)
            .unwrap();

        // Tamper with policy input_data_hash
        chain.steps[1].input_data_hash = "corrupted_hash".to_string();

        let report = chain.verify(&keypair.verifying_key()).unwrap();
        assert!(!report.valid);
        assert_eq!(report.failed_stage, Some(ProvenanceStage::Policy));
        assert!(report.failure_reason.is_some());
    }

    #[test]
    fn test_tampered_signature_fails_verification() {
        let keypair = Keypair::generate();
        let session_id = Uuid::new_v4();
        let intent_id = Uuid::new_v4();

        let mut intent = AgentIntent::new(
            session_id,
            AgentId::new("agent_1").unwrap(),
            IntentKind::Act,
            "Transfer asset",
        );
        intent.intent_id = intent_id;

        let mut chain = ProvenanceChainBuilder::new(session_id, intent_id)
            .with_intent(&intent)
            .unwrap()
            .with_policy("policy_digest", "ALLOW", BTreeMap::new())
            .unwrap()
            .with_receipt("receipt_42", 1_700_000_000, BTreeMap::new())
            .unwrap()
            .build_and_sign(&keypair)
            .unwrap();

        let mut sig_bytes = hex::decode(&chain.signature).unwrap();
        sig_bytes[0] ^= 0xFF;
        chain.signature = hex::encode(sig_bytes);

        let report = chain.verify(&keypair.verifying_key()).unwrap();
        assert!(!report.valid);
        assert!(report.failure_reason.unwrap().contains("signature"));
    }
}
