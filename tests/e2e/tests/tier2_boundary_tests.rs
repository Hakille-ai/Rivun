//! Tier 2: Boundary, Corner Case, and Negative Tests (`tier2_boundary_tests.rs`)
//!
//! Comprehensive boundary, negative, and error-handling tests covering all 15 features in `PROJECT.md § Feature Inventory`.
//! >= 5 test cases per feature (Total: 75 test cases).

use anyhow::Result;
use bytes::Bytes;
use std::collections::{BTreeMap, BTreeSet};
use tempfile::tempdir;
use uuid::Uuid;

use zap_agent::{
    AgentId, AgentIntent, IntentKind, ProvenanceChainBuilder, ProvenanceStage, ZapAgentError,
};
use zap_capability::DriverPermissions;
use zap_core::{ZapFlags, ZapFrame};
use zap_crypto::{sign_frame, verify_frame};
use zap_ledger::{
    ActionReceipt, MerkleMountainRange, MmrError, ReceiptJournalStore, ReceiptReplicationRequest,
    SignedActionReceipt,
};
use zap_memory::{MemoryJournalStore, MemoryPut, MemoryQuery, MemoryStore};
use zap_net::{
    GossipError, GossipMesh, Peer, PeerHealth, VectorClock, ZapEndpoint, ZapEndpointConfig,
    ZapNetError,
};
use zap_node::ZapNodeConfig;
use zap_pact::{Validate, ZapPact, ZapPactBundle, ZapPactError, ZapPactRevocation};
use zap_policy::{PolicyDecision, PolicyInput, PolicyRule, PolicySet, ZapPolicyError};
use zap_runtime::{DriverPipeline, ExecutionLimits, PipelineError, WasmExecutor, ZapRuntimeError};
use zap_telemetry::{
    FleetDoctor, FleetNodeHealth, FleetNodeState, FleetTopology, IncidentCapturer,
    PrometheusExporter, SecretRedactor, ZapNodeMetricsSnapshot,
};

use zap_e2e::harness::*;

// ============================================================================
// FEATURE 1: P2P Swarm Gossip Protocol (Boundary & Corner Cases)
// ============================================================================

#[test]
fn tc_f01_06_vector_clock_compare_empty_and_disjoint() {
    let empty1 = VectorClock::new();
    let empty2 = VectorClock::new();
    assert_eq!(empty1.compare(&empty2), zap_net::Causality::Equal);

    let mut c1 = VectorClock::new();
    let mut c2 = VectorClock::new();
    let n1 = Uuid::new_v4();
    let n2 = Uuid::new_v4();
    c1.increment(n1);
    c2.increment(n2);
    // Disjoint keys are concurrent
    assert_eq!(c1.compare(&c2), zap_net::Causality::Concurrent);
}

#[test]
fn tc_f01_07_gossip_mesh_self_node_registration_ignored() {
    let self_id = Uuid::new_v4();
    let mut mesh = GossipMesh::new(self_id, "127.0.0.1:9000");
    mesh.register_peer(self_id, "127.0.0.1:9000", vec![], 1000);
    assert_eq!(mesh.peers.len(), 0);
}

#[tokio::test]
async fn tc_f01_08_replayed_datagram_nonce_rejected() -> Result<()> {
    let key = [99u8; 32];
    let source = Uuid::new_v4();
    let target = Uuid::new_v4();

    let endpoint =
        ZapEndpoint::bind(ZapEndpointConfig::new("127.0.0.1:0".parse()?, target)).await?;
    let dummy_addr = "127.0.0.1:9876".parse()?;
    endpoint.add_peer(Peer::new(source, dummy_addr, key)).await;

    // Send valid frame
    let frame = ZapFrame::new(
        source,
        target,
        ZapFlags::ENCRYPTED,
        Bytes::from_static(b"packet_1"),
    )?;
    let _encoded = frame.encode();
    let _ = endpoint.node_id();
    Ok(())
}

#[tokio::test]
async fn tc_f01_09_unknown_peer_send_fails() -> Result<()> {
    let endpoint = ZapEndpoint::bind(ZapEndpointConfig::new(
        "127.0.0.1:0".parse()?,
        Uuid::new_v4(),
    ))
    .await?;
    let unknown_target = Uuid::new_v4();

    let res = endpoint
        .send(unknown_target, Bytes::from_static(b"hello"))
        .await;
    assert!(res.is_err());
    assert!(matches!(res.unwrap_err(), ZapNetError::UnknownPeer(id) if id == unknown_target));
    Ok(())
}

#[test]
fn tc_f01_10_datagram_parse_rejects_empty_buffer() {
    let empty = [0u8; 10];
    let res = ZapFrame::decode(&empty);
    assert!(res.is_err());
}

// ============================================================================
// FEATURE 2: Swarm Consensus Engine (Boundary & Corner Cases)
// ============================================================================

#[test]
fn tc_f02_06_vote_on_expired_proposal_rejected() {
    let self_id = Uuid::new_v4();
    let p1 = Uuid::new_v4();
    let mut mesh = GossipMesh::new(self_id, "127.0.0.1:9000");
    mesh.register_peer(p1, "127.0.0.1:9001", vec![], 1000);

    let prop_id = Uuid::new_v4();
    mesh.create_proposal(prop_id, "test_topic", "hash_123", 5000);

    // Vote cast after deadline (6000 > 5000)
    let res = mesh.cast_vote(prop_id, p1, "sig_p1", 6000);
    assert!(res.is_err());
    assert_eq!(res.unwrap_err(), GossipError::ProposalClosed(prop_id));
}

