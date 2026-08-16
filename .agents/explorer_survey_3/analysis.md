# Comprehensive Technical Survey & Architectural Blueprint
# R4 (Pact & Dispute Resolution) & R5 (Cluster Simulator & Swarm Benchmarking)

**Author:** Explorer 3 (ZAP Next-Gen Frontier Survey Phase)  
**Date:** 2026-08-15  
**Working Directory:** `c:\Users\Stagiaire\Documents\Amadou PGC\Prs\ZAP\.agents\explorer_survey_3`  
**Target Codebase:** `c:\Users\Stagiaire\Documents\Amadou PGC\Prs\ZAP`  

---

## Executive Summary

This survey provides an exhaustive architectural inspection, gap analysis, and implementation blueprint for:
1. **R4: Decentralized Agent Pact & Dispute Resolution Engine** (`crates/zap-pact`, `crates/zap-policy`, `crates/zap-agent`, `crates/zap-ledger`, `crates/zap-crypto`).
2. **R5: Cluster Simulator & Swarm Benchmarking Tooling** (`crates/zap-cli`, `crates/zap-telemetry`, workspace test harness, multi-language SDKs).

The investigation verifies that while ZAP possesses robust foundations (single-actor signed PACT records, deterministic TOML policy sets, 6-stage provenance chains, and gossip quorum voting), it currently lacks:
- **Multi-party conditional execution contracts** with escrow lock states, timeout-triggered slashing, and threshold multi-signature releases.
- **Deterministic policy dispute mediation** that programmatically adjudicates breach claims without out-of-band arbitration.
- **Full causal chain linking** from pact negotiation through resource allocation, WASM driver execution, PoA attestations, cryptographic settlement receipts, and MMR root commitments.
- **`zap cluster` & `zap swarm` CLI commands**, live multi-node topology cluster simulation, Byzantine chaos injection fixtures, and automated stress benchmarks verifying **10,000+ consensus operations/sec**.

Below is the detailed survey, state machines, data structures, and backwards-compatible evolution plan.

---

## 1. Codebase Inventory & Current State Assessment

### 1.1 `crates/zap-pact` (Current Implementation)
- **Primary Source:** `crates/zap-pact/src/lib.rs` (720 lines).
- **Core Entities:**
  - `ZapPact`: Schema version `1`. Fields: `pact_id` (Uuid), `actor` (String), `target` (String), `intent` (String), `object` (JSON Value), `terms` (JSON Value), `consent` (JSON Value), `proof` (JSON Value), `created_at_micros` (u64), `expires_at_micros` (Option<u64>), `actor_public_key` (Option<String>), `hash` (Option<String>), `signature` (Option<String>), `status` (`ZapPactStatus`), `verification` (Option<ZapPactVerification>), `revocation` (Option<ZapPactRevocation>), `timeline` (Vec<ZapPactTimelineEntry>).
  - `ZapPactStatus`: Enum with 5 states: `Draft`, `Active`, `Expired`, `Revoked`, `Invalid`.
  - `ZapPactVerification`: Verification status, verified timestamp, hash, reason.
  - `ZapPactRevocation`: `pact_id`, `revoked_by`, `reason`, `revoked_at_micros`, `signature`.
  - `ZapPactBundle`: Groups a `ZapPact` with its verifications and revocations.
- **Canonical Serialization & Cryptography:**
  - Domain Signature: `b"ZAP-PACT-v1"` using Ed25519.
  - Revocation Signature Domain: `b"ZAP-PACT-REVOCATION-v1"`.
  - Canonical Hash: `blake3:<64 lowercase hex>` of normalized JSON payload (`signing_payload_ordered`).
- **Observed Gaps for R4:**
  1. *Single-Party Restriction:* Only supports 1 actor (`actor_public_key`) and 1 target string. Cannot represent N-party agreements or multi-role participant graphs.
  2. *No Escrow State Machine:* No representation of deposited/locked escrow balances, resource tokens, fuel reserves, or unlocking conditions.
  3. *No Slashing Engine:* No timeout-based automated penalization or cryptographic slash claims.
  4. *No Dispute Resolution Engine:* No state or protocol for dispute assertion, counter-evidence submission, or deterministic policy mediation.

---

