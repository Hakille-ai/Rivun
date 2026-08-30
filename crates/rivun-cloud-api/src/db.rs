//! Multi-tenant in-memory and relational storage engine for Rivun Cloud API.

use base64::Engine as _;
use chrono::Utc;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;
use rivun_telemetry::{FleetDoctorCheck, FleetDoctorReport, FleetDoctorStatus};

use crate::models::*;

#[derive(Debug, Clone)]
pub struct CloudDatabase {
    state: Arc<RwLock<DbState>>,
}

#[derive(Debug, Default)]
#[allow(dead_code)]
struct DbState {
    orgs: HashMap<Uuid, Organization>,
    org_slug_index: HashMap<String, Uuid>,
    users: HashMap<Uuid, User>,
    user_email_index: HashMap<String, Uuid>,
    memberships: Vec<Membership>,
    api_tokens: HashMap<Uuid, ApiToken>,
    nodes: HashMap<Uuid, NodeRecord>,
    receipts: Vec<ReceiptRecord>,
    policies: HashMap<Uuid, PolicyRecord>,
    validator_sets: HashMap<Uuid, ValidatorSetRecord>,
    attestations: Vec<AttestationRecord>,
    packs: HashMap<Uuid, PackRecord>,
    incidents: HashMap<Uuid, IncidentRecord>,
    audit_logs: Vec<AuditLogRecord>,
    usage: HashMap<Uuid, UsageCounters>,
}

impl CloudDatabase {
    pub fn new() -> Self {
        let db = Self {
            state: Arc::new(RwLock::new(DbState::default())),
        };
        db
    }

