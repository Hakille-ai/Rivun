//! ZAP Next-Gen E2E Test Harness (`harness.rs`)
//!
//! Provides opaque-box simulation utilities, mock cluster environments,
//! simulated network topology, WASM driver fixtures, and assertion helpers.

use anyhow::{Context, Result};
use base64::{Engine as _, engine::general_purpose::STANDARD_NO_PAD};
use bytes::Bytes;
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    fs,
    net::{SocketAddr, UdpSocket},
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, Instant},
};
use tempfile::{TempDir, tempdir};
use uuid::Uuid;

use zap_agent::{AgentId, AgentIntent, AgentMessage, AgentSession, IntentKind, ProvenanceChainBuilder, ProvenanceStage};
use zap_capability::DriverPermissions;
use zap_core::{ZapFlags, ZapFrame, now_micros};
use zap_crypto::{Keypair, PublicKey, sign_frame, verify_frame};
use zap_ledger::{
    ActionReceipt, MerkleMountainRange, MmrHash, MmrInclusionProof, MmrRollupCommitment,
    PoaReceipt, ReceiptJournalStore, ReceiptReplicationRequest, ReceiptReplicationResponse,
    SignedActionReceipt,
};
use zap_memory::{MemoryJournalStore, MemoryPut, MemoryQuery, MemoryStore};
use zap_net::{GossipMesh, Peer, PeerHealth, QuorumProposal, VectorClock, ZapEndpoint, ZapEndpointConfig};
use zap_node::ZapNodeConfig;
use zap_pact::{Validate, ZapPact, ZapPactBundle, ZapPactRevocation, ZapPactStatus};
use zap_policy::{PolicyDecision, PolicyInput, PolicyRule, PolicySet};
use zap_runtime::{DriverPipeline, ExecutionLimits, WasmExecutor};
use zap_telemetry::{FleetDoctor, FleetNodeHealth, FleetNodeState, FleetTopology, IncidentCapturer, PrometheusExporter, ZapNodeMetricsSnapshot};

/// Simple helper to generate a free UDP port.
pub fn free_udp_addr() -> String {
    let socket = UdpSocket::bind("127.0.0.1:0").expect("bind free port");
    socket.local_addr().expect("local addr").to_string()
}

/// Helper to generate Ed25519 keypair for test node.
pub fn generate_keypair() -> Keypair {
    Keypair::generate()
}

/// Helper to encode public key to standard Base64 string.
pub fn public_key_string(keypair: &Keypair) -> String {
    STANDARD_NO_PAD.encode(keypair.verifying_key().to_bytes())
}

/// A minimal valid WASM echo driver bytecode compiled from WAT.
pub const ECHO_DRIVER_WAT: &str = r#"
(module
  (memory (export "memory") 1)
  (global $heap (mut i32) (i32.const 1024))
  (func (export "zap_alloc") (param $len i32) (result i32)
    global.get $heap
    global.get $heap
    local.get $len
    i32.add
    global.set $heap)
  (func (export "zap_dealloc") (param i32 i32))
  (func (export "zap_execute")
    (param $action_ptr i32) (param $action_len i32)
    (param $payload_ptr i32) (param $payload_len i32)
    (result i64)
    local.get $payload_ptr
    i64.extend_i32_u
    i64.const 32
    i64.shl
    local.get $payload_len
    i64.extend_i32_u
    i64.or))
"#;