### 1.2 `crates/zap-policy` (Current Implementation)
- **Primary Source:** `crates/zap-policy/src/lib.rs` (374 lines).
- **Core Entities:**
  - `PolicySet`: `default_decision` (`Allow` or `Deny`), `rules: Vec<PolicyRule>`.
  - `PolicyRule`: `name`, `kind`, `subject`, `source_node`, `target_node`, `content_type`, `decision` (`PolicyDecision`), `required_capability`, `reason`.
  - `PolicyDecision`: `Allow`, `Deny`, `RequirePoa`, `RequireGrant`, `HumanApproval`, `SimulateFirst`.
  - `PolicyInput`: `kind`, `subject`, `source_node`, `target_node`, `content_type`, `consensus_protected`, `granted_capabilities`, `human_approved`, `simulation_passed`.
- **Observed Gaps for R4:**
  1. Policy rules only evaluate boolean match on frame metadata (`kind`, `subject`, nodes).
  2. No support for evaluating complex PACT dispute constraints (e.g., comparing output hash against agreed terms, checking elapsed duration vs deadline, verifying multi-party attestation quotas).
  3. No dispute adjudication decision outputs (e.g. `MediateRelease`, `MediateSlash`, `MediateSplit`, `RequireArbiterQuorum`).

---

### 1.3 `crates/zap-agent` (Current Implementation)
- **Primary Source:** `crates/zap-agent/src/lib.rs` (1,207 lines) and `crates/zap-agent/src/provenance.rs` (838 lines).
- **Core Entities:**
  - High-level Agent Messages: `AgentIntent`, `AgentSession`, `DelegationRequest`, `DelegationResponse`, `CapabilityNegotiationRequest`, `CapabilityNegotiationResponse`, `AgentStatusUpdate`, `AgentResult`, `AgentErrorReport`.
  - Provenance Engine (`provenance.rs`):
    - `ProvenanceStage`: `Intent`, `Negotiation`, `Policy`, `Driver`, `Poa`, `Receipt`.
    - `ProvenanceStep`: `stage`, `step_hash`, `previous_hash`, `input_data_hash`, `timestamp_micros`, `metadata`.
    - `ProvenanceChainDigest`: Sequential chain of steps verified against SHA-256 links and signed by node's Ed25519 key over `ZAP-PROVENANCE-CHAIN-v1`.
- **Observed Gaps for R4:**
  1. `ProvenanceStage` does not explicitly incorporate `PactCommit`, `EscrowLock`, `DisputeMediation`, or `SettlementReceipt`.
  2. Provenance is currently single-node focused, lacking multi-agent cross-signing for cooperative pact settlements.

---

### 1.4 `crates/zap-cli` (Current Implementation)
- **Primary Source:** `crates/zap-cli/src/main.rs` (11,748 lines).
- **Existing Commands:** 26 commands (`Keygen`, `Run`, `CheckConfig`, `Doctor`, `Send`, `Inspect`, `Capability`, `Discovery`, `Memory`, `Route`, `Trust`, `Peer`, `Schema`, `Agent`, `Pact`, `Policy`, `Pack`, `Fixtures`, `DriverManifest`, `Registry`, `Receipts`, `Incident`, `Fleet`, `Poa`, `Bench`, `Gateway`, `Provenance`).
- **Observed Gaps for R5:**
  1. *No `zap cluster` command:* Missing `zap cluster up --nodes N`, `zap cluster status`, `zap cluster down`.
  2. *No `zap swarm` command:* Missing `zap swarm bench --rate R --duration D`, `zap swarm partition-test`.
  3. `BenchCommand` only contains `Parse { iterations }`.

---

### 1.5 `crates/zap-telemetry` (Current Implementation)
- **Primary Source:** `src/doctor.rs`, `src/incident.rs`, `src/metrics.rs`, `src/topology.rs`.
- **Capabilities:**
  - `FleetDoctor`: 6 diagnostic categories (`network`, `storage`, `replay_guard`, `journal`, `pack_registry`, `certificate_validity`).
  - `ZapNodeMetricsSnapshot`: 17 Prometheus metrics.
  - `FleetTopology`: Multi-node health aggregation (`Healthy`, `Degraded`, `Critical`, `Unreachable`).