    /// Seeds realistic initial data for development, testing, and live demo.
    pub async fn seed_demo_data(&self) {
        let mut s = self.state.write().await;
        let now = Utc::now();

        // 1. Organizations
        let acme_org_id = Uuid::parse_str("a0000000-0000-0000-0000-000000000001").unwrap();
        let acme_org = Organization {
            id: acme_org_id,
            name: "Acme Autonomous Systems".to_string(),
            slug: "acme".to_string(),
            plan: "enterprise".to_string(),
            created_at: now - chrono::Duration::days(30),
        };
        s.orgs.insert(acme_org_id, acme_org.clone());
        s.org_slug_index.insert(acme_org.slug.clone(), acme_org_id);

        let rivun_labs_id = Uuid::parse_str("a0000000-0000-0000-0000-000000000002").unwrap();
        let rivun_labs = Organization {
            id: rivun_labs_id,
            name: "Rivun Global Labs".to_string(),
            slug: "rivun-labs".to_string(),
            plan: "growth".to_string(),
            created_at: now - chrono::Duration::days(15),
        };
        s.orgs.insert(rivun_labs_id, rivun_labs.clone());
        s.org_slug_index.insert(rivun_labs.slug.clone(), rivun_labs_id);

        // 2. Users & Memberships
        let alice_id = Uuid::parse_str("00000001-0000-0000-0000-000000000001").unwrap();
        let alice = User {
            id: alice_id,
            email: "alice@acme.ai".to_string(),
            name: "Alice Vance (Lead Operator)".to_string(),
            created_at: now - chrono::Duration::days(30),
        };
        s.users.insert(alice_id, alice.clone());
        s.user_email_index.insert(alice.email.clone(), alice_id);

        s.memberships.push(Membership {
            user_id: alice_id,
            org_id: acme_org_id,
            role: UserRole::Owner,
            user_email: alice.email.clone(),
            user_name: alice.name.clone(),
            joined_at: now - chrono::Duration::days(30),
        });

        let bob_id = Uuid::parse_str("00000001-0000-0000-0000-000000000002").unwrap();
        let bob = User {
            id: bob_id,
            email: "bob@acme.ai".to_string(),
            name: "Bob Stone (Security Auditor)".to_string(),
            created_at: now - chrono::Duration::days(20),
        };
        s.users.insert(bob_id, bob.clone());
        s.user_email_index.insert(bob.email.clone(), bob_id);

        s.memberships.push(Membership {
            user_id: bob_id,
            org_id: acme_org_id,
            role: UserRole::Auditor,
            user_email: bob.email.clone(),
            user_name: bob.name.clone(),
            joined_at: now - chrono::Duration::days(20),
        });

        // 3. API Tokens
        let token_id = Uuid::parse_str("00000002-0000-0000-0000-000000000001").unwrap();
        s.api_tokens.insert(
            token_id,
            ApiToken {
                id: token_id,
                org_id: acme_org_id,
                name: "Production Edge Bridge Fleet".to_string(),
                token_hash: "rivun_live_secret_token_123456789".to_string(),
                scopes: vec!["ingest:write".to_string(), "policies:read".to_string()],
                created_by: alice_id,
                created_at: now - chrono::Duration::days(10),
                revoked_at: None,
            },
        );

        // 4. Edge Nodes & Fleet Doctor
        let node_names = [
            ("fra1-edge-01", "online", FleetDoctorStatus::Passed, vec!["region:eu-central", "env:prod", "role:gateway"]),
            ("fra1-edge-02", "online", FleetDoctorStatus::Passed, vec!["region:eu-central", "env:prod", "role:validator"]),
            ("iad1-worker-01", "online", FleetDoctorStatus::Passed, vec!["region:us-east", "env:prod", "role:agent-host"]),
            ("iad1-worker-02", "degraded", FleetDoctorStatus::Warning, vec!["region:us-east", "env:prod", "role:worker"]),
            ("sin1-edge-01", "online", FleetDoctorStatus::Passed, vec!["region:ap-southeast", "env:prod", "role:gateway"]),
            ("dev-local-01", "offline", FleetDoctorStatus::Failed, vec!["region:local", "env:dev"]),
        ];

        for (i, (name, status, doc_status, tags)) in node_names.iter().enumerate() {
            let node_id = Uuid::from_u128(0x1000 + i as u128);
            let checks = vec![
                FleetDoctorCheck::new("network", "cluster_network_reachability", FleetDoctorStatus::Passed, "UDP sockets & peer gossip reachable"),
                FleetDoctorCheck::new("storage", "storage_mounts_and_permissions", FleetDoctorStatus::Passed, "Receipts and MMR stores active"),
                FleetDoctorCheck::new("replay_guard", "durable_replay_store_wal", FleetDoctorStatus::Passed, "ZAPFRM01 WAL active with clock skew < 30s"),
                FleetDoctorCheck::new("journal", "segment_rotation_and_manifest_signatures", FleetDoctorStatus::Passed, "Signed manifests and journal segments verified"),
                FleetDoctorCheck::new("pack_registry", "rivun_store_index_and_signatures", FleetDoctorStatus::Passed, "RivunStore pack index signed by authority"),
                FleetDoctorCheck::new("certificate_validity", "node_identity_key_and_poa_quorum", *doc_status, if *doc_status == FleetDoctorStatus::Failed { "Quorum degraded" } else { "Ed25519 keypair valid" }),
                FleetDoctorCheck::new("peer_trust", "peer_trust_status", FleetDoctorStatus::Passed, "All 5 cluster peers marked trusted"),
            ];

            let report = FleetDoctorReport {
                timestamp_micros: (now.timestamp_micros() as u64),
                node_id,
                overall_status: *doc_status,
                checks,
                summary: format!("Fleet Doctor check for {name}: {}", doc_status.as_str()),
            };

            s.nodes.insert(
                node_id,
                NodeRecord {
                    id: node_id,
                    org_id: acme_org_id,
                    public_key: Some(format!("ed25519_pk_{name}")),
                    node_uuid: node_id,
                    label: name.to_string(),
                    tags: tags.iter().map(|s| s.to_string()).collect(),
                    status: status.to_string(),
                    last_seen_at: now - chrono::Duration::seconds((i * 12) as i64),
                    bridge_version: "0.1.0".to_string(),
                    doctor_status: *doc_status,
                    doctor_report: Some(report),
                    metrics: serde_json::json!({
                        "actions_total": 1420 + i * 350,
                        "cpu_usage_pct": 12.4 + (i as f64 * 3.1),
                        "memory_mb": 240 + i * 45,
                        "peer_count": 5,
                        "poa_success_rate": 0.998,
                    }),
                },
            );
        }

        // 5. Receipts & Provenance chains
        let kinds = [
            ("driver.execute:sensor_read", "none", false),
            ("action.smart_building:hvac_tune", "verified", true),
            ("agent.negotiate:resource_pact", "verified", true),
            ("order.settlement:escrow_release", "verified", true),
            ("safety.emergency_brake:actuate", "verified", true),
            ("pack.deploy:industrial_v1", "verified", true),
        ];

        for i in 0..30 {
            let (kind, poa_status, has_provenance) = kinds[i % kinds.len()];
            let receipt_id = Uuid::from_u128(0x2000 + i as u128);
            let receipt_hash = format!("{:064x}", i * 99991 + 0xabc123);
            let root_hash = format!("{:064x}", i * 33331 + 0xdef456);

            let provenance_json = if has_provenance {
                Some(serde_json::json!({
                    "schema_version": 1,
                    "chain_id": receipt_id,
                    "root_hash": root_hash,
                    "steps": [
                        { "stage": "intent", "step_hash": format!("0xintent_{:08x}", i), "timestamp_micros": now.timestamp_micros() },
                        { "stage": "negotiation", "step_hash": format!("0xnegotiation_{:08x}", i), "timestamp_micros": now.timestamp_micros() + 100 },
                        { "stage": "policy", "step_hash": format!("0xpolicy_{:08x}", i), "timestamp_micros": now.timestamp_micros() + 200, "metadata": { "decision": "require_poa" } },
                        { "stage": "consensus", "step_hash": format!("0xconsensus_{:08x}", i), "timestamp_micros": now.timestamp_micros() + 300, "metadata": { "threshold": 3, "total_validators": 4 } },
                        { "stage": "driver", "step_hash": format!("0xdriver_{:08x}", i), "timestamp_micros": now.timestamp_micros() + 400, "metadata": { "driver_id": "wasm.actuator.v1" } },
                        { "stage": "poa", "step_hash": format!("0xpoa_{:08x}", i), "timestamp_micros": now.timestamp_micros() + 500, "metadata": { "signatures_collected": 3 } },
                        { "stage": "receipt", "step_hash": format!("0xreceipt_{:08x}", i), "timestamp_micros": now.timestamp_micros() + 600 }
                    ]
                }))
            } else {
                None
            };

            s.receipts.push(ReceiptRecord {
                id: receipt_id,
                org_id: acme_org_id,
                node_id: Uuid::from_u128(0x1000 + (i % 5) as u128),
                node_label: format!("fra1-edge-{:02}", (i % 2) + 1),
                receipt_hash,
                action_kind: kind.to_string(),
                poa_status: poa_status.to_string(),
                provenance_root_hash: if has_provenance { Some(root_hash) } else { None },
                provenance_chain: provenance_json,
                occurred_at: now - chrono::Duration::minutes((i * 4) as i64),
            });
        }

        // 6. Policies
        let policy_id1 = Uuid::parse_str("00000003-0000-0000-0000-000000000001").unwrap();
        let policy_toml = r#"default_decision = "deny"

[[rules]]
name = "allow_read_telemetry"
kind = "telemetry"
decision = "allow"

[[rules]]
name = "enforce_consensus_on_safety"
subject = "safety.*"
decision = "require_poa"

[[rules]]
name = "grant_driver_echo"
subject = "driver.echo.*"
decision = "require_grant"
required_capability = "driver.execute:echo"
"#;

        let seed_keypair = rivun_crypto::Keypair::generate();
        let seed_pubkey_b64 = base64::engine::general_purpose::STANDARD_NO_PAD.encode(seed_keypair.verifying_key().to_bytes());
        let mut msg1 = Vec::new();
        msg1.extend_from_slice(acme_org.slug.as_bytes());
        msg1.push(b':');
        msg1.extend_from_slice("production-zero-trust-v1".as_bytes());
        msg1.push(b':');
        msg1.extend_from_slice(&1u32.to_be_bytes());
        msg1.push(b':');
        msg1.extend_from_slice(policy_toml.as_bytes());
        let sig1 = seed_keypair.sign_domain_message(b"Rivun-POLICY-BUNDLE-v1", &msg1);
        let sig1_b64 = base64::engine::general_purpose::STANDARD_NO_PAD.encode(sig1);

        s.policies.insert(
            policy_id1,
            PolicyRecord {
                id: policy_id1,
                org_id: acme_org_id,
                name: "production-zero-trust-v1".to_string(),
                version: 1,
                status: PolicyStatus::Active,
                body_toml: policy_toml.to_string(),
                body_json: serde_json::json!({
                    "default_decision": "deny",
                    "rules_count": 3
                }),
                signed_by_pubkey: Some(seed_pubkey_b64),
                signature: Some(sig1_b64),
                created_by: "alice@acme.ai".to_string(),
                created_at: now - chrono::Duration::days(12),
                updated_at: now - chrono::Duration::days(12),
            },
        );

        let policy_id2 = Uuid::parse_str("00000003-0000-0000-0000-000000000002").unwrap();
        s.policies.insert(
            policy_id2,
            PolicyRecord {
                id: policy_id2,
                org_id: acme_org_id,
                name: "production-zero-trust-v2-staged".to_string(),
                version: 2,
                status: PolicyStatus::Staged,
                body_toml: format!("{}\n[[rules]]\nname = \"allow_smart_building\"\nsubject = \"smart_building.*\"\ndecision = \"require_poa\"\n", policy_toml),
                body_json: serde_json::json!({
                    "default_decision": "deny",
                    "rules_count": 4
                }),
                signed_by_pubkey: None,
                signature: None,
                created_by: "alice@acme.ai".to_string(),
                created_at: now - chrono::Duration::hours(3),
                updated_at: now - chrono::Duration::hours(3),
            },
        );

        // 7. Validator Sets
        let val_set_id = Uuid::parse_str("00000004-0000-0000-0000-000000000001").unwrap();
        s.validator_sets.insert(
            val_set_id,
            ValidatorSetRecord {
                id: val_set_id,
                org_id: acme_org_id,
                epoch: 1,
                threshold: 3,
                members: vec![
                    ValidatorMember { node_id: Uuid::from_u128(0x1000), public_key: "ed25519_pk_fra1_01".to_string(), label: "Validator Node FRA-1".to_string(), status: "active".to_string(), uptime_pct: 99.98 },
                    ValidatorMember { node_id: Uuid::from_u128(0x1001), public_key: "ed25519_pk_fra1_02".to_string(), label: "Validator Node FRA-2".to_string(), status: "active".to_string(), uptime_pct: 100.0 },
                    ValidatorMember { node_id: Uuid::from_u128(0x1002), public_key: "ed25519_pk_iad1_01".to_string(), label: "Validator Node IAD-1".to_string(), status: "active".to_string(), uptime_pct: 99.94 },
                    ValidatorMember { node_id: Uuid::from_u128(0x1004), public_key: "ed25519_pk_sin1_01".to_string(), label: "Validator Node SIN-1".to_string(), status: "active".to_string(), uptime_pct: 99.99 },
                ],
                active_from: now - chrono::Duration::days(30),
                status: "active".to_string(),
            },
        );

        // 8. Domain Packs Marketplace (The 7 core packs + custom pack)
        let packs = [
            ("agentic-dev", "0.1.0", "Engineering", "Autonomous agent coordination, git review, and CI dispatch.", "Rivun Foundation", "preview"),
            ("smart-building", "0.1.0", "IoT & Infrastructure", "HVAC, BACnet/Modbus telemetry, occupancy optimization, and energy consensus.", "Rivun Foundation", "preview"),
            ("cloud-ops", "0.1.0", "Cloud", "Kubernetes cluster reconciliation, canary progression, and multi-cloud failover.", "Rivun Foundation", "preview"),
            ("industrial", "0.1.0", "Industrial", "OPC-UA fieldbus bridges, high-frequency PLC vibration analysis, emergency stops.", "Rivun Foundation", "preview"),
            ("personal-ai", "0.1.0", "Agents", "Local edge assistant memory, confidential calendar sync, and permission delegation.", "Rivun Foundation", "preview"),
            ("healthcare", "0.1.0", "Health & Biotech", "HL7/FHIR medical telemetry pipeline with zero-knowledge blinded MMR receipts.", "Rivun Foundation", "preview"),
            ("finance", "0.1.0", "FinTech", "Sub-millisecond multi-party escrow locking, FIX protocol binding, dispute resolution.", "Rivun Foundation", "preview"),
            ("acme-safety-pack", "1.2.0", "Custom Organization", "Acme proprietary safety interlocks and dual-signature physical actuators.", "Acme Autonomous Systems", "private"),
        ];

        for (i, (name, version, cat, desc, author, vis)) in packs.iter().enumerate() {
            let pack_id = Uuid::from_u128(0x3000 + i as u128);
            s.packs.insert(
                pack_id,
                PackRecord {
                    id: pack_id,
                    org_id: if *vis == "private" { Some(acme_org_id) } else { None },
                    name: name.to_string(),
                    version: version.to_string(),
                    category: cat.to_string(),
                    description: desc.to_string(),
                    author: author.to_string(),
                    manifest_hash: format!("{:064x}", i * 7771 + 0x111222),
                    signature: Some(format!("sig_manifest_{:032x}", i * 999)),
                    visibility: vis.to_string(),
                    published_by: author.to_string(),
                    published_at: now - chrono::Duration::days(10 + i as i64),
                    downloads: (1400 - i * 110) as u64,
                },
            );
        }

        // 9. Incidents
        let inc_id = Uuid::parse_str("00000005-0000-0000-0000-000000000001").unwrap();
        s.incidents.insert(
            inc_id,
            IncidentRecord {
                id: inc_id,
                org_id: acme_org_id,
                node_id: Uuid::from_u128(0x1003),
                node_label: "iad1-worker-02".to_string(),
                severity: "warning".to_string(),
                snapshot: serde_json::json!({
                    "reason": "Replay window clock drift warning (> 2.4s)",
                    "redacted_evidence": "All secrets redacted by SecretRedactor",
                    "socket_state": "ESTABLISHED",
                    "mem_bytes": 312000000,
                }),
                resolved: false,
                created_at: now - chrono::Duration::minutes(45),
            },
        );

        // 10. Dashboard Meta-Audit Trail
        s.audit_logs.push(AuditLogRecord {
            id: Uuid::new_v4(),
            org_id: acme_org_id,
            actor_email: "alice@acme.ai".to_string(),
            actor_role: UserRole::Owner,
            action: "policy.stage".to_string(),
            target: "production-zero-trust-v2-staged".to_string(),
            details: serde_json::json!({ "version": 2, "rules_count": 4 }),
            ip_address: Some("192.168.1.42".to_string()),
            created_at: now - chrono::Duration::hours(3),
        });

        s.audit_logs.push(AuditLogRecord {
            id: Uuid::new_v4(),
            org_id: acme_org_id,
            actor_email: "alice@acme.ai".to_string(),
            actor_role: UserRole::Owner,
            action: "token.create".to_string(),
            target: "Production Edge Bridge Fleet".to_string(),
            details: serde_json::json!({ "scopes": ["ingest:write", "policies:read"] }),
            ip_address: Some("192.168.1.42".to_string()),
            created_at: now - chrono::Duration::days(10),
        });

        // 11. Usage Counters
        s.usage.insert(
            acme_org_id,
            UsageCounters {
                org_id: acme_org_id,
                period: "2026-08".to_string(),
                active_nodes: 5,
                receipts_ingested: 48920,
                packs_published: 1,
                policies_deployed: 2,
                last_updated: now,
            },
        );
    }

