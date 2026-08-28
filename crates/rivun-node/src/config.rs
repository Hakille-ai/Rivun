//! Configuration schema extensions for P2P Swarm Gossip, Consensus, and Adaptive Mesh.

use serde::{Deserialize, Serialize};

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

fn default_true() -> bool {
    true
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

fn default_gossip_fanout() -> usize {
    3
}

fn default_gossip_max_hops() -> u8 {
    16
}

fn default_anti_entropy_interval_ms() -> u64 {
    5000
}

fn default_dedup_cache_size() -> usize {
    65536
}

fn default_pex_interval_ms() -> u64 {
    10000
}

fn default_active_view_size() -> usize {
    8
}

fn default_passive_view_size() -> usize {
    32
}

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

fn default_heartbeat_interval_ms() -> u64 {
    1000
}

fn default_heartbeat_jitter_ms() -> u64 {
    250
}

fn default_phi_suspect_threshold() -> f64 {
    8.0
}

fn default_phi_dead_threshold() -> f64 {
    14.0
}

fn default_partition_quorum_ratio() -> f64 {
    0.67
}

fn default_max_relay_hops() -> u8 {
    2
}