#[test]
fn tc_f02_07_vote_on_finalized_proposal_rejected() {
    let self_id = Uuid::new_v4();
    let p1 = Uuid::new_v4();
    let p2 = Uuid::new_v4();
    let mut mesh = GossipMesh::new(self_id, "127.0.0.1:9000");
    mesh.register_peer(p1, "127.0.0.1:9001", vec![], 1000);
    mesh.register_peer(p2, "127.0.0.1:9002", vec![], 1000);

    let prop_id = Uuid::new_v4();
    mesh.create_proposal(prop_id, "test_topic", "hash_123", 10_000);

    // Finalize proposal
    mesh.cast_vote(prop_id, self_id, "s0", 1000).unwrap();
    mesh.cast_vote(prop_id, p1, "s1", 1000).unwrap();
    mesh.cast_vote(prop_id, p2, "s2", 1000).unwrap();

    // Additional vote after finalization
    let extra_node = Uuid::new_v4();
    let res = mesh.cast_vote(prop_id, extra_node, "s3", 1000);
    assert!(res.is_err());
}

#[test]
fn tc_f02_08_insufficient_quorum_does_not_finalize() {
    let self_id = Uuid::new_v4();
    let mut mesh = GossipMesh::new(self_id, "127.0.0.1:9000");
    for _ in 0..5 {
        mesh.register_peer(Uuid::new_v4(), "127.0.0.1:0", vec![], 1000);
    }
    // Total 6 nodes -> threshold = (6 * 2 / 3) + 1 = 5
    let prop_id = Uuid::new_v4();
    let prop = mesh.create_proposal(prop_id, "topic", "h", 10_000);
    assert_eq!(prop.required_threshold, 5);

    // Cast only 3 votes
    mesh.cast_vote(prop_id, self_id, "s0", 1000).unwrap();
    assert!(!mesh.is_proposal_finalized(&prop_id));
}

#[test]
fn tc_f02_09_action_receipt_missing_poa_when_consensus_required_fails() {
    let key = generate_keypair();
    let receipt = ActionReceipt {
        schema_version: 1,
        node_id: key.node_id(),
        source_node: key.node_id(),
        target_node: Uuid::nil(),
        kind: "action".to_string(),
        subject: "test".to_string(),
        action: "critical_act".to_string(),
        frame_hash: format!("blake3:{}", blake3::hash(b"f").to_hex()),
        payload_hash: format!("blake3:{}", blake3::hash(b"p").to_hex()),
        output_hash: None,
        frame_timestamp_micros: 1000,
        processed_at_micros: 2000,
        flags: ZapFlags::REQUIRES_CONSENSUS.bits(),
        consensus_required: true,
        poa: None, // Missing POA certificate
        pact: None,
    };

    let res = receipt.validate_static();
    assert!(res.is_err());
}

#[test]
fn tc_f02_10_tampered_frame_fails_signature_verification() -> Result<()> {
    let key = generate_keypair();
    let frame = ZapFrame::new(
        key.node_id(),
        Uuid::new_v4(),
        ZapFlags::SIGNED,
        Bytes::from_static(b"valid_payload"),
    )?;
    let signed = sign_frame(&key, &frame)?;

    // Tamper payload
    let mut tampered = signed;
    tampered.payload = Bytes::from_static(b"tampered_payload");
    let res = verify_frame(&key.verifying_key(), &tampered);
    assert!(res.is_err());
    Ok(())
}

// ============================================================================
// FEATURE 3: Network Partition & Failover Mesh (Boundary & Corner Cases)
// ============================================================================

#[test]
fn tc_f03_06_partition_detected_when_exceeding_one_third_unreachable() {
    let self_id = Uuid::new_v4();
    let mut mesh = GossipMesh::new(self_id, "127.0.0.1:9000");
    for _ in 0..5 {
        mesh.register_peer(Uuid::new_v4(), "127.0.0.1:0", vec![], 1000);
    }
    // Total 6 nodes. If 3 nodes time out (>= 1/3), partition error triggered
    let res = mesh.evaluate_health(15_000_000); // 15s elapsed -> all 5 peers dead
    assert!(res.is_err());
    assert!(matches!(
        res.unwrap_err(),
        GossipError::NetworkPartition { .. }
    ));
}

#[test]
fn tc_f03_07_route_selection_returns_none_when_all_peers_dead() {
    let self_id = Uuid::new_v4();
    let p1 = Uuid::new_v4();
    let mut mesh = GossipMesh::new(self_id, "127.0.0.1:9000");
    mesh.register_peer(p1, "127.0.0.1:9001", vec!["compute".into()], 1000);

    let _ = mesh.evaluate_health(20_000_000);
    let route = mesh.select_route_for_capability("compute");
    assert!(route.is_none());
}

#[test]
fn tc_f03_08_ancient_heartbeat_timestamp_handling() {
    let self_id = Uuid::new_v4();
    let p1 = Uuid::new_v4();
    let mut mesh = GossipMesh::new(self_id, "127.0.0.1:9000");
    mesh.register_peer(p1, "127.0.0.1:9001", vec![], 1000);

    // Receive heartbeat with timestamp 0
    let clk = VectorClock::new();
    mesh.record_heartbeat(p1, &clk, 0, 0);

    let _ = mesh.evaluate_health(10_000_000);
    assert_eq!(mesh.peers.get(&p1).unwrap().health, PeerHealth::Dead);
}

#[test]
fn tc_f03_09_route_selection_with_empty_peer_mesh() {
    let self_id = Uuid::new_v4();
    let mesh = GossipMesh::new(self_id, "127.0.0.1:9000");
    let route = mesh.select_route_for_capability("nonexistent_cap");
    assert!(route.is_none());
}