    // --- Query and Mutation Methods ---

    pub async fn get_org_by_slug_or_id(&self, slug_or_id: &str) -> Option<Organization> {
        let s = self.state.read().await;
        if let Ok(id) = Uuid::parse_str(slug_or_id) {
            if let Some(org) = s.orgs.get(&id) {
                return Some(org.clone());
            }
        }
        if let Some(id) = s.org_slug_index.get(slug_or_id) {
            return s.orgs.get(id).cloned();
        }
        None
    }

    pub async fn list_organizations(&self) -> Vec<Organization> {
        let s = self.state.read().await;
        s.orgs.values().cloned().collect()
    }

    pub async fn list_members(&self, org_id: Uuid) -> Vec<Membership> {
        let s = self.state.read().await;
        s.memberships.iter().filter(|m| m.org_id == org_id).cloned().collect()
    }

    pub async fn add_member(&self, org_id: Uuid, email: String, role: UserRole, name: String) -> Membership {
        let mut s = self.state.write().await;
        let user_id = s.user_email_index.get(&email).cloned().unwrap_or_else(|| {
            let new_id = Uuid::new_v4();
            s.users.insert(new_id, User {
                id: new_id,
                email: email.clone(),
                name: name.clone(),
                created_at: Utc::now(),
            });
            s.user_email_index.insert(email.clone(), new_id);
            new_id
        });

        let membership = Membership {
            user_id,
            org_id,
            role,
            user_email: email,
            user_name: name,
            joined_at: Utc::now(),
        };
        s.memberships.push(membership.clone());
        membership
    }

