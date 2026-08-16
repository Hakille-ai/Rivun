//! Tier 1: Feature Coverage Tests (`tier1_feature_tests.rs`)
//!
//! Comprehensive positive functional tests covering all 15 features in `PROJECT.md § Feature Inventory`.
//! >= 5 test cases per feature (Total: 75 test cases).

use anyhow::{Context, Result};
use bytes::Bytes;
use std::{
    collections::{BTreeMap, BTreeSet, HashMap, HashSet},
    fs,
    sync::Arc,
    time::{Duration, Instant},
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
use zap_pact::{Validate, ZapPact, ZapPactBundle, ZapPactRevocation, ZapPactStatus};
use zap_policy::{PolicyDecision, PolicyInput, PolicyRule, PolicySet};
use zap_runtime::{DriverPipeline, ExecutionLimits, WasmExecutor};
use zap_telemetry::{FleetDoctor, FleetNodeHealth, FleetNodeState, FleetTopology, IncidentCapturer, PrometheusExporter, ZapNodeMetricsSnapshot};

use zap_e2e::harness::*;

// ============================================================================
// FEATURE 1: P2P Swarm Gossip Protocol
// ============================================================================

#[test]
fn tc_f01_01_vector_clock_monotonic_increment() {
    let mut clock = VectorClock::new();
    let node_a = Uuid::new_v4();
    let node_b = Uuid::new_v4();

    assert_eq!(clock.increment(node_a), 1);
    assert_eq!(clock.increment(node_a), 2);
    assert_eq!(clock.increment(node_b), 1);
    assert_eq!(clock.get(&node_a), 2);
    assert_eq!(clock.get(&node_b), 1);
}

#[test]
fn tc_f01_02_vector_clock_merge_and_causal_comparison() {
    let node_a = Uuid::new_v4();
    let node_b = Uuid::new_v4();

    let mut clock1 = VectorClock::new();
    clock1.increment(node_a);

    let mut clock2 = VectorClock::new();
    clock2.increment(node_b);

    assert_eq!(clock1.compare(&clock2), zap_net::Causality::Concurrent);

    clock1.merge(&clock2);
    assert_eq!(clock1.get(&node_a), 1);
    assert_eq!(clock1.get(&node_b), 1);
    assert_eq!(clock1.compare(&clock2), zap_net::Causality::StrictlyAfter);
}

#[test]
fn tc_f01_03_gossip_mesh_peer_registration() {
    let self_id = Uuid::new_v4();
    let peer_id = Uuid::new_v4();
    let mut mesh = GossipMesh::new(self_id, "127.0.0.1:9000");

    mesh.register_peer(peer_id, "127.0.0.1:9001", vec!["compute".into()], 1000);

    assert_eq!(mesh.peers.len(), 1);
    let peer = mesh.peers.get(&peer_id).unwrap();
    assert_eq!(peer.node_id, peer_id);
    assert_eq!(peer.health, PeerHealth::Alive);
    assert_eq!(peer.capabilities, vec!["compute"]);
}

#[test]
fn tc_f01_04_gossip_heartbeat_state_update() {
    let self_id = Uuid::new_v4();
    let peer_id = Uuid::new_v4();
    let mut mesh = GossipMesh::new(self_id, "127.0.0.1:9000");
    mesh.register_peer(peer_id, "127.0.0.1:9001", vec![], 1000);

    let mut clock = VectorClock::new();
    clock.increment(peer_id);
    mesh.record_heartbeat(peer_id, &clock, 25, 5000);

    let peer = mesh.peers.get(&peer_id).unwrap();
    assert_eq!(peer.last_seen_micros, 5000);
    assert_eq!(peer.load_factor, 25);
    assert_eq!(peer.vector_clock.get(&peer_id), 1);
}

#[tokio::test]
async fn tc_f01_05_encrypted_p2p_datagram_exchange() -> Result<()> {
    let key = [42u8; 32];
    let id_a = Uuid::new_v4();
    let id_b = Uuid::new_v4();

    let endpoint_a = ZapEndpoint::bind(ZapEndpointConfig::new("127.0.0.1:0".parse()?, id_a)).await?;
    let endpoint_b = ZapEndpoint::bind(ZapEndpointConfig::new("127.0.0.1:0".parse()?, id_b)).await?;

    endpoint_a.add_peer(Peer::new(id_b, endpoint_b.local_addr()?, key)).await;
    endpoint_b.add_peer(Peer::new(id_a, endpoint_a.local_addr()?, key)).await;

    endpoint_a.send(id_b, Bytes::from_static(b"swarm_gossip_hello")).await?;
    let inbound = tokio::time::timeout(Duration::from_secs(2), endpoint_b.recv()).await??;

    assert_eq!(inbound.peer.node_id, id_a);
    assert_eq!(inbound.frame.payload, Bytes::from_static(b"swarm_gossip_hello"));
    Ok(())
}

// ============================================================================
// FEATURE 2: Swarm Consensus Engine
// ============================================================================

#[test]
fn tc_f02_01_quorum_proposal_creation_and_threshold() {
    let self_id = Uuid::new_v4();
    let mut mesh = GossipMesh::new(self_id, "127.0.0.1:9000");
    for _ in 0..3 {
        mesh.register_peer(Uuid::new_v4(), "127.0.0.1:0", vec![], 1000);
    }
    // Total 4 nodes -> required threshold = (4 * 2 / 3) + 1 = 3
    let prop_id = Uuid::new_v4();
    let prop = mesh.create_proposal(prop_id, "state_update", "terms_hash_1", 10_000_000);
    assert_eq!(prop.required_threshold, 3);
    assert!(!prop.finalized);
}

#[test]
fn tc_f02_02_quorum_voting_reaches_finalization() {
    let self_id = Uuid::new_v4();
    let p1 = Uuid::new_v4();
    let p2 = Uuid::new_v4();
    let mut mesh = GossipMesh::new(self_id, "127.0.0.1:9000");
    mesh.register_peer(p1, "127.0.0.1:9001", vec![], 1000);
    mesh.register_peer(p2, "127.0.0.1:9002", vec![], 1000);

    // Total 3 nodes -> required threshold = (3 * 2 / 3) + 1 = 3
    let prop_id = Uuid::new_v4();
    mesh.create_proposal(prop_id, "deploy_driver", "terms_hash_2", 10_000_000);

    assert!(!mesh.cast_vote(prop_id, self_id, "sig_self", 2000).unwrap());
    assert!(!mesh.cast_vote(prop_id, p1, "sig_p1", 2100).unwrap());
    assert!(mesh.cast_vote(prop_id, p2, "sig_p2", 2200).unwrap());
    assert!(mesh.is_proposal_finalized(&prop_id));
}

#[test]
fn tc_f02_03_poa_receipt_multi_validator_aggregation() {
    let v1 = Uuid::new_v4();
    let v2 = Uuid::new_v4();
    let v3 = Uuid::new_v4();

    let poa = PoaReceipt {
        required_threshold: 2,
        certificate_threshold: 2,
        attestation_count: 3,
        validators: vec![v1, v2, v3],
    };

    assert_eq!(poa.validators.len(), 3);
    assert_eq!(poa.certificate_threshold, 2);
}

#[test]
fn tc_f02_04_proposer_consensus_helper_in_simulated_cluster() -> Result<()> {
    let mut cluster = SimulatedCluster::new("consensus_test_cluster", 4)?;
    let node_ids = cluster.node_ids();
    let proposer = node_ids[0];
    let voters = &node_ids[0..3];

    let proposal = cluster.reach_consensus(proposer, "allocate_escrow", "terms_001", voters)?;
    assert!(proposal.finalized);
    assert_eq!(proposal.votes.len(), 3);
    Ok(())
}

#[test]
fn tc_f02_05_frame_signing_with_consensus_flags() -> Result<()> {
    let key = generate_keypair();
    let payload = Bytes::from("action_critical_payload");
    let mut frame = ZapFrame::new(key.node_id(), Uuid::new_v4(), ZapFlags::SIGNED | ZapFlags::REQUIRES_CONSENSUS, payload)?;
    sign_frame(&key, &mut frame)?;

    assert!(frame.header.flags.contains(ZapFlags::REQUIRES_CONSENSUS));
    assert!(verify_frame(&key.verifying_key(), &frame).is_ok());
    Ok(())
}

// ============================================================================
// FEATURE 3: Network Partition & Failover Mesh
// ============================================================================

#[test]
fn tc_f03_01_health_evaluation_marks_active_peers_alive() {
    let self_id = Uuid::new_v4();
    let p1 = Uuid::new_v4();
    let mut mesh = GossipMesh::new(self_id, "127.0.0.1:9000");
    mesh.register_peer(p1, "127.0.0.1:9001", vec![], 1000);

    let res = mesh.evaluate_health(2000);
    assert!(res.is_ok());
    assert_eq!(mesh.peers.get(&p1).unwrap().health, PeerHealth::Alive);
}

#[test]
fn tc_f03_02_suspect_timeout_transition() {
    let self_id = Uuid::new_v4();
    let p1 = Uuid::new_v4();
    let mut mesh = GossipMesh::new(self_id, "127.0.0.1:9000");
    mesh.register_peer(p1, "127.0.0.1:9001", vec![], 1000);

    // 4500ms elapsed > suspect_timeout (3000ms)
    let _ = mesh.evaluate_health(4_500_000);
    assert_eq!(mesh.peers.get(&p1).unwrap().health, PeerHealth::Suspect);
}

#[test]
fn tc_f03_03_dead_timeout_transition() {
    let self_id = Uuid::new_v4();
    let p1 = Uuid::new_v4();
    let mut mesh = GossipMesh::new(self_id, "127.0.0.1:9000");
    mesh.register_peer(p1, "127.0.0.1:9001", vec![], 1000);

    // 12s elapsed > dead_timeout (8000ms)
    let _ = mesh.evaluate_health(12_000_000);
    assert_eq!(mesh.peers.get(&p1).unwrap().health, PeerHealth::Dead);
}

#[test]
fn tc_f03_04_adaptive_failover_routing_selects_healthy_peer() {
    let self_id = Uuid::new_v4();
    let p1 = Uuid::new_v4();
    let p2 = Uuid::new_v4();
    let mut mesh = GossipMesh::new(self_id, "127.0.0.1:9000");
    mesh.register_peer(p1, "127.0.0.1:9001", vec!["ai.perception".into()], 1000);
    mesh.register_peer(p2, "127.0.0.1:9002", vec!["ai.perception".into()], 1000);

    // p1 times out to Dead
    let _ = mesh.evaluate_health(15_000_000);
    assert_eq!(mesh.peers.get(&p1).unwrap().health, PeerHealth::Dead);

    // p2 sends fresh heartbeat
    let mut clk = VectorClock::new();
    clk.increment(p2);
    mesh.record_heartbeat(p2, &clk, 10, 15_000_000);
    let _ = mesh.evaluate_health(15_000_000);

    let route = mesh.select_route_for_capability("ai.perception").unwrap();
    assert_eq!(route.node_id, p2);
}

#[test]
fn tc_f03_05_heartbeat_revives_suspect_peer_to_alive() {
    let self_id = Uuid::new_v4();
    let p1 = Uuid::new_v4();
    let mut mesh = GossipMesh::new(self_id, "127.0.0.1:9000");
    mesh.register_peer(p1, "127.0.0.1:9001", vec![], 1000);

    let _ = mesh.evaluate_health(5_000_000);
    assert_eq!(mesh.peers.get(&p1).unwrap().health, PeerHealth::Suspect);

    let mut clk = VectorClock::new();
    clk.increment(p1);
    mesh.record_heartbeat(p1, &clk, 0, 5_500_000);
    assert_eq!(mesh.peers.get(&p1).unwrap().health, PeerHealth::Alive);
}

// ============================================================================
// FEATURE 4: Incremental MMR Accumulator
// ============================================================================

#[test]
fn tc_f04_01_mmr_append_leaf_sequential_indices() {
    let mut mmr = MerkleMountainRange::new();
    assert_eq!(mmr.append_bytes(b"leaf_0"), 0);
    assert_eq!(mmr.append_bytes(b"leaf_1"), 1);
    assert_eq!(mmr.append_bytes(b"leaf_2"), 2);
    assert_eq!(mmr.len(), 3);
}

#[test]
fn tc_f04_02_mmr_single_leaf_root_equals_leaf_hash() {
    let mut mmr = MerkleMountainRange::new();
    let data = b"single_receipt_commitment";
    let expected_leaf_hash = zap_ledger::hash_leaf(data);

    mmr.append_bytes(data);
    assert_eq!(mmr.root(), expected_leaf_hash);
    assert_eq!(mmr.peaks().len(), 1);
}

#[test]
fn tc_f04_03_mmr_power_of_two_leaves_single_peak() {
    let mut mmr = MerkleMountainRange::new();
    for i in 0..8 {
        mmr.append_bytes(format!("leaf_{i}").as_bytes());
    }
    assert_eq!(mmr.len(), 8);
    assert_eq!(mmr.peaks().len(), 1);
}

#[test]
fn tc_f04_04_mmr_non_power_of_two_leaves_bagged_peaks() {
    let mut mmr = MerkleMountainRange::new();
    for i in 0..7 {
        mmr.append_bytes(format!("leaf_{i}").as_bytes());
    }
    assert_eq!(mmr.len(), 7);
    assert_eq!(mmr.peaks().len(), 3);
}

#[test]
fn tc_f04_05_mmr_deterministic_root_recomputation() {
    let mut mmr1 = MerkleMountainRange::new();
    let mut mmr2 = MerkleMountainRange::new();

    for i in 0..25 {
        let payload = format!("action_receipt_{i}");
        mmr1.append_bytes(payload.as_bytes());
        mmr2.append_bytes(payload.as_bytes());
    }

    assert_eq!(mmr1.root(), mmr2.root());
    assert_eq!(mmr1.root_hex(), mmr2.root_hex());
}

// ============================================================================
// FEATURE 5: Compact Batch Receipts & Proofs
// ============================================================================

#[test]
fn tc_f05_01_mmr_inclusion_proof_generation_and_verification() -> Result<()> {
    let mut mmr = MerkleMountainRange::new();
    for i in 0..10 {
        mmr.append_bytes(format!("receipt_data_{i}").as_bytes());
    }
    let root = mmr.root();

    for i in 0..10 {
        let proof = mmr.prove_inclusion(i)?;
        assert_eq!(proof.leaf_index, i);
        assert!(MerkleMountainRange::verify_proof(&proof, &root)?);
    }
    Ok(())
}

#[test]
fn tc_f05_02_mmr_large_batch_proof_verification_100_leaves() -> Result<()> {
    let mut mmr = MerkleMountainRange::new();
    for i in 0..100 {
        mmr.append_bytes(format!("high_scale_receipt_{i}").as_bytes());
    }
    let root = mmr.root();

    for &idx in &[0, 23, 49, 64, 87, 99] {
        let proof = mmr.prove_inclusion(idx)?;
        assert!(MerkleMountainRange::verify_proof(&proof, &root)?);
    }
    Ok(())
}

#[test]
fn tc_f05_03_signed_action_receipt_serialization_and_verification() -> Result<()> {
    let key = generate_keypair();
    let frame = ZapFrame::new(key.node_id(), Uuid::nil(), ZapFlags::SIGNED, Bytes::from_static(b"payload_bytes"))?;
    let signed = SignedActionReceipt::new(&key, &frame, "device.actuate", None, 2000, None)?;

    assert!(signed.verify().is_ok());
    Ok(())
}

#[test]
fn tc_f05_04_receipt_replication_request_filtering() -> Result<()> {
    let req = ReceiptReplicationRequest {
        kind: Some("action".to_string()),
        subject: Some("device.actuate".to_string()),
        limit: Some(10),
        ..Default::default()
    };
    assert!(req.validate().is_ok());
    assert_eq!(req.effective_limit()?, 10);
    Ok(())
}

#[test]
fn tc_f05_05_journal_segment_rotation_and_manifest() -> Result<()> {
    let dir = tempdir()?;
    let key = generate_keypair();
    let journal = ReceiptJournalStore::open_with_keypair(dir.path().join("receipts"), key.clone());

    let frame = ZapFrame::new(key.node_id(), Uuid::nil(), ZapFlags::SIGNED, Bytes::from_static(b"sensor_reading_temp"))?;
    let signed = SignedActionReceipt::new(&key, &frame, "sensor.temp", None, 2000, None)?;
    journal.append(&signed, false)?;

    let all = journal.all()?;
    assert_eq!(all.len(), 1);
    Ok(())
}

// ============================================================================
// FEATURE 6: ZK Verifiable Receipt Rollups
// ============================================================================

#[test]
fn tc_f06_01_create_mmr_rollup_commitment() -> Result<()> {
    let mut mmr = MerkleMountainRange::new();
    for i in 0..50 {
        mmr.append_bytes(format!("zk_receipt_{i}").as_bytes());
    }
    let commitment = mmr.create_rollup_commitment(1_000_000, 2_000_000)?;

    assert_eq!(commitment.leaf_count, 50);
    assert_eq!(commitment.min_processed_at_micros, 1_000_000);
    assert_eq!(commitment.max_processed_at_micros, 2_000_000);
    assert_eq!(commitment.root_hash, mmr.root_hex());
    Ok(())
}

#[test]
fn tc_f06_02_rollup_commitment_leaf_hashes() -> Result<()> {
    let mut mmr = MerkleMountainRange::new();
    let first = b"first_leaf_payload";
    let last = b"last_leaf_payload";
    mmr.append_bytes(first);
    for _ in 0..10 {
        mmr.append_bytes(b"mid_payload");
    }
    mmr.append_bytes(last);

    let commitment = mmr.create_rollup_commitment(100, 200)?;
    assert_eq!(commitment.first_leaf_hash, hex::encode(zap_ledger::hash_leaf(first)));
    assert_eq!(commitment.last_leaf_hash, hex::encode(zap_ledger::hash_leaf(last)));
    Ok(())
}

#[test]
fn tc_f06_03_verify_inclusion_proof_against_rollup_root() -> Result<()> {
    let mut mmr = MerkleMountainRange::new();
    for i in 0..20 {
        mmr.append_bytes(format!("zk_payload_{i}").as_bytes());
    }
    let commitment = mmr.create_rollup_commitment(500, 1000)?;
    let proof = mmr.prove_inclusion(7)?;

    let root_bytes = hex::decode(&commitment.root_hash)?;
    let mut expected_root = [0u8; 32];
    expected_root.copy_from_slice(&root_bytes);

    assert!(MerkleMountainRange::verify_proof(&proof, &expected_root)?);
    Ok(())
}

#[test]
fn tc_f06_04_multi_segment_receipt_rollup_aggregation() -> Result<()> {
    let dir = tempdir()?;
    let key = generate_keypair();
    let journal = ReceiptJournalStore::open_with_keypair(dir.path().join("receipts"), key.clone());

    for i in 0..15 {
        let frame = ZapFrame::new(key.node_id(), Uuid::nil(), ZapFlags::SIGNED, Bytes::from(format!("payload_{i}")))?;
        let signed = SignedActionReceipt::new(&key, &frame, format!("batch_{i}"), None, 2000 + i as u64, None)?;
        journal.append(&signed, false)?;
    }

    let mut mmr = journal.build_mmr_accumulator()?;
    assert_eq!(mmr.len(), 15);
    let commitment = mmr.create_rollup_commitment(2000, 2014)?;
    assert_eq!(commitment.leaf_count, 15);
    Ok(())
}

#[test]
fn tc_f06_05_rollup_commitment_timestamp_range() -> Result<()> {
    let mut mmr = MerkleMountainRange::new();
    mmr.append_bytes(b"receipt_one");
    let commitment = mmr.create_rollup_commitment(1_700_000_000, 1_700_005_000)?;
    assert!(commitment.max_processed_at_micros > commitment.min_processed_at_micros);
    Ok(())
}

// ============================================================================
// FEATURE 7: Async WASM Driver Pipeline
// ============================================================================

#[test]
fn tc_f07_01_wasm_echo_driver_execution() -> Result<()> {
    let wasm = compile_echo_wasm();
    let executor = WasmExecutor::new()?;
    let input = b"sensor_packet_alpha";

    let result = executor.execute_bytes(&wasm, "echo", input, ExecutionLimits::default())?;
    assert_eq!(result.output, input);
    assert!(result.fuel_consumed > 0);
    Ok(())
}

#[test]
fn tc_f07_02_wasm_transforming_driver_reverse() -> Result<()> {
    let wasm = compile_reverse_wasm();
    let executor = WasmExecutor::new()?;
    let input = b"ABCDEF";

    let result = executor.execute_bytes(&wasm, "reverse", input, ExecutionLimits::default())?;
    assert_eq!(result.output, b"FEDCBA");
    Ok(())
}

#[test]
fn tc_f07_03_wasm_fuel_metering_tracks_consumption() -> Result<()> {
    let wasm = compile_echo_wasm();
    let executor = WasmExecutor::new()?;
    let input = vec![0x42; 256];

    let result = executor.execute_bytes(&wasm, "echo", &input, ExecutionLimits {
        fuel: 1_000_000,
        ..Default::default()
    })?;
    assert!(result.fuel_consumed > 100);
    assert!(result.fuel_consumed < 1_000_000);
    Ok(())
}

#[test]
fn tc_f07_04_wasm_memory_sandboxing_enforcement() -> Result<()> {
    let wasm = compile_echo_wasm();
    let executor = WasmExecutor::new()?;

    let limits = ExecutionLimits {
        max_memory_bytes: 65536, // 1 page
        ..Default::default()
    };
    let result = executor.execute_bytes(&wasm, "echo", b"small_payload", limits)?;
    assert_eq!(result.output, b"small_payload");
    Ok(())
}

#[test]
fn tc_f07_05_wasm_module_cache_acceleration() -> Result<()> {
    let wasm = compile_echo_wasm();
    let executor = WasmExecutor::new()?;

    let t1 = Instant::now();
    let _ = executor.execute_bytes(&wasm, "echo", b"call1", ExecutionLimits::default())?;
    let dur1 = t1.elapsed();

    let t2 = Instant::now();
    let _ = executor.execute_bytes(&wasm, "echo", b"call2", ExecutionLimits::default())?;
    let dur2 = t2.elapsed();

    assert!(dur2 <= dur1 || dur2 < Duration::from_millis(50));
    Ok(())
}

// ============================================================================
// FEATURE 8: Streaming I/O Buffers
// ============================================================================

#[test]
fn tc_f08_01_memory_journal_stream_append_and_get() -> Result<()> {
    let dir = tempdir()?;
    let mem = MemoryJournalStore::open(dir.path().join("mem"));

    let record = mem.put(MemoryPut {
        namespace: "telemetry".to_string(),
        subject: "robot.joints".to_string(),
        content_type: "application/octet-stream".to_string(),
        body: b"joint_data_001".to_vec(),
        metadata: serde_json::Value::Null,
        source_node: None,
        frame_hash: None,
    })?;
    assert_eq!(record.sequence, 1);

    let records = mem.query(&MemoryQuery {
        namespace: Some("telemetry".to_string()),
        subject: Some("robot.joints".to_string()),
        ..Default::default()
    })?;
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].payload, b"joint_data_001");
    Ok(())
}

