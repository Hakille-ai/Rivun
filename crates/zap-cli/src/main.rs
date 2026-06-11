use anyhow::{Context, Result, bail};
use base64::{Engine as _, engine::general_purpose::STANDARD_NO_PAD};
use bytes::Bytes;
use clap::{Parser, Subcommand};
use std::{
    fs::{self, OpenOptions},
    io::{ErrorKind, Write},
    net::SocketAddr,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};
use tracing_subscriber::{EnvFilter, fmt};
use uuid::Uuid;
use zap_core::{PoaAttestation, ZapFlags, ZapFrame, ZapHeader};
use zap_crypto::{
    Keypair, POA_ATTESTATION_CONTENT_TYPE, POA_ATTESTATION_REQUEST_SUBJECT,
    POA_ATTESTATION_RESPONSE_SUBJECT, PoaAttestationRequest, PoaAttestationResponse, PublicKey,
    certify_frame, certify_frame_with_attestations, poa_attestation_request, poa_frame_digest,
    sign_frame, sign_poa_attestation_request, verify_frame, verify_poa_attestation_response,
};
use zap_envelope::{
    DEFAULT_CONTENT_TYPE as DEFAULT_ENVELOPE_CONTENT_TYPE, ZapEnvelope, ZapEnvelopeRef,
    ZapMessageKind,
};
use zap_intent::{IntentPolicy, IntentStep, compile_intent, explain_intent};
use zap_ledger::SignedActionReceipt;
use zap_net::{Peer, TransportKey, ZapEndpoint, ZapEndpointConfig};
use zap_node::{ZapNode, ZapNodeConfig};
use zap_runtime::DriverPermissions;
use zap_store::{DriverManifest, DriverRegistry};

#[derive(Debug, Parser)]
#[command(
    name = "zap",
    version,
    about = "Universal low-latency ZAP protocol toolkit"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Generate an Ed25519 node identity file.
    Keygen {
        #[arg(long, default_value = ".zap/node.key")]
        out: PathBuf,
        /// Overwrite an existing key file.
        #[arg(long)]
        force: bool,
    },
    /// Run a ZAP node daemon from a TOML config file.
    Run {
        #[arg(long, default_value = "zap.toml")]
        config: PathBuf,
        /// Fail startup when config validation emits production-safety warnings.
        #[arg(long)]
        strict: bool,
    },
    /// Validate a node config without binding sockets or running the daemon.
    CheckConfig {
        #[arg(long, default_value = "zap.toml")]
        config: PathBuf,
        #[arg(long)]
        json: bool,
        /// Exit non-zero when config validation emits production-safety warnings.
        #[arg(long)]
        strict: bool,
    },
    /// Send one frame payload to a configured peer.
    Send {
        #[arg(long, default_value = "zap.toml")]
        config: PathBuf,
        #[arg(long)]
        target: Uuid,
        /// Compile a local natural-language or JSON intent into action envelope(s).
        #[arg(long)]
        intent: Option<String>,
        /// Apply a JSON intent policy before emitting frames.
        #[arg(long)]
        policy: Option<PathBuf>,
        /// Universal envelope kind: data, event, command, query, response, stream_chunk, action, or control.
        #[arg(long)]
        kind: Option<String>,
        /// Universal envelope subject, such as sensor.temperature or device.echo.
        #[arg(long)]
        subject: Option<String>,
        /// Universal envelope content type. Defaults to text/plain for --payload and application/octet-stream for --payload-file.
        #[arg(long)]
        content_type: Option<String>,
        /// Universal envelope metadata bytes supplied as inline text, JSON, or any shell-safe string.
        #[arg(long)]
        metadata: Option<String>,
        /// Driver action name. When omitted, payload bytes are sent raw.
        #[arg(long)]
        action: Option<String>,
        /// Payload text. Mutually exclusive with --payload-file.
        #[arg(long)]
        payload: Option<String>,
        /// Read payload bytes from a file.
        #[arg(long)]
        payload_file: Option<PathBuf>,
        /// Treat selected payload bytes as opaque and default content type to application/octet-stream.
        #[arg(long)]
        binary_payload: bool,
        /// Validator key used to attach a Proof-of-Action certificate for critical frames.
        #[arg(long = "poa-validator-key")]
        poa_validator_keys: Vec<PathBuf>,
        /// Required Proof-of-Action threshold. Defaults to the number of validator keys.
        #[arg(long)]
        poa_threshold: Option<u16>,
        /// Request Proof-of-Action attestations from configured PoA validator peers.
        #[arg(long)]
        poa_network: bool,
        /// Maximum time to wait for network Proof-of-Action validator responses.
        #[arg(long, default_value_t = 2000, value_parser = clap::value_parser!(u64).range(1..))]
        poa_timeout_ms: u64,
        #[arg(long)]
        unsigned: bool,
    },
    /// Inspect an encoded ZAP frame file.
    Inspect {
        frame: PathBuf,
        #[arg(long)]
        verify_with_key: Option<PathBuf>,
        #[arg(long, conflicts_with = "verify_with_key")]
        verify_with_public_key: Option<String>,
    },
    /// Compile a local intent into an auditable action plan without sending it.
    CompileIntent {
        intent: String,
        /// Include normalized input and rule metadata.
        #[arg(long)]
        explain: bool,
        /// Apply a JSON intent policy to the compiled plan.
        #[arg(long)]
        policy: Option<PathBuf>,
    },
    /// Create or verify signed ZapStore driver manifests.
    #[command(name = "driver-manifest")]
    DriverManifest {
        #[command(subcommand)]
        command: DriverManifestCommand,
    },
    /// Manage local ZapStore registry index files.
    Registry {
        #[command(subcommand)]
        command: RegistryCommand,
    },
    /// Verify signed receipt logs.
    Receipts {
        #[command(subcommand)]
        command: ReceiptsCommand,
    },
    /// Create or sign Proof-of-Action attestation messages.
    Poa {
        #[command(subcommand)]
        command: PoaCommand,
    },
    /// Run local protocol benchmarks.
    Bench {
        #[command(subcommand)]
        command: BenchCommand,
    },
}