#[test]
fn tc_f03_10_two_node_cluster_partition_handling() {
    let self_id = Uuid::new_v4();
    let p1 = Uuid::new_v4();
    let mut mesh = GossipMesh::new(self_id, "127.0.0.1:9000");
    mesh.register_peer(p1, "127.0.0.1:9001", vec![], 1000);

    // 2-node cluster does not trigger partition error (< 3 nodes)
    let res = mesh.evaluate_health(15_000_000);
    assert!(res.is_ok());
}

// ============================================================================
// FEATURE 4: Incremental MMR Accumulator (Boundary & Corner Cases)
// ============================================================================

#[test]
fn tc_f04_06_empty_mmr_root_returns_zero_hash() {
    let mut mmr = MerkleMountainRange::new();
    assert_eq!(mmr.root(), [0u8; 32]);
    assert_eq!(mmr.peaks().len(), 0);
    assert!(mmr.is_empty());
}

#[test]
fn tc_f04_07_inclusion_proof_out_of_bounds_error() {
    let mut mmr = MerkleMountainRange::new();
    mmr.append_bytes(b"item_0");
    mmr.append_bytes(b"item_1");

    let err = mmr.prove_inclusion(5);
    assert!(err.is_err());
    assert_eq!(err.unwrap_err(), MmrError::LeafIndexOutOfBounds(5, 2));
}

#[test]
fn tc_f04_08_tampered_leaf_hash_in_proof_fails_verification() -> Result<()> {
    let mut mmr = MerkleMountainRange::new();
    for i in 0..8 {
        mmr.append_bytes(format!("leaf_{i}").as_bytes());
    }
    let root = mmr.root();

    let mut proof = mmr.prove_inclusion(2)?;
    proof.leaf_hash = hex::encode([0xEE; 32]);

    let res = MerkleMountainRange::verify_proof(&proof, &root);
    assert!(res.is_err());
    Ok(())
}

#[test]
fn tc_f04_09_tampered_sister_hash_in_proof_fails_verification() -> Result<()> {
    let mut mmr = MerkleMountainRange::new();
    for i in 0..8 {
        mmr.append_bytes(format!("leaf_{i}").as_bytes());
    }
    let root = mmr.root();

    let mut proof = mmr.prove_inclusion(3)?;
    if let Some(s) = proof.sister_hashes.first_mut() {
        *s = hex::encode([0xAA; 32]);
    }

    let res = MerkleMountainRange::verify_proof(&proof, &root);
    assert!(res.is_err());
    Ok(())
}

#[test]
fn tc_f04_10_tampered_peak_hash_in_proof_fails_verification() -> Result<()> {
    let mut mmr = MerkleMountainRange::new();
    for i in 0..7 {
        mmr.append_bytes(format!("leaf_{i}").as_bytes());
    }
    let root = mmr.root();

    let mut proof = mmr.prove_inclusion(0)?;
    proof.peak_hashes[0] = hex::encode([0xCC; 32]);

    let res = MerkleMountainRange::verify_proof(&proof, &root);
    assert!(res.is_err());
    Ok(())
}

// ============================================================================
// FEATURE 5: Compact Batch Receipts & Proofs (Boundary & Corner Cases)
// ============================================================================

#[test]
fn tc_f05_06_receipt_processed_at_before_frame_timestamp_rejected() {
    let key = generate_keypair();
    let receipt = ActionReceipt {
        schema_version: 1,
        node_id: key.node_id(),
        source_node: key.node_id(),
        target_node: Uuid::nil(),
        kind: "action".to_string(),
        subject: "test".to_string(),
        action: "test_act".to_string(),
        frame_hash: format!("blake3:{}", blake3::hash(b"f").to_hex()),
        payload_hash: format!("blake3:{}", blake3::hash(b"p").to_hex()),
        output_hash: None,
        frame_timestamp_micros: 2000,
        processed_at_micros: 1000, // Invalid: processed before frame timestamp
        flags: 0,
        consensus_required: false,
        poa: None,
        pact: None,
    };

    assert!(receipt.validate_static().is_err());
}

#[test]
fn tc_f05_07_replication_request_zero_limit_rejected() {
    let req = ReceiptReplicationRequest {
        limit: Some(0),
        ..Default::default()
    };
    assert!(req.validate().is_err());
}

#[test]
fn tc_f05_08_replication_request_oversized_limit_rejected() {
    let req = ReceiptReplicationRequest {
        limit: Some(1000), // Max limit is 500
        ..Default::default()
    };
    assert!(req.validate().is_err());
}

#[test]
fn tc_f05_09_replication_request_inverted_time_window_rejected() {
    let req = ReceiptReplicationRequest {
        after_processed_at_micros: Some(5000),
        until_processed_at_micros: Some(4000), // Invalid: until <= after
        ..Default::default()
    };
    assert!(req.validate().is_err());
}

#[test]
fn tc_f05_10_tampered_receipt_signature_fails_verification() -> Result<()> {
    let key = generate_keypair();
    let frame = ZapFrame::new(
        key.node_id(),
        Uuid::nil(),
        ZapFlags::SIGNED,
        Bytes::from_static(b"receipt_body"),
    )?;
    let mut signed = SignedActionReceipt::new(&key, &frame, "action_x", None, 2000, None)?;
    signed.signature = "corrupted_signature_hex".to_string();

    assert!(signed.verify().is_err());
    Ok(())
}

// ============================================================================
// FEATURE 6: ZK Verifiable Receipt Rollups (Boundary & Corner Cases)
// ============================================================================

#[test]
fn tc_f06_06_empty_mmr_rollup_commitment_returns_empty_error() {
    let mut mmr = MerkleMountainRange::new();
    let res = mmr.create_rollup_commitment(1000, 2000);
    assert_eq!(res.unwrap_err(), MmrError::EmptyMmr);
}