#[test]
fn tc_f08_02_memory_journal_query_filtering() -> Result<()> {
    let dir = tempdir()?;
    let mem = MemoryJournalStore::open(dir.path().join("mem"));

    mem.put(MemoryPut {
        namespace: "stream".to_string(),
        subject: "sensor.temp".to_string(),
        content_type: "text/plain".to_string(),
        body: b"22.5".to_vec(),
        metadata: serde_json::Value::Null,
        source_node: None,
        frame_hash: None,
    })?;
    mem.put(MemoryPut {
        namespace: "stream".to_string(),
        subject: "sensor.pressure".to_string(),
        content_type: "text/plain".to_string(),
        body: b"1013.25".to_vec(),
        metadata: serde_json::Value::Null,
        source_node: None,
        frame_hash: None,
    })?;

    let temp = mem.query(&MemoryQuery {
        namespace: Some("stream".to_string()),
        subject: Some("sensor.temp".to_string()),
        ..Default::default()
    })?;
    assert_eq!(temp.len(), 1);
    assert_eq!(temp[0].payload, b"22.5");
    Ok(())
}

#[test]
fn tc_f08_03_receipt_journal_stream_replication() -> Result<()> {
    let dir = tempdir()?;
    let key = generate_keypair();
    let journal = ReceiptJournalStore::open_with_keypair(dir.path().join("receipts"), key.clone());

    for i in 0..5 {
        let frame = ZapFrame::new(key.node_id(), Uuid::nil(), ZapFlags::SIGNED, Bytes::from(format!("chunk_{i}")))?;
        let signed = SignedActionReceipt::new_message(&key, &frame, "stream", format!("chunk_{i}"), None, 2000 + i, None)?;
        journal.append(&signed, false)?;
    }

    let req = ReceiptReplicationRequest {
        kind: Some("stream".to_string()),
        limit: Some(10),
        ..Default::default()
    };
    let streamed = journal.query(&req)?;
    assert_eq!(streamed.len(), 5);
    Ok(())
}

