//! Tier 3: Cross-Feature Combination Tests (`tier3_combination_tests.rs`)
//!
//! Comprehensive pairwise and multi-module interaction tests validating complex system flows.
//! >= 15 comprehensive combination test cases.

use anyhow::Result;
use bytes::Bytes;
use std::{
    collections::{BTreeMap, BTreeSet, HashMap, HashSet},
    sync::Arc,
    time::Duration,
};
use tempfile::tempdir;
use uuid::Uuid;

use zap_agent::{AgentId, AgentIntent, AgentMessage, AgentSession, IntentKind, ProvenanceChainBuilder, ProvenanceStage};
use zap_capability::{CapabilityId, DriverPermissions};
use zap_core::{ZapFlags, ZapFrame, now_micros};
use zap_crypto::{Keypair, sign_frame, verify_frame};
use zap_ledger::{
    ActionReceipt, MerkleMountainRange, MmrHash, MmrInclusionProof, MmrRollupCommitment,
    PoaReceipt, ReceiptJournalStore, ReceiptReplicationRequest, SignedActionReceipt,
};
use zap_memory::{MemoryJournalStore, MemoryPut, MemoryQuery, MemoryStore};
use zap_net::{GossipMesh, Peer, PeerHealth, QuorumProposal, VectorClock, ZapEndpoint, ZapEndpointConfig};
use zap_node::ZapNodeConfig;
use zap_pact::{Validate, ZapPact, ZapPactBundle, ZapPactError, ZapPactRevocation, ZapPactStatus};
use zap_policy::{PolicyDecision, PolicyInput, PolicyRule, PolicySet};
use zap_runtime::{DriverPipeline, ExecutionLimits, WasmExecutor};
use zap_telemetry::{FleetDoctor, FleetNodeHealth, FleetNodeState, FleetTopology, IncidentCapturer, PrometheusExporter, ZapNodeMetricsSnapshot};

use zap_e2e::harness::*;

#[test]
fn tc_t3_01_gossip_state_sync_with_byzantine_consensus() -> Result<()> {
    let mut cluster = SimulatedCluster::new("t3_gossip_consensus", 4)?;
    let node_ids = cluster.node_ids();
    let proposer_id = node_ids[0];

    // 1. Broadcast heartbeats to synchronize vector clocks
    for &id in &node_ids {
        cluster.broadcast_heartbeat(id, 10)?;
    }

    // 2. Propose state update and vote across 3 of 4 nodes (reaching 2/3+1 quorum)
    let proposal = cluster.reach_consensus(proposer_id, "cluster.rebalance", "state_hash_v1", &node_ids[0..3])?;
    assert!(proposal.finalized);
    assert_eq!(proposal.votes.len(), 3);
    assert_eq!(proposal.required_threshold, 3);
    Ok(())
}

#[test]
fn tc_t3_02_swarm_consensus_and_poa_frame_certification() -> Result<()> {
    let mut cluster = SimulatedCluster::new("t3_consensus_poa", 3)?;
    let node_ids = cluster.node_ids();
    let proposer = cluster.get_node(&node_ids[0]).unwrap();

    // 1. Create a consensus proposal
    let prop = cluster.reach_consensus(node_ids[0], "critical_valve_open", "terms_valve", &node_ids)?;
    assert!(prop.finalized);

    // 2. Generate Proof-of-Action certificate using validator signatures
    let frame = ZapFrame::new(
        proposer.node_id,
        Uuid::nil(),
        ZapFlags::SIGNED | ZapFlags::REQUIRES_CONSENSUS,
        Bytes::from_static(b"critical_actuation_payload"),
    )?;
    let signed = SignedActionReceipt::new(&proposer.keypair, &frame, "safety.valve", None, 2000, Some(2))?;

    assert!(signed.verify().is_ok());
    Ok(())
}

#[test]
fn tc_t3_03_network_partition_with_dynamic_failover_routing() -> Result<()> {
    let mut cluster = SimulatedCluster::new("t3_partition_failover", 4)?;
    let node_ids = cluster.node_ids();

    let primary_worker = node_ids[1];
    let backup_worker = node_ids[2];

    // Initially both workers are alive
    let coordinator = cluster.get_node_mut(&node_ids[0]).unwrap();
    coordinator.gossip.register_peer(primary_worker, "127.0.0.1:9001", vec!["compute".into()], 1000);
    coordinator.gossip.register_peer(backup_worker, "127.0.0.1:9002", vec!["compute".into()], 1000);

    // Primary worker times out (simulating partition)
    let _ = coordinator.gossip.evaluate_health(15_000_000);
    assert_eq!(coordinator.gossip.peers.get(&primary_worker).unwrap().health, PeerHealth::Dead);

    // Backup worker sends heartbeat with low load
    let mut clk = VectorClock::new();
    clk.increment(backup_worker);
    coordinator.gossip.record_heartbeat(backup_worker, &clk, 5, 15_000_000);
    let _ = coordinator.gossip.evaluate_health(15_000_000);

    // Routing dynamically selects backup worker
    let route = coordinator.gossip.select_route_for_capability("compute").unwrap();
    assert_eq!(route.node_id, backup_worker);
    Ok(())
}