- **Observed Gaps for R5:**
  1. Lacks real-time benchmark metric aggregation (throughput ops/sec, latency histograms p50/p95/p99, consensus duration, gossip propagation latency).
  2. Lacks cluster simulation telemetry to monitor Byzantine nodes, packet loss injections, and partition states.

---

### 1.6 Multi-Language SDKs & Golden Fixtures
- **SDKs:** Python (`sdks/python`), Go (`sdks/go`), TypeScript (`sdks/typescript`), Rust (`sdks/rust`).
- **Fixtures:** `fixtures/pact-record-v1.json`, `fixtures/pact-bundle-v1.json`, `fixtures/protocol/*.json`.
- **Integrity Requirement:** All v1 structures and tests must remain backward-compatible without breaking existing binary or JSON signatures.

---

## 2. Requirement R4: Decentralized Agent Pact & Dispute Resolution Engine

### 2.1 Problem Boundary & Scope
In multi-agent collaborative workflows (e.g. autonomous trading, distributed compute delegation, physical robotics actuation), agents must enter binding commitments with conditional execution terms:
- Funds/fuel must be locked in escrow.
- If the service provider completes execution on time and produces valid proof, funds are released via multi-signature consensus.
- If the provider times out or produces invalid output, escrowed stake is slashed and refunded.
- If a dispute arises (e.g., conflicting attestation proofs or SLA breach claims), a deterministic dispute mediation engine resolves the outcome according to predefined policy sets without subjective human intervention.
- The entire transaction sequence must be causally verifiable via cryptographic chain digests.

---

### 2.2 Extended Multi-Party Pact Data Structures (`crates/zap-pact`)

#### 2.2.1 Participant Graph & Roles
```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PactPartyRole {
    Initiator,       // Creator of the pact
    Executor,        // Agent responsible for performing work
    EscrowHolder,    // Node / Quorum holding escrow lock
    Validator,       // Quorum validator certifying execution
    Arbiter,         // Designated mediator for dispute resolution
    Beneficiary,     // Recipient of output or settlement payout
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PactParticipant {
    pub node_id: Uuid,
    pub role: PactPartyRole,
    pub public_key: String, // Base64 Ed25519 public key
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signature: Option<String>, // Base64 signature over canonical pact hash
    pub joined_at_micros: u64,
}
```

#### 2.2.2 Escrow Specification & Lock Mechanism
```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PactEscrowLock {
    pub escrow_id: Uuid,
    pub resource_type: String,       // e.g. "fuel_units", "compute_credits", "zap_token"
    pub amount: u64,
    pub depositor: Uuid,
    pub locked_at_micros: u64,
    pub release_threshold: u16,      // M-of-N signatures required for release
    pub release_signers: Vec<Uuid>,  // Authorized signers (e.g. Executor + Validator Quorum)
    pub timeout_micros: u64,         // Absolute timestamp after which slash/refund can trigger
    pub slash_recipient: Option<Uuid>, // Who receives slashed funds (default: depositor/refund)
    pub slash_burn_percent: u8,      // % of slash penalty permanently burned (anti-collusion)
    pub lock_tx_hash: String,        // Hash of escrow deposit receipt
}
```

#### 2.2.3 Extended Pact Status State Machine
```
       ┌─────────────────┐
       │      Draft      │
       └────────┬────────┘
                │ All parties co-sign
                ▼
       ┌─────────────────┐
       │ PendingDeposit  │
       └────────┬────────┘
                │ Escrow confirmed & locked
                ▼
       ┌─────────────────┐
       │  EscrowLocked   │
       └────────┬────────┘
                │ Execution started
                ▼
       ┌─────────────────┐
       │ ActiveExecution │
       └────┬───┬───┬────┘
            │   │   │
  Proof OK  │   │   │ Timeout reached
  Multi-Sig │   │   │ without proof
            │   │   ▼
            │   │  ┌─────────────────┐
            │   │  │     Slashed     │
            │   │  └─────────────────┘
            │   │
            │   │ SLA breach / dispute filed
            │   ▼
            │  ┌─────────────────┐
            │  │    Disputed     │
            │  └────────┬────────┘
            │           │ Deterministic Mediation
            │           ▼
            │  ┌─────────────────┐
            │  │ MediateSettled  │
            │  └─────────────────┘
            ▼
       ┌─────────────────┐
       │     Settled     │
       └─────────────────┘
```