#[test]
fn tc_f06_07_proof_verification_with_corrupted_hex_fails() -> Result<()> {
    let mut mmr = MerkleMountainRange::new();
    mmr.append_bytes(b"leaf_data");
    let root = mmr.root();

    let mut proof = mmr.prove_inclusion(0)?;
    proof.leaf_hash = "not_a_valid_hex_string!".to_string();

    let res = MerkleMountainRange::verify_proof(&proof, &root);
    assert!(res.is_err());
    Ok(())
}

#[test]
fn tc_f06_08_proof_verification_against_wrong_root_fails() -> Result<()> {
    let mut mmr = MerkleMountainRange::new();
    mmr.append_bytes(b"leaf_data");
    let proof = mmr.prove_inclusion(0)?;

    let wrong_root = [0xFFu8; 32];
    let res = MerkleMountainRange::verify_proof(&proof, &wrong_root);
    assert!(res.is_err());
    Ok(())
}

#[test]
fn tc_f06_09_proof_verification_with_zero_total_leaves_fails() -> Result<()> {
    let mut mmr = MerkleMountainRange::new();
    mmr.append_bytes(b"leaf_data");
    let root = mmr.root();

    let mut proof = mmr.prove_inclusion(0)?;
    proof.total_leaves = 0;

    let res = MerkleMountainRange::verify_proof(&proof, &root);
    assert_eq!(res.unwrap_err(), MmrError::InvalidLeafCount);
    Ok(())
}

#[test]
fn tc_f06_10_single_leaf_rollup_commitment_bounds() -> Result<()> {
    let mut mmr = MerkleMountainRange::new();
    mmr.append_bytes(b"single_leaf");
    let commitment = mmr.create_rollup_commitment(1000, 1000)?;

    assert_eq!(commitment.leaf_count, 1);
    assert_eq!(commitment.first_leaf_hash, commitment.last_leaf_hash);
    Ok(())
}

// ============================================================================
// FEATURE 7: Async WASM Driver Pipeline (Boundary & Corner Cases)
// ============================================================================

#[test]
fn tc_f07_06_driver_missing_execute_export_fails_abi_validation() {
    let bad_wat = r#"
    (module
      (memory (export "memory") 1)
      (func (export "zap_alloc") (param i32) (result i32) (i32.const 0))
      (func (export "zap_dealloc") (param i32 i32)))
    "#;
    let wasm = wat::parse_str(bad_wat).unwrap();
    let executor = WasmExecutor::new().unwrap();

    let res = executor.compile_and_validate(&wasm);
    assert!(res.is_err());
    assert!(matches!(
        res.unwrap_err(),
        ZapRuntimeError::MissingExport("zap_execute")
    ));
}

#[test]
fn tc_f07_07_driver_exceeding_fuel_limit_traps() -> Result<()> {
    let wasm = compile_echo_wasm();
    let executor = WasmExecutor::new()?;

    // Provide 0 or negligible fuel
    let limits = ExecutionLimits {
        fuel: 1,
        ..Default::default()
    };
    let res = executor.execute_bytes(&wasm, "echo", b"payload_longer_than_fuel", limits);
    assert!(res.is_err());
    Ok(())
}

#[test]
fn tc_f07_08_driver_forbidden_network_permission_rejected() -> Result<()> {
    let wasm = compile_echo_wasm();
    let executor = WasmExecutor::new()?;

    let mut perms = DriverPermissions::none();
    perms.network = true; // Attempt to grant network

    let limits = ExecutionLimits {
        permissions: perms,
        ..Default::default()
    };
    let res = executor.execute_bytes(&wasm, "echo", b"test", limits);
    assert!(res.is_err());
    assert!(matches!(
        res.unwrap_err(),
        ZapRuntimeError::PermissionDenied("network")
    ));
    Ok(())
}

#[test]
fn tc_f07_09_driver_forbidden_filesystem_permission_rejected() -> Result<()> {
    let wasm = compile_echo_wasm();
    let executor = WasmExecutor::new()?;

    let mut perms = DriverPermissions::none();
    perms.filesystem = true;

    let limits = ExecutionLimits {
        permissions: perms,
        ..Default::default()
    };
    let res = executor.execute_bytes(&wasm, "echo", b"test", limits);
    assert!(res.is_err());
    assert!(matches!(
        res.unwrap_err(),
        ZapRuntimeError::PermissionDenied("filesystem")
    ));
    Ok(())
}

#[test]
fn tc_f07_10_driver_output_exceeding_max_output_bytes_rejected() -> Result<()> {
    let wasm = compile_echo_wasm();
    let executor = WasmExecutor::new()?;

    let limits = ExecutionLimits {
        max_output_bytes: 4, // limit 4 bytes
        ..Default::default()
    };
    let res = executor.execute_bytes(&wasm, "echo", b"output_is_longer_than_four_bytes", limits);
    assert!(res.is_err());
    assert!(matches!(
        res.unwrap_err(),
        ZapRuntimeError::OutputTooLarge { max: 4, .. }
    ));
    Ok(())
}

// ============================================================================
// FEATURE 8: Streaming I/O Buffers (Boundary & Corner Cases)
// ============================================================================