/// A transforming WASM driver that reverses its input payload.
pub const REVERSE_DRIVER_WAT: &str = r#"
(module
  (memory (export "memory") 1)
  (global $heap (mut i32) (i32.const 1024))
  (func (export "zap_alloc") (param $len i32) (result i32)
    global.get $heap
    global.get $heap
    local.get $len
    i32.add
    global.set $heap)
  (func (export "zap_dealloc") (param i32 i32))
  (func (export "zap_execute")
    (param $action_ptr i32) (param $action_len i32)
    (param $payload_ptr i32) (param $payload_len i32)
    (result i64)
    (local $out_ptr i32)
    (local $i i32)
    (local $src i32)
    (local $dst i32)
    
    ;; Allocate output buffer
    (local.set $out_ptr (call 0 (local.get $payload_len)))
    (local.set $i (i32.const 0))
    
    (block $done
      (loop $loop
        (br_if $done (i32.ge_u (local.get $i) (local.get $payload_len)))
        
        ;; src = payload_ptr + i
        (local.set $src (i32.add (local.get $payload_ptr) (local.get $i)))
        ;; dst = out_ptr + (payload_len - 1 - i)
        (local.set $dst (i32.add (local.get $out_ptr) (i32.sub (i32.sub (local.get $payload_len) (i32.const 1)) (local.get $i))))
        
        ;; copy byte
        (i32.store8 (local.get $dst) (i32.load8_u (local.get $src)))
        
        (local.set $i (i32.add (local.get $i) (i32.const 1)))
        (br $loop)
      )
    )
    
    (i64.or
      (i64.shl (i64.extend_i32_u (local.get $out_ptr)) (i64.const 32))
      (i64.extend_i32_u (local.get $payload_len))))
)
"#;

pub fn compile_echo_wasm() -> Vec<u8> {
    wat::parse_str(ECHO_DRIVER_WAT).expect("compile echo wat")
}

pub fn compile_reverse_wasm() -> Vec<u8> {
    wat::parse_str(REVERSE_DRIVER_WAT).expect("compile reverse wat")
}

/// Simulated in-process mock node for cluster testing.
pub struct SimulatedNode {
    pub node_id: Uuid,
    pub keypair: Keypair,
    pub addr: String,
    pub dir: TempDir,
    pub config_path: PathBuf,
    pub gossip: GossipMesh,
    pub journal: ReceiptJournalStore,
    pub memory: MemoryJournalStore,
    pub topology: FleetTopology,
}

impl SimulatedNode {
    pub fn new(cluster_name: &str) -> Result<Self> {
        let dir = tempdir()?;
        let keypair = generate_keypair();
        let node_id = keypair.node_id();
        let addr = free_udp_addr();

        let key_path = dir.path().join("node.key");
        fs::write(&key_path, keypair.to_key_file_toml()?)?;

        let config_path = dir.path().join("zap.toml");
        let toml_content = format!(
            "bind = \"{addr}\"\nkey_file = \"{}\"\n",
            key_path.display().to_string().replace('\\', "/")
        );
        fs::write(&config_path, toml_content)?;

        let gossip = GossipMesh::new(node_id, addr.clone());
        let journal = ReceiptJournalStore::open_with_keypair(dir.path().join("receipts"), keypair.clone());
        let memory = MemoryJournalStore::open(dir.path().join("memory"));
        let topology = FleetTopology::new(node_id, cluster_name);

        Ok(Self {
            node_id,
            keypair,
            addr,
            dir,
            config_path,
            gossip,
            journal,
            memory,
            topology,
        })
    }

    /// Append a test signed action receipt to this node's journal.
    pub fn record_action(&self, action: &str, payload: &[u8]) -> Result<SignedActionReceipt> {
        let now = now_micros()?;
        let frame = ZapFrame::new(
            self.node_id,
            Uuid::nil(),
            ZapFlags::SIGNED,
            Bytes::copy_from_slice(payload),
        )?;
        let signed = SignedActionReceipt::new(
            &self.keypair,
            &frame,
            action,
            None,
            now,
            None,
        )?;
        self.journal.append(&signed, false)?;
        Ok(signed)
    }
}

/// Simulated Multi-Node Swarm Cluster.
pub struct SimulatedCluster {
    pub cluster_name: String,
    pub nodes: HashMap<Uuid, SimulatedNode>,
}

