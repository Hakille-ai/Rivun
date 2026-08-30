//! Rivun Cloud Bridge
//!
//! Lightweight zero-trust bridge daemon running on edge nodes beside `rivun-node`.
//! It pushes node telemetry (Fleet Doctor reports, Prometheus metrics) and receipt metadata
//! (hashes, action kinds, PoA status) to Rivun Cloud SaaS, and pulls cryptographically signed
//! policy bundles from authorized human operators without ever handling private keys.

pub mod client;
pub mod models;
pub mod policy;

pub use client::{BridgeClientError, CloudBridgeClient};
pub use models::{
    BRIDGE_VERSION, BridgeConfig, IncidentIngestPayload, IngestResponse, POLICY_BUNDLE_SIGNATURE_DOMAIN,
    PolicyBundle, ReceiptIngestBatch, ReceiptIngestItem, TelemetryIngestPayload,
};
pub use policy::{PolicyVerificationError, PolicyVerifier};

use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::time::{Duration, interval};
use tracing::{error, info, warn};
use rivun_telemetry::FleetDoctor;

/// Zero-trust edge bridge daemon.
pub struct CloudBridgeDaemon {
    client: CloudBridgeClient,
    config: BridgeConfig,
    receipt_rx: Option<mpsc::Receiver<ReceiptIngestItem>>,
}

impl CloudBridgeDaemon {
    pub fn new(config: BridgeConfig) -> (Self, mpsc::Sender<ReceiptIngestItem>) {
        let (tx, rx) = mpsc::channel(1024);
        let client = CloudBridgeClient::new(config.clone());
        let daemon = Self {
            client,
            config,
            receipt_rx: Some(rx),
        };
        (daemon, tx)
    }

    /// Spawns the background heartbeat, telemetry, receipt, and policy polling tasks.
    pub async fn run(mut self, shutdown_rx: tokio::sync::watch::Receiver<bool>) -> anyhow::Result<()> {
        info!(
            "Starting Rivun Cloud Bridge for node {} -> org {}",
            self.config.node_id, self.config.org_slug
        );

        let mut receipt_rx = self
            .receipt_rx
            .take()
            .expect("receipt_rx initialized in constructor");
        let client = Arc::new(self.client);
        let config = self.config.clone();

        // 1. Heartbeat & Telemetry task
        let client_telemetry = client.clone();
        let config_telemetry = config.clone();
        let mut shutdown_telemetry = shutdown_rx.clone();
        let telemetry_handle = tokio::spawn(async move {
            let mut ticker = interval(Duration::from_secs(config_telemetry.heartbeat_interval_secs));
            loop {
                tokio::select! {
                    _ = ticker.tick() => {
                        let report = FleetDoctor::evaluate(
                            config_telemetry.node_id,
                            None,
                            None,
                            None,
                            None,
                        );
                        let payload = TelemetryIngestPayload {
                            node_id: config_telemetry.node_id,
                            public_key: None,
                            label: config_telemetry.label.clone(),
                            tags: config_telemetry.tags.clone(),
                            bridge_version: BRIDGE_VERSION.to_string(),
                            timestamp_micros: rivun_core::now_micros().unwrap_or(0),
                            doctor_report: report,
                            metrics: serde_json::json!({
                                "bridge_active": true,
                                "heartbeat_seq": 1,
                            }),
                        };

                        if let Err(err) = client_telemetry.push_telemetry(&payload).await {
                            warn!("Failed to push telemetry to Rivun Cloud: {err}");
                        }
                    }
                    _ = shutdown_telemetry.changed() => {
                        if *shutdown_telemetry.borrow() {
                            break;
                        }
                    }
                }
            }
        });

        // 2. Receipt batching task
        let client_receipts = client.clone();
        let config_receipts = config.clone();
        let mut shutdown_receipts = shutdown_rx.clone();
        let receipts_handle = tokio::spawn(async move {
            let mut batch = Vec::new();
            let mut flush_ticker = interval(Duration::from_secs(5));
            loop {
                tokio::select! {
                    Some(item) = receipt_rx.recv() => {
                        batch.push(item);
                        if batch.len() >= 50 {
                            let payload = ReceiptIngestBatch {
                                node_id: config_receipts.node_id,
                                items: std::mem::take(&mut batch),
                                sent_at_micros: rivun_core::now_micros().unwrap_or(0),
                            };
                            if let Err(err) = client_receipts.push_receipts(&payload).await {
                                warn!("Failed to stream receipts batch: {err}");
                            }
                        }
                    }
                    _ = flush_ticker.tick() => {
                        if !batch.is_empty() {
                            let payload = ReceiptIngestBatch {
                                node_id: config_receipts.node_id,
                                items: std::mem::take(&mut batch),
                                sent_at_micros: rivun_core::now_micros().unwrap_or(0),
                            };
                            if let Err(err) = client_receipts.push_receipts(&payload).await {
                                warn!("Failed to flush receipts batch: {err}");
                            }
                        }
                    }
                    _ = shutdown_receipts.changed() => {
                        if *shutdown_receipts.borrow() {
                            break;
                        }
                    }
                }
            }
        });

        // 3. Policy pull & verify task
        let client_policy = client.clone();
        let config_policy = config.clone();
        let mut shutdown_policy = shutdown_rx.clone();
        let policy_handle = tokio::spawn(async move {
            let mut ticker = interval(Duration::from_secs(config_policy.policy_pull_interval_secs));
            loop {
                tokio::select! {
                    _ = ticker.tick() => {
                        match client_policy.pull_pending_policies().await {
                            Ok(bundles) => {
                                for bundle in bundles {
                                    let path = PathBuf::from(&config_policy.local_policy_path);
                                    match PolicyVerifier::apply_bundle_to_path(
                                        &bundle,
                                        &path,
                                        &config_policy.authorized_operators,
                                    ) {
                                        Ok(_policy_set) => {
                                            info!(
                                                "Successfully verified and applied signed policy bundle v{} (ID: {})",
                                                bundle.version, bundle.id
                                            );
                                            if let Err(e) = client_policy.acknowledge_policy(bundle.id, bundle.version).await {
                                                warn!("Failed to acknowledge policy bundle {}: {}", bundle.id, e);
                                            }
                                        }
                                        Err(e) => {
                                            error!(
                                                "Refusing to apply policy bundle {}: signature/verification failed: {}",
                                                bundle.id, e
                                            );
                                        }
                                    }
                                }
                            }
                            Err(err) => {
                                warn!("Failed to check pending policies: {err}");
                            }
                        }
                    }
                    _ = shutdown_policy.changed() => {
                        if *shutdown_policy.borrow() {
                            break;
                        }
                    }
                }
            }
        });

        let _ = tokio::join!(telemetry_handle, receipts_handle, policy_handle);
        info!("Rivun Cloud Bridge stopped gracefully");
        Ok(())
    }
}