#[derive(Debug, Subcommand)]
enum DriverManifestCommand {
    /// Create a signed manifest for one WASM/WAT driver file.
    Create {
        #[arg(long)]
        driver: PathBuf,
        #[arg(long)]
        action: String,
        #[arg(long)]
        author_key: PathBuf,
        #[arg(long)]
        out: PathBuf,
        #[arg(long)]
        name: Option<String>,
        #[arg(long, default_value = "0.1.0")]
        version: String,
        #[arg(long)]
        description: Option<String>,
        #[arg(long)]
        allow_network: bool,
        #[arg(long)]
        allow_filesystem: bool,
        #[arg(long)]
        allow_clock: bool,
        #[arg(long)]
        allow_environment: bool,
        /// Overwrite an existing manifest file.
        #[arg(long)]
        force: bool,
    },
    /// Verify a signed manifest against a WASM/WAT driver file.
    Verify {
        #[arg(long)]
        driver: PathBuf,
        #[arg(long)]
        manifest: PathBuf,
        #[arg(long)]
        action: Option<String>,
    },
}

#[derive(Debug, Subcommand)]
enum RegistryCommand {
    /// Create an empty local ZapStore registry index.
    Init {
        #[arg(long, default_value = "registry.index.toml")]
        out: PathBuf,
        /// Overwrite an existing registry index.
        #[arg(long)]
        force: bool,
    },
    /// Add or replace one signed driver manifest entry in a registry index.
    Add {
        #[arg(long, default_value = "registry.index.toml")]
        registry: PathBuf,
        #[arg(long)]
        manifest: PathBuf,
        /// Path to record in the registry entry. Defaults to --manifest.
        #[arg(long)]
        manifest_path: Option<String>,
        /// Write to a different registry index path.
        #[arg(long)]
        out: Option<PathBuf>,
    },
    /// Verify one signed manifest is active in a registry index.
    Verify {
        #[arg(long, default_value = "registry.index.toml")]
        registry: PathBuf,
        #[arg(long)]
        manifest: PathBuf,
    },
    /// Sign a registry index with an operator key.
    Sign {
        #[arg(long, default_value = "registry.index.toml")]
        registry: PathBuf,
        #[arg(long)]
        operator_key: PathBuf,
        /// Write to a different registry index path.
        #[arg(long)]
        out: Option<PathBuf>,
    },
    /// Verify the operator signature on a registry index.
    VerifySignature {
        #[arg(long, default_value = "registry.index.toml")]
        registry: PathBuf,
    },
    /// Revoke one manifest version in a registry index.
    Revoke {
        #[arg(long, default_value = "registry.index.toml")]
        registry: PathBuf,
        #[arg(long)]
        action: String,
        #[arg(long)]
        version: String,
        #[arg(long)]
        reason: Option<String>,
        /// Write to a different registry index path.
        #[arg(long)]
        out: Option<PathBuf>,
    },
    /// List registry entries.
    List {
        #[arg(long, default_value = "registry.index.toml")]
        registry: PathBuf,
        #[arg(long)]
        json: bool,
    },
}

#[derive(Debug, Subcommand)]
enum ReceiptsCommand {
    /// Verify every signed JSONL receipt in a log file.
    Verify {
        #[arg(long)]
        path: PathBuf,
        #[arg(long)]
        json: bool,
    },
    /// Write a verified receipt log without records older than a processing timestamp.
    Prune {
        #[arg(long)]
        path: PathBuf,
        /// Drop receipts whose processed_at_micros is lower than this value.
        #[arg(long)]
        before_processed_at_micros: u64,
        #[arg(long)]
        out: PathBuf,
        /// Overwrite the output file if it already exists.
        #[arg(long)]
        force: bool,
        #[arg(long)]
        json: bool,
    },
    /// Merge verified receipt logs into one deduplicated archive.
    Merge {
        /// Input receipt JSONL logs to merge.
        #[arg(required = true)]
        inputs: Vec<PathBuf>,
        #[arg(long)]
        out: PathBuf,
        /// Overwrite the output file if it already exists.
        #[arg(long)]
        force: bool,
        #[arg(long)]
        json: bool,
    },
}

#[derive(Debug, Subcommand)]
enum PoaCommand {
    /// Create a JSON PoA attestation request from a signed consensus frame.
    Request {
        #[arg(long)]
        frame: PathBuf,
        #[arg(long)]
        requester_key: PathBuf,
        #[arg(long)]
        threshold: u16,
    },
    /// Sign a JSON PoA attestation request with a validator key.
    Attest {
        #[arg(long)]
        request: PathBuf,
        #[arg(long)]
        validator_key: PathBuf,
    },
}

#[derive(Debug, Subcommand)]
enum BenchCommand {
    /// Benchmark 64-byte header parsing.
    Parse {
        #[arg(long, default_value_t = 1_000_000)]
        iterations: u64,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    match Cli::parse().command {
        Commands::Keygen { out, force } => keygen(&out, force),
        Commands::Run { config, strict } => run(&config, strict).await,
        Commands::CheckConfig {
            config,
            json,
            strict,
        } => check_config(&config, json, strict),
        Commands::Send {
            config,
            target,
            intent,
            policy,
            kind,
            subject,
            content_type,
            metadata,
            action,
            payload,
            payload_file,
            binary_payload,
            poa_validator_keys,
            poa_threshold,
            poa_network,
            poa_timeout_ms,
            unsigned,
        } => {
            send(SendOptions {
                config_path: &config,
                target,
                intent,
                policy,
                kind,
                subject,
                content_type,
                metadata,
                action,
                payload,
                payload_file,
                binary_payload,
                poa_validator_keys,
                poa_threshold,
                poa_network,
                poa_timeout_ms,
                unsigned,
            })
            .await
        }
        Commands::Inspect {
            frame,
            verify_with_key,
            verify_with_public_key,
        } => inspect(
            &frame,
            verify_with_key.as_deref(),
            verify_with_public_key.as_deref(),
        ),
        Commands::CompileIntent {
            intent,
            explain,
            policy,
        } => compile_intent_command(&intent, explain, policy.as_deref()),
        Commands::DriverManifest { command } => driver_manifest(command),
        Commands::Registry { command } => registry(command),
        Commands::Receipts { command } => receipts(command),
        Commands::Poa { command } => poa(command),
        Commands::Bench { command } => bench(command),
    }
}

fn keygen(out: &Path, force: bool) -> Result<()> {
    if out.exists() && !force {
        bail!(
            "refusing to overwrite existing key file {}; pass --force to replace it",
            out.display()
        );
    }
    if let Some(parent) = out.parent().filter(|parent| !parent.as_os_str().is_empty()) {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create key directory {}", parent.display()))?;
    }
    let keypair = Keypair::generate();
    write_key_file(out, &keypair.to_key_file_toml()?, force)?;
    println!("node_id={}", keypair.node_id());
    println!("public_key={}", keypair.to_key_file().public_key);
    println!("wrote {}", out.display());
    Ok(())
}

fn write_key_file(out: &Path, contents: &str, force: bool) -> Result<()> {
    let mut options = OpenOptions::new();
    options.write(true);
    if force {
        options.create(true).truncate(true);
    } else {
        options.create_new(true);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }

    let mut file = match options.open(out) {
        Ok(file) => file,
        Err(error) if !force && error.kind() == ErrorKind::AlreadyExists => {
            bail!(
                "refusing to overwrite existing key file {}; pass --force to replace it",
                out.display()
            );
        }
        Err(error) => {
            return Err(error)
                .with_context(|| format!("failed to write key file {}", out.display()));
        }
    };
    file.write_all(contents.as_bytes())
        .with_context(|| format!("failed to write key file {}", out.display()))?;
    file.sync_all()
        .with_context(|| format!("failed to flush key file {}", out.display()))?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(out, fs::Permissions::from_mode(0o600))
            .with_context(|| format!("failed to lock down key file {}", out.display()))?;
    }