impl SimulatedCluster {
    pub fn new(cluster_name: impl Into<String>, node_count: usize) -> Result<Self> {
        let cluster_name = cluster_name.into();
        let mut nodes = HashMap::new();

        for _ in 0..node_count {
            let node = SimulatedNode::new(&cluster_name)?;
            nodes.insert(node.node_id, node);
        }

        // Cross-register peers in gossip and topology
        let peer_infos: Vec<(Uuid, String)> = nodes
            .values()
            .map(|n| (n.node_id, n.addr.clone()))
            .collect();

        let now = now_micros()?;
        for node in nodes.values_mut() {
            for (peer_id, peer_addr) in &peer_infos {
                if *peer_id != node.node_id {
                    node.gossip.register_peer(*peer_id, peer_addr.clone(), vec!["compute".into(), "storage".into()], now);
                    node.topology.register_node(FleetNodeState {
                        node_id: *peer_id,
                        addr: peer_addr.parse().ok(),
                        trust_status: "trusted".to_string(),
                        health_status: FleetNodeHealth::Healthy,
                        capabilities: vec!["compute".into(), "storage".into()],
                        rtt_ms: Some(10),
                        last_seen_micros: now,
                    });
                }
            }
        }

        Ok(Self {
            cluster_name,
            nodes,
        })
    }

    pub fn node_ids(&self) -> Vec<Uuid> {
        self.nodes.keys().copied().collect()
    }

    pub fn get_node(&self, id: &Uuid) -> Option<&SimulatedNode> {
        self.nodes.get(id)
    }

    pub fn get_node_mut(&mut self, id: &Uuid) -> Option<&mut SimulatedNode> {
        self.nodes.get_mut(id)
    }

    /// Broadcast a heartbeat from a node across the swarm.
    pub fn broadcast_heartbeat(&mut self, from_id: Uuid, load_factor: u8) -> Result<()> {
        let now = now_micros()?;
        let clock = {
            let from_node = self.nodes.get_mut(&from_id).context("node not found")?;
            from_node.gossip.vector_clock.increment(from_id);
            from_node.gossip.vector_clock.clone()
        };

        for (id, node) in self.nodes.iter_mut() {
            if *id != from_id {
                node.gossip.record_heartbeat(from_id, &clock, load_factor, now);
            }
        }
        Ok(())
    }

    /// Simulate a network partition isolating `isolated_ids` from the rest.
    pub fn simulate_partition(&mut self, isolated_ids: &[Uuid]) -> Result<()> {
        let now = now_micros()?;
        let simulated_dead_time = now + 20_000_000; // 20s in the future

        for (id, node) in self.nodes.iter_mut() {
            if !isolated_ids.contains(id) {
                // Main partition: isolated nodes time out
                let _ = node.gossip.evaluate_health(simulated_dead_time);
            }
        }
        Ok(())
    }

    /// Propose and vote on a swarm consensus item across all available nodes.
    pub fn reach_consensus(
        &mut self,
        proposer_id: Uuid,
        topic: &str,
        terms_hash: &str,
        voting_node_ids: &[Uuid],
    ) -> Result<QuorumProposal> {
        let proposal_id = Uuid::new_v4();
        let now = now_micros()?;
        let deadline = now + 60_000_000;

        let proposer = self.nodes.get_mut(&proposer_id).context("proposer not found")?;
        proposer.gossip.create_proposal(proposal_id, topic, terms_hash, deadline);

        for voter_id in voting_node_ids {
            let voter = self.nodes.get(&voter_id).context("voter not found")?;
            let sig = public_key_string(&voter.keypair);

            let proposer = self.nodes.get_mut(&proposer_id).unwrap();
            let _ = proposer.gossip.cast_vote(proposal_id, *voter_id, sig, now)?;
        }

        let proposer = self.nodes.get(&proposer_id).unwrap();
        let prop = proposer.gossip.proposals.get(&proposal_id).cloned().context("proposal missing")?;
        Ok(prop)
    }
}

/// Helper to create a test signed PACT with sample object and terms.
pub fn create_test_pact(
    actor: &str,
    target: &str,
    intent: &str,
    keypair: &Keypair,
) -> Result<ZapPact> {
    let now = now_micros()?;
    let mut pact = ZapPact::new(actor, target, intent, now);
    pact.object = serde_json::json!({"action": intent, "resource": "asset_42"});
    pact.terms = serde_json::json!({"escrow_amount": 1000, "timeout_micros": 30_000_000});
    pact.consent = serde_json::json!({"approved": true, "actor": actor});
    pact.proof = serde_json::json!({"policy": "allow_transfer", "version": 1});
    pact.expires_at_micros = Some(now + 60_000_000);
    pact.sign(keypair)?;
    Ok(pact)
}
