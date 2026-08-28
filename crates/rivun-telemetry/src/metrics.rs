use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PeerCounter {
    pub peer: Uuid,
    pub value: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReasonCounter {
    pub reason: String,
    pub value: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActionCounter {
    pub action: String,
    pub value: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransportCounter {
    pub transport: String,
    pub status: String,
    pub value: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PeerTrustGauge {
    pub peer: Uuid,
    pub status: String,
    pub value: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RivunNodeMetricsSnapshot {
    pub node_id: Uuid,
    pub frames_sent_total: Vec<PeerCounter>,
    pub frames_received_total: Vec<PeerCounter>,
    pub frames_rejected_total: Vec<ReasonCounter>,
    pub driver_execution_errors_total: Vec<ActionCounter>,
    pub peer_trust_status: Vec<PeerTrustGauge>,
    pub registry_signature_valid: Option<u8>,
    pub capability_cache_age_seconds: Option<u64>,
    pub receipt_log_verify_failures_total: u64,
    pub poa_attestation_failures_total: u64,
    pub replay_rejections_total: u64,
    pub replay_drops_total: u64,
    pub journal_segment_rotations_total: u64,
    pub segment_manifest_errors_total: u64,
    pub pack_verification_failures_total: u64,
    pub store_verifications_total: u64,
    pub agent_gateway_requests_total: Vec<TransportCounter>,
    pub agent_sessions_active: i64,
    pub provenance_verification_failures_total: u64,
    pub peers_active: u64,
}

impl Default for RivunNodeMetricsSnapshot {
    fn default() -> Self {
        Self {
            node_id: Uuid::nil(),
            frames_sent_total: Vec::new(),
            frames_received_total: Vec::new(),
            frames_rejected_total: Vec::new(),
            driver_execution_errors_total: Vec::new(),
            peer_trust_status: Vec::new(),
            registry_signature_valid: Some(1),
            capability_cache_age_seconds: Some(0),
            receipt_log_verify_failures_total: 0,
            poa_attestation_failures_total: 0,
            replay_rejections_total: 0,
            replay_drops_total: 0,
            journal_segment_rotations_total: 0,
            segment_manifest_errors_total: 0,
            pack_verification_failures_total: 0,
            store_verifications_total: 0,
            agent_gateway_requests_total: Vec::new(),
            agent_sessions_active: 0,
            provenance_verification_failures_total: 0,
            peers_active: 0,
        }
    }
}

fn prometheus_escape(input: &str) -> String {
    input
        .replace('\\', "\\\\")
        .replace('\n', "\\n")
        .replace('"', "\\\"")
}

impl RivunNodeMetricsSnapshot {
    pub fn to_prometheus_text(&self) -> String {
        let mut output = String::new();

        // 1. rivun_frames_sent_total
        output.push_str("# HELP rivun_frames_sent_total Rivun frames sent by peer.\n");
        output.push_str("# TYPE rivun_frames_sent_total counter\n");
        for counter in &self.frames_sent_total {
            output.push_str(&format!(
                "rivun_frames_sent_total{{node_id=\"{}\",peer=\"{}\"}} {}\n",
                self.node_id, counter.peer, counter.value
            ));
        }

        // 2. rivun_frames_received_total
        output.push_str("# HELP rivun_frames_received_total Rivun frames received by peer.\n");
        output.push_str("# TYPE rivun_frames_received_total counter\n");
        for counter in &self.frames_received_total {
            output.push_str(&format!(
                "rivun_frames_received_total{{node_id=\"{}\",peer=\"{}\"}} {}\n",
                self.node_id, counter.peer, counter.value
            ));
        }

        // 3. rivun_frames_rejected_total
        output
            .push_str("# HELP rivun_frames_rejected_total Rivun inbound frames rejected by reason.\n");
        output.push_str("# TYPE rivun_frames_rejected_total counter\n");
        for counter in &self.frames_rejected_total {
            output.push_str(&format!(
                "rivun_frames_rejected_total{{node_id=\"{}\",reason=\"{}\"}} {}\n",
                self.node_id,
                prometheus_escape(&counter.reason),
                counter.value
            ));
        }

        // 4. rivun_driver_execution_errors_total
        output.push_str(
            "# HELP rivun_driver_execution_errors_total Rivun WASM driver execution failures.\n",
        );
        output.push_str("# TYPE rivun_driver_execution_errors_total counter\n");
        for counter in &self.driver_execution_errors_total {
            output.push_str(&format!(
                "rivun_driver_execution_errors_total{{node_id=\"{}\",action=\"{}\"}} {}\n",
                self.node_id,
                prometheus_escape(&counter.action),
                counter.value
            ));
        }

        // 5. rivun_peer_trust_status
        output
            .push_str("# HELP rivun_peer_trust_status Peer trust status gauge by peer and status.\n");
        output.push_str("# TYPE rivun_peer_trust_status gauge\n");
        for gauge in &self.peer_trust_status {
            output.push_str(&format!(
                "rivun_peer_trust_status{{node_id=\"{}\",peer=\"{}\",status=\"{}\"}} {}\n",
                self.node_id, gauge.peer, gauge.status, gauge.value
            ));
        }

        // 6. rivun_registry_signature_valid
        if let Some(valid) = self.registry_signature_valid {
            output.push_str(
                "# HELP rivun_registry_signature_valid Whether the local registry signature verifies.\n",
            );
            output.push_str("# TYPE rivun_registry_signature_valid gauge\n");
            output.push_str(&format!(
                "rivun_registry_signature_valid{{node_id=\"{}\"}} {}\n",
                self.node_id, valid
            ));
        }

        // 7. rivun_capability_cache_age_seconds
        if let Some(age) = self.capability_cache_age_seconds {
            output.push_str(
                "# HELP rivun_capability_cache_age_seconds Age of the local capability cache file.\n",
            );
            output.push_str("# TYPE rivun_capability_cache_age_seconds gauge\n");
            output.push_str(&format!(
                "rivun_capability_cache_age_seconds{{node_id=\"{}\"}} {}\n",
                self.node_id, age
            ));
        }

        // 8. rivun_receipt_log_verify_failures_total
        output.push_str(
            "# HELP rivun_receipt_log_verify_failures_total Receipt log verification failures.\n",
        );
        output.push_str("# TYPE rivun_receipt_log_verify_failures_total counter\n");
        output.push_str(&format!(
            "rivun_receipt_log_verify_failures_total{{node_id=\"{}\"}} {}\n",
            self.node_id, self.receipt_log_verify_failures_total
        ));

        // 9. rivun_poa_attestation_failures_total
        output.push_str("# HELP rivun_poa_attestation_failures_total Proof-of-Action validation or attestation failures.\n");
        output.push_str("# TYPE rivun_poa_attestation_failures_total counter\n");
        output.push_str(&format!(
            "rivun_poa_attestation_failures_total{{node_id=\"{}\"}} {}\n",
            self.node_id, self.poa_attestation_failures_total
        ));

        // 10. rivun_replay_rejections_total & rivun_replay_drops_total
        output.push_str(
            "# HELP rivun_replay_rejections_total Total replay rejections across node execution.\n",
        );
        output.push_str("# TYPE rivun_replay_rejections_total counter\n");
        output.push_str(&format!(
            "rivun_replay_rejections_total{{node_id=\"{}\"}} {}\n",
            self.node_id, self.replay_rejections_total
        ));
        output.push_str("# HELP rivun_replay_drops_total Total replay drops recorded.\n");
        output.push_str("# TYPE rivun_replay_drops_total counter\n");
        output.push_str(&format!(
            "rivun_replay_drops_total{{node_id=\"{}\"}} {}\n",
            self.node_id, self.replay_drops_total
        ));

        // 11. rivun_journal_segment_rotations_total
        output.push_str("# HELP rivun_journal_segment_rotations_total Total journal segment rotations executed.\n");
        output.push_str("# TYPE rivun_journal_segment_rotations_total counter\n");
        output.push_str(&format!(
            "rivun_journal_segment_rotations_total{{node_id=\"{}\"}} {}\n",
            self.node_id, self.journal_segment_rotations_total
        ));

        // 12. rivun_segment_manifest_errors_total
        output.push_str("# HELP rivun_segment_manifest_errors_total Total segment manifest verification or signing failures.\n");
        output.push_str("# TYPE rivun_segment_manifest_errors_total counter\n");
        output.push_str(&format!(
            "rivun_segment_manifest_errors_total{{node_id=\"{}\"}} {}\n",
            self.node_id, self.segment_manifest_errors_total
        ));

        // 13. rivun_pack_verification_failures_total & rivun_store_verifications_total
        output.push_str("# HELP rivun_pack_verification_failures_total Total domain pack verification failures.\n");
        output.push_str("# TYPE rivun_pack_verification_failures_total counter\n");
        output.push_str(&format!(
            "rivun_pack_verification_failures_total{{node_id=\"{}\"}} {}\n",
            self.node_id, self.pack_verification_failures_total
        ));
        output.push_str("# HELP rivun_store_verifications_total Total RivunStore driver bundle verifications performed.\n");
        output.push_str("# TYPE rivun_store_verifications_total counter\n");
        output.push_str(&format!(
            "rivun_store_verifications_total{{node_id=\"{}\"}} {}\n",
            self.node_id, self.store_verifications_total
        ));

        // 14. rivun_agent_gateway_requests_total
        output.push_str(
            "# HELP rivun_agent_gateway_requests_total Total requests handled by AI agent gateway.\n",
        );
        output.push_str("# TYPE rivun_agent_gateway_requests_total counter\n");
        for counter in &self.agent_gateway_requests_total {
            output.push_str(&format!(
                "rivun_agent_gateway_requests_total{{node_id=\"{}\",transport=\"{}\",status=\"{}\"}} {}\n",
                self.node_id,
                prometheus_escape(&counter.transport),
                prometheus_escape(&counter.status),
                counter.value
            ));
        }

        // 15. rivun_agent_sessions_active
        output.push_str("# HELP rivun_agent_sessions_active Currently active AI agent sessions.\n");
        output.push_str("# TYPE rivun_agent_sessions_active gauge\n");
        output.push_str(&format!(
            "rivun_agent_sessions_active{{node_id=\"{}\"}} {}\n",
            self.node_id, self.agent_sessions_active
        ));

        // 16. rivun_provenance_verification_failures_total
        output.push_str("# HELP rivun_provenance_verification_failures_total Total cryptographic provenance chain verification failures.\n");
        output.push_str("# TYPE rivun_provenance_verification_failures_total counter\n");
        output.push_str(&format!(
            "rivun_provenance_verification_failures_total{{node_id=\"{}\"}} {}\n",
            self.node_id, self.provenance_verification_failures_total
        ));

        // 17. rivun_peers_active
        output.push_str(
            "# HELP rivun_peers_active Number of active reachable peers in the fleet mesh.\n",
        );
        output.push_str("# TYPE rivun_peers_active gauge\n");
        output.push_str(&format!(
            "rivun_peers_active{{node_id=\"{}\"}} {}\n",
            self.node_id, self.peers_active
        ));

        output
    }
}

pub struct PrometheusExporter;

impl PrometheusExporter {
    pub fn export(snapshot: &RivunNodeMetricsSnapshot) -> String {
        snapshot.to_prometheus_text()
    }
}