    Ok(())
}

async fn run(config_path: &Path, strict: bool) -> Result<()> {
    let config = ZapNodeConfig::from_path(config_path)?;
    if strict {
        let report = config.validate()?;
        fail_on_config_warnings(config_path, &report)?;
    }
    let node = ZapNode::from_config(config).await?;
    node.run_forever().await
}

fn check_config(config_path: &Path, json: bool, strict: bool) -> Result<()> {
    let config = ZapNodeConfig::from_path(config_path)?;
    let report = config.validate()?;
    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        println!("config={} ok", config_path.display());
        println!("bind={}", report.bind);
        println!("node_id={}", report.node_id);
        println!("peers={}", report.peer_count);
        println!("drivers={}", report.driver_count);
        println!("signed_drivers={}", report.signed_driver_count);
        println!("registry_enabled={}", report.registry_enabled);
        println!("registry_entries={}", report.registry_entry_count);
        println!(
            "registry_signature_required={}",
            report.registry_signature_required
        );
        println!("receipt_log_enabled={}", report.receipt_log_enabled);
        println!("require_signed={}", report.require_signed);
        for warning in &report.warnings {
            println!("warning={warning}");
        }
    }
    if strict {
        fail_on_config_warnings(config_path, &report)?;
    }
    Ok(())
}

fn fail_on_config_warnings(
    config_path: &Path,
    report: &zap_node::ConfigValidationReport,
) -> Result<()> {
    if report.warnings.is_empty() {
        return Ok(());
    }
    bail!(
        "strict config validation failed for {}: {}",
        config_path.display(),
        report.warnings.join("; ")
    );
}

struct SendOptions<'a> {
    config_path: &'a Path,
    target: Uuid,
    intent: Option<String>,
    policy: Option<PathBuf>,
    kind: Option<String>,
    subject: Option<String>,
    content_type: Option<String>,
    metadata: Option<String>,
    action: Option<String>,
    payload: Option<String>,
    payload_file: Option<PathBuf>,
    binary_payload: bool,
    poa_validator_keys: Vec<PathBuf>,
    poa_threshold: Option<u16>,
    poa_network: bool,
    poa_timeout_ms: u64,
    unsigned: bool,
}

async fn send(options: SendOptions<'_>) -> Result<()> {
    let SendOptions {
        config_path,
        target,
        intent,
        policy,
        kind,
        subject,
        content_type,
        metadata,
        action,
        payload,
        payload_file,
        binary_payload,
        poa_validator_keys,
        poa_threshold,
        poa_network,
        poa_timeout_ms,
        unsigned,
    } = options;
    let config = ZapNodeConfig::from_path(config_path)?;
    config.validate()?;
    let messages = build_messages(BuildMessageOptions {
        intent,
        policy,
        kind,
        subject,
        content_type,
        metadata,
        action,
        payload,
        payload_file,
        binary_payload,
    })?;
    let keypair = Keypair::from_key_file_toml(
        &fs::read_to_string(&config.key_file)
            .with_context(|| format!("failed to read key file {}", config.key_file.display()))?,
    )?;
    if !config.peers.iter().any(|peer| peer.node_id == target) {
        bail!("target {} not found in {}", target, config_path.display());
    }
    let bind: SocketAddr = config
        .bind
        .parse()
        .with_context(|| format!("invalid bind address {}", config.bind))?;
    let endpoint = ZapEndpoint::bind(ZapEndpointConfig::new(bind, keypair.node_id())).await?;
    for peer in &config.peers {
        endpoint
            .add_peer(Peer {
                node_id: peer.node_id,
                addr: peer.addr.parse()?,
                transport_key: TransportKey::from_hex(&peer.transport_key)?,
            })
            .await;
    }
    let poa_validators = load_keypairs(&poa_validator_keys)?;
    let poa_threshold = poa_threshold.unwrap_or({
        if poa_validators.is_empty() && poa_network {
            config.poa.required_threshold
        } else {
            poa_validators.len() as u16
        }
    });
    let requires_poa = messages.iter().any(|message| message.requires_consensus);
    if requires_poa {
        if unsigned {
            bail!("frames requiring consensus must be signed");
        }
        if poa_validators.is_empty() && !poa_network {
            bail!(
                "at least one --poa-validator-key or --poa-network is required for actions requiring consensus"
            );
        }
        if poa_threshold == 0 {
            bail!("--poa-threshold must be greater than zero");
        }
        if poa_network && poa_validators.is_empty() && config.poa.validators.is_empty() {
            bail!("--poa-network requires configured [poa] validators");
        }
    }

    for message in messages {
        let mut flags = ZapFlags::ENCRYPTED;
        if message.requires_consensus {
            flags |= ZapFlags::REQUIRES_CONSENSUS;
        }
        let frame = ZapFrame::new(
            keypair.node_id(),
            target,
            flags,
            Bytes::from(message.payload),
        )?;
        let mut frame = if unsigned {
            frame
        } else {
            sign_frame(&keypair, &frame)?
        };
        if message.requires_consensus {
            frame = if poa_validators.is_empty() && poa_network {
                certify_frame_with_network_poa(
                    &config,
                    &endpoint,
                    &keypair,
                    &frame,
                    poa_threshold,
                    Duration::from_millis(poa_timeout_ms),
                )
                .await?
            } else {
                certify_frame(&frame, poa_threshold, &poa_validators)?
            };
        }

        endpoint.send_frame(target, &frame).await?;
        match message.display {
            OutboundDisplay::Frame => println!("sent frame to {}", target),
            OutboundDisplay::Action(action) => println!("sent action {} to {}", action, target),
            OutboundDisplay::Envelope { kind, subject } => {
                println!("sent {} {} to {}", kind, subject, target);
            }
        }
    }
    Ok(())
}