```rust
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ZapPactStatus {
    #[default]
    Draft,
    PendingDeposit,
    EscrowLocked,
    ActiveExecution,
    Settled,
    Slashed,
    Disputed,
    MediateSettled,
    Expired,
    Revoked,
    Invalid,
}
```

#### 2.2.4 Multi-Party Pact Contract (`MultiPartyPact`)
To maintain 100% backward compatibility with `ZapPact` (v1 single-party), `MultiPartyPact` is introduced with `schema_version = 2` or as an extended wrapper:
```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MultiPartyPact {
    pub schema_version: u8, // 2
    pub pact_id: Uuid,
    pub intent: String,
    pub participants: Vec<PactParticipant>,
    pub terms: serde_json::Value,
    pub escrow: Option<PactEscrowLock>,
    pub created_at_micros: u64,
    pub deadline_micros: u64,
    pub status: ZapPactStatus,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub settlement_signatures: Vec<PactSettlementSignature>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dispute: Option<PactDisputeRecord>,
    pub hash: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PactSettlementSignature {
    pub signer_node_id: Uuid,
    pub role: PactPartyRole,
    pub signature: String,
    pub signed_at_micros: u64,
}
```

---

### 2.3 Dispute Resolution & Deterministic Policy Mediation

#### 2.3.1 Dispute Assertion & Evidence
```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PactDisputeRecord {
    pub dispute_id: Uuid,
    pub pact_id: Uuid,
    pub raised_by: Uuid,
    pub reason_code: DisputeReasonCode,
    pub evidence_hash: String,
    pub claimed_breach: String,
    pub raised_at_micros: u64,
    pub mediation: Option<DisputeMediationResult>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DisputeReasonCode {
    TimeoutExceeded,
    OutputHashMismatch,
    InvalidExecutionAttestation,
    UnauthorizedResourceConsumption,
    ContractTermsBreach,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DisputeMediationResult {
    pub decision: DisputeDecision,
    pub mediated_by: MediationAuthority,
    pub rationale: String,
    pub payout_allocations: BTreeMap<Uuid, u64>, // node_id -> amount
    pub slash_penalty: u64,
    pub burn_amount: u64,
    pub mediated_at_micros: u64,
    pub mediation_proof_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DisputeDecision {
    ReleaseFullToExecutor,
    RefundFullToDepositor,
    SlashExecutorPenalty,
    SplitSettlement {
        depositor_share_pct: u8,
        executor_share_pct: u8,
    },
    DismissDispute,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MediationAuthority {
    DeterministicPolicyRule { rule_index: usize, rule_name: String },
    ValidatorQuorum { threshold: u16, validator_count: usize },
    DesignatedArbiter { arbiter_node_id: Uuid },
}
```