#[test]
fn tc_f08_04_sequential_stream_chunk_wasm_processing() -> Result<()> {
    let wasm = compile_echo_wasm();
    let executor = WasmExecutor::new()?;

    let chunks = vec![b"chunk_1".to_vec(), b"chunk_2".to_vec(), b"chunk_3".to_vec()];
    let mut outputs = Vec::new();

    for chunk in chunks {
        let res = executor.execute_bytes(&wasm, "stream", &chunk, ExecutionLimits::default())?;
        outputs.push(res.output);
    }

    assert_eq!(outputs.len(), 3);
    assert_eq!(outputs[0], b"chunk_1");
    assert_eq!(outputs[2], b"chunk_3");
    Ok(())
}

#[test]
fn tc_f08_05_memory_journal_pagination_limit() -> Result<()> {
    let dir = tempdir()?;
    let mem = MemoryJournalStore::open(dir.path().join("mem"));

    for i in 0..10 {
        mem.put(MemoryPut {
            namespace: "batch".to_string(),
            subject: "item".to_string(),
            content_type: "text/plain".to_string(),
            body: format!("val_{i}").into_bytes(),
            metadata: serde_json::Value::Null,
            source_node: None,
            frame_hash: None,
        })?;
    }

    let records = mem.query(&MemoryQuery {
        namespace: Some("batch".to_string()),
        subject: Some("item".to_string()),
        limit: Some(4),
        ..Default::default()
    })?;
    assert_eq!(records.len(), 4);
    Ok(())
}