fn load_keypairs(paths: &[PathBuf]) -> Result<Vec<Keypair>> {
    paths
        .iter()
        .map(|path| {
            Keypair::from_key_file_toml(
                &fs::read_to_string(path)
                    .with_context(|| format!("failed to read key file {}", path.display()))?,
            )
            .with_context(|| format!("failed to parse key file {}", path.display()))
        })
        .collect()
}

fn poa(command: PoaCommand) -> Result<()> {
    match command {
        PoaCommand::Request {
            frame,
            requester_key,
            threshold,
        } => create_poa_request(&frame, &requester_key, threshold),
        PoaCommand::Attest {
            request,
            validator_key,
        } => sign_poa_request(&request, &validator_key),
    }
}

fn create_poa_request(frame_path: &Path, requester_key_path: &Path, threshold: u16) -> Result<()> {
    let frame = ZapFrame::decode(
        &fs::read(frame_path)
            .with_context(|| format!("failed to read frame {}", frame_path.display()))?,
    )?;
    let requester = Keypair::from_key_file_toml(
        &fs::read_to_string(requester_key_path).with_context(|| {
            format!(
                "failed to read requester key file {}",
                requester_key_path.display()
            )
        })?,
    )?;
    let request = poa_attestation_request(&frame, requester.node_id(), threshold)?;
    println!("{}", serde_json::to_string_pretty(&request)?);
    Ok(())
}

fn sign_poa_request(request_path: &Path, validator_key_path: &Path) -> Result<()> {
    let request: PoaAttestationRequest = serde_json::from_str(
        &fs::read_to_string(request_path)
            .with_context(|| format!("failed to read PoA request {}", request_path.display()))?,
    )
    .with_context(|| format!("failed to parse PoA request {}", request_path.display()))?;
    let validator = Keypair::from_key_file_toml(
        &fs::read_to_string(validator_key_path).with_context(|| {
            format!(
                "failed to read validator key file {}",
                validator_key_path.display()
            )
        })?,
    )?;
    let response = sign_poa_attestation_request(&validator, &request)?;
    println!("{}", serde_json::to_string_pretty(&response)?);
    Ok(())
}

fn receipts(command: ReceiptsCommand) -> Result<()> {
    match command {
        ReceiptsCommand::Verify { path, json } => verify_receipts(&path, json),
        ReceiptsCommand::Prune {
            path,
            before_processed_at_micros,
            out,
            force,
            json,
        } => prune_receipts(&path, before_processed_at_micros, &out, force, json),
        ReceiptsCommand::Merge {
            inputs,
            out,
            force,
            json,
        } => merge_receipts(&inputs, &out, force, json),
    }
}

fn verify_receipts(path: &Path, json: bool) -> Result<()> {
    let verified = load_verified_receipts(path)?.len();
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "path": path.display().to_string(),
                "receipts": verified,
                "verified": true
            }))?
        );
    } else {
        println!("receipts={verified}");
        println!("verified=true");
        println!("path={}", path.display());
    }
    Ok(())
}

fn prune_receipts(
    path: &Path,
    before_processed_at_micros: u64,
    out: &Path,
    force: bool,
    json: bool,
) -> Result<()> {
    ensure_receipt_output_is_separate(out, &[path])?;
    let receipts = load_verified_receipts(path)?;
    let before = before_processed_at_micros;
    let retained = receipts
        .iter()
        .filter(|receipt| receipt.receipt.processed_at_micros >= before)
        .collect::<Vec<_>>();
    let mut output = String::new();
    for receipt in &retained {
        output.push_str(&receipt.to_json_line()?);
    }
    write_text_file(out, &output, force)?;

    let input_count = receipts.len();
    let retained_count = retained.len();
    let pruned_count = input_count - retained_count;
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "path": path.display().to_string(),
                "out": out.display().to_string(),
                "before_processed_at_micros": before,
                "input_receipts": input_count,
                "retained_receipts": retained_count,
                "pruned_receipts": pruned_count,
                "verified": true
            }))?
        );
    } else {
        println!("path={}", path.display());
        println!("out={}", out.display());
        println!("before_processed_at_micros={before}");
        println!("input_receipts={input_count}");
        println!("retained_receipts={retained_count}");
        println!("pruned_receipts={pruned_count}");
        println!("verified=true");
    }
    Ok(())
}

fn merge_receipts(inputs: &[PathBuf], out: &Path, force: bool, json: bool) -> Result<()> {
    ensure_receipt_output_is_separate(out, inputs)?;
    let mut seen = std::collections::HashSet::new();
    let mut merged = Vec::new();
    let mut input_count = 0_usize;
    for input in inputs {
        let receipts = load_verified_receipts(input)?;
        input_count += receipts.len();
        for receipt in receipts {
            if seen.insert(receipt.signature.clone()) {
                merged.push(receipt);
            }
        }
    }

    let mut output = String::new();
    for receipt in &merged {
        output.push_str(&receipt.to_json_line()?);
    }
    write_text_file(out, &output, force)?;

    let written_count = merged.len();
    let duplicate_count = input_count - written_count;
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "inputs": inputs.iter().map(|path| path.display().to_string()).collect::<Vec<_>>(),
                "out": out.display().to_string(),
                "input_logs": inputs.len(),
                "input_receipts": input_count,
                "written_receipts": written_count,
                "duplicate_receipts": duplicate_count,
                "verified": true
            }))?
        );
    } else {
        println!("out={}", out.display());
        println!("input_logs={}", inputs.len());
        println!("input_receipts={input_count}");
        println!("written_receipts={written_count}");
        println!("duplicate_receipts={duplicate_count}");
        println!("verified=true");
    }
    Ok(())
}

