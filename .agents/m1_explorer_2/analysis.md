# Architectural Blueprint & Implementation Specification: R1 Swarm Agent Coordination, Cryptographic Provenance & Concurrent Node Daemon

**Document Reference**: `rivun-R1-AGENT-NODE-BLUEPRINT-2026`  
**Author**: Explorer 2 (Milestone 1 — R1: P2P Swarm Gossip Consensus & Adaptive Quorum Mesh)  
**Target Crates**: `crates/rivun-agent`, `crates/rivun-node`, with cross-cutting integration in `crates/rivun-core`, `crates/rivun-router`, `crates/rivun-net`, `crates/rivun-crypto`  
**Status**: Final Technical Specification & Implementation Blueprint  

---

## 1. Executive Summary

Milestone 1 (R1: P2P Swarm Gossip Consensus & Adaptive Quorum Mesh) establishes the decentralized networking, consensus, and coordination foundation for rivun Next-Gen Frontier. While `crates/rivun-net` implements the low-level epidemic gossip protocol, BFT consensus state machine, and $\Phi$-accrual mesh health tracker, the application and daemon layers reside in:
1. **`crates/rivun-agent`**: Bridges high-level autonomous agent intents (`AgentIntent`) with distributed swarm consensus, decentralized capability matching, and cryptographic provenance verification.
2. **`crates/rivun-node`**: Serves as the high-performance runtime daemon, orchestrating concurrent networking, gossip dissemination, consensus voting, mesh health tracking, and WASM action dispatch without blocking the event loop.

### Core Objectives of Explorer 2 Blueprint:
- **`crates/rivun-agent/src/swarm.rs`**: Design the `SwarmAgentCoordinator` state machine to manage the lifecycle of consensus-backed agent intents, swarm capability scoring, and multi-agent intent proposals.
- **`crates/rivun-agent/src/provenance.rs`**: Extend the 6-stage cryptographic Provenance Chain Engine to support `ProvenanceStage::Consensus`, mathematically binding `SwarmCommitCertificate` (certificate hash, epoch, round, view, threshold, validator count, and signer bitmask) into the causal Merkle chain.
- **`crates/rivun-node` Concurrent Actor Refactor**: Decompose the single-loop `ZapNode` daemon into concurrent Tokio actors (`UdpRxTask`, `GossipTask`, `ConsensusTask`, `MeshTask`, `ExecutionTask`) communicating over bounded channels with structured graceful shutdown.
- **Configuration Schema Extensions**: Expand `rivun.toml` with `[swarm]`, `[gossip]`, and `[mesh]` tables while maintaining 100% backwards compatibility with existing configuration files, CLI commands, and wire formats.

---

## 2. `crates/rivun-agent` Deep-Dive & Architecture Blueprint

### 2.1 Current State Analysis of `crates/rivun-agent`
- `crates/rivun-agent/src/lib.rs`: Implements strict JSON contracts for `AgentIntent`, `AgentSession`, `DelegationRequest`, `DelegationResponse`, `CapabilityNegotiationRequest`, `CapabilityNegotiationResponse`, `AgentStatusUpdate`, `AgentResult`, and `AgentMessage`. All schemas enforce validation rules (non-empty fields, max text lengths, valid identifier characters, monotonic timestamps).
- `crates/rivun-agent/src/provenance.rs`: Enforces causal hashing over 6 stages:
  $$H_{\text{intent}} \to H_{\text{negotiation}} \to H_{\text{policy}} \to H_{\text{driver}} \to H_{\text{poa}} \to H_{\text{receipt}} \to H_{\text{root}}$$
  signed with the node's Ed25519 identity key.
- **Gaps**:
  - No abstraction exists for coordinating multi-agent intent consensus or querying swarm-wide capability indexes.
  - `ProvenanceStage` only supports static `Poa` (list of validator signatures). It cannot record compact BFT `SwarmCommitCertificate` objects featuring epoch/round metadata and compressed signer bitmasks.

---

### 2.2 `crates/rivun-agent/src/swarm.rs`: Swarm Agent Coordinator Specification

`src/swarm.rs` introduces `SwarmAgentCoordinator`, providing the high-level API for agents to interact with swarm consensus and capability routing.