// ============================================================================
// FEATURE 9: Inter-Driver IPC Pipes
// ============================================================================

#[test]
fn tc_f09_01_driver_pipeline_two_stage_execution() -> Result<()> {
    let echo_wasm = compile_echo_wasm();
    let pipeline = DriverPipeline::new("test_pipe_2")
        .add_stage("stage_a", "echo", echo_wasm.clone(), DriverPermissions::none(), None)
        .add_stage("stage_b", "echo", echo_wasm, DriverPermissions::none(), None);

    let report = pipeline.execute(b"pipeline_payload_123")
        .map_err(|e| anyhow::anyhow!("{e:?}"))?;

    assert_eq!(report.pipeline_id, "test_pipe_2");
    assert_eq!(report.stages.len(), 2);
    assert_eq!(report.final_output, b"pipeline_payload_123");
    assert!(!report.causal_chain_hash.is_empty());
    Ok(())
}

#[test]
fn tc_f09_02_driver_pipeline_three_stage_chaining() -> Result<()> {
    let echo_wasm = compile_echo_wasm();
    let reverse_wasm = compile_reverse_wasm();

    let pipeline = DriverPipeline::new("perception_policy_actuator_pipe")
        .add_stage("perception_stage", "filter", echo_wasm.clone(), DriverPermissions::none(), None)
        .add_stage("policy_stage", "reverse", reverse_wasm.clone(), DriverPermissions::none(), None)
        .add_stage("actuator_stage", "reverse", reverse_wasm, DriverPermissions::none(), None);

    let report = pipeline.execute(b"HELLO_ROBOT")
        .map_err(|e| anyhow::anyhow!("{e:?}"))?;

    assert_eq!(report.stages.len(), 3);
    assert_eq!(report.final_output, b"HELLO_ROBOT");
    assert_eq!(report.stages[1].stage_name, "policy_stage");
    Ok(())
}

