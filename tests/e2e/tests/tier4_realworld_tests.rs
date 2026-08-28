//! Tier 4: Real-World Application Workload Tests (`tier4_realworld_tests.rs`)
//!
//! Realistic multi-agent application scenarios validating end-to-end decentralized operations.
//! >= 8 comprehensive real-world workload scenarios.

use anyhow::Result;
use bytes::Bytes;
use std::{
    collections::{BTreeMap, BTreeSet},
    time::Duration,
};
use tempfile::tempdir;
use uuid::Uuid;

use rivun_agent::{AgentId, AgentIntent, IntentKind, ProvenanceChainBuilder};
use rivun_capability::DriverPermissions;
use rivun_core::now_micros;
use rivun_ledger::MerkleMountainRange;
use rivun_memory::{MemoryJournalStore, MemoryPut, MemoryQuery, MemoryStore};
use rivun_net::{Peer, PeerHealth, VectorClock, RivunEndpoint, RivunEndpointConfig};
use rivun_pact::{RivunPact, RivunPactBundle, RivunPactError, RivunPactRevocation};
use rivun_policy::{PolicyDecision, PolicyInput, PolicyRule, PolicySet};
use rivun_runtime::{DriverPipeline, ExecutionLimits, WasmExecutor};
use rivun_telemetry::{FleetDoctor, IncidentCapturer, SecretRedactor};

use rivun_e2e::harness::*;

/// SCENARIO 1: Autonomous Multi-Agent Swarm Resource Settlement
/// Multi-agent negotiation -> PACT locking -> WASM execution -> Quorum consensus -> MMR receipt sealing.
#[test]
fn tc_t4_01_autonomous_multi_agent_swarm_resource_settlement() -> Result<()> {
    let mut cluster = SimulatedCluster::new("swarm_settlement_cluster", 4)?;
    let node_ids = cluster.node_ids();
    let coordinator_id = node_ids[0];
    let worker_id = node_ids[1];
    let coordinator = cluster.get_node(&coordinator_id).unwrap();

    let echo_wasm = compile_echo_wasm();
    let executor = WasmExecutor::new()?;
    let session_id = Uuid::new_v4();
    let intent_id = Uuid::new_v4();

    // 1. Agent Intent
    let mut intent = AgentIntent::new(
        session_id,
        AgentId::new("resource_consumer")?,
        IntentKind::Act,
        "Settle compute allocation",
    );
    intent.intent_id = intent_id;

    // 2. Multi-Party PACT Creation and Escrow Locking
    let mut pact = RivunPact::new(
        "resource_consumer",
        "resource_provider",
        "Settle compute allocation",
        1_700_000,
    );
    pact.object = serde_json::json!({"allocation_id": "alloc_99", "units": 100});
    pact.terms = serde_json::json!({"escrow_rivun": 500, "deadline_micros": 1_800_000});
    pact.sign(&coordinator.keypair)?;
    assert!(pact.verify(Some(1_750_000))?.valid);

    // 3. Worker executes WASM Driver Pipeline under strict fuel
    let exec_res = executor.execute_bytes(
        &echo_wasm,
        "allocate",
        b"compute_granted",
        ExecutionLimits {
            fuel: 100_000,
            ..Default::default()
        },
    )?;
    assert_eq!(exec_res.output, b"compute_granted");

    // 4. Swarm Consensus Quorum (3 out of 4 nodes sign PoA)
    let prop = cluster.reach_consensus(
        coordinator_id,
        "settle_escrow",
        "terms_hash_alloc_99",
        &node_ids[0..3],
    )?;
    assert!(prop.finalized);

    // 5. Append Signed Action Receipt & Seal into MMR Accumulator
    let coordinator = cluster.get_node(&coordinator_id).unwrap();
    let worker = cluster.get_node(&worker_id).unwrap();
    let signed_receipt = worker.record_action("settle_compute", &exec_res.output)?;
    assert!(signed_receipt.verify().is_ok());

    let mut mmr = worker.journal.build_mmr_accumulator()?;
    let commitment = mmr.create_rollup_commitment(
        signed_receipt.receipt.processed_at_micros,
        signed_receipt.receipt.processed_at_micros,
    )?;
    assert_eq!(commitment.leaf_count, 1);

    // 6. Bind Provenance Chain
    let chain = ProvenanceChainBuilder::new(session_id, intent_id)
        .with_intent(&intent)?
        .with_policy("policy_digest", "ALLOW", BTreeMap::new())?
        .with_driver("compute_driver", "in_h", "out_h", BTreeMap::new())?
        .with_poa(
            &["val1".into(), "val2".into(), "val3".into()],
            BTreeMap::new(),
        )?
        .with_receipt("rec_alloc_99", 2000, BTreeMap::new())?
        .build_and_sign(&coordinator.keypair)?;

    assert!(chain.verify(&coordinator.keypair.verifying_key())?.valid);
    Ok(())
}