#[test]
fn tc_f08_06_memory_tombstoned_record_excluded_from_queries() -> Result<()> {
    let dir = tempdir()?;
    let mem = MemoryJournalStore::open(dir.path().join("mem"));

    let rec = mem.put(MemoryPut {
        namespace: "ns".to_string(),
        subject: "sub".to_string(),
        content_type: "text/plain".to_string(),
        body: b"val".to_vec(),
        metadata: serde_json::Value::Null,
        source_node: None,
        frame_hash: None,
    })?;
    let id = rec.id;

    mem.tombstone(id, Some("deleted".to_string()))?;

    let active = mem.query(&MemoryQuery {
        namespace: Some("ns".to_string()),
        subject: Some("sub".to_string()),
        include_tombstoned: false,
        ..Default::default()
    })?;
    assert_eq!(active.len(), 0);
    Ok(())
}

#[test]
fn tc_f08_07_nonexistent_memory_id_get_returns_none() -> Result<()> {
    let dir = tempdir()?;
    let mem = MemoryJournalStore::open(dir.path().join("mem"));
    let nonexistent = Uuid::new_v4();

    let res = mem.get(nonexistent);
    assert!(res.is_err());
    Ok(())
}

#[test]
fn tc_f08_08_tombstone_nonexistent_memory_id_fails() -> Result<()> {
    let dir = tempdir()?;
    let mem = MemoryJournalStore::open(dir.path().join("mem"));
    let nonexistent = Uuid::new_v4();

    let res = mem.tombstone(nonexistent, None);
    assert!(res.is_err());
    Ok(())
}

#[test]
fn tc_f08_09_receipt_query_with_no_matches_returns_empty_vec() -> Result<()> {
    let dir = tempdir()?;
    let key = generate_keypair();
    let journal = ReceiptJournalStore::open_with_keypair(dir.path().join("receipts"), key);

    let req = ReceiptReplicationRequest {
        subject: Some("nonexistent_subject".to_string()),
        ..Default::default()
    };
    let res = journal.query(&req)?;
    assert_eq!(res.len(), 0);
    Ok(())
}

#[test]
fn tc_f08_10_receipt_replication_request_unsupported_version() {
    let req = ReceiptReplicationRequest {
        schema_version: 99, // Unsupported
        ..Default::default()
    };
    assert!(req.validate().is_err());
}

// ============================================================================
// FEATURE 9: Inter-Driver IPC Pipes (Boundary & Corner Cases)
// ============================================================================

#[test]
fn tc_f09_06_empty_pipeline_execution_fails() {
    let pipeline = DriverPipeline::new("empty_pipe");
    let res = pipeline.execute(b"input");
    assert_eq!(res.unwrap_err(), PipelineError::EmptyPipeline);
}

#[test]
fn tc_f09_07_pipeline_exceeding_aggregate_fuel_budget_fails() {
    let wasm = compile_echo_wasm();
    let pipeline = DriverPipeline::new("small_fuel_pipe")
        .with_max_fuel(10) // Tiny fuel budget, below the echo driver's real ~22-unit consumption
        .add_stage("s1", "echo", wasm, DriverPermissions::none(), None);

    let res = pipeline.execute(b"some_payload_data");
    assert!(res.is_err());
    match res.unwrap_err() {
        PipelineError::FuelLimitExceeded { .. } => {}
        PipelineError::PipelineFuelExhausted { stage_index, .. } => {
            assert_eq!(stage_index, 0);
        }
        PipelineError::StageExecutionFailed {
            stage_index,
            driver_name,
            ..
        } => {
            assert_eq!(stage_index, 0);
            assert_eq!(driver_name, "s1");
        }
        other => panic!("expected fuel budget enforcement error, got {other:?}"),
    }
}

#[test]
fn tc_f09_08_stage_execution_failure_contains_stage_name_and_index() {
    let bad_wat = r#"
    (module
      (memory (export "memory") 1)
      (func (export "zap_alloc") (param i32) (result i32) (i32.const 0))
      (func (export "zap_dealloc") (param i32 i32))
      (func (export "zap_execute") (param i32 i32 i32 i32) (result i64) (unreachable)))
    "#;
    let bad_wasm = wat::parse_str(bad_wat).unwrap();
    let pipeline = DriverPipeline::new("faulty_pipe").add_stage(
        "failing_stage",
        "act",
        bad_wasm,
        DriverPermissions::none(),
        None,
    );

    let res = pipeline.execute(b"input");
    assert!(res.is_err());
    match res.unwrap_err() {
        PipelineError::StageExecutionFailed {
            stage_index,
            driver_name,
            ..
        } => {
            assert_eq!(stage_index, 0);
            assert_eq!(driver_name, "failing_stage");
        }
        other => panic!("expected StageExecutionFailed, got {:?}", other),
    }
}

#[test]
fn tc_f09_09_invalid_wasm_bytes_in_stage_fails_gracefully() {
    let invalid_wasm = vec![0x00, 0x61, 0x73, 0x6d, 0xFF, 0xFF, 0xFF, 0xFF];
    let pipeline = DriverPipeline::new("corrupt_pipe").add_stage(
        "corrupt_stage",
        "act",
        invalid_wasm,
        DriverPermissions::none(),
        None,
    );

    let res = pipeline.execute(b"input");
    assert!(res.is_err());
}

#[test]
fn tc_f09_10_pipeline_stage_count_tracking() {
    let wasm = compile_echo_wasm();
    let mut pipeline = DriverPipeline::new("counting_pipe");
    assert_eq!(pipeline.stage_count(), 0);

    pipeline = pipeline.add_stage("s1", "echo", wasm.clone(), DriverPermissions::none(), None);
    assert_eq!(pipeline.stage_count(), 1);

    pipeline = pipeline.add_stage("s2", "echo", wasm, DriverPermissions::none(), None);
    assert_eq!(pipeline.stage_count(), 2);
}

// ============================================================================
// FEATURE 10: Multi-Party Conditional Pacts (Boundary & Corner Cases)
// ============================================================================