    pub async fn list_nodes(&self, org_id: Uuid) -> Vec<NodeRecord> {
        let s = self.state.read().await;
        s.nodes.values().filter(|n| n.org_id == org_id).cloned().collect()
    }

    pub async fn get_node(&self, org_id: Uuid, node_id: Uuid) -> Option<NodeRecord> {
        let s = self.state.read().await;
        s.nodes.values().find(|n| n.org_id == org_id && n.node_uuid == node_id).cloned()
    }

    pub async fn upsert_node_telemetry(
        &self,
        org_id: Uuid,
        node_id: Uuid,
        label: Option<String>,
        tags: Vec<String>,
        bridge_version: String,
        doctor_report: FleetDoctorReport,
        metrics: serde_json::Value,
    ) {
        let mut s = self.state.write().await;
        let doctor_status = doctor_report.overall_status;
        let now = Utc::now();

        if let Some(node) = s.nodes.get_mut(&node_id) {
            node.last_seen_at = now;
            node.status = if doctor_status == FleetDoctorStatus::Failed { "degraded".to_string() } else { "online".to_string() };
            node.doctor_status = doctor_status;
            node.doctor_report = Some(doctor_report);
            node.metrics = metrics;
            node.bridge_version = bridge_version;
            if let Some(l) = label { node.label = l; }
            if !tags.is_empty() { node.tags = tags; }
        } else {
            let label = label.unwrap_or_else(|| format!("node-{}", &node_id.to_string()[..8]));
            s.nodes.insert(
                node_id,
                NodeRecord {
                    id: node_id,
                    org_id,
                    public_key: None,
                    node_uuid: node_id,
                    label,
                    tags,
                    status: if doctor_status == FleetDoctorStatus::Failed { "degraded".to_string() } else { "online".to_string() },
                    last_seen_at: now,
                    bridge_version,
                    doctor_status,
                    doctor_report: Some(doctor_report),
                    metrics,
                },
            );
        }
    }