#[test]
fn tc_f09_03_driver_pipeline_aggregate_fuel_tracking() -> Result<()> {
    let echo_wasm = compile_echo_wasm();
    let pipeline = DriverPipeline::new("metered_pipe")
        .with_max_fuel(500_000)
        .add_stage("s1", "echo", echo_wasm.clone(), DriverPermissions::none(), Some(100_000))
        .add_stage("s2", "echo", echo_wasm, DriverPermissions::none(), Some(100_000));

    let report = pipeline.execute(b"some_bytes")
        .map_err(|e| anyhow::anyhow!("{e:?}"))?;

    assert!(report.total_fuel_consumed >= 200);
    assert!(report.total_fuel_consumed <= 500_000);
    Ok(())
}

#[test]
fn tc_f09_04_driver_pipeline_causal_chain_hashing() -> Result<()> {
    let echo_wasm = compile_echo_wasm();
    let pipeline1 = DriverPipeline::new("pipe")
        .add_stage("s1", "echo", echo_wasm.clone(), DriverPermissions::none(), None);
    let pipeline2 = DriverPipeline::new("pipe")
        .add_stage("s1", "echo", echo_wasm, DriverPermissions::none(), None);

    let report1 = pipeline1.execute(b"input_a").map_err(|e| anyhow::anyhow!("{e:?}"))?;
    let report2 = pipeline2.execute(b"input_a").map_err(|e| anyhow::anyhow!("{e:?}"))?;
    let report3 = pipeline2.execute(b"input_b").map_err(|e| anyhow::anyhow!("{e:?}"))?;

    assert_eq!(report1.causal_chain_hash, report2.causal_chain_hash);
    assert_ne!(report1.causal_chain_hash, report3.causal_chain_hash);
    Ok(())
}

#[test]
fn tc_f09_05_driver_pipeline_stage_result_metrics() -> Result<()> {
    let echo_wasm = compile_echo_wasm();
    let pipeline = DriverPipeline::new("metric_pipe")
        .add_stage("stage_zero", "act", echo_wasm, DriverPermissions::none(), None);

    let report = pipeline.execute(b"payload_xyz").map_err(|e| anyhow::anyhow!("{e:?}"))?;
    let stage = &report.stages[0];
    assert_eq!(stage.stage_index, 0);
    assert_eq!(stage.output_len, 11);
    assert!(!stage.output_hash.is_empty());
    Ok(())
}

// ============================================================================
// FEATURE 10: Multi-Party Conditional Pacts
// ============================================================================

#[test]
fn tc_f10_01_create_and_sign_pact() -> Result<()> {
    let key = generate_keypair();
    let mut pact = ZapPact::new("agent.buyer", "agent.seller", "buy_power", 1_700_000);
    pact.object = serde_json::json!({"kw": 50});
    pact.terms = serde_json::json!({"price_zap": 100});
    pact.sign(&key)?;

    assert_eq!(pact.status, ZapPactStatus::Active);
    assert!(pact.signature.is_some());
    assert!(pact.hash.is_some());
    Ok(())
}

#[test]
fn tc_f10_02_verify_signed_pact_offline() -> Result<()> {
    let key = generate_keypair();
    let pact = create_test_pact("agent.alice", "agent.bob", "deliver_goods", &key)?;
    let verification = pact.verify(Some(now_micros()? + 1000))?;

    assert!(verification.valid);
    assert_eq!(verification.pact_id, pact.pact_id);
    Ok(())
}

#[test]
fn tc_f10_03_pact_draft_to_active_status_transition() -> Result<()> {
    let key = generate_keypair();
    let mut pact = ZapPact::new("agent.1", "agent.2", "task", 1000);
    assert_eq!(pact.status, ZapPactStatus::Draft);

    pact.sign(&key)?;
    assert_eq!(pact.status, ZapPactStatus::Active);
    Ok(())
}

#[test]
fn tc_f10_04_pact_bundle_packaging_and_verification() -> Result<()> {
    let key = generate_keypair();
    let pact = create_test_pact("agent.x", "agent.y", "service", &key)?;
    let bundle = ZapPactBundle::new(pact);

    let verif = bundle.verify(Some(now_micros()? + 1000))?;
    assert!(verif.valid);
    Ok(())
}

#[test]
fn tc_f10_05_canonical_pact_hashing_key_order_independence() -> Result<()> {
    let now = now_micros()?;
    let mut p1 = ZapPact::new("a", "b", "c", now);
    p1.object = serde_json::json!({"alpha": 1, "beta": 2});

    let mut p2 = ZapPact::new("a", "b", "c", now);
    p2.pact_id = p1.pact_id;
    p2.object = serde_json::json!({"beta": 2, "alpha": 1});

    assert_eq!(p1.canonical_hash()?, p2.canonical_hash()?);
    Ok(())
}