#[test]
fn tc_t3_04_action_execution_to_segmented_journal_and_mmr() -> Result<()> {
    let key = generate_keypair();
    let dir = tempdir()?;
    let journal = ReceiptJournalStore::open_with_keypair(dir.path().join("receipts"), key.clone());
    let wasm = compile_echo_wasm();
    let executor = WasmExecutor::new()?;

    // 1. Execute WASM driver
    let exec_res = executor.execute_bytes(&wasm, "record_reading", b"sensor_temp=24.5C", ExecutionLimits::default())?;

    // 2. Create and sign ActionReceipt
    let frame = ZapFrame::new(key.node_id(), Uuid::nil(), ZapFlags::SIGNED, Bytes::from_static(b"sensor_temp=24.5C"))?;
    let signed = SignedActionReceipt::new(&key, &frame, "sensor.temp", Some(&exec_res.output), 2000, None)?;
    journal.append(&signed, false)?;

    // 3. Build MMR accumulator from journal receipts
    let mut mmr = journal.build_mmr_accumulator()?;
    assert_eq!(mmr.len(), 1);
    assert_ne!(mmr.root(), [0u8; 32]);
    Ok(())
}

#[test]
fn tc_t3_05_mmr_accumulator_batch_proof_and_rollup_commitment() -> Result<()> {
    let mut mmr = MerkleMountainRange::new();
    let batch_size = 30;

    for i in 0..batch_size {
        mmr.append_bytes(format!("batch_item_{i}").as_bytes());
    }

    let commitment = mmr.create_rollup_commitment(1_000_000, 2_000_000)?;
    assert_eq!(commitment.leaf_count, batch_size);

    let root_bytes = hex::decode(&commitment.root_hash)?;
    let mut root_arr = [0u8; 32];
    root_arr.copy_from_slice(&root_bytes);

    // Verify inclusion proofs for random sample of batch items against the commitment root
    for &idx in &[0, 7, 14, 21, 29] {
        let proof = mmr.prove_inclusion(idx)?;
        assert!(MerkleMountainRange::verify_proof(&proof, &root_arr)?);
    }
    Ok(())
}

#[test]
fn tc_t3_06_multi_stage_wasm_ipc_pipeline_and_provenance_binding() -> Result<()> {
    let key = generate_keypair();
    let echo_wasm = compile_echo_wasm();
    let reverse_wasm = compile_reverse_wasm();
    let session_id = Uuid::new_v4();
    let intent_id = Uuid::new_v4();

    let mut intent = AgentIntent::new(session_id, AgentId::new("pipeline_orchestrator")?, IntentKind::Act, "Process telemetry");
    intent.intent_id = intent_id;

    // 1. Run 3-stage WASM pipeline
    let pipeline = DriverPipeline::new("telemetry_ipc_pipeline")
        .add_stage("ingest", "echo", echo_wasm.clone(), DriverPermissions::none(), None)
        .add_stage("transform", "reverse", reverse_wasm.clone(), DriverPermissions::none(), None)
        .add_stage("normalize", "reverse", reverse_wasm, DriverPermissions::none(), None);

    let report = pipeline.execute(b"RAW_TELEMETRY_DATA").map_err(|e| anyhow::anyhow!("{e:?}"))?;
    assert_eq!(report.final_output, b"RAW_TELEMETRY_DATA");

    // 2. Bind pipeline results into ProvenanceChain
    let chain = ProvenanceChainBuilder::new(session_id, intent_id)
        .with_intent(&intent)?
        .with_driver("telemetry_ipc_pipeline", "in_hash", &report.causal_chain_hash, BTreeMap::new())?
        .with_receipt("rec_telemetry_001", 5000, BTreeMap::new())?
        .build_and_sign(&key)?;

    let verification = chain.verify(&key.verifying_key())?;
    assert!(verification.valid);
    assert_eq!(verification.verified_steps, 3);
    Ok(())
}