#### 2.3.2 Deterministic Dispute Policy Evaluation in `zap-policy`
When a dispute occurs, the dispute engine executes `PolicySet::evaluate_dispute`:
```rust
pub struct DisputePolicyInput<'a> {
    pub pact_id: Uuid,
    pub dispute_reason: DisputeReasonCode,
    pub claimed_breach: &'a str,
    pub elapsed_micros: u64,
    pub timeout_micros: u64,
    pub output_matches_terms: bool,
    pub poa_quorum_verified: bool,
    pub arbiter_attestation: bool,
    pub escrow_amount: u64,
}

impl PolicySet {
    pub fn evaluate_dispute(&self, input: &DisputePolicyInput<'_>) -> DisputeMediationResult {
        // Deterministic adjudication rules:
        // 1. If timeout is exceeded and execution is unproven -> Refund + Slash
        if input.elapsed_micros > input.timeout_micros && !input.poa_quorum_verified {
            let slash_penalty = (input.escrow_amount * 20) / 100; // 20% slash penalty
            let burn_amount = (slash_penalty * 50) / 100;          // 50% of penalty burned
            let refund_amount = input.escrow_amount;
            return DisputeMediationResult {
                decision: DisputeDecision::SlashExecutorPenalty,
                mediated_by: MediationAuthority::DeterministicPolicyRule {
                    rule_index: 0,
                    rule_name: "timeout_default_slash".to_string(),
                },
                rationale: format!(
                    "Execution elapsed {}us exceeded deadline {}us without PoA quorum",
                    input.elapsed_micros, input.timeout_micros
                ),
                payout_allocations: BTreeMap::new(),
                slash_penalty,
                burn_amount,
                mediated_at_micros: zap_core::now_micros().unwrap_or(0),
                mediation_proof_hash: format!("blake3:{}", blake3::hash(b"timeout_slash").to_hex()),
            };
        }

        // 2. If PoA quorum is verified and output matches terms -> Release
        if input.poa_quorum_verified && input.output_matches_terms {
            return DisputeMediationResult {
                decision: DisputeDecision::ReleaseFullToExecutor,
                mediated_by: MediationAuthority::DeterministicPolicyRule {
                    rule_index: 1,
                    rule_name: "valid_poa_release".to_string(),
                },
                rationale: "Execution output verified with PoA validator quorum".to_string(),
                payout_allocations: BTreeMap::new(),
                slash_penalty: 0,
                burn_amount: 0,
                mediated_at_micros: zap_core::now_micros().unwrap_or(0),
                mediation_proof_hash: format!("blake3:{}", blake3::hash(b"valid_poa_release").to_hex()),
            };
        }

        // 3. Fallback: Split settlement
        DisputeMediationResult {
            decision: DisputeDecision::SplitSettlement {
                depositor_share_pct: 50,
                executor_share_pct: 50,
            },
            mediated_by: MediationAuthority::DeterministicPolicyRule {
                rule_index: 2,
                rule_name: "default_equal_split".to_string(),
            },
            rationale: "Disputed execution without conclusive quorum defaults to 50/50 split".to_string(),
            payout_allocations: BTreeMap::new(),
            slash_penalty: 0,
            burn_amount: 0,
            mediated_at_micros: zap_core::now_micros().unwrap_or(0),
            mediation_proof_hash: format!("blake3:{}", blake3::hash(b"equal_split").to_hex()),
        }
    }
}
```

---

### 2.4 Causal Execution Chains Across Lifecycle Stages
We expand `ProvenanceStage` and `ProvenanceChainBuilder` to bind the full lifecycle:
$$\text{PactNegotiation} \to \text{EscrowLock} \to \text{PolicyCheck} \to \text{DriverExecution} \to \text{PoAAttestation} \to \text{SettlementReceipt} \to \text{MMRRoot}$$

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProvenanceStage {
    Intent,
    Negotiation,
    PactCommit,        // NEW: Multi-party pact hash binding
    EscrowLock,        // NEW: Escrow deposit transaction & resource allocation
    Policy,
    Driver,
    Poa,
    DisputeMediation,  // NEW: Dispute resolution stage if contested
    Receipt,           // Cryptographic action receipt
    MmrCommitment,     // NEW: Batch MMR inclusion root
}
```

---

## 3. Requirement R5: Cluster Simulator & Swarm Benchmarking Tooling

### 3.1 Problem Boundary & Scope
To validate ZAP's real-world scalability and fault tolerance:
1. Operators require direct CLI commands (`zap cluster up`, `zap swarm bench`, `zap swarm partition-test`) to spin up local virtual meshes and evaluate latency/throughput.
2. The benchmark tooling must generate and verify **10,000+ consensus operations per second** under concurrent client workloads.
3. The cluster simulator must deterministically inject network chaos (packet loss, latency spikes, Byzantine nodes, split-brain partitions) and prove that the swarm detects partitions, recovers gracefully, and preserves ledger consistency.

---

### 3.2 CLI Command Specifications (`zap-cli`)

#### 3.2.1 `zap cluster` Command Tree
```
zap cluster
├── up        # Launch an in-process / multi-process simulated cluster topology
├── status    # Query live health and topology across active cluster nodes
└── down      # Gracefully tear down running simulated cluster
```

**Options for `zap cluster up`:**
- `--nodes <N>`: Number of cluster nodes (default: 3).
- `--topology <ring|mesh|star|bipartite>`: Interconnection network topology (default: `mesh`).
- `--bind-base-port <PORT>`: Starting UDP port for simulated nodes (default: `9000`).
- `--data-dir <PATH>`: Directory for ephemeral keys, memory, and receipt journals (default: `.zap/cluster`).
- `--poa-threshold <T>`: Required PoA quorum threshold (default: $\lfloor 2N/3 \rfloor + 1$).
- `--daemon`: Run in background; emit PID file and socket path.
- `--json`: Output machine-readable cluster topology metadata.

**Example Invocations:**
```bash
# Launch a 5-node full-mesh cluster with 4-of-5 PoA threshold
zap cluster up --nodes 5 --topology mesh --poa-threshold 4