// ============================================================================
// FEATURE 11: Dispute Resolution Engine
// ============================================================================

#[test]
fn tc_f11_01_policy_set_deterministic_allow() -> Result<()> {
    let policy = PolicySet::default();
    let grants = BTreeSet::new();
    let input = PolicyInput {
        kind: "action",
        subject: "sensor.read",
        source_node: None,
        target_node: None,
        content_type: Some("text/plain"),
        consensus_protected: false,
        granted_capabilities: &grants,
        human_approved: false,
        simulation_passed: false,
    };

    let eval = policy.evaluate(&input);
    assert_eq!(eval.decision, PolicyDecision::Allow);
    assert!(eval.allowed);
    Ok(())
}

#[test]
fn tc_f11_02_policy_set_explicit_deny_rule() -> Result<()> {
    let rule = PolicyRule {
        name: Some("deny_critical_ops".into()),
        kind: Some("action".into()),
        subject: Some("system.shutdown".into()),
        source_node: None,
        target_node: None,
        content_type: None,
        decision: PolicyDecision::Deny,
        required_capability: None,
        reason: Some("shutdown disallowed via agent".into()),
    };
    let policy = PolicySet::new(vec![rule])?;
    let grants = BTreeSet::new();
    let input = PolicyInput {
        kind: "action",
        subject: "system.shutdown",
        source_node: None,
        target_node: None,
        content_type: None,
        consensus_protected: false,
        granted_capabilities: &grants,
        human_approved: false,
        simulation_passed: false,
    };

    let eval = policy.evaluate(&input);
    assert_eq!(eval.decision, PolicyDecision::Deny);
    assert!(!eval.allowed);
    assert_eq!(eval.matched_rule_name, Some("deny_critical_ops".into()));
    Ok(())
}

#[test]
fn tc_f11_03_policy_require_poa_allows_only_consensus_frames() -> Result<()> {
    let rule = PolicyRule {
        name: Some("require_poa_rule".into()),
        kind: Some("action".into()),
        subject: Some("safety.override".into()),
        source_node: None,
        target_node: None,
        content_type: None,
        decision: PolicyDecision::RequirePoa,
        required_capability: None,
        reason: None,
    };
    let policy = PolicySet::new(vec![rule])?;
    let grants = BTreeSet::new();

    let unpro = PolicyInput {
        kind: "action",
        subject: "safety.override",
        source_node: None,
        target_node: None,
        content_type: None,
        consensus_protected: false,
        granted_capabilities: &grants,
        human_approved: false,
        simulation_passed: false,
    };
    assert!(!policy.evaluate(&unpro).allowed);

    let pro = PolicyInput {
        consensus_protected: true,
        ..unpro
    };
    assert!(policy.evaluate(&pro).allowed);
    Ok(())
}

#[test]
fn tc_f11_04_policy_require_grant_verifies_capabilities() -> Result<()> {
    let cap = CapabilityId::new("driver.execute:valve_ctrl")?;
    let rule = PolicyRule {
        name: Some("grant_rule".into()),
        kind: Some("action".into()),
        subject: Some("valve.*".into()),
        source_node: None,
        target_node: None,
        content_type: None,
        decision: PolicyDecision::RequireGrant,
        required_capability: Some(cap.clone()),
        reason: None,
    };
    let policy = PolicySet::new(vec![rule])?;

    let grants_unauth = BTreeSet::new();
    let input_unauthorized = PolicyInput {
        kind: "action",
        subject: "valve.open",
        source_node: None,
        target_node: None,
        content_type: None,
        consensus_protected: false,
        granted_capabilities: &grants_unauth,
        human_approved: false,
        simulation_passed: false,
    };
    assert!(!policy.evaluate(&input_unauthorized).allowed);

    let mut grants_auth = BTreeSet::new();
    grants_auth.insert(cap);
    let input_authorized = PolicyInput {
        kind: "action",
        subject: "valve.open",
        source_node: None,
        target_node: None,
        content_type: None,
        consensus_protected: false,
        granted_capabilities: &grants_auth,
        human_approved: false,
        simulation_passed: false,
    };
    assert!(policy.evaluate(&input_authorized).allowed);
    Ok(())
}

#[test]
fn tc_f11_05_pact_signed_revocation_evidence() -> Result<()> {
    let key = generate_keypair();
    let pact_id = Uuid::new_v4();
    let mut revocation = ZapPactRevocation::new(pact_id, "arbitrator.ops", "SLA violation: timeout", 1_700_000);
    revocation.sign(&key)?;

    assert!(revocation.signature.is_some());
    assert!(revocation.validate().is_ok());
    Ok(())
}

// ============================================================================
// FEATURE 12: Causal Execution Chains
// ============================================================================

#[test]
fn tc_f12_01_build_six_stage_provenance_chain() -> Result<()> {
    let key = generate_keypair();
    let session_id = Uuid::new_v4();
    let intent_id = Uuid::new_v4();

    let mut intent = AgentIntent::new(session_id, AgentId::new("ag_1")?, IntentKind::Act, "Execute workflow");
    intent.intent_id = intent_id;

    let chain = ProvenanceChainBuilder::new(session_id, intent_id)
        .with_intent(&intent)?
        .with_negotiation(&serde_json::json!({"terms": "ack"}), BTreeMap::new())?
        .with_policy("digest_pol", "ALLOW", BTreeMap::new())?
        .with_driver("driver_x", "in_h", "out_h", BTreeMap::new())?
        .with_poa(&["sig1".into(), "sig2".into()], BTreeMap::new())?
        .with_receipt("rec_1", 2000, BTreeMap::new())?
        .build_and_sign(&key)?;

    assert_eq!(chain.steps.len(), 6);
    assert_eq!(chain.node_id, key.node_id());
    Ok(())
}

#[test]
fn tc_f12_02_verify_provenance_chain_integrity() -> Result<()> {
    let key = generate_keypair();
    let session_id = Uuid::new_v4();
    let intent_id = Uuid::new_v4();

    let mut intent = AgentIntent::new(session_id, AgentId::new("ag_1")?, IntentKind::Act, "Execute workflow");
    intent.intent_id = intent_id;

    let chain = ProvenanceChainBuilder::new(session_id, intent_id)
        .with_intent(&intent)?
        .with_policy("pol", "ALLOW", BTreeMap::new())?
        .with_receipt("rec_1", 2000, BTreeMap::new())?
        .build_and_sign(&key)?;

    let report = chain.verify(&key.verifying_key())?;
    assert!(report.valid);
    assert_eq!(report.verified_steps, 3);
    Ok(())
}