fn load_verified_receipts(path: &Path) -> Result<Vec<SignedActionReceipt>> {
    let input = fs::read_to_string(path)
        .with_context(|| format!("failed to read receipt log {}", path.display()))?;
    let mut receipts = Vec::new();
    for (index, line) in input.lines().enumerate() {
        let line_number = index + 1;
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let receipt = SignedActionReceipt::from_json_str(line).with_context(|| {
            format!(
                "failed to parse receipt at {} line {}",
                path.display(),
                line_number
            )
        })?;
        receipt.verify().with_context(|| {
            format!(
                "invalid receipt signature at {} line {}",
                path.display(),
                line_number
            )
        })?;
        receipts.push(receipt);
    }
    if receipts.is_empty() {
        bail!("receipt log {} contains no receipts", path.display());
    }
    Ok(receipts)
}

fn ensure_receipt_output_is_separate(out: &Path, inputs: &[impl AsRef<Path>]) -> Result<()> {
    let out = normalize_path_for_comparison(out)?;
    for input in inputs {
        let input = normalize_path_for_comparison(input.as_ref())?;
        if out == input {
            bail!("receipt output must not point at an input receipt log");
        }
    }
    Ok(())
}

fn normalize_path_for_comparison(path: &Path) -> Result<PathBuf> {
    let path = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()?.join(path)
    };
    Ok(path.components().collect())
}

async fn certify_frame_with_network_poa(
    config: &ZapNodeConfig,
    endpoint: &ZapEndpoint,
    keypair: &Keypair,
    frame: &ZapFrame,
    threshold: u16,
    timeout_budget: Duration,
) -> Result<ZapFrame> {
    if timeout_budget.is_zero() {
        bail!("--poa-timeout-ms must be greater than zero");
    }
    if config.poa.validators.len() < threshold as usize {
        bail!(
            "configured PoA validator count {} is below threshold {}",
            config.poa.validators.len(),
            threshold
        );
    }

    let request = poa_attestation_request(frame, keypair.node_id(), threshold)?;
    let request_body = serde_json::to_vec(&request)?;
    let validator_keys = config
        .poa
        .validators
        .iter()
        .map(|validator| {
            Ok((
                validator.node_id,
                decode_public_key(&validator.public_key).with_context(|| {
                    format!("invalid PoA validator public key {}", validator.node_id)
                })?,
            ))
        })
        .collect::<Result<Vec<_>>>()?;

    for (validator_node, _) in &validator_keys {
        if !config
            .peers
            .iter()
            .any(|peer| peer.node_id == *validator_node)
        {
            bail!(
                "PoA validator {} is not configured as a peer",
                validator_node
            );
        }
        let envelope = ZapEnvelope::new(
            ZapMessageKind::Control,
            POA_ATTESTATION_REQUEST_SUBJECT,
            POA_ATTESTATION_CONTENT_TYPE,
            Bytes::from(request_body.clone()),
        )?;
        let request_frame = ZapFrame::new(
            keypair.node_id(),
            *validator_node,
            ZapFlags::ENCRYPTED,
            envelope.encode(),
        )?;
        let request_frame = sign_frame(keypair, &request_frame)?;
        endpoint.send_frame(*validator_node, &request_frame).await?;
    }

    let expected_digest = poa_frame_digest(frame);
    let mut attestations = Vec::with_capacity(threshold as usize);
    let started = Instant::now();
    while attestations.len() < threshold as usize && started.elapsed() < timeout_budget {
        let remaining = timeout_budget.saturating_sub(started.elapsed());
        let inbound = match tokio::time::timeout(remaining, endpoint.recv()).await {
            Ok(Ok(inbound)) => inbound,
            Ok(Err(error)) => return Err(error.into()),
            Err(_) => break,
        };
        let Some((_, public_key)) = validator_keys
            .iter()
            .find(|(node_id, _)| *node_id == inbound.peer.node_id)
        else {
            continue;
        };
        verify_frame(public_key, &inbound.frame)?;
        let envelope = match ZapEnvelopeRef::parse(&inbound.frame.payload) {
            Ok(envelope) => envelope,
            Err(_) => continue,
        };
        if envelope.kind() != ZapMessageKind::Control
            || envelope.subject() != POA_ATTESTATION_RESPONSE_SUBJECT
        {
            continue;
        }
        let response: PoaAttestationResponse = serde_json::from_slice(envelope.body())?;
        let attestation = verify_poa_attestation_response(&response, public_key, &expected_digest)?;
        if attestations
            .iter()
            .all(|seen: &PoaAttestation| seen.validator_node != attestation.validator_node)
        {
            attestations.push(attestation);
        }
    }

    if attestations.len() < threshold as usize {
        bail!(
            "PoA network threshold not met: required {}, got {}",
            threshold,
            attestations.len()
        );
    }
    certify_frame_with_attestations(frame, threshold, attestations).map_err(Into::into)
}

fn decode_public_key(encoded: &str) -> Result<PublicKey> {
    let bytes = STANDARD_NO_PAD.decode(encoded)?;
    if bytes.len() != 32 {
        bail!(
            "invalid public key length: expected 32 bytes, got {}",
            bytes.len()
        );
    }
    Ok(PublicKey::from_bytes(bytes.try_into().unwrap())?)
}

#[derive(Debug)]
struct OutboundMessage {
    display: OutboundDisplay,
    payload: Vec<u8>,
    requires_consensus: bool,
}

#[derive(Debug)]
enum OutboundDisplay {
    Frame,
    Action(String),
    Envelope {
        kind: ZapMessageKind,
        subject: String,
    },
}

struct BuildMessageOptions {
    intent: Option<String>,
    policy: Option<PathBuf>,
    kind: Option<String>,
    subject: Option<String>,
    content_type: Option<String>,
    metadata: Option<String>,
    action: Option<String>,
    payload: Option<String>,
    payload_file: Option<PathBuf>,
    binary_payload: bool,
}