# Query status
zap cluster status --json

# Terminate cluster
zap cluster down
```

#### 3.2.2 `zap swarm` Command Tree
```
zap swarm
├── bench             # Execute high-throughput concurrent consensus benchmark
└── partition-test    # Run automated network partition and chaos recovery test
```

**Options for `zap swarm bench`:**
- `--rate <R>`: Target operations per second (e.g. `10000` or `0` for uncapped max throughput).
- `--duration <D>`: Test duration in seconds (e.g. `10s`, `30s`, `60s`).
- `--concurrency <C>`: Number of concurrent worker tasks/threads (default: `num_cpus * 4`).
- `--nodes <N>`: Target cluster nodes for distributed load (default: all active).
- `--batch-size <B>`: Transactions per cryptographic receipt batch (default: `100`).
- `--payload-size <BYTES>`: Size of dummy payload per frame (default: `128`).
- `--metrics-out <PATH>`: Optional JSON path to write benchmark summary and latency percentiles.

**Options for `zap swarm partition-test`:**
- `--nodes <N>`: Number of nodes in test cluster (default: 5).
- `--partition-mode <MODE>`: `split-brain` (e.g. 3 vs 2), `isolate-node`, `packet-loss`, `latency-spike`.
- `--loss-rate <FLOAT>`: Packet drop probability (0.0 to 1.0) when testing packet loss.
- `--latency-ms <MS>`: Synthetic delay added to cross-partition packets (e.g. `200ms`).
- `--heal-after <SECONDS>`: Duration before automatically restoring network connectivity.

---

### 3.3 High-Throughput Benchmarking Architecture (10,000+ Ops/Sec)

#### 3.3.1 Bottleneck Analysis & Performance Budget
To sustain 10,000+ consensus operations/sec:
- Budget per operation: $\le 100\,\mu\text{s}$ per consensus decision.
- Cryptographic Signatures: Single-thread Ed25519 verification takes $\approx 40\,\mu\text{s}$. 10,000 ops/sec requires:
  1. Rayon parallel verification / batch verification (`ed25519_dalek::verify_batch`).
  2. BLAKE3 SIMD hashing (sub-microsecond for frames).
  3. Merkle Mountain Range (MMR) batched leaf insertion and peak bagging.
  4. Lock-free channel buffers (Tokio unbounded / crossbeam ring-buffer) between network receive and consensus workers.

#### 3.3.2 Benchmark Engine Architecture
```
  ┌─────────────────────────────────────────────────────────────┐
  │                   Load Generator Engine                     │
  │  (Tokio Concurrency Workers C=16..64, Pipelined Frame Gen)  │
  └──────────────┬───────────────────────────────┬──────────────┘
                 │ Frame Stream                  │ Frame Stream
                 ▼                               ▼
  ┌──────────────────────────────┐ ┌──────────────────────────────┐
  │       Node 1 (Leader)        │ │       Node 2 (Follower)      │
  │ ┌──────────────────────────┐ │ │ ┌──────────────────────────┐ │
  │ │ Lock-Free Inbound Ring   │ │ │ │ Lock-Free Inbound Ring   │ │
  │ └────────────┬─────────────┘ │ │ └────────────┬─────────────┘ │
  │              ▼               │ │              ▼               │
  │ ┌──────────────────────────┐ │ │ ┌──────────────────────────┐ │
  │ │ Rayon Batch Ed25519 & MMR│ │ │ │ Rayon Batch Ed25519 & MMR│ │
  │ └────────────┬─────────────┘ │ │ └────────────┬─────────────┘ │
  │              ▼               │ │              ▼               │
  │ ┌──────────────────────────┐ │ │ ┌──────────────────────────┐ │
  │ │ Gossip Quorum T-of-N Vote│ │ │ │ Gossip Quorum T-of-N Vote│ │
  │ └────────────┬─────────────┘ │ │ └────────────┬─────────────┘ │
  └──────────────┼───────────────┘ └──────────────┼───────────────┘
                 │                                │
                 ▼                                ▼
  ┌─────────────────────────────────────────────────────────────┐
  │                 Telemetry & Stats Collector                 │
  │   - Ops/Sec Throughput (Current, Average, Peak)             │
  │   - Latency Histogram (p50, p90, p95, p99, p99.9, max)      │
  │   - Memory & CPU Core Utilization                           │
  │   - Zero-Copy MMR Batch Rollup Commitments                  │
  └─────────────────────────────────────────────────────────────┘
