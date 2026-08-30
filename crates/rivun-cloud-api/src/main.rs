use clap::Parser;
use std::net::SocketAddr;
use tracing::info;

use rivun_cloud_api::{build_app, AppState, CloudDatabase, EventBroker};

#[derive(Parser, Debug)]
#[command(name = "rivun-cloud-api", about = "Rivun Cloud Multi-Tenant SaaS Server")]
struct CliArgs {
    /// Bind address / host
    #[arg(short, long, alias = "host", default_value = "0.0.0.0")]
    bind: String,

    /// HTTP listening port
    #[arg(short, long, default_value_t = 8080)]
    port: u16,

    /// Seed demo data for local development
    #[arg(long, default_value_t = true)]
    seed: bool,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let args = CliArgs::parse();
    let db = CloudDatabase::new();

    if args.seed {
        info!("Seeding initial demo data for Rivun Cloud...");
        db.seed_demo_data().await;
    }

    let events = EventBroker::new(1024);
    let state = AppState { db, events };
    let app = build_app(state);

    let addr: SocketAddr = format!("{}:{}", args.bind, args.port).parse()?;
    info!("🚀 Rivun Cloud API running on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