fn build_messages(options: BuildMessageOptions) -> Result<Vec<OutboundMessage>> {
    let BuildMessageOptions {
        intent,
        policy,
        kind,
        subject,
        content_type,
        metadata,
        action,
        payload,
        payload_file,
        binary_payload,
    } = options;

    if let Some(intent) = intent {
        if kind.is_some()
            || subject.is_some()
            || content_type.is_some()
            || metadata.is_some()
            || action.is_some()
            || payload.is_some()
            || payload_file.is_some()
            || binary_payload
        {
            bail!(
                "--intent cannot be combined with --kind, --subject, --content-type, --metadata, --action, --payload, --payload-file, or --binary-payload"
            );
        }
        let mut plan = compile_intent(&intent)?;
        if let Some(policy) = policy {
            let policy = load_intent_policy(&policy)?;
            plan.apply_policy(&policy)?;
        }
        return plan
            .steps
            .iter()
            .map(message_from_intent_step)
            .collect::<Result<Vec<_>>>();
    }

    if policy.is_some() {
        bail!("--policy requires --intent");
    }

    if let Some(kind) = kind {
        if action.is_some() {
            bail!("--kind cannot be combined with --action");
        }
        let Some(subject) = subject else {
            bail!("--subject is required when --kind is provided");
        };
        if subject.trim().is_empty() {
            bail!("--subject must not be empty");
        }
        let kind: ZapMessageKind = kind.parse()?;
        let payload_from_file = payload_file.is_some();
        let payload_bytes = payload_bytes_from_input(payload, payload_file)?;
        let content_type = content_type.unwrap_or_else(|| {
            if payload_from_file || binary_payload {
                DEFAULT_ENVELOPE_CONTENT_TYPE.to_string()
            } else {
                "text/plain".to_string()
            }
        });
        let mut envelope = ZapEnvelope::new(kind, subject.clone(), content_type, payload_bytes)?;
        if let Some(metadata) = metadata {
            envelope = envelope.with_metadata(metadata.into_bytes())?;
        }
        return Ok(vec![OutboundMessage {
            display: OutboundDisplay::Envelope { kind, subject },
            payload: envelope.encode().to_vec(),
            requires_consensus: false,
        }]);
    }

    if subject.is_some() || content_type.is_some() || metadata.is_some() {
        bail!("--subject, --content-type, and --metadata require --kind");
    }

    Ok(vec![OutboundMessage {
        display: action
            .as_ref()
            .map(|action| OutboundDisplay::Action(action.clone()))
            .unwrap_or(OutboundDisplay::Frame),
        payload: build_payload(action, payload, payload_file, binary_payload)?,
        requires_consensus: false,
    }])
}

fn message_from_intent_step(step: &IntentStep) -> Result<OutboundMessage> {
    let kind: ZapMessageKind = step
        .kind
        .parse()
        .with_context(|| format!("unsupported intent message kind `{}`", step.kind))?;
    let envelope = ZapEnvelope::new(
        kind,
        step.subject.clone(),
        step.content_type.clone(),
        Bytes::from(step.payload.clone()),
    )?;
    Ok(OutboundMessage {
        display: OutboundDisplay::Envelope {
            kind,
            subject: step.subject.clone(),
        },
        payload: envelope.encode().to_vec(),
        requires_consensus: step.requires_consensus,
    })
}

fn payload_bytes_from_input(
    payload: Option<String>,
    payload_file: Option<PathBuf>,
) -> Result<Vec<u8>> {
    match (payload, payload_file) {
        (Some(_), Some(_)) => bail!("use either --payload or --payload-file, not both"),
        (Some(payload), None) => Ok(payload.into_bytes()),
        (None, Some(path)) => fs::read(&path)
            .with_context(|| format!("failed to read payload file {}", path.display())),
        (None, None) => bail!("missing payload: provide --payload or --payload-file"),
    }
}

fn build_payload(
    action: Option<String>,
    payload: Option<String>,
    payload_file: Option<PathBuf>,
    binary_payload: bool,
) -> Result<Vec<u8>> {
    let payload_from_file = payload_file.is_some();
    let payload_bytes = payload_bytes_from_input(payload, payload_file)?;

    let Some(action) = action else {
        if binary_payload {
            bail!("--binary-payload requires --action");
        }
        return Ok(payload_bytes);
    };
    if action.trim().is_empty() {
        bail!("--action must not be empty");
    }

    let content_type = if payload_from_file || binary_payload {
        DEFAULT_ENVELOPE_CONTENT_TYPE
    } else {
        "text/plain"
    };
    let envelope = ZapEnvelope::new(
        ZapMessageKind::Action,
        action,
        content_type,
        Bytes::from(payload_bytes),
    )?;
    Ok(envelope.encode().to_vec())
}

fn compile_intent_command(intent: &str, explain: bool, policy_path: Option<&Path>) -> Result<()> {
    let mut plan = compile_intent(intent)?;
    let policy_report = match policy_path {
        Some(path) => {
            let policy = load_intent_policy(path)?;
            Some(plan.apply_policy(&policy)?)
        }
        None => None,
    };

    if explain {
        let mut explanation = explain_intent(intent)?;
        explanation.plan = plan;
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "explanation": explanation,
                "policy": policy_report,
            }))?
        );
    } else if let Some(policy_report) = policy_report {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "plan": plan,
                "policy": policy_report,
            }))?
        );
    } else {
        println!("{}", serde_json::to_string_pretty(&plan)?);
    }
    Ok(())
}

fn load_intent_policy(path: &Path) -> Result<IntentPolicy> {
    let input = fs::read_to_string(path)
        .with_context(|| format!("failed to read intent policy {}", path.display()))?;
    serde_json::from_str(&input)
        .with_context(|| format!("failed to parse intent policy {}", path.display()))
}

fn driver_manifest(command: DriverManifestCommand) -> Result<()> {
    match command {
        DriverManifestCommand::Create {
            driver,
            action,
            author_key,
            out,
            name,
            version,
            description,
            allow_network,
            allow_filesystem,
            allow_clock,
            allow_environment,
            force,
        } => create_driver_manifest(CreateDriverManifestOptions {
            driver_path: &driver,
            action: &action,
            author_key_path: &author_key,
            out: &out,
            name,
            version: &version,
            description,
            permissions: DriverPermissions {
                network: allow_network,
                filesystem: allow_filesystem,
                clock: allow_clock,
                environment: allow_environment,
            },
            force,
        }),
        DriverManifestCommand::Verify {
            driver,
            manifest,
            action,
        } => verify_driver_manifest(&driver, &manifest, action),
    }
}

struct CreateDriverManifestOptions<'a> {
    driver_path: &'a Path,
    action: &'a str,
    author_key_path: &'a Path,
    out: &'a Path,
    name: Option<String>,
    version: &'a str,
    description: Option<String>,
    permissions: DriverPermissions,
    force: bool,
}