```

#### 3.3.3 Benchmark Result Data Structure
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwarmBenchmarkSummary {
    pub test_name: String,
    pub target_rate_ops: u64,
    pub duration_seconds: f64,
    pub total_operations: u64,
    pub successful_operations: u64,
    pub failed_operations: u64,
    pub throughput_ops_per_sec: f64,
    pub latency_p50_micros: u64,
    pub latency_p90_micros: u64,
    pub latency_p95_micros: u64,
    pub latency_p99_micros: u64,
    pub latency_p999_micros: u64,
    pub latency_max_micros: u64,
    pub mmr_batch_count: usize,
    pub consensus_quorum_threshold: u16,
    pub active_nodes: usize,
}
```

---

### 3.4 Simulated Byzantine Network Chaos Harness

#### 3.4.1 Chaos Channel Adapter (`ChaosTransport`)
To simulate real-world conditions without requiring separate physical machines, a virtual network channel adapter sits between the UDP sockets and the `ZapEndpoint`:
```rust
#[derive(Debug, Clone)]
pub struct ChaosConfig {
    pub packet_loss_rate: f64,          // 0.0 to 1.0 (e.g. 0.05 = 5% drop)
    pub min_latency_ms: u64,            // Artificial delay floor
    pub max_latency_ms: u64,            // Artificial delay ceiling
    pub duplicate_rate: f64,            // Probability of frame duplication
    pub corrupt_signature_rate: f64,    // Byzantine bit-flip mutation rate
    pub partitions: Vec<HashSet<Uuid>>, // Disjoint node partition groups
}
```

#### 3.4.2 Chaos Injection Matrix
| Chaos Scenario | Failure Mode Injected | Expected Swarm Behavior | Pass Criteria |
|---|---|---|---|
| **Split-Brain Partition (3 vs 2 in 5-node cluster)** | Drop all UDP packets between Group A {N1,N2,N3} and Group B {N4,N5} | Group A maintains 3/5 quorum and continues processing. Group B halts consensus ($2 < 3$). | 0 conflicting state forks; Group B catches up via anti-entropy gossip upon partition healing. |
| **Random Packet Loss (10%)** | 10% packet drop on all inter-node channels | Heartbeat jitter backoff and retransmissions handle drops. | Overall consensus throughput maintains $\ge 80\%$ peak rate; no lost ledger receipts. |
| **Byzantine Equivocator Node** | Single node signs conflicting frame digests for the same slot | Gossip quorum rejects invalid second vote; marks node as `Suspect`/`Untrusted`. | 0 invalid commits; Byzantine node isolated from quorum. |
| **Transient Latency Spike (500ms on 1 node)** | Injects 500ms delay on Node 3 | Swarm detects latency degradation via `VectorClock` and routes around Node 3. | P99 latency of healthy quorum remains $< 50\,\text{ms}$. |

---

## 4. Golden Protocol Fixtures & Multi-Language SDK Compatibility

### 4.1 Backward Compatibility Verification
1. **Existing Golden Fixtures:**
   - `fixtures/pact-record-v1.json`: Uses single-actor `actor`, `target`, `hash`, `signature`. Must continue validating under `zap-pact::ZapPact`.
   - `fixtures/pact-bundle-v1.json`: Must continue verifying offline with `ZapPactBundle`.
   - `fixtures/protocol/*.json`: Wire frames and control envelopes must maintain unchanged binary offsets and JSON tags.
