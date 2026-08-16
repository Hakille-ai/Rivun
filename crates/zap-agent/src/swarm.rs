//! Swarm Agent Coordinator and Capability Matching Subsystem.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::Digest;
use std::collections::{BTreeMap, HashMap};
use uuid::Uuid;
use zap_capability::CapabilityId;
use zap_core::now_micros;
use zap_crypto::Keypair;

use crate::{
    AgentId, AgentIntent, AgentResult, ProvenanceChainBuilder,
    ProvenanceChainDigest, Result, Validate, ZapAgentError,
};

pub const SWARM_PROTOCOL_SCHEMA_VERSION: u8 = 1;
pub const SWARM_INTENT_PROPOSAL_SUBJECT: &str = "zap.swarm.intent.propose";
pub const SWARM_INTENT_COMMIT_SUBJECT: &str = "zap.swarm.intent.commit";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SwarmIntentStatus {
    Submitted,
    Proposed,
    Prevoting,
    Precommitting,
    Committed,
    Executing,
    Finalized,
    Rejected,
    TimedOut,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SwarmIntentProposal {
    pub schema_version: u8,
    pub proposal_id: Uuid,
    pub session_id: Uuid,
    pub intent: AgentIntent,
    pub proposer_agent: AgentId,
    pub proposer_node: Uuid,
    pub required_quorum: u16,
    pub intent_digest: String, // hex-encoded SHA-256 of canonical intent JSON
    pub created_at_micros: u64,
    pub deadline_micros: u64,
    #[serde(default)]
    pub metadata: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SwarmCommitCertificateRef {
    pub certificate_hash: String,
    pub epoch: u64,
    pub view: u64,
    pub round: u64,
    pub block_height: u64,
    pub proposal_digest: [u8; 32],
    pub threshold: u16,
    pub total_validators: u16,
    pub signer_bitmask: Vec<u8>,
    pub signatures_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SwarmPeerCapabilityScore {
    pub node_id: Uuid,
    pub agent_id: Option<AgentId>,
    pub capability: CapabilityId,
    pub trust_score: f64,        // Range 0.0 to 1.0
    pub latency_ms: f64,         // Measured RTT in milliseconds
    pub load_factor: u8,         // Range 0 (idle) to 100 (saturated)
    pub composite_score: f64,    // Computed composite ranking
    pub last_updated_micros: u64,
}

#[derive(Debug, Clone, Default)]
pub struct SwarmCapabilityIndex {
    pub capabilities: HashMap<CapabilityId, Vec<SwarmPeerCapabilityScore>>,
}

impl SwarmCapabilityIndex {
    #[must_use]
    pub fn new() -> Self {
        Self {
            capabilities: HashMap::new(),
        }
    }

    pub fn register_or_update(
        &mut self,
        node_id: Uuid,
        agent_id: Option<AgentId>,
        capability: CapabilityId,
        trust_score: f64,
        latency_ms: f64,
        load_factor: u8,
        now_micros: u64,
    ) {
        let trust_norm = trust_score.clamp(0.0, 1.0);
        let latency_norm = (1.0 - (latency_ms / 1000.0)).clamp(0.0, 1.0);
        let load_norm = (1.0 - (f64::from(load_factor) / 100.0)).clamp(0.0, 1.0);
        let composite_score = (0.4 * trust_norm) + (0.3 * latency_norm) + (0.3 * load_norm);

        let entry = SwarmPeerCapabilityScore {
            node_id,
            agent_id,
            capability: capability.clone(),
            trust_score: trust_norm,
            latency_ms,
            load_factor,
            composite_score,
            last_updated_micros: now_micros,
        };

        let scores = self.capabilities.entry(capability).or_default();
        if let Some(pos) = scores.iter().position(|s| s.node_id == node_id) {
            scores[pos] = entry;
        } else {
            scores.push(entry);
        }
        scores.sort_by(|a, b| {
            b.composite_score
                .partial_cmp(&a.composite_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
    }

    #[must_use]
    pub fn select_best_peer(&self, capability: &CapabilityId) -> Option<&SwarmPeerCapabilityScore> {
        self.capabilities.get(capability).and_then(|scores| scores.first())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SwarmIntentRecord {
    pub proposal: SwarmIntentProposal,
    pub status: SwarmIntentStatus,
    pub commit_certificate: Option<SwarmCommitCertificateRef>,
    pub provenance_chain: Option<ProvenanceChainDigest>,
    pub execution_result: Option<AgentResult>,
    pub updated_at_micros: u64,
}

pub struct SwarmAgentCoordinator {
    self_node_id: Uuid,
    self_agent_id: AgentId,
    default_quorum_threshold: u16,
    capability_index: SwarmCapabilityIndex,
    active_intents: HashMap<Uuid, SwarmIntentRecord>,
}

impl SwarmAgentCoordinator {
    #[must_use]
    pub fn new(self_node_id: Uuid, self_agent_id: AgentId, default_quorum_threshold: u16) -> Self {
        Self {
            self_node_id,
            self_agent_id,
            default_quorum_threshold: default_quorum_threshold.max(1),
            capability_index: SwarmCapabilityIndex::new(),
            active_intents: HashMap::new(),
        }
    }

    pub fn submit_intent(
        &mut self,
        intent: AgentIntent,
        deadline_micros: u64,
    ) -> Result<Uuid> {
        intent.validate()?;
        let canonical_bytes = serde_json::to_vec(&intent)
            .map_err(|e| ZapAgentError::InvalidIdentifier {
                entity: "agent_intent",
                field: "json",
                value: e.to_string(),
            })?;
        let mut hasher = sha2::Sha256::new();
        sha2::Digest::update(&mut hasher, &canonical_bytes);
        let intent_digest = hex::encode(sha2::Digest::finalize(hasher));

        let proposal_id = Uuid::new_v4();
        let proposal = SwarmIntentProposal {
            schema_version: SWARM_PROTOCOL_SCHEMA_VERSION,
            proposal_id,
            session_id: intent.session_id,
            intent: intent.clone(),
            proposer_agent: self.self_agent_id.clone(),
            proposer_node: self.self_node_id,
            required_quorum: self.default_quorum_threshold,
            intent_digest,
            created_at_micros: now_micros().unwrap_or(0),
            deadline_micros,
            metadata: BTreeMap::new(),
        };

        self.active_intents.insert(
            proposal_id,
            SwarmIntentRecord {
                proposal,
                status: SwarmIntentStatus::Submitted,
                commit_certificate: None,
                provenance_chain: None,
                execution_result: None,
                updated_at_micros: now_micros().unwrap_or(0),
            },
        );

        Ok(proposal_id)
    }

    pub fn mark_proposed(&mut self, proposal_id: Uuid) -> Result<&SwarmIntentProposal> {
        let record = self
            .active_intents
            .get_mut(&proposal_id)
            .ok_or_else(|| ZapAgentError::InvalidIdentifier {
                entity: "swarm_intent",
                field: "proposal_id",
                value: proposal_id.to_string(),
            })?;
        record.status = SwarmIntentStatus::Proposed;
        record.updated_at_micros = now_micros().unwrap_or(0);
        Ok(&record.proposal)
    }

    pub fn attach_commit_certificate(
        &mut self,
        proposal_id: Uuid,
        cert: SwarmCommitCertificateRef,
    ) -> Result<()> {
        let record = self
            .active_intents
            .get_mut(&proposal_id)
            .ok_or_else(|| ZapAgentError::InvalidIdentifier {
                entity: "swarm_intent",
                field: "proposal_id",
                value: proposal_id.to_string(),
            })?;
        record.commit_certificate = Some(cert);
        record.status = SwarmIntentStatus::Committed;
        record.updated_at_micros = now_micros().unwrap_or(0);
        Ok(())
    }

    pub fn finalize_intent_with_provenance(
        &mut self,
        proposal_id: Uuid,
        result: AgentResult,
        receipt_id: &str,
        processed_at_micros: u64,
        keypair: &Keypair,
    ) -> Result<ProvenanceChainDigest> {
        result.validate()?;
        let record = self
            .active_intents
            .get_mut(&proposal_id)
            .ok_or_else(|| ZapAgentError::InvalidIdentifier {
                entity: "swarm_intent",
                field: "proposal_id",
                value: proposal_id.to_string(),
            })?;

        let cert = record
            .commit_certificate
            .as_ref()
            .ok_or_else(|| ZapAgentError::MissingStep(crate::provenance::ProvenanceStage::Consensus))?;

        let mut builder = ProvenanceChainBuilder::new(record.proposal.session_id, record.proposal.intent.intent_id)
            .with_intent(&record.proposal.intent)?
            .with_consensus(
                &cert.certificate_hash,
                cert.epoch,
                cert.round,
                cert.threshold,
                cert.total_validators,
                &cert.signer_bitmask,
                cert.signatures_count,
                BTreeMap::new(),
            )?;

        if let Some(err) = &result.error {
            let mut err_meta = BTreeMap::new();
            err_meta.insert("error_code".to_string(), serde_json::Value::String(err.code.clone()));
            builder = builder.with_receipt(receipt_id, processed_at_micros, err_meta)?;
        } else {
            builder = builder.with_receipt(receipt_id, processed_at_micros, BTreeMap::new())?;
        }

        let chain = builder.build_and_sign(keypair)?;
        record.provenance_chain = Some(chain.clone());
        record.execution_result = Some(result);
        record.status = SwarmIntentStatus::Finalized;
        record.updated_at_micros = now_micros().unwrap_or(0);

        Ok(chain)
    }

    pub fn capability_index_mut(&mut self) -> &mut SwarmCapabilityIndex {
        &mut self.capability_index
    }

    #[must_use]
    pub fn capability_index(&self) -> &SwarmCapabilityIndex {
        &self.capability_index
    }

    #[must_use]
    pub fn get_intent(&self, proposal_id: &Uuid) -> Option<&SwarmIntentRecord> {
        self.active_intents.get(proposal_id)
    }
}