fn create_driver_manifest(options: CreateDriverManifestOptions<'_>) -> Result<()> {
    let CreateDriverManifestOptions {
        driver_path,
        action,
        author_key_path,
        out,
        name,
        version,
        description,
        permissions,
        force,
    } = options;
    if out.exists() && !force {
        bail!(
            "refusing to overwrite existing manifest file {}; pass --force to replace it",
            out.display()
        );
    }
    if let Some(parent) = out.parent().filter(|parent| !parent.as_os_str().is_empty()) {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create manifest directory {}", parent.display()))?;
    }
    let wasm = fs::read(driver_path)
        .with_context(|| format!("failed to read driver {}", driver_path.display()))?;
    let author = Keypair::from_key_file_toml(
        &fs::read_to_string(author_key_path)
            .with_context(|| format!("failed to read key file {}", author_key_path.display()))?,
    )?;
    let manifest = DriverManifest::new(
        name.unwrap_or_else(|| format!("{action}-driver")),
        version,
        action,
        &wasm,
        permissions,
        description,
        &author,
    )?;
    fs::write(out, manifest.to_toml_string()?)
        .with_context(|| format!("failed to write manifest {}", out.display()))?;
    println!("manifest={}", out.display());
    println!("action={}", manifest.action);
    println!("wasm_hash={}", manifest.wasm_hash);
    println!("author_node_id={}", manifest.author_node_id);
    Ok(())
}

fn verify_driver_manifest(
    driver_path: &Path,
    manifest_path: &Path,
    action: Option<String>,
) -> Result<()> {
    let wasm = fs::read(driver_path)
        .with_context(|| format!("failed to read driver {}", driver_path.display()))?;
    let manifest = load_driver_manifest(manifest_path)?;
    let configured_action = action.unwrap_or_else(|| manifest.action.clone());
    manifest.verify_for_driver(&configured_action, &wasm)?;
    println!("manifest={} ok", manifest_path.display());
    println!("action={}", manifest.action);
    println!("wasm_hash={}", manifest.wasm_hash);
    println!("author_node_id={}", manifest.author_node_id);
    Ok(())
}

fn registry(command: RegistryCommand) -> Result<()> {
    match command {
        RegistryCommand::Init { out, force } => create_registry(&out, force),
        RegistryCommand::Add {
            registry,
            manifest,
            manifest_path,
            out,
        } => add_registry_entry(&registry, &manifest, manifest_path, out.as_deref()),
        RegistryCommand::Verify { registry, manifest } => {
            verify_registry_entry(&registry, &manifest)
        }
        RegistryCommand::Sign {
            registry,
            operator_key,
            out,
        } => sign_registry(&registry, &operator_key, out.as_deref()),
        RegistryCommand::VerifySignature { registry } => verify_registry_signature(&registry),
        RegistryCommand::Revoke {
            registry,
            action,
            version,
            reason,
            out,
        } => revoke_registry_entry(&registry, &action, &version, reason, out.as_deref()),
        RegistryCommand::List { registry, json } => list_registry(&registry, json),
    }
}

fn create_registry(out: &Path, force: bool) -> Result<()> {
    if out.exists() && !force {
        bail!(
            "refusing to overwrite existing registry {}; pass --force to replace it",
            out.display()
        );
    }
    let registry = DriverRegistry::empty(Some(format!("zap-cli {}", env!("CARGO_PKG_VERSION"))));
    write_text_file(out, &registry.to_toml_string()?, force)?;
    println!("registry={}", out.display());
    println!("entries=0");
    Ok(())
}

fn add_registry_entry(
    registry_path: &Path,
    manifest_path: &Path,
    recorded_manifest_path: Option<String>,
    out: Option<&Path>,
) -> Result<()> {
    let mut registry = load_driver_registry(registry_path)?;
    let manifest = load_driver_manifest(manifest_path)?;
    let recorded_manifest_path =
        recorded_manifest_path.or_else(|| Some(manifest_path.display().to_string()));
    registry.add_manifest(&manifest, recorded_manifest_path)?;
    let out = out.unwrap_or(registry_path);
    write_text_file(out, &registry.to_toml_string()?, true)?;
    println!("registry={}", out.display());
    println!("action={}", manifest.action);
    println!("version={}", manifest.version);
    println!("wasm_hash={}", manifest.wasm_hash);
    Ok(())
}

fn verify_registry_entry(registry_path: &Path, manifest_path: &Path) -> Result<()> {
    let registry = load_driver_registry(registry_path)?;
    let manifest = load_driver_manifest(manifest_path)?;
    registry.verify_manifest(&manifest)?;
    println!("registry={} ok", registry_path.display());
    println!("manifest={} ok", manifest_path.display());
    println!("action={}", manifest.action);
    println!("version={}", manifest.version);
    Ok(())
}

fn sign_registry(registry_path: &Path, operator_key_path: &Path, out: Option<&Path>) -> Result<()> {
    let mut registry = load_driver_registry(registry_path)?;
    let operator = load_keypair(operator_key_path)?;
    registry.sign(&operator)?;
    let out = out.unwrap_or(registry_path);
    write_text_file(out, &registry.to_toml_string()?, true)?;
    println!("registry={}", out.display());
    println!("operator_node_id={}", operator.node_id());
    println!("signature=ok");
    Ok(())
}

fn verify_registry_signature(registry_path: &Path) -> Result<()> {
    let registry = load_driver_registry(registry_path)?;
    registry.verify_signature()?;
    println!("registry={} signature=ok", registry_path.display());
    if let Some(operator_node_id) = registry.operator_node_id {
        println!("operator_node_id={operator_node_id}");
    }
    Ok(())
}

fn revoke_registry_entry(
    registry_path: &Path,
    action: &str,
    version: &str,
    reason: Option<String>,
    out: Option<&Path>,
) -> Result<()> {
    let mut registry = load_driver_registry(registry_path)?;
    let reason = reason.unwrap_or_else(|| "revoked by operator".to_string());
    registry.revoke(action, version, reason.clone())?;
    let out = out.unwrap_or(registry_path);
    write_text_file(out, &registry.to_toml_string()?, true)?;
    println!("registry={}", out.display());
    println!("action={action}");
    println!("version={version}");
    println!("status=revoked");
    println!("reason={reason}");
    println!("signature=cleared");
    Ok(())
}

