use clap::Parser;
use tracing::info;
use uuid::Uuid;

use rivun_cloud_bridge::{BridgeConfig, CloudBridgeDaemon};

#[derive(Parser, Debug)]
#[command(name = "rivun-cloud-bridge", about = "Rivun Cloud Edge Bridge Daemon")]
struct CliArgs {
    /// Cloud API URL
    #[arg(short, long, default_value = "http://localhost:8080")]
    cloud_url: String,

    /// Organization slug or ID
    #[arg(short, long, default_value = "acme")]
    org: String,

    /// API bearer token
    #[arg(short, long, default_value = "rivun_live_secret_token_123456789")]
    token: String,

    /// Edge Node UUID
    #[arg(long, default_value = "00000000-0000-0000-0000-000000001000")]
    node_id: Uuid,

    /// Edge Node human label
    #[arg(long, default_value = "fra1-edge-01")]
    node_label: String,

    /// Active policy destination path
    #[arg(long, default_value = ".rivun/active_policy.toml")]
    active_policy_path: String,

    /// Heartbeat interval in seconds
    #[arg(long, default_value_t = 10)]
    heartbeat_interval_secs: u64,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let args = CliArgs::parse();

    let config = BridgeConfig {
        org_slug: args.org,
        node_id: args.node_id,
        label: Some(args.node_label),
        tags: vec!["env:local".to_string(), "role:edge-bridge".to_string()],
        cloud_url: args.cloud_url,
        api_token: args.token,
        authorized_operators: vec![],
        heartbeat_interval_secs: args.heartbeat_interval_secs,
        policy_pull_interval_secs: 5,
        local_policy_path: args.active_policy_path,
    };

    let (daemon, _tx) = CloudBridgeDaemon::new(config);
    let (_shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);

    info!("🛰️ Starting Rivun Cloud Edge Bridge Daemon...");
    daemon.run(shutdown_rx).await?;

    Ok(())
}