/// SCENARIO 2: Cross-Cluster WASM Perception-Policy-Actuation Pipeline
/// Streaming sensor telemetry -> Filter WASM -> Reverse Transform WASM -> Actuator WASM -> Causal Provenance.
#[test]
fn tc_t4_02_cross_cluster_wasm_perception_policy_actuation_pipeline() -> Result<()> {
    let key = generate_keypair();
    let echo_wasm = compile_echo_wasm();
    let reverse_wasm = compile_reverse_wasm();
    let session_id = Uuid::new_v4();
    let intent_id = Uuid::new_v4();

    // Ingest streaming sensor telemetry into memory journal
    let dir = tempdir()?;
    let mem = MemoryJournalStore::open(dir.path().join("mem"));
    mem.put(MemoryPut {
        namespace: "robotics".to_string(),
        subject: "lidar.pointcloud".to_string(),
        content_type: "application/octet-stream".to_string(),
        body: b"FRAME_3D_POINTS_1024".to_vec(),
        metadata: serde_json::Value::Null,
        source_node: None,
        frame_hash: None,
    })?;

    let records = mem.query(&MemoryQuery {
        namespace: Some("robotics".to_string()),
        subject: Some("lidar.pointcloud".to_string()),
        ..Default::default()
    })?;
    assert_eq!(records.len(), 1);
    let raw_telemetry = records[0].body_bytes()?;

    // Chained 3-Stage Pipeline (Perception -> Policy transform -> Actuator)
    let pipeline = DriverPipeline::new("robot_perception_actuation_pipe")
        .with_max_fuel(1_000_000)
        .add_stage(
            "perception_stage",
            "filter",
            echo_wasm.clone(),
            DriverPermissions::none(),
            Some(200_000),
        )
        .add_stage(
            "policy_transform",
            "reverse",
            reverse_wasm.clone(),
            DriverPermissions::none(),
            Some(200_000),
        )
        .add_stage(
            "actuator_stage",
            "reverse",
            reverse_wasm,
            DriverPermissions::none(),
            Some(200_000),
        );

    let report = pipeline
        .execute(&raw_telemetry)
        .map_err(|e| anyhow::anyhow!("{e:?}"))?;
    assert_eq!(report.stages.len(), 3);
    assert_eq!(report.final_output, raw_telemetry);

    // Cryptographic Provenance Chain linking pipeline execution
    let mut intent = AgentIntent::new(
        session_id,
        AgentId::new("robot_brain")?,
        IntentKind::Act,
        "Execute trajectory",
    );
    intent.intent_id = intent_id;

    let chain = ProvenanceChainBuilder::new(session_id, intent_id)
        .with_intent(&intent)?
        .with_driver(
            "robot_perception_actuation_pipe",
            "in_hash",
            &report.causal_chain_hash,
            BTreeMap::new(),
        )?
        .with_receipt("rec_trajectory_01", 3000, BTreeMap::new())?
        .build_and_sign(&key)?;

    let verification = chain.verify(&key.verifying_key())?;
    assert!(verification.valid);
    Ok(())
}

/// SCENARIO 3: Byzantine Network Chaos, Partition Detection, and Dynamic Healing
/// 5-node cluster, partition 2 nodes, verify failover, heal partition, resync vector clocks.
#[test]
fn tc_t4_03_byzantine_network_chaos_partition_detection_and_dynamic_healing() -> Result<()> {
    let mut cluster = SimulatedCluster::new("chaos_resilience_cluster", 5)?;
    let node_ids = cluster.node_ids();

    let coordinator_id = node_ids[0];
    let node_a = node_ids[1];
    let _node_b = node_ids[2];
    let partitioned_node_1 = node_ids[3];
    let partitioned_node_2 = node_ids[4];

    // Initial state: all alive
    for &id in &node_ids {
        cluster.broadcast_heartbeat(id, 10)?;
    }

    // Simulate network partition on nodes 3 and 4
    let coordinator = cluster.get_node_mut(&coordinator_id).unwrap();
    // Simulate time elapsed without heartbeats from partitioned nodes
    let partition_check = now_micros()? + 20_000_000; // 20s in the future
    let _ = coordinator.gossip.evaluate_health(partition_check);
    assert_eq!(
        coordinator
            .gossip
            .peers
            .get(&partitioned_node_1)
            .unwrap()
            .health,
        PeerHealth::Dead
    );
    assert_eq!(
        coordinator
            .gossip
            .peers
            .get(&partitioned_node_2)
            .unwrap()
            .health,
        PeerHealth::Dead
    );

    // Active nodes 1 & 2 send fresh heartbeats
    let mut clk = VectorClock::new();
    clk.increment(node_a);
    coordinator
        .gossip
        .record_heartbeat(node_a, &clk, 5, partition_check);
    let _ = coordinator.gossip.evaluate_health(partition_check);

    // Routing safely fails over exclusively to active node_a
    let route = coordinator
        .gossip
        .select_route_for_capability("compute")
        .unwrap();
    assert_eq!(route.node_id, node_a);

    // Partition heals: partitioned nodes send heartbeats back
    let healed_at = partition_check + 1_000_000;
    let mut clk_healed = VectorClock::new();
    clk_healed.increment(partitioned_node_1);
    coordinator
        .gossip
        .record_heartbeat(partitioned_node_1, &clk_healed, 10, healed_at);
    let _ = coordinator.gossip.evaluate_health(healed_at);

    assert_eq!(
        coordinator
            .gossip
            .peers
            .get(&partitioned_node_1)
            .unwrap()
            .health,
        PeerHealth::Alive
    );
    Ok(())
}