    pub async fn list_receipts(
        &self,
        org_id: Uuid,
        node_id: Option<Uuid>,
        kind: Option<String>,
        poa_status: Option<String>,
        limit: usize,
    ) -> Vec<ReceiptRecord> {
        let s = self.state.read().await;
        s.receipts
            .iter()
            .rev()
            .filter(|r| r.org_id == org_id)
            .filter(|r| node_id.map(|n| r.node_id == n).unwrap_or(true))
            .filter(|r| kind.as_ref().map(|k| r.action_kind.contains(k)).unwrap_or(true))
            .filter(|r| poa_status.as_ref().map(|p| &r.poa_status == p).unwrap_or(true))
            .take(limit)
            .cloned()
            .collect()
    }

    pub async fn get_receipt_by_hash(&self, org_id: Uuid, hash: &str) -> Option<ReceiptRecord> {
        let s = self.state.read().await;
        s.receipts.iter().find(|r| r.org_id == org_id && r.receipt_hash == hash).cloned()
    }

    pub async fn ingest_receipts_batch(
        &self,
        org_id: Uuid,
        node_id: Uuid,
        items: Vec<crate::models::ReceiptRecord>,
    ) -> usize {
        let mut s = self.state.write().await;
        let count = items.len();
        for mut item in items {
            item.org_id = org_id;
            item.node_id = node_id;
            s.receipts.push(item);
        }
        if let Some(u) = s.usage.get_mut(&org_id) {
            u.receipts_ingested += count as u64;
            u.last_updated = Utc::now();
        }
        count
    }