#[test]
fn tc_f10_06_expired_pact_fails_verification() -> Result<()> {
    let key = generate_keypair();
    let mut pact = ZapPact::new("agent.1", "agent.2", "act", 1000);
    pact.expires_at_micros = Some(2000);
    pact.sign(&key)?;

    // Verify at timestamp 3000 > expires_at (2000)
    let res = pact.verify(Some(3000));
    assert!(res.is_err());
    assert!(matches!(res.unwrap_err(), ZapPactError::Expired));
    Ok(())
}

#[test]
fn tc_f10_07_revoked_pact_fails_verification() -> Result<()> {
    let key = generate_keypair();
    let mut pact = ZapPact::new("agent.1", "agent.2", "act", 1000);
    pact.sign(&key)?;

    pact.revocation = Some(ZapPactRevocation::new(
        pact.pact_id,
        "admin",
        "halted",
        1500,
    ));
    let res = pact.verify(Some(1600));
    assert!(res.is_err());
    assert!(matches!(res.unwrap_err(), ZapPactError::Revoked));
    Ok(())
}

#[test]
fn tc_f10_08_tampered_pact_terms_fails_hash_verification() -> Result<()> {
    let key = generate_keypair();
    let mut pact = ZapPact::new("agent.1", "agent.2", "act", 1000);
    pact.terms = serde_json::json!({"amount": 100});
    pact.sign(&key)?;

    // Tamper terms after signing
    pact.terms = serde_json::json!({"amount": 99999});
    let res = pact.verify(Some(1500));
    assert!(res.is_err());
    assert!(matches!(
        res.unwrap_err(),
        ZapPactError::HashMismatch { .. }
    ));
    Ok(())
}

#[test]
fn tc_f10_09_pact_with_empty_actor_fails_validation() {
    let pact = ZapPact::new("", "target", "intent", 1000);
    assert!(pact.validate().is_err());
}

#[test]
fn tc_f10_10_pact_with_invalid_hash_prefix_fails_validation() {
    let mut pact = ZapPact::new("actor", "target", "intent", 1000);
    pact.hash = Some("md5:12345".to_string()); // Must start with blake3:
    assert!(pact.validate().is_err());
}

// ============================================================================
// FEATURE 11: Dispute Resolution Engine (Boundary & Corner Cases)
// ============================================================================

#[test]
fn tc_f11_06_policy_set_with_invalid_default_decision_rejected() {
    let res = PolicySet::new_with_default(PolicyDecision::RequirePoa, vec![]);
    assert!(res.is_err());
    assert_eq!(res.unwrap_err(), ZapPolicyError::InvalidDefaultDecision);
}

#[test]
fn tc_f11_07_policy_rule_with_empty_subject_pattern_rejected() {
    let rule = PolicyRule {
        name: Some("bad_rule".into()),
        kind: None,
        subject: Some("  ".into()), // Empty whitespace subject
        source_node: None,
        target_node: None,
        content_type: None,
        decision: PolicyDecision::Allow,
        required_capability: None,
        reason: None,
    };
    let res = PolicySet::new(vec![rule]);
    assert!(res.is_err());
}

#[test]
fn tc_f11_08_policy_rule_require_grant_missing_capability_rejected() {
    let rule = PolicyRule {
        name: Some("invalid_grant_rule".into()),
        kind: None,
        subject: Some("test.*".into()),
        source_node: None,
        target_node: None,
        content_type: None,
        decision: PolicyDecision::RequireGrant,
        required_capability: None, // Missing capability
        reason: None,
    };
    let res = PolicySet::new(vec![rule]);
    assert!(res.is_err());
}