```
+---------------------------------------------------------------------------------------+
|                               SWARM AGENT COORDINATOR                                 |
+---------------------------------------------------------------------------------------+
|                                                                                       |
|   Agent Submits Intent         Capability Matcher              Consensus Dispatch     |
|   +-------------------+      +--------------------+         +---------------------+   |
|   |    AgentIntent    | ===> | SwarmCapabilityIdx | ======> | SwarmIntentProposal |   |
|   | (Kind::Act, etc.) |      | (Scores & Latency) |         | (Consensus Proposal)|   |
|   +-------------------+      +--------------------+         +----------+----------+   |
|                                                                        |              |
|                                                                        v              |
|   Cryptographic Provenance         Result Finalization       Quorum Verification      |
|   +-------------------+         +--------------------+      +---------------------+   |
|   | ProvenanceChain   | <====== |    AgentResult     | <=== | SwarmCommitCertRef  |   |
|   | (Bound Consensus) |         | (Driver Execution) |      | (T-of-N Threshold)  |   |
|   +-------------------+         +--------------------+      +---------------------+   |
|                                                                                       |
+---------------------------------------------------------------------------------------+
```

#### 2.2.1 Data Structures & Enums

```rust
// Proposed in crates/rivun-agent/src/swarm.rs

use std::collections::{BTreeMap, HashMap};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;
use @@rivun_HEADER@@capability::CapabilityId;
use @@rivun_HEADER@@core::now_micros;
use @@rivun_HEADER@@crypto::Keypair;

use crate::{
    AgentId, AgentIntent, AgentResult, IntentKind, ProvenanceChainBuilder,
    ProvenanceChainDigest, Result, ZapAgentError,
};

pub const SWARM_PROTOCOL_SCHEMA_VERSION: u8 = 1;
pub const SWARM_INTENT_PROPOSAL_SUBJECT: &str = "rivun.swarm.intent.propose";
pub const SWARM_INTENT_COMMIT_SUBJECT: &str = "rivun.swarm.intent.commit";

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
        let load_norm = (1.0 - (load_factor as f64 / 100.0)).clamp(0.0, 1.0);
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
        scores.sort_by(|a, b| b.composite_score.partial_cmp(&a.composite_score).unwrap_or(std::cmp::Ordering::Equal));
    }

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
        let canonical_bytes = serde_json::to_vec(&intent)?;
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
            .ok_or_else(|| ZapAgentError::MissingStep(crate::ProvenanceStage::Consensus))?;

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

    pub fn capability_index(&self) -> &SwarmCapabilityIndex {
        &self.capability_index
    }

    pub fn get_intent(&self, proposal_id: &Uuid) -> Option<&SwarmIntentRecord> {
        self.active_intents.get(proposal_id)
    }
}
```

---

### 2.3 `crates/rivun-agent/src/provenance.rs`: Cryptographic Swarm Consensus Binding

#### 2.3.1 Extending `ProvenanceStage`
Add `ProvenanceStage::Consensus` while preserving backwards compatibility:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProvenanceStage {
    Intent,
    Negotiation,
    Policy,
    Consensus,  // <-- Extended for BFT Swarm Commit Certificates
    Driver,
    Poa,        // <-- Kept for legacy static PoA
    Receipt,
}
```

#### 2.3.2 Extending `ProvenanceChainBuilder`
Add `consensus_step: Option<ProvenanceStep>` to `ProvenanceChainBuilder` and provide the constructor `with_consensus()`:

```rust
pub struct ProvenanceChainBuilder {
    chain_id: Uuid,
    session_id: Uuid,
    intent_id: Uuid,
    intent_step: Option<ProvenanceStep>,
    negotiation_step: Option<ProvenanceStep>,
    policy_step: Option<ProvenanceStep>,
    consensus_step: Option<ProvenanceStep>, // <-- Added field
    driver_step: Option<ProvenanceStep>,
    poa_step: Option<ProvenanceStep>,
    receipt_step: Option<ProvenanceStep>,
}

impl ProvenanceChainBuilder {
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

        // Canonical input hash: SHA256(cert_hash:epoch:round:threshold:total_validators:bitmask)
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

        // Causal step hash: SHA256(prev_hash:input_hash)
        let mut step_hasher = Sha256::new();
        step_hasher.update(prev_hash.as_bytes());
        step_hasher.update(b":");
        step_hasher.update(input_hash.as_bytes());
        let step_hash = hex::encode(step_hasher.finalize());