    pub async fn list_policies(&self, org_id: Uuid) -> Vec<PolicyRecord> {
        let s = self.state.read().await;
        s.policies.values().filter(|p| p.org_id == org_id).cloned().collect()
    }

    pub async fn get_policy(&self, org_id: Uuid, policy_id: Uuid) -> Option<PolicyRecord> {
        let s = self.state.read().await;
        s.policies.get(&policy_id).filter(|p| p.org_id == org_id).cloned()
    }

    pub async fn create_policy(
        &self,
        org_id: Uuid,
        name: String,
        body_toml: String,
        creator: String,
    ) -> Result<PolicyRecord, String> {
        let parsed = rivun_policy::PolicySet::from_toml_str(&body_toml)
            .map_err(|e| format!("Invalid TOML policy: {e}"))?;

        let mut s = self.state.write().await;
        let id = Uuid::new_v4();
        let now = Utc::now();

        let record = PolicyRecord {
            id,
            org_id,
            name,
            version: 1,
            status: PolicyStatus::Draft,
            body_toml,
            body_json: serde_json::json!({
                "default_decision": format!("{:?}", parsed.default_decision).to_lowercase(),
                "rules_count": parsed.rules.len()
            }),
            signed_by_pubkey: None,
            signature: None,
            created_by: creator,
            created_at: now,
            updated_at: now,
        };

        s.policies.insert(id, record.clone());
        Ok(record)
    }