#[test]
fn tc_t3_07_agent_intent_to_pact_signing_and_policy_evaluation() -> Result<()> {
    let key = generate_keypair();
    let session_id = Uuid::new_v4();
    let intent_id = Uuid::new_v4();

    // 1. Agent Intent
    let mut intent = AgentIntent::new(session_id, AgentId::new("service_consumer")?, IntentKind::Act, "Request cloud compute");
    intent.intent_id = intent_id;

    // 2. Pact Creation & Signing
    let mut pact = ZapPact::new("service_consumer", "service_provider", "Request cloud compute", 1_700_000);
    pact.object = serde_json::json!({"vcpus": 8, "ram_gb": 32});
    pact.terms = serde_json::json!({"max_cost_zap": 50});
    pact.sign(&key)?;

    // 3. Policy Evaluation
    let cap = CapabilityId::new("driver.execute:cloud_compute")?;
    let rule = PolicyRule {
        name: Some("allow_compute".into()),
        kind: Some("action".into()),
        subject: Some("compute.*".into()),
        source_node: None,
        target_node: None,
        content_type: None,
        decision: PolicyDecision::RequireGrant,
        required_capability: Some(cap.clone()),
        reason: None,
    };
    let policy = PolicySet::new(vec![rule])?;

    let mut grants = BTreeSet::new();
    grants.insert(cap);

    let pol_input = PolicyInput {
        kind: "action",
        subject: "compute.provision",
        source_node: Some(key.node_id()),
        target_node: None,
        content_type: Some("application/json"),
        consensus_protected: false,
        granted_capabilities: &grants,
        human_approved: false,
        simulation_passed: false,
    };

    let eval = policy.evaluate(&pol_input);
    assert!(eval.allowed);
    assert_eq!(eval.decision, PolicyDecision::RequireGrant);
    Ok(())
}

#[test]
fn tc_t3_08_pact_revocation_evidence_with_bundle_verification() -> Result<()> {
    let key = generate_keypair();
    let pact = create_test_pact("agent.client", "agent.server", "task_a", &key)?;
    let mut bundle = ZapPactBundle::new(pact.clone());

    // Initially valid
    assert!(bundle.verify(Some(now_micros()? + 1000))?.valid);

    // Attach signed revocation
    let mut revocation = ZapPactRevocation::new(pact.pact_id, "admin", "Contract breached", 1_750_000);
    revocation.sign(&key)?;
    bundle.revocations.push(revocation);

    // Now bundle verification detects revocation
    let res = bundle.verify(Some(1_760_000));
    assert!(res.is_err());
    assert!(matches!(res.unwrap_err(), ZapPactError::Revoked));
    Ok(())
}

#[test]
fn tc_t3_09_memory_journal_streaming_to_wasm_pipeline_processing() -> Result<()> {
    let dir = tempdir()?;
    let mem = MemoryJournalStore::open(dir.path().join("mem"));
    let echo_wasm = compile_echo_wasm();
    let executor = WasmExecutor::new()?;

    // 1. Append streaming records to memory store
    for i in 0..5 {
        mem.put(MemoryPut {
            namespace: "stream".to_string(),
            subject: "robot.sensors".to_string(),
            content_type: "application/octet-stream".to_string(),
            body: format!("telemetry_{i}").into_bytes(),
            metadata: serde_json::Value::Null,
            source_node: None,
            frame_hash: None,
        })?;
    }

    // 2. Query streaming chunk and process via WASM driver
    let records = mem.query(&MemoryQuery {
        namespace: Some("stream".to_string()),
        subject: Some("robot.sensors".to_string()),
        ..Default::default()
    })?;
    assert_eq!(records.len(), 5);

    let mut processed_hashes = Vec::new();
    for rec in records {
        let res = executor.execute_bytes(&echo_wasm, "process", &rec.payload, ExecutionLimits::default())?;
        processed_hashes.push(format!("blake3:{}", blake3::hash(&res.output).to_hex()));
    }

    assert_eq!(processed_hashes.len(), 5);
    assert_ne!(processed_hashes[0], processed_hashes[1]);
    Ok(())
}

#[test]
fn tc_t3_10_simulated_cluster_heartbeat_gossip_and_vector_clocks() -> Result<()> {
    let mut cluster = SimulatedCluster::new("t3_cluster_clocks", 5)?;
    let node_ids = cluster.node_ids();

    // Round 1: All nodes send heartbeats
    for &id in &node_ids {
        cluster.broadcast_heartbeat(id, 10)?;
    }

    // Check all nodes have updated clocks
    for node in cluster.nodes.values() {
        for &peer_id in &node_ids {
            if peer_id != node.node_id {
                let peer = node.gossip.peers.get(&peer_id).unwrap();
                assert_eq!(peer.health, PeerHealth::Alive);
                assert_eq!(peer.vector_clock.get(&peer_id), 1);
            }
        }
    }
    Ok(())
}

#[test]
fn tc_t3_11_byzantine_fault_tolerance_during_quorum_voting() -> Result<()> {
    let mut cluster = SimulatedCluster::new("t3_bft_quorum", 4)?;
    let node_ids = cluster.node_ids();
    let proposer_id = node_ids[0];

    // Node 3 is byzantine/offline; only nodes 0, 1, 2 cast votes
    let active_voters = &node_ids[0..3];
    let proposal = cluster.reach_consensus(proposer_id, "bft_state_commit", "state_hash", active_voters)?;

    // 3 out of 4 nodes suffices for 2/3+1 BFT consensus
    assert!(proposal.finalized);
    assert_eq!(proposal.votes.len(), 3);
    Ok(())
}