        let mut meta = metadata;
        meta.insert("certificate_hash".to_string(), serde_json::Value::String(certificate_hash.to_string()));
        meta.insert("epoch".to_string(), serde_json::Value::Number(epoch.into()));
        meta.insert("round".to_string(), serde_json::Value::Number(round.into()));
        meta.insert("threshold".to_string(), serde_json::Value::Number(threshold.into()));
        meta.insert("total_validators".to_string(), serde_json::Value::Number(total_validators.into()));
        meta.insert("signer_bitmask".to_string(), serde_json::Value::String(hex::encode(signer_bitmask)));
        meta.insert("signatures_count".to_string(), serde_json::Value::Number(signatures_count.into()));

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

    // Update with_driver to resolve previous_hash from consensus_step if present
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

    // Update with_receipt to resolve previous_hash across all predecessor options
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
```

---

## 3. `crates/rivun-node` Concurrent Tokio Actor Architecture

### 3.1 Overview & Concurrency Model

In the single-loop `ZapNode`, all transport receive, replay checks, cryptographic verification, discovery handling, routing, and WASM execution happened sequentially on a single thread. Under high-throughput swarm gossip (10,000+ ops/sec) and distributed BFT consensus rounds, this serial execution bottlenecks packet ingress.

The Next-Gen `ZapNode` refactors the daemon into **5 asynchronous Tokio actor tasks**:

```
                                  +-----------------------+
                                  |    UDP Socket / NIC   |
                                  +-----------+-----------+
                                              |
                                              v
                              +-------------------------------+
                              |           UdpRxTask           |
                              | - Non-blocking Recv           |
                              | - Sliding-Window Replay Check |
                              | - Fast Packet Classifier      |
                              +---------------+---------------+
                                              |
             +--------------------------------+--------------------------------+
             |                                |                                |
             v (Gossip Messages)              v (Consensus Votes)              v (Heartbeats/Pings)
+-------------------------+      +-------------------------+      +-------------------------+
|       GossipTask        |      |      ConsensusTask      |      |        MeshTask         |
| - k-Fanout Dissemination|      | - BFT State Machine     |      | - Jittered Heartbeats   |
| - LRU Dedup Cache (64k) |      | - Propose/Prevote/Precom|      | - Phi Accrual Detector  |
| - Peer Sampling (PEX)   |      | - Bitmask Signatures    |      | - Partition Mitigation  |
| - Anti-Entropy Sync     |      | - Equivocation Slashing |      | - 2-Hop Relay Mesh      |
+------------+------------+      +------------+------------+      +------------+------------+
             |                                |                                |
             |                                v (Finalized Commits)            | (Healthy Routes)
             |                   +-------------------------+                   |
             +-----------------> |      ExecutionTask      | <-----------------+
                                 | - rivun-router Evaluation |
                                 | - WASM Driver Host      |
                                 | - Receipt Journal + MMR |
                                 +-------------------------+
```

---

### 3.2 Inter-Task Channel Graph & Data Contracts

```rust
// Proposed channel definitions in crates/rivun-node/src/actors/mod.rs

pub struct NodeActorChannels {
    pub udp_to_gossip_tx: tokio::sync::mpsc::Sender<InboundGossipPacket>,
    pub udp_to_consensus_tx: tokio::sync::mpsc::Sender<InboundConsensusPacket>,
    pub udp_to_mesh_tx: tokio::sync::mpsc::Sender<InboundMeshPacket>,
    pub udp_to_execution_tx: tokio::sync::mpsc::Sender<InboundExecutionPacket>,
    pub consensus_to_execution_tx: tokio::sync::mpsc::Sender<ConsensusFinalizedBlock>,
    pub mesh_to_execution_watch_rx: tokio::sync::watch::Receiver<MeshHealthStatus>,
    pub shutdown_tx: tokio::sync::broadcast::Sender<()>,
}

#[derive(Debug)]
pub struct InboundGossipPacket {
    pub peer: Uuid,
    pub topic: String,
    pub raw_envelope: bytes::Bytes,
    pub received_at_micros: u64,
}

#[derive(Debug)]
pub struct InboundConsensusPacket {
    pub peer: Uuid,
    pub epoch: u64,
    pub view: u64,
    pub round: u64,
    pub payload: bytes::Bytes,
}

#[derive(Debug)]
pub struct InboundMeshPacket {
    pub peer: Uuid,
    pub kind: MeshPacketKind,
    pub timestamp_micros: u64,
    pub echo_rtt_micros: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MeshPacketKind {
    HeartbeatProbe,
    HeartbeatAck,
    RelayEncapsulation,
}

#[derive(Debug)]
pub struct InboundExecutionPacket {
    pub peer: Uuid,
    pub frame: @@rivun_HEADER@@core::ZapFrame,
    pub message: @@rivun_HEADER@@envelope::ZapEnvelope,
}

#[derive(Debug, Clone)]
pub struct ConsensusFinalizedBlock {
    pub epoch: u64,
    pub round: u64,
    pub block_height: u64,
    pub payload_digest: [u8; 32],
    pub certificate: SwarmCommitCertificateRef,
}

#[derive(Debug, Clone)]
pub struct MeshHealthStatus {
    pub is_partitioned: bool,
    pub quorum_ratio: f64,
    pub reachable_validators: usize,
    pub total_validators: usize,
    pub peer_phi_scores: HashMap<Uuid, f64>,
    pub relay_paths: HashMap<Uuid, Uuid>,
}
```

---

### 3.3 Task Implementation Blueprints

#### 3.3.1 `UdpRxTask`
- **Function**: Continuously polls `endpoint.recv()`, decrypts the ChaCha20 AEAD payload, verifies replay with sliding-window nonce filter, checks peer trust, and performs sub-microsecond classification.
- **Classification Rules**:
  - If envelope subject starts with `rivun.gossip.` or frame flags contain `ZapFlags::BROADCAST`: forward to `udp_to_gossip_tx`.
  - If envelope subject is `rivun.gossip.consensus` or frame flags contain `ZapFlags::REQUIRES_CONSENSUS`: forward to `udp_to_consensus_tx`.
  - If envelope subject is `rivun.p2p.heartbeat` or `rivun.p2p.heartbeat.ack`: forward to `udp_to_mesh_tx`.
  - If envelope subject is an Action/Control message: forward to `udp_to_execution_tx`.

#### 3.3.2 `GossipTask`
- **Function**: Manages epidemic gossip dissemination.
- **State**:
  - `dedup_cache: LruCache<[u8; 32], u64>` (capacity 65,536 message IDs, 60s TTL).
  - `active_view: HashSet<Uuid>` (size $k_{\text{active}} = 8$), `passive_view: HashSet<Uuid>` (size $k_{\text{passive}} = 32$).
  - `capability_index: SwarmCapabilityIndex`.
- **Interval Timers**:
  - **PEX Timer** (every 10s): Selects random active peer, sends `PeerExchangeRequest`, updates passive view using XOR distance metric.
  - **Anti-Entropy Sync Timer** (every 5s): Exchanges digest/state hash with active peers over `rivun.gossip.sync`.
- **Dissemination Logic**:
  1. Computes $M_{\text{id}} = \text{Blake3}(\text{topic} \parallel \text{origin} \parallel \text{seq} \parallel \text{digest})$.
  2. If found in `dedup_cache`, drops message.
  3. Inserts $M_{\text{id}}$ into cache.
  4. If `current_hop >= max_hops`, drops.
  5. Selects $k_{\text{fanout}}$ random peers from `active_view` (excluding sender).
  6. Dispatches encrypted frame copies via `endpoint.send_frame()`.

#### 3.3.3 `ConsensusTask`
- **Function**: Drives 4-phase Byzantine-Fault-Tolerant State Machine Replication:
  1. **Propose**: Proposer builds `SwarmProposal` and gossips to swarm.
  2. **Prevote**: Validates proposal $\to$ signs and gossips `PrevoteVote`. Collects $\ge T$ prevotes $\to$ **Polka Certificate**.
  3. **Precommit**: Observes Polka Certificate $\to$ signs and gossips `PrecommitVote`.
  4. **Commit**: Collects $\ge T$ precommit votes $\to$ compiles `SwarmCommitCertificate` with compact signer bitmask.
- **Equivocation Detection**: Observes two distinct votes for same $(epoch, view, round, voter) \implies$ builds `EquivocationProof`, immediately sets peer trust to `Quarantined`/`Revoked`, and broadcasts slashing evidence.
- **Emission**: Sends `ConsensusFinalizedBlock` to `consensus_to_execution_tx`.

#### 3.3.4 `MeshTask`
- **Function**: Manages peer liveness, $\Phi$-accrual failure detection, partition mitigation, and 2-hop relay mesh.
- **Jitter Algorithm**:
  $$T_{\text{next}} = \min(T_{\text{max}}, T_{\text{base}} \cdot \gamma^{\text{fail}}) + \text{Uniform}(0, J_{\text{max}})$$
- **$\Phi$-Accrual Detection**:
  - Maintains sliding window of last $W=100$ heartbeat intervals.
  - Computes normal distribution suspicion metric $\Phi = -\log_{10}(P_{\text{later}}(t))$.
  - $\Phi < 8.0 \implies \text{Alive}$; $8.0 \le \Phi < 14.0 \implies \text{Suspect}$; $\Phi \ge 14.0 \implies \text{Dead}$.
- **Partition Detector**:
  - Calculates $R = \frac{N_{\text{alive\_validators}}}{N_{\text{total\_validators}}}$.
  - If $R < 0.67 \implies$ marks `is_partitioned = true`, signals `watch_tx` to enter `PartitionDegraded` mode (halts new state mutations, allows read-only queries).
- **Dynamic 2-Hop Relay**:
  - If direct path to Node B has $\Phi_B \ge 8.0$, discovers intermediary Node C with $\Phi_C < 8.0$ connected to B.
  - Encapsulates traffic to B in `ZapRelayEnvelope` targeting C.

#### 3.3.5 `ExecutionTask`
- **Function**: Routes actions, enforces policy rules, executes WASM drivers, and journals receipts.
- **Execution Flow**:
  1. Receives frame from `udp_to_execution_tx` or `consensus_to_execution_tx`.
  2. Checks mesh health status: if `is_partitioned` and frame modifies state, rejects with `PartitionDegradedError`.
  3. Evaluates `rivun-router` `RouteTable`:
     - `RouteTarget::local_driver`: Executes WASM driver in `WasmExecutor` with strict fuel metering.
     - `RouteTarget::peer`: Dispatches to destination peer (via direct UDP or 2-hop mesh relay).
     - `RouteTarget::drop`: Discards frame.
  4. Appends `SignedActionReceipt` to `ReceiptJournalStore`.
  5. Updates MMR leaf accumulator in `rivun-ledger`.

---

### 3.4 Structured Graceful Shutdown Protocol

```rust
// Proposed graceful shutdown in crates/rivun-node/src/node.rs

pub struct ZapNodeHandle {
    pub shutdown_tx: tokio::sync::broadcast::Sender<()>,
    pub task_handles: Vec<tokio::task::JoinHandle<Result<()>>>,
}

impl ZapNodeHandle {
    pub async fn shutdown(self) -> Result<()> {
        info!("Initiating rivun node graceful shutdown...");
        let _ = self.shutdown_tx.send(());

        // Await all background actor tasks
        for handle in self.task_handles {
            if let Err(join_err) = handle.await {
                warn!(%join_err, "Actor task join error during shutdown");
            }
        }

        info!("All rivun node actors terminated cleanly. Flushing journals.");
        Ok(())
    }
}
```

---

## 4. Configuration Schema Extensions (`rivun.toml`)

### 4.1 TOML Configuration Specification

```toml
[node]
bind = "0.0.0.0:9000"
key_file = ".rivun/node.key"
require_signed = true

[swarm]
enabled = true
cluster_id = "rivun-mainnet-alpha"
min_quorum_threshold = 3
auto_rebalance = true
epoch_duration_ms = 60000
max_round_timeout_ms = 3000

[gossip]
fanout = 3
max_hops = 16
anti_entropy_interval_ms = 5000
dedup_cache_size = 65536
pex_interval_ms = 10000
active_view_size = 8
passive_view_size = 32
bootnodes = [
    "192.168.1.10:9000@01914b10-0000-7000-8000-000000000001",
    "192.168.1.11:9000@01914b10-0000-7000-8000-000000000002"
]

[mesh]
heartbeat_interval_ms = 1000
heartbeat_jitter_ms = 250
phi_suspect_threshold = 8.0
phi_dead_threshold = 14.0
partition_quorum_ratio = 0.67
enable_relay_failover = true
max_relay_hops = 2
```

### 4.2 Rust Data Structures for Configuration

```rust
// Proposed additions to crates/rivun-node/src/config.rs

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SwarmConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_cluster_id")]
    pub cluster_id: String,
    #[serde(default)]
    pub min_quorum_threshold: Option<u16>,
    #[serde(default = "default_true")]
    pub auto_rebalance: bool,
    #[serde(default)]
    pub epoch_duration_ms: Option<u64>,
    #[serde(default)]
    pub max_round_timeout_ms: Option<u64>,
}

impl Default for SwarmConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            cluster_id: default_cluster_id(),
            min_quorum_threshold: None,
            auto_rebalance: true,
            epoch_duration_ms: None,
            max_round_timeout_ms: None,
        }
    }
}