    pub async fn stage_policy(&self, org_id: Uuid, policy_id: Uuid) -> Result<PolicyRecord, String> {
        let mut s = self.state.write().await;
        let policy = s.policies.get_mut(&policy_id).filter(|p| p.org_id == org_id)
            .ok_or_else(|| "Policy not found".to_string())?;

        policy.status = PolicyStatus::Staged;
        policy.updated_at = Utc::now();
        Ok(policy.clone())
    }

    pub async fn submit_policy_signature(
        &self,
        org_id: Uuid,
        policy_id: Uuid,
        public_key: String,
        signature: String,
    ) -> Result<PolicyRecord, String> {
        let mut s = self.state.write().await;
        let policy = s.policies.get_mut(&policy_id).filter(|p| p.org_id == org_id)
            .ok_or_else(|| "Policy not found".to_string())?;

        policy.signed_by_pubkey = Some(public_key);
        policy.signature = Some(signature);
        policy.status = PolicyStatus::Signed;
        policy.updated_at = Utc::now();
        Ok(policy.clone())
    }

    pub async fn get_pending_signed_policies(&self, org_id: Uuid) -> Vec<PolicyRecord> {
        let s = self.state.read().await;
        s.policies
            .values()
            .filter(|p| p.org_id == org_id && (p.status == PolicyStatus::Signed || p.status == PolicyStatus::Active))
            .cloned()
            .collect()
    }

    pub async fn list_validator_sets(&self, org_id: Uuid) -> Vec<ValidatorSetRecord> {
        let s = self.state.read().await;
        s.validator_sets.values().filter(|v| v.org_id == org_id).cloned().collect()
    }

    pub async fn list_packs(&self, org_id: Option<Uuid>) -> Vec<PackRecord> {
        let s = self.state.read().await;
        s.packs
            .values()
            .filter(|p| p.visibility == "public" || p.visibility == "preview" || (org_id.is_some() && p.org_id == org_id))
            .cloned()
            .collect()
    }

    pub async fn list_incidents(&self, org_id: Uuid) -> Vec<IncidentRecord> {
        let s = self.state.read().await;
        s.incidents.values().filter(|i| i.org_id == org_id).cloned().collect()
    }

    pub async fn list_audit_logs(&self, org_id: Uuid) -> Vec<AuditLogRecord> {
        let s = self.state.read().await;
        s.audit_logs.iter().filter(|a| a.org_id == org_id).cloned().collect()
    }

    pub async fn record_audit_log(&self, entry: AuditLogRecord) {
        let mut s = self.state.write().await;
        s.audit_logs.push(entry);
    }

    pub async fn get_usage(&self, org_id: Uuid) -> Option<UsageCounters> {
        let s = self.state.read().await;
        s.usage.get(&org_id).cloned()
    }
}