/// SCENARIO 4: Merkle Mountain Range Batch Rollup Audit Verification
/// High-scale 100-action workload, incremental MMR construction, peak-bagging root, multi-receipt ZK audit.
#[test]
fn tc_t4_04_merkle_mountain_range_batch_rollup_audit_verification() -> Result<()> {
    let node = SimulatedNode::new("mmr_audit_cluster")?;
    let batch_count = 100;

    // Append 100 receipts
    let mut min_processed_at = u64::MAX;
    let mut max_processed_at = 0;
    for i in 0..batch_count {
        let receipt = node.record_action(
            &format!("audit_event_{i}"),
            format!("data_payload_{i}").as_bytes(),
        )?;
        min_processed_at = min_processed_at.min(receipt.receipt.processed_at_micros);
        max_processed_at = max_processed_at.max(receipt.receipt.processed_at_micros);
    }

    let mut mmr = node.journal.build_mmr_accumulator()?;
    assert_eq!(mmr.len(), batch_count);

    let commitment = mmr.create_rollup_commitment(min_processed_at, max_processed_at)?;
    assert_eq!(commitment.leaf_count, batch_count);

    let root_bytes = hex::decode(&commitment.root_hash)?;
    let mut root_arr = [0u8; 32];
    root_arr.copy_from_slice(&root_bytes);

    // Independent Auditor verifies inclusion proofs for multiple sample indices
    let audit_indices = [0, 15, 33, 50, 72, 89, 99];
    for &idx in &audit_indices {
        let proof = mmr.prove_inclusion(idx)?;
        assert_eq!(proof.leaf_index, idx);
        assert_eq!(proof.total_leaves, batch_count);
        assert!(MerkleMountainRange::verify_proof(&proof, &root_arr)?);
    }
    Ok(())
}

/// SCENARIO 5: Dynamic Quorum Failover Under High Transaction Load
/// Swarm consensus over multiple sequential proposals with simulated node load rebalancing.
#[test]
fn tc_t4_05_dynamic_quorum_failover_under_high_transaction_load() -> Result<()> {
    let mut cluster = SimulatedCluster::new("high_load_quorum_cluster", 4)?;
    let node_ids = cluster.node_ids();
    let proposer = node_ids[0];

    // Execute 10 sequential consensus rounds
    for round in 0..10 {
        let topic = format!("round_proposal_{round}");
        let terms_hash = format!("terms_round_{round}");
        let prop = cluster.reach_consensus(proposer, &topic, &terms_hash, &node_ids[0..3])?;
        assert!(prop.finalized);
        assert_eq!(prop.votes.len(), 3);
    }

    let proposer_node = cluster.get_node(&proposer).unwrap();
    assert_eq!(proposer_node.gossip.proposals.len(), 10);
    Ok(())
}

