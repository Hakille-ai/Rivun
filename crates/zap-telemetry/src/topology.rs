use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::net::SocketAddr;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FleetNodeHealth {
    Healthy,
    Degraded,
    Critical,
    Unreachable,
}

impl FleetNodeHealth {
    pub fn as_str(&self) -> &'static str {
        match self {
            FleetNodeHealth::Healthy => "healthy",
            FleetNodeHealth::Degraded => "degraded",
            FleetNodeHealth::Critical => "critical",
            FleetNodeHealth::Unreachable => "unreachable",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FleetNodeState {
    pub node_id: Uuid,
    pub addr: Option<SocketAddr>,
    pub trust_status: String,
    pub health_status: FleetNodeHealth,
    pub capabilities: Vec<String>,
    pub rtt_ms: Option<u64>,
    pub last_seen_micros: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct FleetTopology {
    pub cluster_id: String,
    pub local_node_id: Uuid,
    pub nodes: BTreeMap<Uuid, FleetNodeState>,
}

impl FleetTopology {
    pub fn new(local_node_id: Uuid, cluster_id: impl Into<String>) -> Self {
        let mut topology = Self {
            cluster_id: cluster_id.into(),
            local_node_id,
            nodes: BTreeMap::new(),
        };
        // Register local node as healthy
        topology.register_node(FleetNodeState {
            node_id: local_node_id,
            addr: None,
            trust_status: "trusted".to_string(),
            health_status: FleetNodeHealth::Healthy,
            capabilities: vec!["core".to_string(), "telemetry".to_string()],
            rtt_ms: Some(0),
            last_seen_micros: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_micros() as u64,
        });
        topology
    }

    pub fn register_node(&mut self, state: FleetNodeState) {
        self.nodes.insert(state.node_id, state);
    }

    pub fn active_peer_count(&self) -> usize {
        self.nodes
            .values()
            .filter(|n| {
                n.node_id != self.local_node_id && n.health_status != FleetNodeHealth::Unreachable
            })
            .count()
    }

    pub fn overall_health(&self) -> FleetNodeHealth {
        let mut has_degraded = false;
        let mut has_unreachable = false;
        let mut has_critical = false;

        for node in self.nodes.values() {
            match node.health_status {
                FleetNodeHealth::Critical => has_critical = true,
                FleetNodeHealth::Unreachable => has_unreachable = true,
                FleetNodeHealth::Degraded => has_degraded = true,
                FleetNodeHealth::Healthy => {}
            }
        }

        if has_critical || (has_unreachable && self.nodes.len() > 1) {
            FleetNodeHealth::Critical
        } else if has_degraded {
            FleetNodeHealth::Degraded
        } else {
            FleetNodeHealth::Healthy
        }
    }
}