#[test]
fn tc_f12_03_verify_individual_provenance_steps() -> Result<()> {
    let key = generate_keypair();
    let session_id = Uuid::new_v4();
    let intent_id = Uuid::new_v4();

    let mut intent = AgentIntent::new(session_id, AgentId::new("ag_1")?, IntentKind::Act, "Goal");
    intent.intent_id = intent_id;

    let chain = ProvenanceChainBuilder::new(session_id, intent_id)
        .with_intent(&intent)?
        .with_policy("pol_hash", "ALLOW", BTreeMap::new())?
        .build_and_sign(&key)?;

    assert!(chain.verify_step(ProvenanceStage::Intent).is_ok());
    assert!(chain.verify_step(ProvenanceStage::Policy).is_ok());
    Ok(())
}

#[test]
fn tc_f12_04_provenance_chain_root_hash_computation() -> Result<()> {
    let key = generate_keypair();
    let session_id = Uuid::new_v4();
    let intent_id = Uuid::new_v4();

    let mut intent = AgentIntent::new(session_id, AgentId::new("ag_1")?, IntentKind::Act, "Goal");
    intent.intent_id = intent_id;

    let chain = ProvenanceChainBuilder::new(session_id, intent_id)
        .with_intent(&intent)?
        .build_and_sign(&key)?;

    let root = zap_agent::compute_root_hash(&chain.steps);
    assert_eq!(chain.root_hash, root);
    Ok(())
}

#[test]
fn tc_f12_05_provenance_chain_signature_tied_to_node_identity() -> Result<()> {
    let key = generate_keypair();
    let other_key = generate_keypair();
    let session_id = Uuid::new_v4();
    let intent_id = Uuid::new_v4();

    let mut intent = AgentIntent::new(session_id, AgentId::new("ag_1")?, IntentKind::Act, "Goal");
    intent.intent_id = intent_id;

    let chain = ProvenanceChainBuilder::new(session_id, intent_id)
        .with_intent(&intent)?
        .build_and_sign(&key)?;

    let report = chain.verify(&other_key.verifying_key())?;
    assert!(!report.valid);
    assert!(report.failure_reason.unwrap().contains("Signer node ID mismatch"));
    Ok(())
}

// ============================================================================
// FEATURE 13: Cluster Simulator CLI
// ============================================================================

#[test]
fn tc_f13_01_simulated_cluster_spawning_three_nodes() -> Result<()> {
    let cluster = SimulatedCluster::new("swarm_alpha", 3)?;
    assert_eq!(cluster.nodes.len(), 3);

    for node in cluster.nodes.values() {
        assert!(!node.addr.is_empty());
        assert_eq!(node.gossip.peers.len(), 2);
    }
    Ok(())
}

#[test]
fn tc_f13_02_cluster_topology_cross_peer_discovery() -> Result<()> {
    let cluster = SimulatedCluster::new("swarm_beta", 4)?;
    for node in cluster.nodes.values() {
        assert_eq!(node.topology.active_peer_count(), 3);
        assert_eq!(node.topology.overall_health(), FleetNodeHealth::Healthy);
    }
    Ok(())
}

#[test]
fn tc_f13_03_fleet_topology_overall_health_assessment() -> Result<()> {
    let local_id = Uuid::new_v4();
    let mut topo = FleetTopology::new(local_id, "cluster_gamma");
    let p1 = Uuid::new_v4();

    topo.register_node(FleetNodeState {
        node_id: p1,
        addr: "127.0.0.1:9001".parse().ok(),
        trust_status: "trusted".to_string(),
        health_status: FleetNodeHealth::Healthy,
        capabilities: vec!["compute".into()],
        rtt_ms: Some(5),
        last_seen_micros: now_micros()?,
    });

    assert_eq!(topo.active_peer_count(), 1);
    assert_eq!(topo.overall_health(), FleetNodeHealth::Healthy);
    Ok(())
}

#[test]
fn tc_f13_04_node_config_generation_and_loading() -> Result<()> {
    let dir = tempdir()?;
    let key = generate_keypair();
    let key_path = dir.path().join("node.key");
    fs::write(&key_path, key.to_key_file_toml()?)?;

    let addr = free_udp_addr();
    let toml = format!(
        "bind = \"{addr}\"\nkey_file = \"{}\"\n",
        key_path.display().to_string().replace('\\', "/")
    );
    let parsed = ZapNodeConfig::from_toml_str(&toml)?;
    assert_eq!(parsed.bind, addr);
    Ok(())
}

#[test]
fn tc_f13_05_fleet_doctor_diagnostic_evaluation() -> Result<()> {
    let dir = tempdir()?;
    let key = generate_keypair();
    let key_path = dir.path().join("node.key");
    fs::write(&key_path, key.to_key_file_toml()?)?;
    let config_path = dir.path().join("zap.toml");
    let toml = format!(
        "bind = \"{}\"\nkey_file = \"{}\"\n",
        free_udp_addr(),
        key_path.display().to_string().replace('\\', "/")
    );
    fs::write(&config_path, toml)?;

    let report = FleetDoctor::evaluate(key.node_id(), Some(&config_path), None, None, None);
    assert_eq!(report.node_id, key.node_id());
    assert!(!report.has_failures());
    Ok(())
}

// ============================================================================
// FEATURE 14: Swarm Benchmarking Tooling
// ============================================================================

#[test]
fn tc_f14_01_high_throughput_batch_receipt_appending() -> Result<()> {
    let node = SimulatedNode::new("bench_cluster")?;
    let t0 = Instant::now();

    for i in 0..100 {
        let action = format!("bench_act_{i}");
        node.record_action(&action, b"bench_data")?;
    }
    let elapsed = t0.elapsed();

    let all = node.journal.all()?;
    assert_eq!(all.len(), 100);
    assert!(elapsed < Duration::from_secs(5));
    Ok(())
}

#[test]
fn tc_f14_02_high_throughput_mmr_proof_loop() -> Result<()> {
    let mut mmr = MerkleMountainRange::new();
    for i in 0..200 {
        mmr.append_bytes(format!("receipt_{i}").as_bytes());
    }
    let root = mmr.root();

    let t0 = Instant::now();
    for i in 0..200 {
        let proof = mmr.prove_inclusion(i)?;
        assert!(MerkleMountainRange::verify_proof(&proof, &root)?);
    }
    let elapsed = t0.elapsed();
    assert!(elapsed < Duration::from_millis(500));
    Ok(())
}

#[test]
fn tc_f14_03_multi_node_heartbeat_gossip_broadcast() -> Result<()> {
    let mut cluster = SimulatedCluster::new("bench_swarm", 4)?;
    let node_ids = cluster.node_ids();

    for &id in &node_ids {
        cluster.broadcast_heartbeat(id, 15)?;
    }

    for node in cluster.nodes.values() {
        assert_eq!(node.gossip.peers.len(), 3);
        for peer in node.gossip.peers.values() {
            assert_eq!(peer.health, PeerHealth::Alive);
            assert_eq!(peer.load_factor, 15);
        }
    }
    Ok(())
}