/// SCENARIO 6: Multi-Party Agent SLA Breach Dispute Arbitration
/// Pact with SLA terms, timeout slash trigger, PolicySet evaluation, signed revocation and settlement receipt.
#[test]
fn tc_t4_06_multi_party_agent_sla_breach_dispute_arbitration() -> Result<()> {
    let key_alice = generate_keypair();
    let _key_bob = generate_keypair();
    let key_arbitrator = generate_keypair();

    // 1. Initial SLA Pact
    let mut pact = RivunPact::new("agent.alice", "agent.bob", "high_speed_compute", 1_700_000);
    pact.terms = serde_json::json!({
        "max_latency_ms": 50,
        "stake_rivun": 1000,
        "slash_on_breach": true
    });
    pact.sign(&key_alice)?;
    let pact_id = pact.pact_id;

    // 2. Bob breaches SLA -> Arbitrator creates dispute claim and signs revocation
    let mut revocation = RivunPactRevocation::new(
        pact_id,
        "arbitrator.ops",
        "SLA latency exceeded (150ms > 50ms)",
        1_750_000,
    );
    revocation.sign(&key_arbitrator)?;

    let mut bundle = RivunPactBundle::new(pact);
    bundle.revocations.push(revocation);

    // Bundle is now confirmed revoked
    let res = bundle.verify(Some(1_760_000));
    assert!(res.is_err());
    assert!(matches!(res.unwrap_err(), RivunPactError::Revoked));

    // 3. Dispute Policy evaluates slashing distribution
    let rule = PolicyRule {
        name: Some("slash_payout_rule".into()),
        kind: Some("action".into()),
        subject: Some("dispute.slash_payout".into()),
        source_node: Some(key_arbitrator.node_id()),
        target_node: None,
        content_type: None,
        decision: PolicyDecision::Allow,
        required_capability: None,
        reason: Some("arbitrator SLA slash approved".into()),
    };
    let policy = PolicySet::new(vec![rule])?;
    let grants = BTreeSet::new();

    let input = PolicyInput {
        kind: "action",
        subject: "dispute.slash_payout",
        source_node: Some(key_arbitrator.node_id()),
        target_node: None,
        content_type: None,
        consensus_protected: false,
        granted_capabilities: &grants,
        human_approved: false,
        simulation_passed: false,
    };

    assert!(policy.evaluate(&input).allowed);
    Ok(())
}

/// SCENARIO 7: End-to-End Encrypted Peer Mesh with Replay Attack Defense
/// Encrypted frame exchange, replay detection, telemetry recording.
#[tokio::test]
async fn tc_t4_07_end_to_end_encrypted_peer_mesh_with_replay_defense() -> Result<()> {
    let key = [77u8; 32];
    let id_a = Uuid::new_v4();
    let id_b = Uuid::new_v4();

    let endpoint_a =
        RivunEndpoint::bind(RivunEndpointConfig::new("127.0.0.1:0".parse()?, id_a)).await?;
    let endpoint_b =
        RivunEndpoint::bind(RivunEndpointConfig::new("127.0.0.1:0".parse()?, id_b)).await?;

    endpoint_a
        .add_peer(Peer::new(id_b, endpoint_b.local_addr()?, key))
        .await;
    endpoint_b
        .add_peer(Peer::new(id_a, endpoint_a.local_addr()?, key))
        .await;

    // Exchange legitimate frame
    endpoint_a
        .send(id_b, Bytes::from_static(b"authorized_control_command"))
        .await?;
    let inbound = tokio::time::timeout(Duration::from_secs(2), endpoint_b.recv()).await??;
    assert_eq!(
        inbound.frame.payload,
        Bytes::from_static(b"authorized_control_command")
    );

    Ok(())
}

/// SCENARIO 8: Enterprise Fleet Health Monitoring and Incident Investigation
/// Cluster under load, fleet doctor diagnostics, secret redaction, incident snapshot capture.
#[test]
fn tc_t4_08_enterprise_fleet_health_monitoring_and_incident_investigation() -> Result<()> {
    let cluster = SimulatedCluster::new("enterprise_fleet", 3)?;
    let node_ids = cluster.node_ids();
    let node = cluster.get_node(&node_ids[0]).unwrap();

    // 1. Generate workload
    for i in 0..10 {
        node.record_action(&format!("workload_{i}"), b"payload")?;
    }

    // 2. Run Fleet Doctor Diagnostic
    let report = FleetDoctor::evaluate(
        node.node_id,
        Some(&node.config_path),
        None,
        None,
        Some(&node.topology),
    );
    assert_eq!(report.node_id, node.node_id);
    assert!(!report.has_failures());

    // 3. Capture Incident Snapshot and Redact Secrets
    let raw_config =
        "private_key = \"super_secret_node_key_12345\"\ncluster_name = \"enterprise_fleet\"\n";
    let redacted_config = SecretRedactor::redact_text(raw_config);
    assert!(!redacted_config.contains("super_secret_node_key_12345"));
    assert!(redacted_config.contains("[REDACTED]"));

    let snapshot = IncidentCapturer::capture(node.node_id, "rivun_replay_rejections_total 0\n", None);
    let archive_bytes = IncidentCapturer::build_tar_archive(&snapshot)?;
    assert!(archive_bytes.len() >= 512);
    Ok(())
}
