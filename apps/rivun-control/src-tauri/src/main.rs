use clap::{Parser, Subcommand};
use uuid::Uuid;
use rivun_control::{CloudSyncClient, KeyVault, OperatorSigner};

#[derive(Parser, Debug)]
#[command(name = "rivun-control", about = "Rivun Control — Operator Desktop Station & Key Vault")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Generate a new Ed25519 identity keypair and store in local vault
    Keygen {
        #[arg(short, long)]
        label: Option<String>,
    },
    /// List all local keys in the secure vault
    ListKeys,
    /// Fetch pending staged policies from Rivun Cloud
    Staged {
        #[arg(short, long, default_value = "http://localhost:8080")]
        cloud_url: String,
        #[arg(short, long, default_value = "acme")]
        org: String,
        #[arg(short, long, default_value = "rivun_live_secret_token_123456789")]
        token: String,
    },
    /// Inspect and cryptographically sign a staged policy locally
    Sign {
        #[arg(short, long, default_value = "http://localhost:8080")]
        cloud_url: String,
        #[arg(short, long, default_value = "acme")]
        org: String,
        #[arg(short, long, default_value = "rivun_live_secret_token_123456789")]
        token: String,
        #[arg(short, long)]
        policy_id: Uuid,
        #[arg(short, long)]
        key_node: Uuid,
    },
    /// Evaluate local node health diagnostic using FleetDoctor
    Doctor {
        #[arg(short, long)]
        config_path: Option<std::path::PathBuf>,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    let cli = Cli::parse();
    let vault = KeyVault::new(KeyVault::default_path())?;

    match cli.command {
        Commands::Keygen { label } => {
            let (node_id, pubkey) = vault.generate_and_save_key(label.as_deref())?;
            println!("🔑 Generated new Ed25519 identity keypair:");
            println!("   Node UUID:  {}", node_id);
            println!("   Public Key: {}", pubkey);
            println!("   Location:   {}", KeyVault::default_path().display());
            println!("   Invariant:  Private key securely stored in local vault; never sent to cloud.");
        }
        Commands::ListKeys => {
            let keys = vault.list_identities()?;
            println!("🔐 Stored Operator Keypairs ({} found):", keys.len());
            for (id, pubkey, path) in keys {
                println!("   • Node ID: {}", id);
                println!("     Pubkey:  {}", pubkey);
                println!("     File:    {}", path.display());
            }
        }
        Commands::Staged { cloud_url, org, token } => {
            let client = CloudSyncClient::new(&cloud_url, &org, &token);
            let staged = client.fetch_staged_policies().await?;
            println!("📋 Staged Policies on Rivun Cloud ({} found):", staged.len());
            for p in staged {
                println!("   • [{}] {} (v{})", p.status, p.name, p.version);
                println!("     ID: {}", p.id);
                println!("     Diff preview:\n{}", p.body_toml);
            }
        }
        Commands::Sign { cloud_url, org, token, policy_id, key_node } => {
            let client = CloudSyncClient::new(&cloud_url, &org, &token);
            let staged = client.fetch_staged_policies().await?;
            let target = staged
                .into_iter()
                .find(|p| p.id == policy_id)
                .ok_or_else(|| anyhow::anyhow!("Staged policy {} not found on cloud", policy_id))?;

            let keypair = vault.load_keypair(key_node)?;
            println!("🔏 Signing policy '{}' (v{}) with local key {}...", target.name, target.version, key_node);

            let (pubkey, sig) = OperatorSigner::sign_policy_bundle(
                &keypair,
                &org,
                &target.name,
                target.version,
                &target.body_toml,
            );

            client.submit_signature(policy_id, &pubkey, &sig).await?;
            println!("✅ Cryptographic Ed25519 signature submitted to Rivun Cloud!");
            println!("   Signed by Public Key: {}", pubkey);
            println!("   Signature:            {}", sig);
        }
        Commands::Doctor { config_path } => {
            let report = rivun_telemetry::FleetDoctor::evaluate(
                Uuid::nil(),
                config_path.as_deref(),
                None,
                None,
                None,
            );
            println!("{}", report.to_json()?);
        }
    }

    Ok(())
}
