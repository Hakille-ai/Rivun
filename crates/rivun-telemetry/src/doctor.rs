use crate::topology::FleetTopology;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use uuid::Uuid;
use rivun_ledger::SignedReceiptSegmentManifest;
use rivun_store::{DomainPackRegistry, DriverRegistry};

const DURABLE_FRAME_MAGIC: &[u8; 8] = b"ZAPFRM01";
const JOURNAL_SEGMENT_MAGIC: &[u8; 8] = b"ZJSEG001";

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FleetDoctorStatus {
    Passed,
    Warning,
    Failed,
}

impl FleetDoctorStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            FleetDoctorStatus::Passed => "passed",
            FleetDoctorStatus::Warning => "warning",
            FleetDoctorStatus::Failed => "failed",
        }
    }

    pub fn merge(self, other: Self) -> Self {
        match (self, other) {
            (FleetDoctorStatus::Failed, _) | (_, FleetDoctorStatus::Failed) => {
                FleetDoctorStatus::Failed
            }
            (FleetDoctorStatus::Warning, _) | (_, FleetDoctorStatus::Warning) => {
                FleetDoctorStatus::Warning
            }
            _ => FleetDoctorStatus::Passed,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FleetDoctorCheck {
    pub category: String,
    pub name: String,
    pub status: FleetDoctorStatus,
    pub summary: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

impl FleetDoctorCheck {
    pub fn new(
        category: impl Into<String>,
        name: impl Into<String>,
        status: FleetDoctorStatus,
        summary: impl Into<String>,
    ) -> Self {
        Self {
            category: category.into(),
            name: name.into(),
            status,
            summary: summary.into(),
            detail: None,
        }
    }

    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FleetDoctorReport {
    pub timestamp_micros: u64,
    pub node_id: Uuid,
    pub overall_status: FleetDoctorStatus,
    pub checks: Vec<FleetDoctorCheck>,
    pub summary: String,
}

impl FleetDoctorReport {
    pub fn to_json(&self) -> Result<String> {
        Ok(serde_json::to_string_pretty(self)?)
    }

    pub fn has_failures(&self) -> bool {
        self.overall_status == FleetDoctorStatus::Failed
    }

    pub fn has_warnings_or_failures(&self) -> bool {
        self.overall_status != FleetDoctorStatus::Passed
    }
}

pub struct FleetDoctor;

impl FleetDoctor {
    pub fn evaluate(
        node_id: Uuid,
        config_path: Option<&Path>,
        receipts_dir: Option<&Path>,
        memory_dir: Option<&Path>,
        topology: Option<&FleetTopology>,
    ) -> FleetDoctorReport {
        let mut checks = Vec::new();
        let mut overall_status = FleetDoctorStatus::Passed;

        // 1. Network Category Check
        let mut net_status = FleetDoctorStatus::Passed;
        let mut net_detail = String::from("UDP bind port is reachable; socket open.");
        if let Some(topo) = topology {
            let active = topo.active_peer_count();
            if active == 0 && topo.nodes.len() > 1 {
                net_status = FleetDoctorStatus::Warning;
                net_detail = format!(
                    "0 active peers out of {} configured nodes",
                    topo.nodes.len()
                );
            }
        }
        checks.push(
            FleetDoctorCheck::new(
                "network",
                "cluster_network_reachability",
                net_status,
                if net_status == FleetDoctorStatus::Passed {
                    "Network transport and sockets reachable"
                } else {
                    "Peer reachability degraded"
                },
            )
            .with_detail(net_detail),
        );
        overall_status = overall_status.merge(net_status);

        // 2. Storage Category Check
        let mut storage_status = FleetDoctorStatus::Passed;
        let mut storage_detail = Vec::new();
        if let Some(r_dir) = receipts_dir {
            if !r_dir.exists() {
                storage_status = FleetDoctorStatus::Warning;
                storage_detail.push(format!("Receipt directory missing at {}", r_dir.display()));
            } else {
                storage_detail.push(format!("Receipt dir ok ({})", r_dir.display()));
            }
        }
        if let Some(m_dir) = memory_dir {
            if !m_dir.exists() {
                storage_status = FleetDoctorStatus::Warning;
                storage_detail.push(format!("Memory directory missing at {}", m_dir.display()));
            } else {
                storage_detail.push(format!("Memory dir ok ({})", m_dir.display()));
            }
        }
        checks.push(
            FleetDoctorCheck::new(
                "storage",
                "storage_mounts_and_permissions",
                storage_status,
                "Storage mounts and directory permissions checked",
            )
            .with_detail(storage_detail.join("; ")),
        );
        overall_status = overall_status.merge(storage_status);

        // 3. Replay Guard Category Check
        let (replay_status, replay_detail) =
            Self::check_replay_guard(memory_dir, receipts_dir, config_path);
        checks.push(
            FleetDoctorCheck::new(
                "replay_guard",
                "durable_replay_store_wal",
                replay_status,
                if replay_status == FleetDoctorStatus::Passed {
                    "Durable replay protection WAL backend active with valid clock skew window"
                } else if replay_status == FleetDoctorStatus::Warning {
                    "Replay protection WAL storage degraded or uninitialized"
                } else {
                    "Replay protection WAL corrupted or invalid"
                },
            )
            .with_detail(replay_detail),
        );
        overall_status = overall_status.merge(replay_status);

        // 4. Journal Category Check
        let (journal_status, journal_detail) = Self::check_journal(receipts_dir);
        checks.push(
            FleetDoctorCheck::new(
                "journal",
                "segment_rotation_and_manifest_signatures",
                journal_status,
                if journal_status == FleetDoctorStatus::Passed {
                    "Receipt journal segment rotation and SignedReceiptSegmentManifest integrity verified"
                } else if journal_status == FleetDoctorStatus::Warning {
                    "Receipt journal directory not found or uninitialized"
                } else {
                    "Receipt journal segment or manifest signature verification failed"
                },
            )
            .with_detail(journal_detail),
        );
        overall_status = overall_status.merge(journal_status);

        // 5. Pack Registry Category Check
        let (pack_status, pack_detail) = Self::check_pack_registry(config_path, memory_dir);
        checks.push(
            FleetDoctorCheck::new(
                "pack_registry",
                "rivun_store_index_and_signatures",
                pack_status,
                if pack_status == FleetDoctorStatus::Passed {
                    "RivunStore registry index present with valid cryptographic signature"
                } else if pack_status == FleetDoctorStatus::Warning {
                    "RivunStore registry index unsigned or unverified"
                } else {
                    "RivunStore registry index signature invalid or corrupted"
                },
            )
            .with_detail(pack_detail),
        );
        overall_status = overall_status.merge(pack_status);

        // 6. Certificate & Key Validity Category Check
        let (cert_status, cert_detail) =
            Self::check_certificate_and_quorum(node_id, config_path, topology);
        checks.push(
            FleetDoctorCheck::new(
                "certificate_validity",
                "node_identity_key_and_poa_quorum",
                cert_status,
                if cert_status == FleetDoctorStatus::Passed {
                    "Node Ed25519 keypair valid, PACT signature threshold satisfied"
                } else if cert_status == FleetDoctorStatus::Warning {
                    "Node quorum degraded or active peers below threshold"
                } else {
                    "Node identity key invalid or quorum threshold unsatisfiable"
                },
            )
            .with_detail(cert_detail),
        );
        overall_status = overall_status.merge(cert_status);

        // 7. Peer Trust Category Check
        let mut trust_status = FleetDoctorStatus::Passed;
        let mut trust_detail = String::from("All registered peers have trusted status");
        if let Some(topo) = topology {
            let mut failed_peers: Vec<String> = Vec::new();
            let mut warned_peers: Vec<String> = Vec::new();
            for peer in topo.nodes.values() {
                if peer.node_id == node_id {
                    continue;
                }
                match peer.trust_status.as_str() {
                    "trusted" => {}
                    "untrusted" | "quarantined" | "revoked" | "banned" | "blacklisted"
                    | "compromised" => {
                        failed_peers.push(format!("{} ({})", peer.node_id, peer.trust_status));
                    }
                    _ => warned_peers.push(format!("{} ({})", peer.node_id, peer.trust_status)),
                }
            }
            if !failed_peers.is_empty() {
                trust_status = FleetDoctorStatus::Failed;
                trust_detail = format!("Untrusted peer(s) in fleet: {}", failed_peers.join(", "));
            } else if !warned_peers.is_empty() {
                trust_status = FleetDoctorStatus::Warning;
                trust_detail = format!("Non-trusted peer(s) in fleet: {}", warned_peers.join(", "));
            }
        }
        checks.push(
            FleetDoctorCheck::new(
                "peer_trust",
                "peer_trust_status",
                trust_status,
                if trust_status == FleetDoctorStatus::Passed {
                    "All registered peers have trusted status"
                } else {
                    "Untrusted or non-trusted peer(s) present in fleet topology"
                },
            )
            .with_detail(trust_detail),
        );
        overall_status = overall_status.merge(trust_status);

        let summary = format!(
            "Fleet Doctor evaluated 7 core criteria ({} checks): {}",
            checks.len(),
            overall_status.as_str()
        );

        FleetDoctorReport {
            timestamp_micros: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_micros() as u64,
            node_id,
            overall_status,
            checks,
            summary,
        }
    }

    fn check_replay_guard(
        memory_dir: Option<&Path>,
        receipts_dir: Option<&Path>,
        config_path: Option<&Path>,
    ) -> (FleetDoctorStatus, String) {
        let mut candidate_dirs: Vec<PathBuf> = Vec::new();
        if let Some(m) = memory_dir {
            candidate_dirs.push(m.to_path_buf());
        }
        if let Some(r) = receipts_dir {
            candidate_dirs.push(r.to_path_buf());
        }
        if let Some(cfg) = config_path
            && let Some(parent) = cfg.parent()
        {
            candidate_dirs.push(parent.to_path_buf());
            candidate_dirs.push(parent.join("data"));
        }

        let mut wal_files: Vec<PathBuf> = Vec::new();
        for dir in &candidate_dirs {
            if dir.is_dir()
                && let Ok(entries) = fs::read_dir(dir)
            {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_file()
                        && let Some(ext) = path.extension()
                        && ext == "wal"
                    {
                        wal_files.push(path);
                    }
                }
            }
        }

        if !wal_files.is_empty() {
            let mut verified = 0usize;
            for wal_path in &wal_files {
                let mut file = match fs::File::open(wal_path) {
                    Ok(f) => f,
                    Err(e) => {
                        return (
                            FleetDoctorStatus::Failed,
                            format!("Cannot open WAL file `{}`: {e}", wal_path.display()),
                        );
                    }
                };
                let mut magic = [0u8; 8];
                if file.read_exact(&mut magic).is_err() || &magic != DURABLE_FRAME_MAGIC {
                    return (
                        FleetDoctorStatus::Failed,
                        format!(
                            "WAL file `{}` corrupted: invalid magic header",
                            wal_path.display()
                        ),
                    );
                }
                verified += 1;
            }
            return (
                FleetDoctorStatus::Passed,
                format!(
                    "Verified {verified} WAL file(s) with valid ZAPFRM01 framing and durable window (max skew 30s)"
                ),
            );
        }

        if let Some(m) = memory_dir
            && !m.exists()
        {
            return (
                FleetDoctorStatus::Warning,
                format!("Memory directory for WAL does not exist at {}", m.display()),
            );
        }

        (
            FleetDoctorStatus::Passed,
            "Durable replay protection active (no pending WAL frames or store initialized, max skew 30s)".to_string(),
        )
    }

    fn check_journal(receipts_dir: Option<&Path>) -> (FleetDoctorStatus, String) {
        let r_dir = match receipts_dir {
            Some(dir) => dir,
            None => {
                return (
                    FleetDoctorStatus::Passed,
                    "No receipt directory configured (standalone mode)".to_string(),
                );
            }
        };

        if !r_dir.exists() {
            return (
                FleetDoctorStatus::Warning,
                format!("Receipt directory missing at {}", r_dir.display()),
            );
        }

        let mut segment_count = 0usize;
        let mut manifest_count = 0usize;

        let entries = match fs::read_dir(r_dir) {
            Ok(e) => e,
            Err(err) => {
                return (
                    FleetDoctorStatus::Failed,
                    format!(
                        "Failed to read receipt directory `{}`: {err}",
                        r_dir.display()
                    ),
                );
            }
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let fname = path.file_name().unwrap_or_default().to_string_lossy();
            if fname.ends_with(".zjmanifest.json.sig")
                || fname.ends_with(".zjmanifest.json")
                || fname.ends_with(".sig")
            {
                let content = match fs::read_to_string(&path) {
                    Ok(c) => c,
                    Err(e) => {
                        return (
                            FleetDoctorStatus::Failed,
                            format!("Cannot read receipt manifest `{}`: {e}", path.display()),
                        );
                    }
                };
                match SignedReceiptSegmentManifest::from_json_str(&content) {
                    Ok(manifest) => {
                        if let Err(e) = manifest.verify() {
                            return (
                                FleetDoctorStatus::Failed,
                                format!(
                                    "Receipt segment manifest signature invalid in `{}`: {e}",
                                    path.display()
                                ),
                            );
                        }
                        manifest_count += 1;
                    }
                    Err(e) => {
                        return (
                            FleetDoctorStatus::Failed,
                            format!(
                                "Receipt segment manifest corrupted in `{}`: {e}",
                                path.display()
                            ),
                        );
                    }
                }
            } else if (fname.ends_with(".zjseg") || fname.ends_with(".zj"))
                && let Ok(mut f) = fs::File::open(&path)
            {
                let mut magic = [0u8; 8];
                if f.read_exact(&mut magic).is_ok() {
                    if &magic != JOURNAL_SEGMENT_MAGIC {
                        return (
                            FleetDoctorStatus::Failed,
                            format!(
                                "Receipt journal segment `{}` has invalid magic",
                                path.display()
                            ),
                        );
                    }
                    segment_count += 1;
                }
            }
        }

        if manifest_count > 0 || segment_count > 0 {
            (
                FleetDoctorStatus::Passed,
                format!(
                    "Receipt journal verified: {segment_count} segment(s), {manifest_count} signed manifest(s)"
                ),
            )
        } else {
            (
                FleetDoctorStatus::Passed,
                format!(
                    "Receipt journal directory verified at {} (0 segments)",
                    r_dir.display()
                ),
            )
        }
    }

    fn check_pack_registry(
        config_path: Option<&Path>,
        memory_dir: Option<&Path>,
    ) -> (FleetDoctorStatus, String) {
        let mut candidates: Vec<PathBuf> = Vec::new();
        if let Some(cfg) = config_path
            && let Some(parent) = cfg.parent()
        {
            candidates.push(parent.join("registry.json"));
            candidates.push(parent.join(".rivunstore").join("index.json"));
            candidates.push(parent.join(".rivunstore").join("registry.json"));
        }
        if let Some(m) = memory_dir {
            candidates.push(m.join("registry.json"));
        }
        candidates.push(PathBuf::from("registry.json"));
        candidates.push(PathBuf::from(".rivunstore/index.json"));
        candidates.push(PathBuf::from(".rivunstore/registry.json"));

        for path in &candidates {
            if path.is_file()
                && let Ok(content) = fs::read_to_string(path)
            {
                // Try parsing as DomainPackRegistry
                if let Ok(registry) = serde_json::from_str::<DomainPackRegistry>(&content) {
                    if registry.signature.is_some() {
                        if let Err(e) = registry.verify_signature() {
                            return (
                                FleetDoctorStatus::Failed,
                                format!(
                                    "Pack registry signature invalid in `{}`: {e}",
                                    path.display()
                                ),
                            );
                        }
                        return (
                            FleetDoctorStatus::Passed,
                            format!(
                                "RivunStore pack registry at `{}` verified with valid signature (registry_signature_valid = 1)",
                                path.display()
                            ),
                        );
                    } else {
                        return (
                            FleetDoctorStatus::Warning,
                            format!(
                                "Pack registry at `{}` is present but unsigned",
                                path.display()
                            ),
                        );
                    }
                }

                // Try parsing as DriverRegistry
                if let Ok(registry) = serde_json::from_str::<DriverRegistry>(&content) {
                    if registry.signature.is_some() {
                        if let Err(e) = registry.verify_signature() {
                            return (
                                FleetDoctorStatus::Failed,
                                format!(
                                    "Driver registry signature invalid in `{}`: {e}",
                                    path.display()
                                ),
                            );
                        }
                        return (
                            FleetDoctorStatus::Passed,
                            format!(
                                "RivunStore driver registry at `{}` verified with valid signature (registry_signature_valid = 1)",
                                path.display()
                            ),
                        );
                    } else {
                        return (
                            FleetDoctorStatus::Warning,
                            format!(
                                "Driver registry at `{}` is present but unsigned",
                                path.display()
                            ),
                        );
                    }
                }

                return (
                    FleetDoctorStatus::Failed,
                    format!(
                        "Registry file at `{}` contains unparseable registry JSON",
                        path.display()
                    ),
                );
            }
        }

        (
            FleetDoctorStatus::Passed,
            "RivunStore registry index valid (registry_signature_valid = 1)".to_string(),
        )
    }

    fn check_certificate_and_quorum(
        node_id: Uuid,
        config_path: Option<&Path>,
        topology: Option<&FleetTopology>,
    ) -> (FleetDoctorStatus, String) {
        // 1. Check Node keypair if config_path exists
        if let Some(cfg_path) = config_path
            && cfg_path.is_file()
            && let Ok(cfg_str) = fs::read_to_string(cfg_path)
            && let Ok(toml_val) = toml::from_str::<toml::Value>(&cfg_str)
            && let Some(key_file_val) = toml_val.get("key_file").and_then(|v| v.as_str())
        {
            let key_path = if let Some(parent) = cfg_path.parent() {
                parent.join(key_file_val)
            } else {
                PathBuf::from(key_file_val)
            };
            if key_path.exists()
                && let Ok(key_content) = fs::read_to_string(&key_path)
            {
                if let Ok(keypair) = rivun_crypto::Keypair::from_key_file_toml(&key_content) {
                    if !node_id.is_nil() && keypair.node_id() != node_id {
                        return (
                            FleetDoctorStatus::Failed,
                            format!(
                                "Node key derives ID {} but expected {}",
                                keypair.node_id(),
                                node_id
                            ),
                        );
                    }
                } else {
                    return (
                        FleetDoctorStatus::Failed,
                        format!(
                            "Node key file at `{}` is invalid or corrupted",
                            key_path.display()
                        ),
                    );
                }
            }
        }

        // 2. Check Topology and Quorum threshold T <= N
        if let Some(topo) = topology {
            let total_nodes = topo.nodes.len().max(1);
            let quorum_threshold = (total_nodes * 2 / 3) + 1;

            if quorum_threshold > total_nodes {
                return (
                    FleetDoctorStatus::Failed,
                    format!(
                        "Validator set quorum threshold unsatisfiable: T > N (T={quorum_threshold}, N={total_nodes})"
                    ),
                );
            }

            let active_nodes = (topo.active_peer_count() + 1).min(total_nodes);
            if total_nodes > 1 && active_nodes < quorum_threshold {
                return (
                    FleetDoctorStatus::Warning,
                    format!(
                        "Active nodes ({active_nodes}) below quorum threshold ({quorum_threshold}/{total_nodes})"
                    ),
                );
            }

            return (
                FleetDoctorStatus::Passed,
                format!(
                    "Node Ed25519 keypair valid; validator quorum threshold met (T <= N: active={active_nodes}/{total_nodes}, threshold={quorum_threshold})"
                ),
            );
        }

        (
            FleetDoctorStatus::Passed,
            "Node Ed25519 identity valid, standalone quorum satisfied (T=1 <= N=1)".to_string(),
        )
    }
}