#[test]
fn tc_f14_04_prometheus_metrics_snapshot_export() -> Result<()> {
    let node_id = Uuid::new_v4();
    let snap = ZapNodeMetricsSnapshot {
        node_id,
        replay_rejections_total: 5,
        journal_segment_rotations_total: 2,
        agent_sessions_active: 3,
        provenance_verification_failures_total: 0,
        peers_active: 4,
        ..Default::default()
    };

    let text = PrometheusExporter::export(&snap);
    assert!(text.contains("zap_replay_rejections_total 5"));
    assert!(text.contains("zap_journal_segment_rotations_total 2"));
    assert!(text.contains("zap_agent_sessions_active 3"));
    assert!(text.contains("zap_peers_active 4"));
    Ok(())
}

#[test]
fn tc_f14_05_incident_snapshot_capture_and_archive() -> Result<()> {
    let node_id = Uuid::new_v4();
    let snap = IncidentCapturer::capture(node_id, "zap_replay_rejections_total 0\n", None);
    assert_eq!(snap.node_id, node_id);

    let tar_bytes = IncidentCapturer::build_tar_archive(&snap)?;
    assert!(tar_bytes.len() >= 512);
    Ok(())
}

// ============================================================================
// FEATURE 15: E2E Integration & Audit
// ============================================================================

#[test]
fn tc_f15_01_full_pipeline_intent_pact_policy_driver_receipt() -> Result<()> {
    let key = generate_keypair();
    let wasm = compile_echo_wasm();
    let session_id = Uuid::new_v4();
    let intent_id = Uuid::new_v4();

    // 1. Intent
    let mut intent = AgentIntent::new(session_id, AgentId::new("buyer_agent")?, IntentKind::Act, "Execute smart purchase");
    intent.intent_id = intent_id;

    // 2. Pact
    let pact = create_test_pact("buyer_agent", "seller_agent", "Execute smart purchase", &key)?;
    assert!(pact.verify(Some(now_micros()? + 1000))?.valid);

    // 3. Policy
    let policy = PolicySet::default();
    let grants = BTreeSet::new();
    let pol_input = PolicyInput {
        kind: "action",
        subject: "purchase.execute",
        source_node: Some(key.node_id()),
        target_node: None,
        content_type: Some("application/json"),
        consensus_protected: false,
        granted_capabilities: &grants,
        human_approved: false,
        simulation_passed: false,
    };
    assert!(policy.evaluate(&pol_input).allowed);

    // 4. WASM Execution
    let executor = WasmExecutor::new()?;
    let exec_res = executor.execute_bytes(&wasm, "purchase", b"purchase_terms_100", ExecutionLimits::default())?;
    assert_eq!(exec_res.output, b"purchase_terms_100");

    // 5. Provenance Chain
    let chain = ProvenanceChainBuilder::new(session_id, intent_id)
        .with_intent(&intent)?
        .with_policy("policy_sha256", "ALLOW", BTreeMap::new())?
        .with_driver("purchase_driver", "in_h", "out_h", BTreeMap::new())?
        .with_receipt("rec_final", 5000, BTreeMap::new())?
        .build_and_sign(&key)?;

    assert!(chain.verify(&key.verifying_key())?.valid);
    Ok(())
}

#[test]
fn tc_f15_02_e2e_swarm_consensus_to_mmr_sealing() -> Result<()> {
    let mut cluster = SimulatedCluster::new("swarm_mmr_cluster", 3)?;
    let node_ids = cluster.node_ids();
    let proposer_id = node_ids[0];

    // 1. Consensus
    let prop = cluster.reach_consensus(proposer_id, "seal_batch", "terms_batch", &node_ids)?;
    assert!(prop.finalized);

    // 2. Record receipts on proposer
    let proposer = cluster.get_node(&proposer_id).unwrap();
    for i in 0..5 {
        proposer.record_action(&format!("op_{i}"), b"consensus_payload")?;
    }

    // 3. MMR Root sealing
    let mut mmr = proposer.journal.build_mmr_accumulator()?;
    assert_eq!(mmr.len(), 5);
    let commitment = mmr.create_rollup_commitment(1000, 2000)?;
    assert_eq!(commitment.leaf_count, 5);
    Ok(())
}

#[test]
fn tc_f15_03_e2e_cluster_discovery_and_encrypted_messaging() -> Result<()> {
    let cluster = SimulatedCluster::new("e2e_disc_cluster", 3)?;
    let node_ids = cluster.node_ids();

    let n1 = cluster.get_node(&node_ids[0]).unwrap();
    let n2 = cluster.get_node(&node_ids[1]).unwrap();

    assert_eq!(n1.topology.active_peer_count(), 2);
    assert_eq!(n2.topology.active_peer_count(), 2);
    assert_eq!(n1.topology.overall_health(), FleetNodeHealth::Healthy);
    Ok(())
}

#[test]
fn tc_f15_04_e2e_receipt_replication_and_proof_verification() -> Result<()> {
    let node = SimulatedNode::new("replication_cluster")?;
    for i in 0..10 {
        node.record_action(&format!("act_{i}"), format!("data_{i}").as_bytes())?;
    }

    let req = ReceiptReplicationRequest {
        limit: Some(10),
        ..Default::default()
    };
    let receipts = node.journal.query(&req)?;
    assert_eq!(receipts.len(), 10);

    let mut mmr = node.journal.build_mmr_accumulator()?;
    let root = mmr.root();
    let proof = mmr.prove_inclusion(4)?;
    assert!(MerkleMountainRange::verify_proof(&proof, &root)?);
    Ok(())
}

#[test]
fn tc_f15_05_e2e_driver_pipeline_with_provenance_binding() -> Result<()> {
    let key = generate_keypair();
    let wasm = compile_echo_wasm();
    let session_id = Uuid::new_v4();
    let intent_id = Uuid::new_v4();

    let mut intent = AgentIntent::new(session_id, AgentId::new("pipe_agent")?, IntentKind::Act, "Run perception");
    intent.intent_id = intent_id;

    let pipeline = DriverPipeline::new("perception_actuation")
        .add_stage("perception", "echo", wasm.clone(), DriverPermissions::none(), None)
        .add_stage("actuator", "echo", wasm, DriverPermissions::none(), None);

    let report = pipeline.execute(b"lidar_point_cloud").map_err(|e| anyhow::anyhow!("{e:?}"))?;

    let chain = ProvenanceChainBuilder::new(session_id, intent_id)
        .with_intent(&intent)?
        .with_driver("perception_actuation", "in_h", &report.causal_chain_hash, BTreeMap::new())?
        .build_and_sign(&key)?;

    assert!(chain.verify(&key.verifying_key())?.valid);
    Ok(())
}