fn list_registry(registry_path: &Path, json: bool) -> Result<()> {
    let registry = load_driver_registry(registry_path)?;
    if json {
        println!("{}", serde_json::to_string_pretty(&registry)?);
    } else {
        println!("registry={}", registry_path.display());
        println!("signed={}", registry.signature.is_some());
        if let Some(operator_node_id) = registry.operator_node_id {
            println!("operator_node_id={operator_node_id}");
        }
        println!("entries={}", registry.entries.len());
        for entry in &registry.entries {
            println!(
                "entry action={} version={} status={:?} wasm_hash={} author_node_id={}",
                entry.action, entry.version, entry.status, entry.wasm_hash, entry.author_node_id
            );
        }
    }
    Ok(())
}

fn load_driver_registry(path: &Path) -> Result<DriverRegistry> {
    let registry = DriverRegistry::from_toml_str(
        &fs::read_to_string(path)
            .with_context(|| format!("failed to read registry {}", path.display()))?,
    )
    .with_context(|| format!("failed to parse registry {}", path.display()))?;
    registry
        .validate()
        .with_context(|| format!("invalid registry {}", path.display()))?;
    Ok(registry)
}

fn load_driver_manifest(path: &Path) -> Result<DriverManifest> {
    let manifest = DriverManifest::from_toml_str(
        &fs::read_to_string(path)
            .with_context(|| format!("failed to read manifest {}", path.display()))?,
    )
    .with_context(|| format!("failed to parse manifest {}", path.display()))?;
    manifest
        .verify_static_and_signature()
        .with_context(|| format!("invalid signed driver manifest {}", path.display()))?;
    Ok(manifest)
}

fn load_keypair(path: &Path) -> Result<Keypair> {
    Keypair::from_key_file_toml(
        &fs::read_to_string(path)
            .with_context(|| format!("failed to read key file {}", path.display()))?,
    )
    .with_context(|| format!("invalid key file {}", path.display()))
}

fn write_text_file(out: &Path, contents: &str, force: bool) -> Result<()> {
    if let Some(parent) = out.parent().filter(|parent| !parent.as_os_str().is_empty()) {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create directory {}", parent.display()))?;
    }
    let mut options = OpenOptions::new();
    options.write(true);
    if force {
        options.create(true).truncate(true);
    } else {
        options.create_new(true);
    }
    let mut file = match options.open(out) {
        Ok(file) => file,
        Err(error) if !force && error.kind() == ErrorKind::AlreadyExists => {
            bail!(
                "refusing to overwrite existing file {}; pass --force to replace it",
                out.display()
            );
        }
        Err(error) => {
            return Err(error).with_context(|| format!("failed to write {}", out.display()));
        }
    };
    file.write_all(contents.as_bytes())
        .with_context(|| format!("failed to write {}", out.display()))?;
    file.sync_all()
        .with_context(|| format!("failed to flush {}", out.display()))
}

fn inspect(
    frame_path: &Path,
    verify_with_key: Option<&Path>,
    verify_with_public_key: Option<&str>,
) -> Result<()> {
    let bytes = fs::read(frame_path)
        .with_context(|| format!("failed to read frame {}", frame_path.display()))?;
    let frame = ZapFrame::decode(&bytes)?;
    let mut verified = None;
    let envelope = ZapEnvelopeRef::parse(&frame.payload).ok();

    if let Some(public_key) = inspect_verification_key(verify_with_key, verify_with_public_key)? {
        verify_frame(&public_key, &frame)?;
        verified = Some(true);
    }

    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "version": frame.header.version,
            "flags": frame.header.flags.bits(),
            "source_node": frame.header.source_node,
            "target_node": frame.header.target_node,
            "timestamp_micros": frame.header.timestamp_micros,
            "payload_len": frame.header.zap_len,
            "signature_hint": hex::encode(frame.header.zap_sign),
            "has_auth_trailer": frame.auth.is_some(),
            "has_poa_trailer": frame.poa.is_some(),
            "poa_threshold": frame.poa.as_ref().map(|poa| poa.threshold),
            "poa_attestations": frame.poa.as_ref().map(|poa| poa.attestations.len()),
            "envelope_kind": envelope.map(|envelope| envelope.kind().as_str()),
            "subject": envelope.map(|envelope| envelope.subject()),
            "content_type": envelope.map(|envelope| envelope.content_type()),
            "metadata_len": envelope.map(|envelope| envelope.metadata().len()),
            "body_len": envelope.map(|envelope| envelope.body().len()),
            "verified": verified,
        }))?
    );
    Ok(())
}

fn inspect_verification_key(
    verify_with_key: Option<&Path>,
    verify_with_public_key: Option<&str>,
) -> Result<Option<PublicKey>> {
    match (verify_with_key, verify_with_public_key) {
        (Some(key_path), None) => {
            let keypair = Keypair::from_key_file_toml(
                &fs::read_to_string(key_path)
                    .with_context(|| format!("failed to read key file {}", key_path.display()))?,
            )?;
            Ok(Some(keypair.verifying_key()))
        }
        (None, Some(public_key)) => decode_public_key(public_key)
            .with_context(|| "invalid --verify-with-public-key".to_string())
            .map(Some),
        (None, None) => Ok(None),
        (Some(_), Some(_)) => bail!("use either --verify-with-key or --verify-with-public-key"),
    }
}

fn bench(command: BenchCommand) -> Result<()> {
    match command {
        BenchCommand::Parse { iterations } => {
            if iterations == 0 {
                bail!("iterations must be greater than zero");
            }
            let frame = ZapFrame::with_timestamp(
                Uuid::from_bytes([1_u8; 16]),
                Uuid::from_bytes([2_u8; 16]),
                ZapFlags::PRIORITY,
                42,
                Bytes::from_static(b"bench"),
            )?;
            let bytes = frame.header.to_bytes();
            let started = Instant::now();
            for _ in 0..iterations {
                let _ = ZapHeader::parse(&bytes)?;
            }
            let elapsed = started.elapsed();
            let ns_per_parse = elapsed.as_nanos() as f64 / iterations as f64;
            println!(
                "parse iterations={} elapsed_ms={} ns_per_parse={:.2}",
                iterations,
                elapsed.as_millis(),
                ns_per_parse
            );
            Ok(())
        }
    }
}