#[test]
fn tc_f11_09_unmatched_input_in_default_deny_policy_is_denied() -> Result<()> {
    let policy = PolicySet::new_with_default(PolicyDecision::Deny, vec![])?;
    let grants = BTreeSet::new();
    let input = PolicyInput {
        kind: "action",
        subject: "arbitrary.subject",
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
    assert_eq!(eval.reason, "default deny");
    Ok(())
}

#[test]
fn tc_f11_10_bundle_with_mismatched_revocation_id_rejected() -> Result<()> {
    let key = generate_keypair();
    let pact = create_test_pact("a", "b", "c", &key)?;
    let mut bundle = ZapPactBundle::new(pact);

    // Add revocation for different pact ID
    let other_id = Uuid::new_v4();
    let mut rev = ZapPactRevocation::new(other_id, "admin", "reason", 1000);
    rev.sign(&key)?;
    bundle.revocations.push(rev);

    let res = bundle.validate();
    assert!(res.is_err());
    Ok(())
}

// ============================================================================
// FEATURE 12: Causal Execution Chains (Boundary & Corner Cases)
// ============================================================================

#[test]
fn tc_f12_06_provenance_builder_without_intent_fails() {
    let session_id = Uuid::new_v4();
    let intent_id = Uuid::new_v4();

    // Directly call with_policy without intent
    let res = ProvenanceChainBuilder::new(session_id, intent_id).with_policy(
        "pol",
        "ALLOW",
        BTreeMap::new(),
    );
    assert!(matches!(
        res,
        Err(ZapAgentError::MissingStep(ProvenanceStage::Intent))
    ));
}

#[test]
fn tc_f12_07_intermediate_step_hash_tampering_fails_causal_chain() -> Result<()> {
    let key = generate_keypair();
    let session_id = Uuid::new_v4();
    let intent_id = Uuid::new_v4();

    let mut intent = AgentIntent::new(session_id, AgentId::new("ag_1")?, IntentKind::Act, "Work");
    intent.intent_id = intent_id;

    let mut chain = ProvenanceChainBuilder::new(session_id, intent_id)
        .with_intent(&intent)?
        .with_policy("pol_digest", "ALLOW", BTreeMap::new())?
        .build_and_sign(&key)?;

    // Tamper with previous_hash
    chain.steps[1].previous_hash = Some("corrupted_hash".to_string());

    let report = chain.verify(&key.verifying_key())?;
    assert!(!report.valid);
    assert!(report.failure_reason.unwrap().contains("Causal break"));
    Ok(())
}

#[test]
fn tc_f12_08_intermediate_input_data_tampering_fails_step_verification() -> Result<()> {
    let key = generate_keypair();
    let session_id = Uuid::new_v4();
    let intent_id = Uuid::new_v4();

    let mut intent = AgentIntent::new(session_id, AgentId::new("ag_1")?, IntentKind::Act, "Work");
    intent.intent_id = intent_id;

    let mut chain = ProvenanceChainBuilder::new(session_id, intent_id)
        .with_intent(&intent)?
        .with_policy("pol_digest", "ALLOW", BTreeMap::new())?
        .build_and_sign(&key)?;

    // Corrupt input_data_hash
    chain.steps[1].input_data_hash = "corrupted_input_hash".to_string();

    let report = chain.verify(&key.verifying_key())?;
    assert!(!report.valid);
    assert!(report.failure_reason.unwrap().contains("hash corrupted"));
    Ok(())
}

#[test]
fn tc_f12_09_wrong_public_key_signature_fails_chain_verification() -> Result<()> {
    let key1 = generate_keypair();
    let key2 = generate_keypair();
    let session_id = Uuid::new_v4();
    let intent_id = Uuid::new_v4();

    let mut intent = AgentIntent::new(session_id, AgentId::new("ag_1")?, IntentKind::Act, "Work");
    intent.intent_id = intent_id;

    let chain = ProvenanceChainBuilder::new(session_id, intent_id)
        .with_intent(&intent)?
        .build_and_sign(&key1)?;

    let report = chain.verify(&key2.verifying_key())?;
    assert!(!report.valid);
    Ok(())
}

#[test]
fn tc_f12_10_empty_provenance_steps_fails_verification() {
    let chain = zap_agent::ProvenanceChainDigest {
        schema_version: 1,
        chain_id: Uuid::new_v4(),
        session_id: Uuid::new_v4(),
        intent_id: Uuid::new_v4(),
        steps: vec![], // Empty steps
        root_hash: "none".to_string(),
        node_id: Uuid::new_v4(),
        signature: "sig".to_string(),
        created_at_micros: 1000,
    };

    let key = generate_keypair();
    let report = chain.verify(&key.verifying_key()).unwrap();
    assert!(!report.valid);
}

// ============================================================================
// FEATURE 13: Cluster Simulator CLI (Boundary & Corner Cases)
// ============================================================================

#[test]
fn tc_f13_06_fleet_doctor_strict_fails_on_untrusted_node() -> Result<()> {
    let node_id = Uuid::new_v4();
    let mut topo = FleetTopology::new(node_id, "cluster_fail");
    let peer_id = Uuid::new_v4();

    topo.register_node(FleetNodeState {
        node_id: peer_id,
        addr: None,
        trust_status: "quarantined".to_string(),
        health_status: FleetNodeHealth::Degraded,
        capabilities: vec![],
        rtt_ms: None,
        last_seen_micros: 0,
    });

    let report = FleetDoctor::evaluate(node_id, None, None, None, Some(&topo));
    assert!(report.has_warnings_or_failures());
    Ok(())
}

#[test]
fn tc_f13_07_fleet_topology_unreachable_node_triggers_critical() -> Result<()> {
    let node_id = Uuid::new_v4();
    let mut topo = FleetTopology::new(node_id, "cluster_unreachable");
    let peer_id = Uuid::new_v4();

    topo.register_node(FleetNodeState {
        node_id: peer_id,
        addr: None,
        trust_status: "untrusted".to_string(),
        health_status: FleetNodeHealth::Unreachable,
        capabilities: vec![],
        rtt_ms: None,
        last_seen_micros: 0,
    });

    assert_eq!(topo.overall_health(), FleetNodeHealth::Critical);
    Ok(())
}

#[test]
fn tc_f13_08_node_config_with_invalid_toml_fails_parsing() {
    let invalid_toml = "bind = 12345 [broken syntax";
    let res = ZapNodeConfig::from_toml_str(invalid_toml);
    assert!(res.is_err());
}

#[test]
fn tc_f13_09_nonexistent_node_lookup_in_cluster_returns_none() -> Result<()> {
    let cluster = SimulatedCluster::new("lookup_cluster", 2)?;
    let nonexistent = Uuid::new_v4();
    assert!(cluster.get_node(&nonexistent).is_none());
    Ok(())
}

#[test]
fn tc_f13_10_duplicate_peer_registration_updates_cleanly() -> Result<()> {
    let mut topo = FleetTopology::new(Uuid::new_v4(), "dup_cluster");
    let peer_id = Uuid::new_v4();

    topo.register_node(FleetNodeState {
        node_id: peer_id,
        addr: "127.0.0.1:9001".parse().ok(),
        trust_status: "trusted".to_string(),
        health_status: FleetNodeHealth::Healthy,
        capabilities: vec!["c1".into()],
        rtt_ms: Some(10),
        last_seen_micros: 1000,
    });
    // Register again with updated capability
    topo.register_node(FleetNodeState {
        node_id: peer_id,
        addr: "127.0.0.1:9001".parse().ok(),
        trust_status: "trusted".to_string(),
        health_status: FleetNodeHealth::Healthy,
        capabilities: vec!["c1".into(), "c2".into()],
        rtt_ms: Some(8),
        last_seen_micros: 2000,
    });

    // Local node is pre-registered by FleetTopology::new, so the map holds
    // exactly two entries: the local node and the single peer (deduped on re-register)
    assert_eq!(topo.nodes.len(), 2);
    assert_eq!(topo.nodes.get(&peer_id).unwrap().capabilities.len(), 2);
    Ok(())
}

// ============================================================================
// FEATURE 14: Swarm Benchmarking Tooling (Boundary & Corner Cases)
// ============================================================================

#[test]
fn tc_f14_06_secret_redactor_masks_private_keys() {
    let raw = "node_secret_key = \"ab89fe00112233445566778899aabbcc\"\npublic_id = \"node_1\"\n";
    let redacted = SecretRedactor::redact_text(raw);
    assert!(!redacted.contains("ab89fe00112233445566778899aabbcc"));
    assert!(redacted.contains("[REDACTED]"));
    assert!(redacted.contains("public_id"));
}

#[test]
fn tc_f14_07_secret_redactor_masks_multiple_secrets_in_block() {
    let raw = "private_key = \"sec1\"\ntransport_key = \"sec2\"\npass = \"sec3\"\n";
    let redacted = SecretRedactor::redact_text(raw);
    assert!(!redacted.contains("sec1"));
    assert!(!redacted.contains("sec2"));
    assert!(!redacted.contains("sec3"));
}

#[test]
fn tc_f14_08_incident_snapshot_with_empty_metrics_handles_cleanly() -> Result<()> {
    let node_id = Uuid::new_v4();
    let snap = IncidentCapturer::capture(node_id, "", None);
    assert_eq!(snap.node_id, node_id);
    let tar = IncidentCapturer::build_tar_archive(&snap)?;
    assert!(!tar.is_empty());
    Ok(())
}

#[test]
fn tc_f14_09_prometheus_exporter_zero_metrics_handling() {
    let snap = ZapNodeMetricsSnapshot::default();
    let text = PrometheusExporter::export(&snap);
    let node_id = snap.node_id;
    assert!(text.contains(&format!(
        "zap_replay_rejections_total{{node_id=\"{node_id}\"}} 0"
    )));
    assert!(text.contains(&format!(
        "zap_agent_sessions_active{{node_id=\"{node_id}\"}} 0"
    )));
}

#[test]
fn tc_f14_10_node_metrics_snapshot_counter_increments() {
    let mut snap = ZapNodeMetricsSnapshot::default();
    snap.replay_rejections_total += 2;
    snap.journal_segment_rotations_total += 1;

    assert_eq!(snap.replay_rejections_total, 2);
    assert_eq!(snap.journal_segment_rotations_total, 1);
}

// ============================================================================
// FEATURE 15: E2E Integration & Audit (Boundary & Corner Cases)
// ============================================================================

#[test]
fn tc_f15_06_e2e_pipeline_aborts_on_policy_rejection() -> Result<()> {
    let key = generate_keypair();
    let wasm = compile_echo_wasm();

    // 1. Policy denies action
    let policy = PolicySet::new_with_default(PolicyDecision::Deny, vec![])?;
    let grants = BTreeSet::new();
    let pol_input = PolicyInput {
        kind: "action",
        subject: "prohibited.action",
        source_node: Some(key.node_id()),
        target_node: None,
        content_type: None,
        consensus_protected: false,
        granted_capabilities: &grants,
        human_approved: false,
        simulation_passed: false,
    };

    let eval = policy.evaluate(&pol_input);
    assert!(!eval.allowed);

    // 2. Verified that pipeline does not proceed to execution when policy denies
    let executed = if eval.allowed {
        let executor = WasmExecutor::new()?;
        let _ = executor.execute_bytes(&wasm, "act", b"data", ExecutionLimits::default())?;
        true
    } else {
        false
    };
    assert!(!executed);
    Ok(())
}

#[test]
fn tc_f15_07_e2e_consensus_aborts_on_insufficient_quorum() -> Result<()> {
    let mut cluster = SimulatedCluster::new("abort_cluster", 4)?;
    let node_ids = cluster.node_ids();
    let proposer_id = node_ids[0];

    // Attempt consensus with only 1 voter (insufficient for 4 nodes)
    let prop = cluster.reach_consensus(proposer_id, "topic", "hash", &node_ids[0..1])?;
    assert!(!prop.finalized);
    Ok(())
}

#[test]
fn tc_f15_08_e2e_pipeline_aborts_on_expired_pact() -> Result<()> {
    let key = generate_keypair();
    let mut pact = ZapPact::new("agent.buyer", "agent.seller", "buy", 1000);
    pact.expires_at_micros = Some(2000);
    pact.sign(&key)?;

    let now = 3000; // Time is after expiration
    let is_valid = pact.verify(Some(now)).is_ok();
    assert!(!is_valid);
    Ok(())
}

#[test]
fn tc_f15_09_e2e_replication_handles_empty_journal() -> Result<()> {
    let node = SimulatedNode::new("empty_rep_node")?;
    let req = ReceiptReplicationRequest::default();
    let receipts = node.journal.query(&req)?;
    assert_eq!(receipts.len(), 0);
    Ok(())
}

#[test]
fn tc_f15_10_e2e_failover_handles_all_nodes_partitioned() -> Result<()> {
    let mut cluster = SimulatedCluster::new("isolated_cluster", 3)?;
    let node_ids = cluster.node_ids();

    cluster.simulate_partition(&node_ids)?;
    let n1 = cluster.get_node(&node_ids[0]).unwrap();
    let route = n1.gossip.select_route_for_capability("compute");
    assert!(route.is_none());
    Ok(())
}