fn default_cluster_id() -> String {
    "rivun-default-swarm".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GossipConfig {
    #[serde(default = "default_gossip_fanout")]
    pub fanout: usize,
    #[serde(default = "default_gossip_max_hops")]
    pub max_hops: u8,
    #[serde(default = "default_anti_entropy_interval_ms")]
    pub anti_entropy_interval_ms: u64,
    #[serde(default = "default_dedup_cache_size")]
    pub dedup_cache_size: usize,
    #[serde(default = "default_pex_interval_ms")]
    pub pex_interval_ms: u64,
    #[serde(default = "default_active_view_size")]
    pub active_view_size: usize,
    #[serde(default = "default_passive_view_size")]
    pub passive_view_size: usize,
    #[serde(default)]
    pub bootnodes: Vec<String>,
}

impl Default for GossipConfig {
    fn default() -> Self {
        Self {
            fanout: default_gossip_fanout(),
            max_hops: default_gossip_max_hops(),
            anti_entropy_interval_ms: default_anti_entropy_interval_ms(),
            dedup_cache_size: default_dedup_cache_size(),
            pex_interval_ms: default_pex_interval_ms(),
            active_view_size: default_active_view_size(),
            passive_view_size: default_passive_view_size(),
            bootnodes: Vec::new(),
        }
    }
}

fn default_gossip_fanout() -> usize { 3 }
fn default_gossip_max_hops() -> u8 { 16 }
fn default_anti_entropy_interval_ms() -> u64 { 5000 }
fn default_dedup_cache_size() -> usize { 65536 }
fn default_pex_interval_ms() -> u64 { 10000 }
fn default_active_view_size() -> usize { 8 }
fn default_passive_view_size() -> usize { 32 }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MeshConfig {
    #[serde(default = "default_heartbeat_interval_ms")]
    pub heartbeat_interval_ms: u64,
    #[serde(default = "default_heartbeat_jitter_ms")]
    pub heartbeat_jitter_ms: u64,
    #[serde(default = "default_phi_suspect_threshold")]
    pub phi_suspect_threshold: f64,
    #[serde(default = "default_phi_dead_threshold")]
    pub phi_dead_threshold: f64,
    #[serde(default = "default_partition_quorum_ratio")]
    pub partition_quorum_ratio: f64,
    #[serde(default = "default_true")]
    pub enable_relay_failover: bool,
    #[serde(default = "default_max_relay_hops")]
    pub max_relay_hops: u8,
}

impl Default for MeshConfig {
    fn default() -> Self {
        Self {
            heartbeat_interval_ms: default_heartbeat_interval_ms(),
            heartbeat_jitter_ms: default_heartbeat_jitter_ms(),
            phi_suspect_threshold: default_phi_suspect_threshold(),
            phi_dead_threshold: default_phi_dead_threshold(),
            partition_quorum_ratio: default_partition_quorum_ratio(),
            enable_relay_failover: true,
            max_relay_hops: default_max_relay_hops(),
        }
    }
}

fn default_heartbeat_interval_ms() -> u64 { 1000 }
fn default_heartbeat_jitter_ms() -> u64 { 250 }
fn default_phi_suspect_threshold() -> f64 { 8.0 }
fn default_phi_dead_threshold() -> f64 { 14.0 }
fn default_partition_quorum_ratio() -> f64 { 0.67 }
fn default_max_relay_hops() -> u8 { 2 }
```

### 4.3 Validation Rules & Operator Diagnostics
- `validate_config()` checks:
  - `gossip.fanout > 0`
  - `gossip.max_hops > 0 && gossip.max_hops <= 64`
  - `mesh.phi_dead_threshold > mesh.phi_suspect_threshold`
  - `mesh.partition_quorum_ratio >= 0.5 && mesh.partition_quorum_ratio <= 1.0`
  - Validates all `bootnodes` have format `<addr>:<port>@<uuid>`.

---

## 5. Backwards Compatibility & Integration Strategy

| Component | Backwards Compatibility Guarantee | Verification Mechanism |
| :--- | :--- | :--- |
| **`rivun.toml` Parsing** | Older `rivun.toml` files missing `[swarm]`, `[gossip]`, `[mesh]` deserialize with default values. Node behaves identically to v1 (point-to-point UDP, static PoA). | `test_legacy_config_deserialization` succeeds without error. |
| **CLI Commands** | `rivun run`, `rivun send`, `rivun capability`, `rivun pact`, `rivun agent`, `rivun receipts`, `rivun provenance` maintain 100% parameter and output compatibility. | Workspace test suite and CLI integration tests pass. |
| **Wire Protocol** | 64-byte `ZapHeader`, `AuthTrailer` (`ZSIG`), `PoaTrailer` (`ZPOA`) unaltered. New `SwarmConsensusTrailer` (`ZSC1`) activates only when `ZapFlags::REQUIRES_CONSENSUS` is combined with Swarm consensus. | Byte-level round-trip properties in `crates/rivun-core/tests/properties.rs`. |
| **Provenance Verification** | Older 6-stage chains (`Intent` $\to$ `Negotiation` $\to$ `Policy` $\to$ `Driver` $\to$ `Poa` $\to$ `Receipt`) verify with 100% success. New chains with `Consensus` verify with identical cryptographic strength. | `test_full_provenance_chain_generation_and_verification` passes on both legacy and swarm chains. |
| **`rivun-router` & `rivun-core`** | Route evaluation rules (`RouteMatch`, `RouteTarget`) evaluate without modification. Mesh relay routes wrap frames in standard `ZapRelayEnvelope` without mutating the inner `ZapFrame`. | Integration tests in `rivun-router` pass. |

---

## 6. Implementation File Plan for Implementers

| File Path | Action | Scope & Key Additions |
| :--- | :--- | :--- |
| `crates/rivun-agent/src/swarm.rs` | **Create** | `SwarmAgentCoordinator`, `SwarmIntentProposal`, `SwarmIntentStatus`, `SwarmCommitCertificateRef`, `SwarmCapabilityIndex`, `SwarmPeerCapabilityScore`. |
| `crates/rivun-agent/src/provenance.rs` | **Modify** | Add `ProvenanceStage::Consensus`, `with_consensus()` method on `ProvenanceChainBuilder`, update causal step resolution and verification. |
| `crates/rivun-agent/src/lib.rs` | **Modify** | Export `pub mod swarm;` and re-export swarm types. |
| `crates/rivun-node/src/config.rs` | **Create/Modify** | Add `SwarmConfig`, `GossipConfig`, `MeshConfig` structs and default functions. |
| `crates/rivun-node/src/actors/` | **Create** | Submodules for `udp_rx.rs`, `gossip.rs`, `consensus.rs`, `mesh.rs`, `execution.rs`. |
| `crates/rivun-node/src/lib.rs` | **Modify** | Refactor `ZapNode` into concurrent supervisor with `run_actors()`, `spawn_observability_http()`, and graceful shutdown. |

---

## 7. Verification & Acceptance Criteria Alignment

1. **Unit Test Coverage**:
   - `test_swarm_agent_coordinator_lifecycle`: Tests `submit_intent` $\to$ `mark_proposed` $\to$ `attach_commit_certificate` $\to$ `finalize_intent_with_provenance`.
   - `test_provenance_consensus_stage_verification`: Generates a chain containing `Consensus` step; verifies `ProvenanceVerificationReport::valid == true` and verifies tampering fails.
   - `test_swarm_capability_scoring`: Validates normalized composite scoring with varying trust, latency, and load factors.
   - `test_concurrent_actor_message_routing`: Spawns node actor channels; injects gossip, consensus, and action packets; validates routing to respective actor queues.
   - `test_config_with_swarm_gossip_mesh_defaults`: Validates TOML parsing with and without new tables.
2. **Build & Clippy Integrity**:
   - `cargo test -p rivun-agent -p rivun-node` passes with 0 failures.
   - `cargo clippy -p rivun-agent -p rivun-node -- -D warnings` runs with 0 warnings.