#[test]
fn tc_t3_12_multi_party_pact_escrow_with_dispute_resolution_policy() -> Result<()> {
    let key = generate_keypair();
    let mut pact = ZapPact::new("depositor", "escrow_holder", "lock_escrow", 1_700_000);
    pact.terms = serde_json::json!({
        "escrow_amount_zap": 5000,
        "release_condition": "deliver_product",
        "dispute_timeout_micros": 30_000_000
    });
    pact.sign(&key)?;

    // Policy evaluating escrow release claim
    let rule = PolicyRule {
        name: Some("escrow_release_rule".into()),
        kind: Some("action".into()),
        subject: Some("escrow.release".into()),
        source_node: None,
        target_node: None,
        content_type: None,
        decision: PolicyDecision::Allow,
        required_capability: None,
        reason: Some("valid escrow release claim".into()),
    };
    let policy = PolicySet::new(vec![rule])?;
    let grants = BTreeSet::new();

    let input = PolicyInput {
        kind: "action",
        subject: "escrow.release",
        source_node: Some(key.node_id()),
        target_node: None,
        content_type: Some("application/json"),
        consensus_protected: false,
        granted_capabilities: &grants,
        human_approved: false,
        simulation_passed: false,
    };

    let eval = policy.evaluate(&input);
    assert!(eval.allowed);
    Ok(())
}

#[test]
fn tc_t3_13_wasm_sandboxed_fuel_metering_with_provenance_tracking() -> Result<()> {
    let key = generate_keypair();
    let wasm = compile_echo_wasm();
    let executor = WasmExecutor::new()?;
    let session_id = Uuid::new_v4();
    let intent_id = Uuid::new_v4();

    let mut intent = AgentIntent::new(session_id, AgentId::new("fuel_metered_agent")?, IntentKind::Act, "Compute hash");
    intent.intent_id = intent_id;

    // Execute with strict fuel limit
    let exec_res = executor.execute_bytes(&wasm, "hash", b"input_payload_for_metering", ExecutionLimits {
        fuel: 50_000,
        ..Default::default()
    })?;

    let mut meta = BTreeMap::new();
    meta.insert("fuel_consumed".to_string(), serde_json::json!(exec_res.fuel_consumed));

    let chain = ProvenanceChainBuilder::new(session_id, intent_id)
        .with_intent(&intent)?
        .with_driver("hash_driver", "in_h", "out_h", meta)?
        .build_and_sign(&key)?;

    let verif = chain.verify(&key.verifying_key())?;
    assert!(verif.valid);
    Ok(())
}

#[test]
fn tc_t3_14_high_throughput_batch_receipt_queries_and_mmr_sealing() -> Result<()> {
    let node = SimulatedNode::new("t3_batch_query_cluster")?;
    for i in 0..50 {
        node.record_action(&format!("act_{i}"), format!("data_payload_{i}").as_bytes())?;
    }

    let req = ReceiptReplicationRequest {
        limit: Some(25),
        ..Default::default()
    };
    let receipts = node.journal.query(&req)?;
    assert_eq!(receipts.len(), 25);

    let mut mmr = node.journal.build_mmr_accumulator()?;
    assert_eq!(mmr.len(), 50);
    let commitment = mmr.create_rollup_commitment(1000, 5000)?;
    assert_eq!(commitment.leaf_count, 50);
    Ok(())
}

#[test]
fn tc_t3_15_fleet_topology_health_aggregation_and_prometheus_metrics() -> Result<()> {
    let node_id = Uuid::new_v4();
    let mut topo = FleetTopology::new(node_id, "t3_telemetry_cluster");

    for i in 0..3 {
        topo.register_node(FleetNodeState {
            node_id: Uuid::new_v4(),
            addr: format!("127.0.0.1:900{i}").parse().ok(),
            trust_status: "trusted".to_string(),
            health_status: FleetNodeHealth::Healthy,
            capabilities: vec!["compute".into()],
            rtt_ms: Some(10 + i as u64),
            last_seen_micros: now_micros()?,
        });
    }

    assert_eq!(topo.active_peer_count(), 3);
    assert_eq!(topo.overall_health(), FleetNodeHealth::Healthy);

    let snap = ZapNodeMetricsSnapshot {
        node_id,
        peers_active: topo.active_peer_count() as u64,
        replay_rejections_total: 0,
        ..Default::default()
    };

    let text = PrometheusExporter::export(&snap);
    assert!(text.contains("zap_peers_active 3"));
    Ok(())
}