2. **SDK Parity Matrix:**
   - **Rust:** Native implementation across all crates.
   - **Python (`sdks/python`):** `zap_sdk/zapstore.py` and `protocol.py` verify PACT hashes (`pact_hash`) and signatures (`verify_pact`).
   - **Go (`sdks/go`):** `protocol_test.go` verifies `PactCanonicalSigningBytes` and `PactHash`.
   - **TypeScript (`sdks/typescript`):** `protocol.ts` and `zapstore.ts` verify offline PACT bundles.
3. **Compatibility Strategy:**
   - Single-party `ZapPact` remains intact with `schema_version = 1`.
   - Multi-party contracts use `MultiPartyPact` or optional additive fields (`parties: Option<Vec<PactParticipant>>`, `escrow: Option<PactEscrowLock>`, `dispute: Option<PactDisputeRecord>`).
   - Old SDKs ignore unrecognized optional JSON keys while continuing to verify base signatures.

---

## 5. File Inventory & Proposed Architectural Modifications

### 5.1 Crates to Enhance / Create

| Crate / Path | Current Status | Proposed Changes for R4 & R5 |
|---|---|---|
| `crates/zap-pact/src/lib.rs` | 720 lines (V1 Single-Party) | Add `PactPartyRole`, `PactParticipant`, `PactEscrowLock`, `PactDisputeRecord`, `MultiPartyPact`, `ZapPactStatus` states (`PendingDeposit`, `EscrowLocked`, `Disputed`, `MediateSettled`), multi-sig verification. |
| `crates/zap-policy/src/lib.rs` | 374 lines (Standard Policy) | Add `DisputePolicyInput`, `DisputeMediationResult`, `DisputeDecision`, and `PolicySet::evaluate_dispute()`. |
| `crates/zap-agent/src/provenance.rs` | 838 lines (6 stages) | Add `PactCommit`, `EscrowLock`, `DisputeMediation`, and `MmrCommitment` stages to `ProvenanceStage`. |
| `crates/zap-cli/src/main.rs` | 11,748 lines | Add `Commands::Cluster { command: ClusterCommand }` and `Commands::Swarm { command: SwarmCommand }`. Implement in-process topology runner and live benchmarks. |
| `crates/zap-telemetry/src/metrics.rs` | 288 lines | Add swarm benchmark metrics (`zap_consensus_ops_total`, `zap_consensus_latency_micros`, `zap_pact_disputes_total`, `zap_escrow_locked_total`). |
| `crates/zap-telemetry/src/doctor.rs` | 595 lines | Add cluster simulation health checks to `FleetDoctor`. |
| `tests/e2e/tests/e2e_suite.rs` | 2,142 lines | Add Tier 1-4 tests for Multi-Party Pacts, Escrow Locks, Timeout Slashes, Dispute Mediation, `zap cluster`, and `zap swarm`. |
| `crates/zap-node/benches/` | Existing micro-benchmarks | Add `benches/swarm_consensus.rs` validating 10,000+ ops/sec throughput. |

---

## 6. Synthesis & Recommended Next Steps

1. **R4 Implementation Sequencing:**
   - Step 1: Extend `zap-pact` data models (Multi-party, Escrow, Slashes, Disputes) while preserving V1 compatibility.
   - Step 2: Implement dispute mediation in `zap-policy` and integrate with `ProvenanceChainBuilder` in `zap-agent`.
   - Step 3: Connect pact settlement receipts to `zap-ledger` MMR batch accumulator.
2. **R5 Implementation Sequencing:**
   - Step 1: Implement `zap cluster` (`up`, `status`, `down`) in `zap-cli` using virtual in-process `ZapNode` actors.
   - Step 2: Implement `zap swarm` (`bench`, `partition-test`) with chaos transport injection.
   - Step 3: Build high-concurrency benchmark harness targeting 10,000+ consensus ops/sec with Rayon/Tokio pipelining.
3. **Verification Protocol:**
   - Run `cargo test --workspace --all-targets`.
   - Run `cargo clippy --workspace --all-targets -- -D warnings`.
   - Verify golden fixtures in `fixtures/` and run Python/Go/TS SDK tests.

---
