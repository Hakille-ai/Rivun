use anyhow::{Context, Result, bail};
use base64::{Engine as _, engine::general_purpose::STANDARD_NO_PAD};
use bytes::Bytes;
use clap::{Parser, Subcommand};
use rand_core::{OsRng, RngCore};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, OpenOptions},
    io::{ErrorKind, Write},
    net::SocketAddr,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};
use tracing_subscriber::{EnvFilter, fmt};
use uuid::Uuid;
use zap_capability::{
    CAPABILITY_CONTENT_TYPE, CAPABILITY_QUERY_SUBJECT, CAPABILITY_RESPONSE_SUBJECT,
    CapabilityCacheEntry, CapabilityId, CapabilityQuery, CapabilityResponse, DriverPermissions,
    JsonlCapabilityCache, capabilities_for_driver,
};
use zap_core::{ED25519_SIGNATURE_LEN, PoaAttestation, ZapFlags, ZapFrame, ZapHeader, now_micros};
use zap_crypto::{
    Keypair, POA_ATTESTATION_CONTENT_TYPE, POA_ATTESTATION_REQUEST_SUBJECT,
    POA_ATTESTATION_RESPONSE_SUBJECT, POA_VALIDATOR_SET_CONTENT_TYPE,
    POA_VALIDATOR_SET_REQUEST_SUBJECT, POA_VALIDATOR_SET_RESPONSE_SUBJECT,
    POA_VALIDATOR_SET_SCHEMA_VERSION, PoaAttestationRequest, PoaAttestationResponse,
    PoaValidatorDescriptor, PoaValidatorSet, PoaValidatorSetRequest, PoaValidatorSetResponse,
    PublicKey, SignedPoaValidatorSet, certify_frame, certify_frame_with_attestations,
    poa_attestation_request, poa_frame_digest, sign_frame, sign_poa_attestation_request,
    sign_poa_validator_set, verify_frame, verify_poa_attestation_response,
};
use zap_envelope::{
    DEFAULT_CONTENT_TYPE as DEFAULT_ENVELOPE_CONTENT_TYPE, ZapEnvelope, ZapEnvelopeRef,
    ZapMessageKind,
};
use zap_ledger::{
    DEFAULT_RECEIPT_REPLICATION_LIMIT, RECEIPT_REPLICATION_CONTENT_TYPE,
    RECEIPT_REPLICATION_REQUEST_SUBJECT, RECEIPT_REPLICATION_RESPONSE_SUBJECT,
    ReceiptReplicationRequest, ReceiptReplicationResponse, SignedActionReceipt,
};
use zap_memory::{JsonlMemoryStore, MemoryPut, MemoryQuery, MemoryStore};
use zap_net::{Peer, TransportKey, ZapEndpoint, ZapEndpointConfig};
use zap_node::{
    PeerConfig, PeerTrustConfig, PeerTrustStatus, ZapNode, ZapNodeConfig, describe_capabilities,
};
use zap_policy::{PolicyInput, PolicySet};
use zap_router::{RouteMessage, RouteTable};
use zap_schema::{MessageContract, MessageParts};
use zap_store::{
    DriverManifest, DriverRegistry, DriverRegistryMergeReport, REGISTRY_INDEX_CONTENT_TYPE,
    REGISTRY_INDEX_REQUEST_SUBJECT, REGISTRY_INDEX_RESPONSE_SUBJECT, RegistryBundleEntry,
    RegistryBundleManifest, RegistryIndexRequest, RegistryIndexResponse, RegistryPublication,
    artifact_hash,
};

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
    /// Run an operator readiness diagnostic for one node config.
    Doctor {
        #[arg(long, default_value = "zap.toml")]
        config: PathBuf,
        #[arg(long)]
        json: bool,
        /// Exit non-zero unless the node is production-ready with no warnings.
        #[arg(long)]
        strict: bool,
    },
    /// Send one frame payload to a configured peer.
    Send {
        #[arg(long, default_value = "zap.toml")]
        config: PathBuf,
        #[arg(long)]
        target: Uuid,
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
        /// Mark the frame as consensus-protected and attach a Proof-of-Action certificate.
        #[arg(long)]
        requires_consensus: bool,
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
    /// Inspect and query ZAP capabilities.
    Capability {
        #[command(subcommand)]
        command: CapabilityCommand,
    },
    /// Operate on a local auditable memory JSONL store.
    Memory {
        #[command(subcommand)]
        command: MemoryCommand,
    },
    /// Explain deterministic message routing.
    Route {
        #[command(subcommand)]
        command: RouteCommand,
    },
    /// Inspect and generate explicit peer trust contracts.
    Trust {
        #[command(subcommand)]
        command: TrustCommand,
    },
    /// Create, accept, rotate, and revoke machine peer enrollment material.
    Peer {
        #[command(subcommand)]
        command: PeerCommand,
    },
    /// Validate typed message contracts for agents and machines.
    Schema {
        #[command(subcommand)]
        command: SchemaCommand,
    },
    /// Evaluate deterministic message policies.
    Policy {
        #[command(subcommand)]
        command: PolicyCommand,
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
enum CapabilityCommand {
    /// List capabilities advertised by a local node config.
    List {
        #[arg(long, default_value = "zap.toml")]
        config: PathBuf,
        #[arg(long)]
        json: bool,
    },
    /// Inspect capabilities implied by a signed driver manifest.
    InspectManifest {
        #[arg(long)]
        manifest: PathBuf,
        #[arg(long)]
        json: bool,
    },
    /// Query a configured peer for its capability advertisement.
    Query {
        #[arg(long, default_value = "zap.toml")]
        config: PathBuf,
        #[arg(long)]
        target: Uuid,
        #[arg(long = "capability")]
        capabilities: Vec<String>,
        /// Append the signed peer response to a local capability cache JSONL file.
        #[arg(long)]
        cache: Option<PathBuf>,
        #[arg(long, default_value_t = 2000, value_parser = clap::value_parser!(u64).range(1..))]
        timeout_ms: u64,
        #[arg(long)]
        json: bool,
    },
    /// Inspect or verify a local capability cache.
    Cache {
        #[command(subcommand)]
        command: CapabilityCacheCommand,
    },
}

#[derive(Debug, Subcommand)]
enum CapabilityCacheCommand {
    /// Query configured peers and append fresh advertisements to the cache.
    Refresh {
        #[arg(long, default_value = "zap.toml")]
        config: PathBuf,
        /// Override [capability_cache].path for one refresh run.
        #[arg(long)]
        path: Option<PathBuf>,
        /// Refresh only selected peer(s). Defaults to every configured peer.
        #[arg(long = "peer")]
        peers: Vec<Uuid>,
        #[arg(long = "capability")]
        capabilities: Vec<String>,
        #[arg(long, default_value_t = 2000, value_parser = clap::value_parser!(u64).range(1..))]
        timeout_ms: u64,
        /// Exit non-zero when any selected peer cannot be refreshed.
        #[arg(long)]
        strict: bool,
        #[arg(long)]
        json: bool,
    },
    /// List cached capability advertisements.
    List {
        #[arg(long, default_value = ".zap/capabilities.jsonl")]
        path: PathBuf,
        #[arg(long)]
        peer: Option<Uuid>,
        #[arg(long)]
        json: bool,
    },
    /// Verify cache hashes, chain integrity, and advertisement consistency.
    Verify {
        #[arg(long, default_value = ".zap/capabilities.jsonl")]
        path: PathBuf,
        #[arg(long)]
        json: bool,
    },
}

#[derive(Debug, Subcommand)]
enum MemoryCommand {
    /// Append one memory record.
    Put {
        #[arg(long, default_value = ".zap/memory.jsonl")]
        path: PathBuf,
        #[arg(long, default_value = "default")]
        namespace: String,
        #[arg(long)]
        subject: String,
        #[arg(long, default_value = "text/plain")]
        content_type: String,
        #[arg(long)]
        metadata: Option<String>,
        #[arg(long)]
        payload: Option<String>,
        #[arg(long)]
        payload_file: Option<PathBuf>,
        #[arg(long, default_value_t = zap_memory::DEFAULT_MEMORY_MAX_RECORD_BYTES)]
        max_record_bytes: usize,
        #[arg(long)]
        json: bool,
    },
    /// Read one memory record by id.
    Get {
        #[arg(long, default_value = ".zap/memory.jsonl")]
        path: PathBuf,
        id: Uuid,
        #[arg(long)]
        json: bool,
    },
    /// Query memory records.
    Query {
        #[arg(long, default_value = ".zap/memory.jsonl")]
        path: PathBuf,
        #[arg(long)]
        namespace: Option<String>,
        #[arg(long)]
        subject: Option<String>,
        #[arg(long)]
        content_type: Option<String>,
        #[arg(long)]
        include_tombstoned: bool,
        #[arg(long)]
        limit: Option<usize>,
        #[arg(long)]
        json: bool,
    },
    /// Tombstone one memory record.
    Tombstone {
        #[arg(long, default_value = ".zap/memory.jsonl")]
        path: PathBuf,
        id: Uuid,
        #[arg(long)]
        reason: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// Verify a memory JSONL store.
    Verify {
        #[arg(long, default_value = ".zap/memory.jsonl")]
        path: PathBuf,
        #[arg(long)]
        json: bool,
    },
    /// Copy a memory store without entries older than a creation timestamp.
    Prune {
        #[arg(long, default_value = ".zap/memory.jsonl")]
        path: PathBuf,
        #[arg(long)]
        before_created_at_micros: u64,
        #[arg(long)]
        out: PathBuf,
        #[arg(long)]
        force: bool,
        #[arg(long)]
        json: bool,
    },
}

#[derive(Debug, Subcommand)]
enum RouteCommand {
    /// Explain how one message would route under a node config.
    Explain {
        #[arg(long, default_value = "zap.toml")]
        config: PathBuf,
        #[arg(long)]
        kind: String,
        #[arg(long)]
        subject: String,
        #[arg(long)]
        source_node: Option<Uuid>,
        #[arg(long)]
        target_node: Option<Uuid>,
        #[arg(long)]
        content_type: Option<String>,
        #[arg(long)]
        json: bool,
    },
}

#[derive(Debug, Subcommand)]
enum TrustCommand {
    /// Generate a verified TOML peer enrollment block.
    Enroll {
        #[arg(long)]
        node_id: Uuid,
        #[arg(long)]
        addr: String,
        #[arg(long)]
        public_key: String,
        #[arg(long)]
        transport_key: String,
        #[arg(long)]
        transport_key_epoch: Option<u64>,
        #[arg(long)]
        transport_key_rotated_at_micros: Option<u64>,
        #[arg(long)]
        expires_at_micros: Option<u64>,
        #[arg(long = "label")]
        labels: Vec<String>,
        #[arg(long)]
        out: Option<PathBuf>,
        #[arg(long)]
        force: bool,
    },
    /// Inspect configured peer trust status and machine permissions.
    Inspect {
        #[arg(long, default_value = "zap.toml")]
        config: PathBuf,
        #[arg(long)]
        json: bool,
    },
}

#[derive(Debug, Subcommand)]
enum PeerCommand {
    /// Create a signed invitation describing this local node as a peer.
    Invite {
        #[arg(long, default_value = "zap.toml")]
        config: PathBuf,
        /// Advertised socket address for the invited remote to use.
        #[arg(long)]
        addr: Option<String>,
        /// Hex-encoded 32-byte transport key. Generated when omitted.
        #[arg(long)]
        transport_key: Option<String>,
        #[arg(long)]
        transport_key_epoch: Option<u64>,
        #[arg(long)]
        transport_key_rotated_at_micros: Option<u64>,
        #[arg(long)]
        expires_at_micros: Option<u64>,
        #[arg(long = "label")]
        labels: Vec<String>,
        #[arg(long)]
        out: Option<PathBuf>,
        #[arg(long)]
        force: bool,
    },
    /// Verify a signed peer invitation and emit a peer config block or config file.
    Accept {
        #[arg(long)]
        invite: PathBuf,
        /// Optional existing config used to reject duplicate peers and produce a full updated config.
        #[arg(long)]
        config: Option<PathBuf>,
        /// Write a full updated config here when --config is supplied, or a peer block otherwise.
        #[arg(long)]
        out: Option<PathBuf>,
        #[arg(long)]
        force: bool,
        #[arg(long)]
        json: bool,
    },
    /// Rotate a configured peer transport key and bump its epoch.
    Rotate {
        #[arg(long, default_value = "zap.toml")]
        config: PathBuf,
        #[arg(long)]
        node_id: Uuid,
        /// Hex-encoded 32-byte transport key. Generated when omitted.
        #[arg(long)]
        transport_key: Option<String>,
        #[arg(long)]
        transport_key_epoch: Option<u64>,
        #[arg(long)]
        transport_key_rotated_at_micros: Option<u64>,
        /// Write the updated config here. Prints the config when omitted.
        #[arg(long)]
        out: Option<PathBuf>,
        #[arg(long)]
        force: bool,
        #[arg(long)]
        json: bool,
    },
    /// Revoke a configured peer by disabling machine permissions and marking trust revoked.
    Revoke {
        #[arg(long, default_value = "zap.toml")]
        config: PathBuf,
        #[arg(long)]
        node_id: Uuid,
        /// Write the updated config here. Prints the config when omitted.
        #[arg(long)]
        out: Option<PathBuf>,
        #[arg(long)]
        force: bool,
        #[arg(long)]
        json: bool,
    },
}

#[derive(Debug, Subcommand)]
enum SchemaCommand {
    /// Validate one encoded ZENV envelope against a TOML or JSON contract.
    Validate {
        #[arg(long)]
        contract: PathBuf,
        #[arg(long)]
        envelope: PathBuf,
        #[arg(long)]
        json: bool,
    },
    /// Print a normalized contract after parsing and static validation.
    Inspect {
        #[arg(long)]
        contract: PathBuf,
        #[arg(long)]
        json: bool,
    },
}

#[derive(Debug, Subcommand)]
enum PolicyCommand {
    /// Evaluate one message against a TOML policy file.
    Evaluate {
        #[arg(long)]
        policy: PathBuf,
        #[arg(long)]
        kind: String,
        #[arg(long)]
        subject: String,
        #[arg(long)]
        source_node: Option<Uuid>,
        #[arg(long)]
        target_node: Option<Uuid>,
        #[arg(long)]
        content_type: Option<String>,
        #[arg(long)]
        requires_consensus: bool,
        #[arg(long = "grant")]
        grants: Vec<String>,
        #[arg(long)]
        human_approved: bool,
        #[arg(long)]
        simulation_passed: bool,
        #[arg(long)]
        strict: bool,
        #[arg(long)]
        json: bool,
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
        #[arg(long)]
        allow_emit_event: bool,
        #[arg(long)]
        allow_memory_read: bool,
        #[arg(long)]
        allow_memory_write: bool,
        #[arg(long)]
        allow_device_call: bool,
        #[arg(long, default_value_t = zap_capability::DEFAULT_MAX_HOST_CALL_BYTES)]
        max_host_call_bytes: u32,
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
    /// Pull a remote peer's ZapStore registry index over ZAP control messages.
    Pull {
        #[arg(long, default_value = "zap.toml")]
        config: PathBuf,
        #[arg(long)]
        target: Uuid,
        #[arg(long)]
        out: PathBuf,
        /// Require the remote registry index to carry a valid operator signature.
        #[arg(long)]
        require_signature: bool,
        /// Require the remote registry index to be signed by this operator public key.
        #[arg(long)]
        operator_public_key: Option<String>,
        #[arg(long, default_value_t = 2000, value_parser = clap::value_parser!(u64).range(1..))]
        timeout_ms: u64,
        #[arg(long)]
        force: bool,
        #[arg(long)]
        json: bool,
    },
    /// Mirror registry indexes from multiple configured peers into one local index.
    Mirror {
        #[arg(long, default_value = "zap.toml")]
        config: PathBuf,
        /// Peer to mirror. Repeat to select specific peers; omit to use all send-allowed peers.
        #[arg(long = "peer")]
        peers: Vec<Uuid>,
        #[arg(long)]
        out: PathBuf,
        /// Require every remote registry index to carry a valid operator signature.
        #[arg(long)]
        require_signature: bool,
        /// Require every remote registry index to be signed by this operator public key.
        #[arg(long)]
        operator_public_key: Option<String>,
        #[arg(long, default_value_t = 2000, value_parser = clap::value_parser!(u64).range(1..))]
        timeout_ms: u64,
        /// Write a merged index from successful peers even if some peers fail.
        #[arg(long)]
        allow_partial: bool,
        #[arg(long)]
        force: bool,
        #[arg(long)]
        json: bool,
    },
    /// Create or verify signed registry publication metadata.
    Publication {
        #[command(subcommand)]
        command: RegistryPublicationCommand,
    },
    /// Export, verify, or import filesystem ZapStore bundles.
    Bundle {
        #[command(subcommand)]
        command: RegistryBundleCommand,
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
enum RegistryBundleCommand {
    /// Export a signed registry, publication metadata, manifests, and optional drivers.
    Export {
        #[arg(long, default_value = "registry.index.toml")]
        registry: PathBuf,
        #[arg(long)]
        publication: Option<PathBuf>,
        #[arg(long)]
        out: PathBuf,
        /// Base directory for resolving registry manifest_path values.
        #[arg(long)]
        base_dir: Option<PathBuf>,
        /// Include a driver artifact as action@version=path. Repeat for multiple entries.
        #[arg(long = "driver")]
        drivers: Vec<String>,
        #[arg(long)]
        force: bool,
        #[arg(long)]
        json: bool,
    },
    /// Verify a ZapStore bundle directory.
    Verify {
        #[arg(long)]
        bundle: PathBuf,
        #[arg(long)]
        publisher_public_key: Option<String>,
        #[arg(long)]
        require_drivers: bool,
        #[arg(long)]
        json: bool,
    },
    /// Import a verified ZapStore bundle into a local directory.
    Import {
        #[arg(long)]
        bundle: PathBuf,
        #[arg(long)]
        out: PathBuf,
        #[arg(long)]
        publisher_public_key: Option<String>,
        #[arg(long)]
        require_drivers: bool,
        #[arg(long)]
        force: bool,
        #[arg(long)]
        json: bool,
    },
}

#[derive(Debug, Subcommand)]
enum RegistryPublicationCommand {
    /// Create signed publication metadata for an approved registry index.
    Create {
        #[arg(long, default_value = "registry.index.toml")]
        registry: PathBuf,
        #[arg(long)]
        publisher_key: PathBuf,
        #[arg(long)]
        out: PathBuf,
        #[arg(long)]
        published_at_micros: Option<u64>,
        #[arg(long)]
        channel: Option<String>,
        #[arg(long = "label")]
        labels: Vec<String>,
        #[arg(long)]
        force: bool,
        #[arg(long)]
        json: bool,
    },
    /// Verify signed publication metadata against a registry index.
    Verify {
        #[arg(long, default_value = "registry.index.toml")]
        registry: PathBuf,
        #[arg(long)]
        publication: PathBuf,
        #[arg(long)]
        publisher_public_key: Option<String>,
        #[arg(long)]
        json: bool,
    },
}

#[derive(Debug, Subcommand)]
enum ReceiptsCommand {
    /// Pull signed receipts from a configured peer over ZAP control messages.
    Pull {
        #[arg(long, default_value = "zap.toml")]
        config: PathBuf,
        #[arg(long)]
        target: Uuid,
        #[arg(long)]
        out: PathBuf,
        #[arg(long)]
        after_processed_at_micros: Option<u64>,
        #[arg(long, default_value_t = DEFAULT_RECEIPT_REPLICATION_LIMIT)]
        limit: usize,
        #[arg(long)]
        kind: Option<String>,
        #[arg(long)]
        subject: Option<String>,
        #[arg(long)]
        source_node: Option<Uuid>,
        #[arg(long)]
        target_node: Option<Uuid>,
        #[arg(long, default_value_t = 2000, value_parser = clap::value_parser!(u64).range(1..))]
        timeout_ms: u64,
        #[arg(long)]
        force: bool,
        #[arg(long)]
        json: bool,
    },
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
    /// Create, verify, and apply signed versioned PoA validator sets.
    #[command(name = "validator-set")]
    ValidatorSet {
        #[command(subcommand)]
        command: PoaValidatorSetCommand,
    },
}

#[derive(Debug, Subcommand)]
enum PoaValidatorSetCommand {
    /// Create a signed versioned PoA validator-set JSON file.
    Create {
        #[arg(long)]
        authority_key: PathBuf,
        #[arg(long, default_value_t = Uuid::new_v4())]
        set_id: Uuid,
        #[arg(long, default_value_t = 1)]
        epoch: u64,
        #[arg(long)]
        threshold: u16,
        /// Validator as <node-id>=<base64-public-key>. Repeat for each validator.
        #[arg(long = "validator")]
        validators: Vec<String>,
        #[arg(long)]
        valid_from_micros: Option<u64>,
        #[arg(long)]
        expires_at_micros: Option<u64>,
        #[arg(long = "label")]
        labels: Vec<String>,
        #[arg(long)]
        out: Option<PathBuf>,
        #[arg(long)]
        force: bool,
    },
    /// Verify a signed PoA validator-set JSON file.
    Verify {
        #[arg(long)]
        path: PathBuf,
        #[arg(long)]
        authority_public_key: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// Pull a signed PoA validator-set JSON file from a configured peer.
    Pull {
        #[arg(long, default_value = "zap.toml")]
        config: PathBuf,
        #[arg(long)]
        target: Uuid,
        #[arg(long)]
        out: PathBuf,
        #[arg(long)]
        authority_public_key: Option<String>,
        #[arg(long)]
        min_epoch: Option<u64>,
        #[arg(long, default_value_t = 2000, value_parser = clap::value_parser!(u64).range(1..))]
        timeout_ms: u64,
        #[arg(long)]
        force: bool,
        #[arg(long)]
        json: bool,
    },
    /// Apply a verified validator-set file to a node config.
    Apply {
        #[arg(long, default_value = "zap.toml")]
        config: PathBuf,
        #[arg(long)]
        set: PathBuf,
        #[arg(long)]
        authority_public_key: Option<String>,
        #[arg(long)]
        out: PathBuf,
        #[arg(long)]
        force: bool,
        #[arg(long)]
        json: bool,
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
        Commands::Doctor {
            config,
            json,
            strict,
        } => doctor(&config, json, strict),
        Commands::Send {
            config,
            target,
            kind,
            subject,
            content_type,
            metadata,
            action,
            payload,
            payload_file,
            binary_payload,
            requires_consensus,
            poa_validator_keys,
            poa_threshold,
            poa_network,
            poa_timeout_ms,
            unsigned,
        } => {
            send(SendOptions {
                config_path: &config,
                target,
                kind,
                subject,
                content_type,
                metadata,
                action,
                payload,
                payload_file,
                binary_payload,
                requires_consensus,
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
        Commands::Capability { command } => capability(command).await,
        Commands::Memory { command } => memory(command),
        Commands::Route { command } => route(command),
        Commands::Trust { command } => trust(command),
        Commands::Peer { command } => peer(command),
        Commands::Schema { command } => schema(command),
        Commands::Policy { command } => policy(command),
        Commands::DriverManifest { command } => driver_manifest(command),
        Commands::Registry { command } => registry(command).await,
        Commands::Receipts { command } => receipts(command).await,
        Commands::Poa { command } => poa(command).await,
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
        println!("trusted_peers={}", report.trusted_peer_count);
        println!("restricted_peers={}", report.restricted_peer_count);
        println!("peer_send_enabled={}", report.peer_send_enabled_count);
        println!("peer_receive_enabled={}", report.peer_receive_enabled_count);
        println!("peer_forward_enabled={}", report.peer_forward_enabled_count);
        println!("drivers={}", report.driver_count);
        println!("signed_drivers={}", report.signed_driver_count);
        println!("registry_enabled={}", report.registry_enabled);
        println!("registry_entries={}", report.registry_entry_count);
        println!(
            "registry_signature_required={}",
            report.registry_signature_required
        );
        println!("receipt_log_enabled={}", report.receipt_log_enabled);
        println!("memory_enabled={}", report.memory_enabled);
        println!("routes={}", report.route_count);
        println!("capabilities={}", report.capability_count);
        println!("capability_grants={}", report.capability_grant_count);
        println!(
            "capability_requirements={}",
            report.capability_requirement_count
        );
        println!(
            "ungranted_capabilities={}",
            report.ungranted_capability_count
        );
        println!(
            "capability_cache_enabled={}",
            report.capability_cache_enabled
        );
        println!("message_policy_rules={}", report.message_policy_rule_count);
        println!(
            "message_schema_contracts={}",
            report.message_schema_contract_count
        );
        println!(
            "message_schema_require_match={}",
            report.message_schema_require_match
        );
        println!("peer_grant_routes={}", report.peer_grant_route_count);
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

#[derive(Debug, Serialize)]
struct DoctorReport {
    config: String,
    status: String,
    score: u8,
    summary: String,
    checks: Vec<DoctorCheck>,
    warnings: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

#[derive(Debug, Serialize)]
struct DoctorCheck {
    name: String,
    status: String,
    detail: String,
}

impl DoctorCheck {
    fn pass(name: impl Into<String>, detail: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            status: "pass".to_string(),
            detail: detail.into(),
        }
    }

    fn warn(name: impl Into<String>, detail: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            status: "warn".to_string(),
            detail: detail.into(),
        }
    }

    fn fail(name: impl Into<String>, detail: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            status: "fail".to_string(),
            detail: detail.into(),
        }
    }
}

fn doctor(config_path: &Path, json: bool, strict: bool) -> Result<()> {
    let report = match ZapNodeConfig::from_path(config_path).and_then(|config| config.validate()) {
        Ok(validation) => build_doctor_report(config_path, &validation),
        Err(error) => DoctorReport {
            config: config_path.display().to_string(),
            status: "failed".to_string(),
            score: 0,
            summary: "configuration is invalid".to_string(),
            checks: vec![DoctorCheck::fail("config validation", format!("{error:#}"))],
            warnings: Vec::new(),
            error: Some(format!("{error:#}")),
        },
    };

    print_doctor_report(&report, json)?;
    if report.status == "failed" {
        bail!("doctor failed for {}", config_path.display());
    }
    if strict && report.status != "ready" {
        bail!(
            "doctor strict gate failed for {}: {}",
            config_path.display(),
            report.summary
        );
    }
    Ok(())
}

fn build_doctor_report(
    config_path: &Path,
    report: &zap_node::ConfigValidationReport,
) -> DoctorReport {
    let mut checks = Vec::new();

    checks.push(DoctorCheck::pass(
        "identity",
        format!("node_id={}", report.node_id),
    ));
    checks.push(if report.require_signed {
        DoctorCheck::pass("signed frames", "require_signed=true")
    } else {
        DoctorCheck::warn("signed frames", "require_signed=false")
    });
    checks.push(if report.peer_count > 0 {
        DoctorCheck::pass("peer topology", format!("peers={}", report.peer_count))
    } else {
        DoctorCheck::warn("peer topology", "no configured peers")
    });
    checks.push(peer_trust_check(report));
    checks.push(if report.driver_count > 0 {
        DoctorCheck::pass("driver surface", format!("drivers={}", report.driver_count))
    } else {
        DoctorCheck::warn(
            "driver surface",
            "no local drivers configured; node can only route, drop, or answer control messages",
        )
    });
    checks.push(driver_provenance_check(report));
    checks.push(registry_policy_check(report));
    checks.push(if report.receipt_log_enabled {
        DoctorCheck::pass("receipt audit", "signed receipt log enabled")
    } else {
        DoctorCheck::warn("receipt audit", "receipts.path is not configured")
    });
    checks.push(if report.poa_validator_count > 0 {
        DoctorCheck::pass(
            "poa quorum",
            format!(
                "threshold={} validators={}",
                report.poa_required_threshold, report.poa_validator_count
            ),
        )
    } else {
        DoctorCheck::warn("poa quorum", "no PoA validators configured")
    });
    checks.push(if report.memory_enabled {
        DoctorCheck::pass("memory audit", "local memory path configured")
    } else {
        DoctorCheck::warn("memory audit", "memory.path is not configured")
    });
    checks.push(if report.route_count > 0 {
        DoctorCheck::pass("routing policy", format!("routes={}", report.route_count))
    } else {
        DoctorCheck::warn("routing policy", "default route behavior only")
    });
    checks.push(if report.capability_count > 0 {
        DoctorCheck::pass(
            "capability surface",
            format!("capabilities={}", report.capability_count),
        )
    } else {
        DoctorCheck::warn("capability surface", "no advertised capabilities")
    });
    checks.push(capability_policy_check(report));
    checks.push(message_policy_check(report));
    checks.push(message_schema_check(report));
    checks.push(peer_grant_routes_check(report));
    for warning in &report.warnings {
        checks.push(DoctorCheck::warn("config warning", warning.clone()));
    }

    let score = doctor_score(&checks);
    let status = if checks.iter().any(|check| check.status == "fail") {
        "failed"
    } else if checks.iter().any(|check| check.status == "warn") {
        "needs_attention"
    } else {
        "ready"
    }
    .to_string();
    let warnings = checks
        .iter()
        .filter(|check| check.status == "warn")
        .map(|check| format!("{}: {}", check.name, check.detail))
        .collect::<Vec<_>>();
    let summary = match status.as_str() {
        "ready" => "node config is production-ready".to_string(),
        "needs_attention" => format!(
            "node config is valid but has {} readiness warning(s)",
            warnings.len()
        ),
        _ => "node config failed readiness checks".to_string(),
    };

    DoctorReport {
        config: config_path.display().to_string(),
        status,
        score,
        summary,
        checks,
        warnings,
        error: None,
    }
}

fn driver_provenance_check(report: &zap_node::ConfigValidationReport) -> DoctorCheck {
    if report.driver_count == 0 {
        return DoctorCheck::warn("driver provenance", "no local drivers to sign");
    }
    if report.signed_driver_count == report.driver_count {
        return DoctorCheck::pass(
            "driver provenance",
            format!("signed_drivers={}", report.signed_driver_count),
        );
    }
    DoctorCheck::warn(
        "driver provenance",
        format!(
            "signed_drivers={} unsigned_drivers={}",
            report.signed_driver_count,
            report.driver_count - report.signed_driver_count
        ),
    )
}

fn registry_policy_check(report: &zap_node::ConfigValidationReport) -> DoctorCheck {
    if !report.registry_enabled {
        return DoctorCheck::warn("registry policy", "registry.path is not configured");
    }
    if report.registry_signature_required {
        return DoctorCheck::pass(
            "registry policy",
            format!(
                "signed registry required; entries={}",
                report.registry_entry_count
            ),
        );
    }
    DoctorCheck::warn(
        "registry policy",
        format!(
            "registry configured with entries={} but require_signature=false",
            report.registry_entry_count
        ),
    )
}

fn peer_trust_check(report: &zap_node::ConfigValidationReport) -> DoctorCheck {
    if report.peer_count == 0 {
        return DoctorCheck::warn("peer trust", "no peer trust contracts to validate");
    }
    if report.trusted_peer_count == report.peer_count && report.restricted_peer_count == 0 {
        return DoctorCheck::pass(
            "peer trust",
            format!(
                "trusted={} send={} receive={} forward={}",
                report.trusted_peer_count,
                report.peer_send_enabled_count,
                report.peer_receive_enabled_count,
                report.peer_forward_enabled_count
            ),
        );
    }
    DoctorCheck::warn(
        "peer trust",
        format!(
            "trusted={} restricted={} send={} receive={} forward={}",
            report.trusted_peer_count,
            report.restricted_peer_count,
            report.peer_send_enabled_count,
            report.peer_receive_enabled_count,
            report.peer_forward_enabled_count
        ),
    )
}

fn capability_policy_check(report: &zap_node::ConfigValidationReport) -> DoctorCheck {
    if report.capability_count == 0 {
        return DoctorCheck::warn("capability policy", "no capabilities to cover");
    }
    if report.ungranted_capability_count == 0 {
        return DoctorCheck::pass(
            "capability policy",
            format!(
                "grants={} requirements={}",
                report.capability_grant_count, report.capability_requirement_count
            ),
        );
    }
    DoctorCheck::warn(
        "capability policy",
        format!(
            "ungranted_capabilities={} grants={} requirements={}",
            report.ungranted_capability_count,
            report.capability_grant_count,
            report.capability_requirement_count
        ),
    )
}

fn message_policy_check(report: &zap_node::ConfigValidationReport) -> DoctorCheck {
    if report.message_policy_rule_count == 0 {
        return DoctorCheck::warn("message policy", "no message policy rules configured");
    }
    DoctorCheck::pass(
        "message policy",
        format!("rules={}", report.message_policy_rule_count),
    )
}

fn message_schema_check(report: &zap_node::ConfigValidationReport) -> DoctorCheck {
    if report.message_schema_contract_count == 0 {
        return DoctorCheck::warn("message schema", "no typed message contracts configured");
    }
    if report.message_schema_require_match {
        return DoctorCheck::pass(
            "message schema",
            format!(
                "contracts={} require_match=true",
                report.message_schema_contract_count
            ),
        );
    }
    DoctorCheck::warn(
        "message schema",
        format!(
            "contracts={} but require_match=false",
            report.message_schema_contract_count
        ),
    )
}

fn peer_grant_routes_check(report: &zap_node::ConfigValidationReport) -> DoctorCheck {
    if report.peer_grant_route_count == 0 {
        return DoctorCheck::warn(
            "peer capability gates",
            "no routes require cached peer capability grants",
        );
    }
    DoctorCheck::pass(
        "peer capability gates",
        format!(
            "peer_grant_routes={} cache_enabled={}",
            report.peer_grant_route_count, report.capability_cache_enabled
        ),
    )
}

fn doctor_score(checks: &[DoctorCheck]) -> u8 {
    let mut score = 100_i16;
    for check in checks {
        match check.status.as_str() {
            "warn" => score -= 8,
            "fail" => score -= 35,
            _ => {}
        }
    }
    score.clamp(0, 100) as u8
}

fn print_doctor_report(report: &DoctorReport, json: bool) -> Result<()> {
    if json {
        println!("{}", serde_json::to_string_pretty(report)?);
        return Ok(());
    }

    println!("doctor={}", report.config);
    println!("status={}", report.status);
    println!("score={}", report.score);
    println!("summary={}", report.summary);
    for check in &report.checks {
        println!(
            "check={} status={} detail={}",
            check.name, check.status, check.detail
        );
    }
    if let Some(error) = &report.error {
        println!("error={error}");
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
    kind: Option<String>,
    subject: Option<String>,
    content_type: Option<String>,
    metadata: Option<String>,
    action: Option<String>,
    payload: Option<String>,
    payload_file: Option<PathBuf>,
    binary_payload: bool,
    requires_consensus: bool,
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
        kind,
        subject,
        content_type,
        metadata,
        action,
        payload,
        payload_file,
        binary_payload,
        requires_consensus,
        poa_validator_keys,
        poa_threshold,
        poa_network,
        poa_timeout_ms,
        unsigned,
    } = options;
    let config = ZapNodeConfig::from_path(config_path)?;
    config.validate()?;
    let messages = build_messages(BuildMessageOptions {
        kind,
        subject,
        content_type,
        metadata,
        action,
        payload,
        payload_file,
        binary_payload,
        requires_consensus,
    })?;
    let keypair = Keypair::from_key_file_toml(
        &fs::read_to_string(&config.key_file)
            .with_context(|| format!("failed to read key file {}", config.key_file.display()))?,
    )?;
    let target_peer = config
        .peers
        .iter()
        .find(|peer| peer.node_id == target)
        .ok_or_else(|| {
            anyhow::anyhow!("target {} not found in {}", target, config_path.display())
        })?;
    if !target_peer.trust.allows_send() {
        bail!(
            "target {} is not permitted by local peer trust policy for outbound send",
            target
        );
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

async fn poa(command: PoaCommand) -> Result<()> {
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
        PoaCommand::ValidatorSet { command } => poa_validator_set(command).await,
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

async fn poa_validator_set(command: PoaValidatorSetCommand) -> Result<()> {
    match command {
        PoaValidatorSetCommand::Create {
            authority_key,
            set_id,
            epoch,
            threshold,
            validators,
            valid_from_micros,
            expires_at_micros,
            labels,
            out,
            force,
        } => create_poa_validator_set(PoaValidatorSetCreateOptions {
            authority_key: &authority_key,
            set_id,
            epoch,
            threshold,
            validators,
            valid_from_micros,
            expires_at_micros,
            labels,
            out,
            force,
        }),
        PoaValidatorSetCommand::Verify {
            path,
            authority_public_key,
            json,
        } => verify_poa_validator_set_file(&path, authority_public_key.as_deref(), json),
        PoaValidatorSetCommand::Pull {
            config,
            target,
            out,
            authority_public_key,
            min_epoch,
            timeout_ms,
            force,
            json,
        } => {
            pull_poa_validator_set(PoaValidatorSetPullOptions {
                config_path: &config,
                target,
                out: &out,
                authority_public_key,
                min_epoch,
                timeout_ms,
                force,
                json,
            })
            .await
        }
        PoaValidatorSetCommand::Apply {
            config,
            set,
            authority_public_key,
            out,
            force,
            json,
        } => apply_poa_validator_set(
            &config,
            &set,
            authority_public_key.as_deref(),
            &out,
            force,
            json,
        ),
    }
}

struct PoaValidatorSetCreateOptions<'a> {
    authority_key: &'a Path,
    set_id: Uuid,
    epoch: u64,
    threshold: u16,
    validators: Vec<String>,
    valid_from_micros: Option<u64>,
    expires_at_micros: Option<u64>,
    labels: Vec<String>,
    out: Option<PathBuf>,
    force: bool,
}

#[derive(Debug, Serialize)]
struct PoaValidatorSetReport {
    path: Option<String>,
    set_id: Uuid,
    epoch: u64,
    required_threshold: u16,
    validators: usize,
    authority_node: Uuid,
    valid_from_micros: Option<u64>,
    expires_at_micros: Option<u64>,
    labels: Vec<String>,
}

#[derive(Debug, Serialize)]
struct PoaValidatorSetApplyReport {
    config: String,
    out: String,
    validator_set: String,
    set_id: Uuid,
    epoch: u64,
    required_threshold: u16,
    validators: usize,
    authority_node: Uuid,
}

struct PoaValidatorSetPullOptions<'a> {
    config_path: &'a Path,
    target: Uuid,
    out: &'a Path,
    authority_public_key: Option<String>,
    min_epoch: Option<u64>,
    timeout_ms: u64,
    force: bool,
    json: bool,
}

#[derive(Debug, Serialize)]
struct PoaValidatorSetPullReport {
    peer: Uuid,
    out: String,
    set_id: Uuid,
    epoch: u64,
    required_threshold: u16,
    validators: usize,
    authority_node: Uuid,
}

fn create_poa_validator_set(options: PoaValidatorSetCreateOptions<'_>) -> Result<()> {
    let authority = load_keypair(options.authority_key)?;
    let validators = parse_poa_validator_descriptors(&options.validators)?;
    validate_trust_labels(&options.labels)?;
    let set = PoaValidatorSet {
        schema_version: POA_VALIDATOR_SET_SCHEMA_VERSION,
        set_id: options.set_id,
        epoch: options.epoch,
        required_threshold: options.threshold,
        validators,
        valid_from_micros: options.valid_from_micros,
        expires_at_micros: options.expires_at_micros,
        labels: options.labels,
    };
    let signed = sign_poa_validator_set(&authority, set)?;
    let output = format!("{}\n", serde_json::to_string_pretty(&signed)?);
    if let Some(out) = options.out {
        write_text_file(&out, &output, options.force)?;
        println!("poa_validator_set={}", out.display());
        println!("set_id={}", signed.set.set_id);
        println!("epoch={}", signed.set.epoch);
    } else {
        print!("{output}");
    }
    Ok(())
}

async fn pull_poa_validator_set(options: PoaValidatorSetPullOptions<'_>) -> Result<()> {
    let PoaValidatorSetPullOptions {
        config_path,
        target,
        out,
        authority_public_key,
        min_epoch,
        timeout_ms,
        force,
        json,
    } = options;
    let config = ZapNodeConfig::from_path(config_path)?;
    config.validate()?;
    let keypair = load_keypair(&config.key_file)?;
    let target_peer = config
        .peers
        .iter()
        .find(|peer| peer.node_id == target)
        .with_context(|| format!("target {} not found in {}", target, config_path.display()))?;
    if !target_peer.trust.allows_send() {
        bail!(
            "target {} is not permitted by local peer trust policy for validator-set pull",
            target
        );
    }
    let authority = authority_public_key
        .as_deref()
        .map(decode_public_key)
        .transpose()
        .context("invalid --authority-public-key")?;
    let request = PoaValidatorSetRequest {
        schema_version: POA_VALIDATOR_SET_SCHEMA_VERSION,
        min_epoch,
    };
    request.validate()?;

    let endpoint = build_capability_endpoint(&config, &keypair).await?;
    let envelope = ZapEnvelope::new(
        ZapMessageKind::Control,
        POA_VALIDATOR_SET_REQUEST_SUBJECT,
        POA_VALIDATOR_SET_CONTENT_TYPE,
        Bytes::from(serde_json::to_vec(&request)?),
    )?;
    let frame = ZapFrame::new(
        keypair.node_id(),
        target,
        ZapFlags::ENCRYPTED,
        envelope.encode(),
    )?;
    let frame = sign_frame(&keypair, &frame)?;
    endpoint.send_frame(target, &frame).await?;

    let public_key = decode_public_key(&target_peer.public_key)?;
    let deadline = Instant::now() + Duration::from_millis(timeout_ms);
    let response = loop {
        let now = Instant::now();
        if now >= deadline {
            bail!(
                "timed out waiting for PoA validator-set response from {}",
                target
            );
        }
        let remaining = deadline.saturating_duration_since(now);
        let inbound = tokio::time::timeout(remaining, endpoint.recv()).await??;
        if inbound.peer.node_id != target {
            continue;
        }
        if config.require_signed {
            verify_frame(&public_key, &inbound.frame)?;
        }
        let envelope = ZapEnvelopeRef::parse(&inbound.frame.payload)
            .context("invalid PoA validator-set response envelope")?;
        if envelope.kind() != ZapMessageKind::Control
            || envelope.subject() != POA_VALIDATOR_SET_RESPONSE_SUBJECT
        {
            continue;
        }
        let response: PoaValidatorSetResponse = serde_json::from_slice(envelope.body())
            .context("invalid PoA validator-set response body")?;
        if response.node_id != target {
            bail!(
                "PoA validator-set response from {} advertised node_id {}",
                target,
                response.node_id
            );
        }
        response.verify(authority.as_ref())?;
        break response;
    };
    let signed = response.validator_set.with_context(|| {
        format!(
            "peer {} did not return a PoA validator set: {}",
            target,
            response
                .unavailable_reason
                .unwrap_or_else(|| "unavailable".to_string())
        )
    })?;
    let output = format!("{}\n", serde_json::to_string_pretty(&signed)?);
    write_text_file(out, &output, force)?;
    let report = PoaValidatorSetPullReport {
        peer: target,
        out: out.display().to_string(),
        set_id: signed.set.set_id,
        epoch: signed.set.epoch,
        required_threshold: signed.set.required_threshold,
        validators: signed.set.validators.len(),
        authority_node: signed.authority_node,
    };
    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        println!("peer={}", report.peer);
        println!("out={}", report.out);
        println!("set_id={}", report.set_id);
        println!("epoch={}", report.epoch);
        println!("required_threshold={}", report.required_threshold);
        println!("validators={}", report.validators);
        println!("authority_node={}", report.authority_node);
    }
    Ok(())
}

fn verify_poa_validator_set_file(
    path: &Path,
    authority_public_key: Option<&str>,
    json: bool,
) -> Result<()> {
    let signed = load_signed_poa_validator_set_file(path)?;
    let authority = authority_public_key
        .map(decode_public_key)
        .transpose()
        .context("invalid --authority-public-key")?;
    signed.verify(authority.as_ref())?;
    let report = poa_validator_set_report(Some(path), &signed);
    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        print_poa_validator_set_report(&report);
    }
    Ok(())
}

fn apply_poa_validator_set(
    config_path: &Path,
    set_path: &Path,
    authority_public_key: Option<&str>,
    out: &Path,
    force: bool,
    json: bool,
) -> Result<()> {
    let signed = load_signed_poa_validator_set_file(set_path)?;
    let authority = authority_public_key
        .map(decode_public_key)
        .transpose()
        .context("invalid --authority-public-key")?;
    signed.verify(authority.as_ref())?;
    let mut config = load_raw_node_config(config_path)?;
    config.poa.validator_set = Some(set_path.to_path_buf());
    config.poa.validator_set_authority = Some(
        authority_public_key
            .map(str::to_string)
            .unwrap_or_else(|| signed.authority_public_key.clone()),
    );
    config.poa.required_threshold = config
        .poa
        .required_threshold
        .max(signed.set.required_threshold);
    config.poa.validators.clear();
    let output = toml::to_string_pretty(&config)?;
    write_text_file(out, &output, force)?;
    let report = PoaValidatorSetApplyReport {
        config: config_path.display().to_string(),
        out: out.display().to_string(),
        validator_set: set_path.display().to_string(),
        set_id: signed.set.set_id,
        epoch: signed.set.epoch,
        required_threshold: config.poa.required_threshold,
        validators: signed.set.validators.len(),
        authority_node: signed.authority_node,
    };
    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        println!("config={}", report.config);
        println!("out={}", report.out);
        println!("validator_set={}", report.validator_set);
        println!("set_id={}", report.set_id);
        println!("epoch={}", report.epoch);
        println!("required_threshold={}", report.required_threshold);
        println!("validators={}", report.validators);
        println!("authority_node={}", report.authority_node);
    }
    Ok(())
}

fn parse_poa_validator_descriptors(inputs: &[String]) -> Result<Vec<PoaValidatorDescriptor>> {
    if inputs.is_empty() {
        bail!("at least one --validator is required");
    }
    let mut validators = Vec::with_capacity(inputs.len());
    let mut seen = BTreeSet::new();
    for input in inputs {
        let (node_id, public_key) = input.split_once('=').with_context(|| {
            format!("invalid --validator `{input}`; expected <node-id>=<public-key>")
        })?;
        let node_id = node_id
            .parse::<Uuid>()
            .with_context(|| format!("invalid validator node id `{node_id}`"))?;
        let decoded = decode_public_key(public_key)
            .with_context(|| format!("invalid validator public key for {node_id}"))?;
        if decoded.node_id() != node_id {
            bail!(
                "validator public_key derives node_id {}, but --validator declares {}",
                decoded.node_id(),
                node_id
            );
        }
        if !seen.insert(node_id) {
            bail!("duplicate validator {}", node_id);
        }
        validators.push(PoaValidatorDescriptor {
            node_id,
            public_key: public_key.to_string(),
        });
    }
    Ok(validators)
}

fn load_signed_poa_validator_set_file(path: &Path) -> Result<SignedPoaValidatorSet> {
    let input = fs::read_to_string(path)
        .with_context(|| format!("failed to read PoA validator set {}", path.display()))?;
    serde_json::from_str(&input)
        .with_context(|| format!("failed to parse PoA validator set {}", path.display()))
}

fn poa_validator_set_report(
    path: Option<&Path>,
    signed: &SignedPoaValidatorSet,
) -> PoaValidatorSetReport {
    PoaValidatorSetReport {
        path: path.map(|path| path.display().to_string()),
        set_id: signed.set.set_id,
        epoch: signed.set.epoch,
        required_threshold: signed.set.required_threshold,
        validators: signed.set.validators.len(),
        authority_node: signed.authority_node,
        valid_from_micros: signed.set.valid_from_micros,
        expires_at_micros: signed.set.expires_at_micros,
        labels: signed.set.labels.clone(),
    }
}

fn print_poa_validator_set_report(report: &PoaValidatorSetReport) {
    if let Some(path) = &report.path {
        println!("path={path}");
    }
    println!("set_id={}", report.set_id);
    println!("epoch={}", report.epoch);
    println!("required_threshold={}", report.required_threshold);
    println!("validators={}", report.validators);
    println!("authority_node={}", report.authority_node);
    println!(
        "valid_from_micros={}",
        report
            .valid_from_micros
            .map(|value| value.to_string())
            .unwrap_or_else(|| "none".to_string())
    );
    println!(
        "expires_at_micros={}",
        report
            .expires_at_micros
            .map(|value| value.to_string())
            .unwrap_or_else(|| "none".to_string())
    );
    println!(
        "labels={}",
        if report.labels.is_empty() {
            "none".to_string()
        } else {
            report.labels.join(",")
        }
    );
}

async fn receipts(command: ReceiptsCommand) -> Result<()> {
    match command {
        ReceiptsCommand::Pull {
            config,
            target,
            out,
            after_processed_at_micros,
            limit,
            kind,
            subject,
            source_node,
            target_node,
            timeout_ms,
            force,
            json,
        } => {
            pull_receipts(ReceiptPullOptions {
                config_path: &config,
                target,
                out: &out,
                after_processed_at_micros,
                limit,
                kind,
                subject,
                source_node,
                target_node,
                timeout_ms,
                force,
                json,
            })
            .await
        }
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

struct ReceiptPullOptions<'a> {
    config_path: &'a Path,
    target: Uuid,
    out: &'a Path,
    after_processed_at_micros: Option<u64>,
    limit: usize,
    kind: Option<String>,
    subject: Option<String>,
    source_node: Option<Uuid>,
    target_node: Option<Uuid>,
    timeout_ms: u64,
    force: bool,
    json: bool,
}

#[derive(Debug, Serialize)]
struct ReceiptPullReport {
    peer: Uuid,
    out: String,
    receipts: usize,
    truncated: bool,
    earliest_processed_at_micros: Option<u64>,
    latest_processed_at_micros: Option<u64>,
}

async fn pull_receipts(options: ReceiptPullOptions<'_>) -> Result<()> {
    let ReceiptPullOptions {
        config_path,
        target,
        out,
        after_processed_at_micros,
        limit,
        kind,
        subject,
        source_node,
        target_node,
        timeout_ms,
        force,
        json,
    } = options;
    let config = ZapNodeConfig::from_path(config_path)?;
    config.validate()?;
    ensure_receipt_output_is_separate(out, &[config.key_file.as_path()])?;
    let keypair = load_keypair(&config.key_file)?;
    let target_peer = config
        .peers
        .iter()
        .find(|peer| peer.node_id == target)
        .with_context(|| format!("target {} not found in {}", target, config_path.display()))?;
    if !target_peer.trust.allows_send() {
        bail!(
            "target {} is not permitted by local peer trust policy for receipt pull",
            target
        );
    }
    let request = ReceiptReplicationRequest {
        schema_version: zap_ledger::RECEIPT_REPLICATION_SCHEMA_VERSION,
        after_processed_at_micros,
        limit: Some(limit),
        kind,
        subject,
        source_node,
        target_node,
    };
    request.validate()?;

    let endpoint = build_capability_endpoint(&config, &keypair).await?;
    let envelope = ZapEnvelope::new(
        ZapMessageKind::Control,
        RECEIPT_REPLICATION_REQUEST_SUBJECT,
        RECEIPT_REPLICATION_CONTENT_TYPE,
        Bytes::from(serde_json::to_vec(&request)?),
    )?;
    let frame = ZapFrame::new(
        keypair.node_id(),
        target,
        ZapFlags::ENCRYPTED,
        envelope.encode(),
    )?;
    let frame = sign_frame(&keypair, &frame)?;
    endpoint.send_frame(target, &frame).await?;

    let public_key = decode_public_key(&target_peer.public_key)?;
    let deadline = Instant::now() + Duration::from_millis(timeout_ms);
    let response = loop {
        let now = Instant::now();
        if now >= deadline {
            bail!(
                "timed out waiting for receipt replication response from {}",
                target
            );
        }
        let remaining = deadline.saturating_duration_since(now);
        let inbound = tokio::time::timeout(remaining, endpoint.recv()).await??;
        if inbound.peer.node_id != target {
            continue;
        }
        if config.require_signed {
            verify_frame(&public_key, &inbound.frame)?;
        }
        let envelope = ZapEnvelopeRef::parse(&inbound.frame.payload)
            .context("invalid receipt replication response envelope")?;
        if envelope.kind() != ZapMessageKind::Control
            || envelope.subject() != RECEIPT_REPLICATION_RESPONSE_SUBJECT
        {
            continue;
        }
        let response: ReceiptReplicationResponse = serde_json::from_slice(envelope.body())
            .context("invalid receipt replication response body")?;
        if response.node_id != target {
            bail!(
                "receipt replication response from {} advertised node_id {}",
                target,
                response.node_id
            );
        }
        response.verify()?;
        break response;
    };

    let mut output = String::new();
    for receipt in &response.receipts {
        output.push_str(&receipt.to_json_line()?);
    }
    write_text_file(out, &output, force)?;
    let earliest = response
        .receipts
        .iter()
        .map(|receipt| receipt.receipt.processed_at_micros)
        .min();
    let latest = response
        .receipts
        .iter()
        .map(|receipt| receipt.receipt.processed_at_micros)
        .max();
    let report = ReceiptPullReport {
        peer: target,
        out: out.display().to_string(),
        receipts: response.receipts.len(),
        truncated: response.truncated,
        earliest_processed_at_micros: earliest,
        latest_processed_at_micros: latest,
    };
    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        println!("peer={}", report.peer);
        println!("out={}", report.out);
        println!("receipts={}", report.receipts);
        println!("truncated={}", report.truncated);
        println!(
            "earliest_processed_at_micros={}",
            report
                .earliest_processed_at_micros
                .map(|value| value.to_string())
                .unwrap_or_else(|| "none".to_string())
        );
        println!(
            "latest_processed_at_micros={}",
            report
                .latest_processed_at_micros
                .map(|value| value.to_string())
                .unwrap_or_else(|| "none".to_string())
        );
    }
    Ok(())
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
    kind: Option<String>,
    subject: Option<String>,
    content_type: Option<String>,
    metadata: Option<String>,
    action: Option<String>,
    payload: Option<String>,
    payload_file: Option<PathBuf>,
    binary_payload: bool,
    requires_consensus: bool,
}

fn build_messages(options: BuildMessageOptions) -> Result<Vec<OutboundMessage>> {
    let BuildMessageOptions {
        kind,
        subject,
        content_type,
        metadata,
        action,
        payload,
        payload_file,
        binary_payload,
        requires_consensus,
    } = options;

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
            requires_consensus,
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
        requires_consensus,
    }])
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

async fn capability(command: CapabilityCommand) -> Result<()> {
    match command {
        CapabilityCommand::List { config, json } => capability_list(&config, json),
        CapabilityCommand::InspectManifest { manifest, json } => {
            capability_inspect_manifest(&manifest, json)
        }
        CapabilityCommand::Query {
            config,
            target,
            capabilities,
            cache,
            timeout_ms,
            json,
        } => {
            capability_query(
                &config,
                target,
                capabilities,
                cache.as_deref(),
                timeout_ms,
                json,
            )
            .await
        }
        CapabilityCommand::Cache { command } => capability_cache(command).await,
    }
}

fn capability_list(config_path: &Path, json: bool) -> Result<()> {
    let config = ZapNodeConfig::from_path(config_path)?;
    config.validate()?;
    let advertisement = describe_capabilities(&config)?;
    if json {
        println!("{}", serde_json::to_string_pretty(&advertisement)?);
    } else {
        println!("node_id={}", advertisement.node_id);
        for capability in advertisement.capabilities.iter() {
            println!("capability={capability}");
        }
    }
    Ok(())
}

fn capability_inspect_manifest(manifest_path: &Path, json: bool) -> Result<()> {
    let manifest = load_driver_manifest(manifest_path)?;
    manifest.verify_static_and_signature()?;
    let capabilities = capabilities_for_driver(&manifest.action, manifest.permissions)?;
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "manifest": manifest_path.display().to_string(),
                "action": manifest.action,
                "permissions": manifest.permissions,
                "capabilities": capabilities
            }))?
        );
    } else {
        println!("manifest={} ok", manifest_path.display());
        println!("action={}", manifest.action);
        for capability in capabilities.iter() {
            println!("capability={capability}");
        }
    }
    Ok(())
}

async fn capability_query(
    config_path: &Path,
    target: Uuid,
    requested: Vec<String>,
    cache_path: Option<&Path>,
    timeout_ms: u64,
    json: bool,
) -> Result<()> {
    let config = ZapNodeConfig::from_path(config_path)?;
    config.validate()?;
    let keypair = load_keypair(&config.key_file)?;
    let requested = parse_capability_ids(&requested)?;
    let endpoint = build_capability_endpoint(&config, &keypair).await?;
    let result = query_capability_peer(
        &config, &endpoint, &keypair, target, &requested, cache_path, timeout_ms,
    )
    .await?;

    print_capability_query_result(&result, cache_path, json)
}

#[derive(Debug, Serialize)]
struct CapabilityQueryCommandResult {
    response: CapabilityResponse,
    #[serde(skip_serializing_if = "Option::is_none")]
    cached_entry: Option<CapabilityCacheEntry>,
}

#[derive(Debug, Serialize)]
struct CapabilityCacheRefreshReport {
    config: String,
    cache: String,
    requested_peer_count: usize,
    refreshed: usize,
    skipped: usize,
    failed: usize,
    results: Vec<CapabilityCacheRefreshPeerResult>,
}

#[derive(Debug, Serialize)]
struct CapabilityCacheRefreshPeerResult {
    peer: Uuid,
    status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    cache_entry: Option<Uuid>,
    capabilities: usize,
    grants: usize,
    requirements: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

fn parse_capability_ids(requested: &[String]) -> Result<Vec<CapabilityId>> {
    requested
        .iter()
        .map(|capability| CapabilityId::new(capability.clone()).map_err(Into::into))
        .collect()
}

async fn build_capability_endpoint(
    config: &ZapNodeConfig,
    keypair: &Keypair,
) -> Result<ZapEndpoint> {
    let bind: SocketAddr = config
        .bind
        .parse()
        .with_context(|| format!("invalid bind address {}", config.bind))?;
    let endpoint = ZapEndpoint::bind(ZapEndpointConfig::new(bind, keypair.node_id())).await?;
    for peer in &config.peers {
        if !peer.trust.allows_transport() {
            continue;
        }
        endpoint
            .add_peer(Peer {
                node_id: peer.node_id,
                addr: peer.addr.parse()?,
                transport_key: TransportKey::from_hex(&peer.transport_key)?,
            })
            .await;
    }
    Ok(endpoint)
}

async fn query_capability_peer(
    config: &ZapNodeConfig,
    endpoint: &ZapEndpoint,
    keypair: &Keypair,
    target: Uuid,
    requested: &[CapabilityId],
    cache_path: Option<&Path>,
    timeout_ms: u64,
) -> Result<CapabilityQueryCommandResult> {
    let target_peer = config
        .peers
        .iter()
        .find(|peer| peer.node_id == target)
        .with_context(|| format!("target {} not found in node config", target))?;
    if !target_peer.trust.allows_send() {
        bail!(
            "target {} is not permitted by local peer trust policy for capability query",
            target
        );
    }

    let query = CapabilityQuery {
        requested: requested.to_vec(),
    };
    let envelope = ZapEnvelope::new(
        ZapMessageKind::Control,
        CAPABILITY_QUERY_SUBJECT,
        CAPABILITY_CONTENT_TYPE,
        Bytes::from(serde_json::to_vec(&query)?),
    )?;
    let frame = ZapFrame::new(
        keypair.node_id(),
        target,
        ZapFlags::ENCRYPTED,
        envelope.encode(),
    )?;
    let frame = sign_frame(keypair, &frame)?;
    endpoint.send_frame(target, &frame).await?;

    let public_key = decode_public_key(&target_peer.public_key)?;
    let deadline = Instant::now() + Duration::from_millis(timeout_ms);
    loop {
        let now = Instant::now();
        if now >= deadline {
            bail!("timed out waiting for capability response from {}", target);
        }
        let remaining = deadline.saturating_duration_since(now);
        let inbound = tokio::time::timeout(remaining, endpoint.recv()).await??;
        if inbound.peer.node_id != target {
            continue;
        }
        if config.require_signed {
            verify_frame(&public_key, &inbound.frame)?;
        }
        let envelope = ZapEnvelopeRef::parse(&inbound.frame.payload)
            .context("invalid capability response envelope")?;
        if envelope.kind() != ZapMessageKind::Control
            || envelope.subject() != CAPABILITY_RESPONSE_SUBJECT
        {
            continue;
        }
        let response: CapabilityResponse =
            serde_json::from_slice(envelope.body()).context("invalid capability response body")?;
        if response.advertisement.node_id != target {
            bail!(
                "capability response from {} advertised node_id {}",
                target,
                response.advertisement.node_id
            );
        }
        let cached_entry = match cache_path {
            Some(path) => Some(JsonlCapabilityCache::open(path).put(
                response.advertisement.node_id,
                response.advertisement.clone(),
            )?),
            None => None,
        };
        return Ok(CapabilityQueryCommandResult {
            response,
            cached_entry,
        });
    }
}

fn print_capability_query_result(
    result: &CapabilityQueryCommandResult,
    cache_path: Option<&Path>,
    json: bool,
) -> Result<()> {
    if json {
        if result.cached_entry.is_some() {
            println!("{}", serde_json::to_string_pretty(result)?);
        } else {
            println!("{}", serde_json::to_string_pretty(&result.response)?);
        }
    } else {
        println!("node_id={}", result.response.advertisement.node_id);
        for capability in result.response.advertisement.capabilities.iter() {
            println!("capability={capability}");
        }
        if let Some(entry) = &result.cached_entry {
            println!("cache_entry={}", entry.id);
            if let Some(path) = cache_path {
                println!("cache={}", path.display());
            }
        }
    }
    Ok(())
}

async fn capability_cache(command: CapabilityCacheCommand) -> Result<()> {
    match command {
        CapabilityCacheCommand::Refresh {
            config,
            path,
            peers,
            capabilities,
            timeout_ms,
            strict,
            json,
        } => {
            capability_cache_refresh(
                &config,
                path.as_deref(),
                &peers,
                &capabilities,
                timeout_ms,
                strict,
                json,
            )
            .await
        }
        CapabilityCacheCommand::List { path, peer, json } => {
            capability_cache_list(&path, peer, json)
        }
        CapabilityCacheCommand::Verify { path, json } => capability_cache_verify(&path, json),
    }
}

async fn capability_cache_refresh(
    config_path: &Path,
    cache_path_override: Option<&Path>,
    peers: &[Uuid],
    capabilities: &[String],
    timeout_ms: u64,
    strict: bool,
    json: bool,
) -> Result<()> {
    let config = ZapNodeConfig::from_path(config_path)?;
    config.validate()?;
    let cache_path = cache_path_override
        .map(Path::to_path_buf)
        .or_else(|| config.capability_cache.path.clone())
        .ok_or_else(|| {
            anyhow::anyhow!(
                "capability cache refresh requires --path or [capability_cache].path in {}",
                config_path.display()
            )
        })?;
    let requested = parse_capability_ids(capabilities)?;
    let selected_peers = select_refresh_peers(&config, peers)?;
    if selected_peers.is_empty() {
        bail!("no peers selected for capability cache refresh");
    }
    let keypair = load_keypair(&config.key_file)?;
    let endpoint = build_capability_endpoint(&config, &keypair).await?;
    let mut results = Vec::with_capacity(selected_peers.len());

    for peer in selected_peers {
        let peer_config = config
            .peers
            .iter()
            .find(|candidate| candidate.node_id == peer)
            .expect("selected peer came from config");
        if !peer_config.trust.allows_send() {
            results.push(CapabilityCacheRefreshPeerResult {
                peer,
                status: "skipped".to_string(),
                cache_entry: None,
                capabilities: 0,
                grants: 0,
                requirements: 0,
                error: Some("peer trust policy disallows outbound capability query".to_string()),
            });
            continue;
        }

        match query_capability_peer(
            &config,
            &endpoint,
            &keypair,
            peer,
            &requested,
            Some(&cache_path),
            timeout_ms,
        )
        .await
        {
            Ok(result) => {
                let advertisement = &result.response.advertisement;
                results.push(CapabilityCacheRefreshPeerResult {
                    peer,
                    status: "ok".to_string(),
                    cache_entry: result.cached_entry.as_ref().map(|entry| entry.id),
                    capabilities: advertisement.capabilities.capabilities.len(),
                    grants: advertisement.grants.len(),
                    requirements: advertisement.requirements.len(),
                    error: None,
                });
            }
            Err(error) => {
                results.push(CapabilityCacheRefreshPeerResult {
                    peer,
                    status: "failed".to_string(),
                    cache_entry: None,
                    capabilities: 0,
                    grants: 0,
                    requirements: 0,
                    error: Some(format!("{error:#}")),
                });
            }
        }
    }

    let refreshed = results
        .iter()
        .filter(|result| result.status == "ok")
        .count();
    let skipped = results
        .iter()
        .filter(|result| result.status == "skipped")
        .count();
    let failed = results
        .iter()
        .filter(|result| result.status == "failed")
        .count();
    let report = CapabilityCacheRefreshReport {
        config: config_path.display().to_string(),
        cache: cache_path.display().to_string(),
        requested_peer_count: results.len(),
        refreshed,
        skipped,
        failed,
        results,
    };
    print_capability_cache_refresh_report(&report, json)?;
    if strict && (report.failed > 0 || report.skipped > 0) {
        bail!(
            "capability cache refresh strict failed: refreshed={} skipped={} failed={}",
            report.refreshed,
            report.skipped,
            report.failed
        );
    }
    Ok(())
}

fn select_refresh_peers(config: &ZapNodeConfig, selected: &[Uuid]) -> Result<Vec<Uuid>> {
    let configured = config
        .peers
        .iter()
        .map(|peer| peer.node_id)
        .collect::<BTreeSet<_>>();
    let mut deduped = BTreeSet::new();
    let peers = if selected.is_empty() {
        config
            .peers
            .iter()
            .filter_map(|peer| deduped.insert(peer.node_id).then_some(peer.node_id))
            .collect::<Vec<_>>()
    } else {
        selected
            .iter()
            .map(|peer| {
                if !configured.contains(peer) {
                    bail!("selected peer {} is not configured", peer);
                }
                Ok(*peer)
            })
            .filter_map(|peer| match peer {
                Ok(peer) if deduped.insert(peer) => Some(Ok(peer)),
                Ok(_) => None,
                Err(error) => Some(Err(error)),
            })
            .collect::<Result<Vec<_>>>()?
    };
    Ok(peers)
}

fn print_capability_cache_refresh_report(
    report: &CapabilityCacheRefreshReport,
    json: bool,
) -> Result<()> {
    if json {
        println!("{}", serde_json::to_string_pretty(report)?);
        return Ok(());
    }
    println!("config={}", report.config);
    println!("cache={}", report.cache);
    println!("requested_peers={}", report.requested_peer_count);
    println!("refreshed={}", report.refreshed);
    println!("skipped={}", report.skipped);
    println!("failed={}", report.failed);
    for result in &report.results {
        println!(
            "peer={} status={} cache_entry={} capabilities={} grants={} requirements={} error={}",
            result.peer,
            result.status,
            result
                .cache_entry
                .map(|id| id.to_string())
                .unwrap_or_else(|| "none".to_string()),
            result.capabilities,
            result.grants,
            result.requirements,
            result.error.as_deref().unwrap_or("none")
        );
    }
    Ok(())
}

fn capability_cache_list(path: &Path, peer: Option<Uuid>, json: bool) -> Result<()> {
    let cache = JsonlCapabilityCache::open(path);
    let entries = match peer {
        Some(peer) => cache.latest_for_peer(peer)?.into_iter().collect::<Vec<_>>(),
        None => cache.entries()?,
    };
    if json {
        println!("{}", serde_json::to_string_pretty(&entries)?);
    } else {
        println!("cache={}", path.display());
        println!("entries={}", entries.len());
        for entry in entries {
            println!(
                "entry={} peer={} observed_at_micros={} capabilities={} grants={} requirements={}",
                entry.id,
                entry.peer_node_id,
                entry.observed_at_micros,
                entry.advertisement.capabilities.capabilities.len(),
                entry.advertisement.grants.len(),
                entry.advertisement.requirements.len()
            );
        }
    }
    Ok(())
}

fn capability_cache_verify(path: &Path, json: bool) -> Result<()> {
    let report = JsonlCapabilityCache::open(path).verify()?;
    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        println!("cache={}", path.display());
        println!("entries={}", report.entries);
        println!("peers={}", report.peers);
        println!("verified=true");
    }
    Ok(())
}

fn memory(command: MemoryCommand) -> Result<()> {
    match command {
        MemoryCommand::Put {
            path,
            namespace,
            subject,
            content_type,
            metadata,
            payload,
            payload_file,
            max_record_bytes,
            json,
        } => memory_put(MemoryPutCommand {
            path: &path,
            namespace,
            subject,
            content_type,
            metadata,
            payload,
            payload_file,
            max_record_bytes,
            json,
        }),
        MemoryCommand::Get { path, id, json } => memory_get(&path, id, json),
        MemoryCommand::Query {
            path,
            namespace,
            subject,
            content_type,
            include_tombstoned,
            limit,
            json,
        } => memory_query(
            &path,
            MemoryQuery {
                namespace,
                subject,
                content_type,
                include_tombstoned,
                limit,
            },
            json,
        ),
        MemoryCommand::Tombstone {
            path,
            id,
            reason,
            json,
        } => memory_tombstone(&path, id, reason, json),
        MemoryCommand::Verify { path, json } => memory_verify(&path, json),
        MemoryCommand::Prune {
            path,
            before_created_at_micros,
            out,
            force,
            json,
        } => memory_prune(&path, before_created_at_micros, &out, force, json),
    }
}

struct MemoryPutCommand<'a> {
    path: &'a Path,
    namespace: String,
    subject: String,
    content_type: String,
    metadata: Option<String>,
    payload: Option<String>,
    payload_file: Option<PathBuf>,
    max_record_bytes: usize,
    json: bool,
}

fn memory_put(options: MemoryPutCommand<'_>) -> Result<()> {
    let MemoryPutCommand {
        path,
        namespace,
        subject,
        content_type,
        metadata,
        payload,
        payload_file,
        max_record_bytes,
        json,
    } = options;
    let body = payload_bytes_from_input(payload, payload_file)?;
    let metadata = parse_metadata(metadata)?;
    let store = JsonlMemoryStore::open(path).with_max_record_bytes(max_record_bytes);
    let record = store.put(MemoryPut {
        namespace,
        subject,
        content_type,
        body,
        metadata,
        source_node: None,
        frame_hash: None,
    })?;
    if json {
        println!("{}", serde_json::to_string_pretty(&record)?);
    } else {
        println!("id={}", record.id);
        println!("namespace={}", record.namespace);
        println!("subject={}", record.subject);
        println!("body_hash={}", record.body_hash);
        println!("path={}", path.display());
    }
    Ok(())
}

fn memory_get(path: &Path, id: Uuid, json: bool) -> Result<()> {
    let store = JsonlMemoryStore::open(path);
    let record = store.get(id)?;
    if json {
        println!("{}", serde_json::to_string_pretty(&record)?);
    } else {
        println!("id={}", record.id);
        println!("namespace={}", record.namespace);
        println!("subject={}", record.subject);
        println!("content_type={}", record.content_type);
        println!("body_hash={}", record.body_hash);
        println!("created_at_micros={}", record.created_at_micros);
    }
    Ok(())
}

fn memory_query(path: &Path, query: MemoryQuery, json: bool) -> Result<()> {
    let store = JsonlMemoryStore::open(path);
    let records = store.query(&query)?;
    if json {
        println!("{}", serde_json::to_string_pretty(&records)?);
    } else {
        println!("records={}", records.len());
        for record in records {
            println!(
                "record={} namespace={} subject={} body_hash={}",
                record.id, record.namespace, record.subject, record.body_hash
            );
        }
    }
    Ok(())
}

fn memory_tombstone(path: &Path, id: Uuid, reason: Option<String>, json: bool) -> Result<()> {
    let store = JsonlMemoryStore::open(path);
    let tombstone = store.tombstone(id, reason)?;
    if json {
        println!("{}", serde_json::to_string_pretty(&tombstone)?);
    } else {
        println!("tombstone={}", tombstone.id);
        println!("record={}", tombstone.record_id);
        println!("namespace={}", tombstone.namespace);
    }
    Ok(())
}

fn memory_verify(path: &Path, json: bool) -> Result<()> {
    let store = JsonlMemoryStore::open(path);
    let report = store.verify()?;
    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        println!("path={}", report.path.display());
        println!("entries={}", report.entries);
        println!("records={}", report.records);
        println!("tombstones={}", report.tombstones);
        println!("verified=true");
    }
    Ok(())
}

fn memory_prune(
    path: &Path,
    before_created_at_micros: u64,
    out: &Path,
    force: bool,
    json: bool,
) -> Result<()> {
    let store = JsonlMemoryStore::open(path);
    let pruned = store.prune_to(before_created_at_micros, out, force)?;
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "path": path.display().to_string(),
                "out": out.display().to_string(),
                "before_created_at_micros": before_created_at_micros,
                "pruned_entries": pruned
            }))?
        );
    } else {
        println!("path={}", path.display());
        println!("out={}", out.display());
        println!("before_created_at_micros={before_created_at_micros}");
        println!("pruned_entries={pruned}");
    }
    Ok(())
}

fn parse_metadata(metadata: Option<String>) -> Result<serde_json::Value> {
    match metadata {
        Some(metadata) => {
            serde_json::from_str(&metadata).context("failed to parse --metadata JSON")
        }
        None => Ok(serde_json::Value::Null),
    }
}

fn route(command: RouteCommand) -> Result<()> {
    match command {
        RouteCommand::Explain {
            config,
            kind,
            subject,
            source_node,
            target_node,
            content_type,
            json,
        } => route_explain(
            &config,
            RouteMessage {
                source_node: source_node.unwrap_or_else(Uuid::nil),
                target_node: target_node.unwrap_or_else(Uuid::nil),
                kind,
                subject,
                content_type,
            },
            json,
        ),
    }
}

fn route_explain(config_path: &Path, message: RouteMessage, json: bool) -> Result<()> {
    let config = ZapNodeConfig::from_path(config_path)?;
    config.validate()?;
    let table = RouteTable::new(config.routes.clone())?;
    let explanation = table.explain(&message);
    if json {
        println!("{}", serde_json::to_string_pretty(&explanation)?);
    } else {
        println!("kind={}", explanation.message.kind);
        println!("subject={}", explanation.message.subject);
        println!("route_count={}", explanation.route_count);
        println!("reason={}", explanation.decision.reason);
        println!(
            "matched_rule={}",
            explanation
                .decision
                .matched_rule_name
                .as_deref()
                .unwrap_or("default")
        );
        println!("target={:?}", explanation.decision.target);
    }
    Ok(())
}

fn trust(command: TrustCommand) -> Result<()> {
    match command {
        TrustCommand::Enroll {
            node_id,
            addr,
            public_key,
            transport_key,
            transport_key_epoch,
            transport_key_rotated_at_micros,
            expires_at_micros,
            labels,
            out,
            force,
        } => trust_enroll(TrustEnrollOptions {
            node_id,
            addr,
            public_key,
            transport_key,
            transport_key_epoch,
            transport_key_rotated_at_micros,
            expires_at_micros,
            labels,
            out,
            force,
        }),
        TrustCommand::Inspect { config, json } => trust_inspect(&config, json),
    }
}

struct TrustEnrollOptions {
    node_id: Uuid,
    addr: String,
    public_key: String,
    transport_key: String,
    transport_key_epoch: Option<u64>,
    transport_key_rotated_at_micros: Option<u64>,
    expires_at_micros: Option<u64>,
    labels: Vec<String>,
    out: Option<PathBuf>,
    force: bool,
}

#[derive(Serialize)]
struct TrustEnrollDocument {
    peers: Vec<zap_node::PeerConfig>,
}

#[derive(Serialize)]
struct PeerTrustInspection {
    node_id: Uuid,
    addr: String,
    status: &'static str,
    allow_send: bool,
    allow_receive: bool,
    allow_forward: bool,
    allow_poa_attestation: bool,
    transport_key_epoch: Option<u64>,
    transport_key_rotated_at_micros: Option<u64>,
    expires_at_micros: Option<u64>,
    labels: Vec<String>,
}

fn trust_enroll(options: TrustEnrollOptions) -> Result<()> {
    let public_key = decode_public_key(&options.public_key)
        .with_context(|| format!("invalid public key for peer {}", options.node_id))?;
    if public_key.node_id() != options.node_id {
        bail!(
            "public_key derives node_id {}, but --node-id is {}",
            public_key.node_id(),
            options.node_id
        );
    }
    let transport_key = TransportKey::from_hex(&options.transport_key)?;
    if transport_key.0 == [0_u8; 32] {
        bail!("transport_key must not be all zeros");
    }
    if matches!(options.transport_key_epoch, Some(0)) {
        bail!("transport_key_epoch must be greater than zero");
    }
    validate_trust_labels(&options.labels)?;

    let document = TrustEnrollDocument {
        peers: vec![zap_node::PeerConfig {
            node_id: options.node_id,
            addr: options.addr,
            public_key: options.public_key,
            transport_key: options.transport_key,
            transport_key_epoch: options.transport_key_epoch,
            transport_key_rotated_at_micros: options.transport_key_rotated_at_micros,
            trust: PeerTrustConfig {
                expires_at_micros: options.expires_at_micros,
                labels: options.labels,
                ..PeerTrustConfig::default()
            },
        }],
    };
    let output = toml::to_string_pretty(&document)?;
    if let Some(out) = options.out {
        write_text_file(&out, &output, options.force)?;
        println!("trust_enrollment={}", out.display());
    } else {
        print!("{output}");
    }
    Ok(())
}

fn trust_inspect(config_path: &Path, json: bool) -> Result<()> {
    let config = ZapNodeConfig::from_path(config_path)?;
    let report = config.validate()?;
    let peers = config
        .peers
        .iter()
        .map(|peer| PeerTrustInspection {
            node_id: peer.node_id,
            addr: peer.addr.clone(),
            status: peer_trust_status_name(peer.trust.status),
            allow_send: peer.trust.allow_send,
            allow_receive: peer.trust.allow_receive,
            allow_forward: peer.trust.allow_forward,
            allow_poa_attestation: peer.trust.allow_poa_attestation,
            transport_key_epoch: peer.transport_key_epoch,
            transport_key_rotated_at_micros: peer.transport_key_rotated_at_micros,
            expires_at_micros: peer.trust.expires_at_micros,
            labels: peer.trust.labels.clone(),
        })
        .collect::<Vec<_>>();
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "config": config_path.display().to_string(),
                "peer_count": report.peer_count,
                "trusted_peer_count": report.trusted_peer_count,
                "restricted_peer_count": report.restricted_peer_count,
                "peer_send_enabled_count": report.peer_send_enabled_count,
                "peer_receive_enabled_count": report.peer_receive_enabled_count,
                "peer_forward_enabled_count": report.peer_forward_enabled_count,
                "peers": peers
            }))?
        );
    } else {
        println!("config={}", config_path.display());
        println!("peers={}", report.peer_count);
        println!("trusted_peers={}", report.trusted_peer_count);
        println!("restricted_peers={}", report.restricted_peer_count);
        for peer in peers {
            println!(
                "peer={} addr={} status={} send={} receive={} forward={} poa_attestation={} key_epoch={} expires_at_micros={} labels={}",
                peer.node_id,
                peer.addr,
                peer.status,
                peer.allow_send,
                peer.allow_receive,
                peer.allow_forward,
                peer.allow_poa_attestation,
                peer.transport_key_epoch
                    .map(|value| value.to_string())
                    .unwrap_or_else(|| "none".to_string()),
                peer.expires_at_micros
                    .map(|value| value.to_string())
                    .unwrap_or_else(|| "none".to_string()),
                if peer.labels.is_empty() {
                    "none".to_string()
                } else {
                    peer.labels.join(",")
                }
            );
        }
    }
    Ok(())
}

fn validate_trust_labels(labels: &[String]) -> Result<()> {
    let mut seen = BTreeSet::new();
    for label in labels {
        if label.trim().is_empty() {
            bail!("trust labels must not be empty");
        }
        if label.len() > 64 {
            bail!("trust label `{}` exceeds 64 bytes", label);
        }
        if label.chars().any(char::is_control) {
            bail!(
                "trust label `{}` must not contain control characters",
                label
            );
        }
        if !seen.insert(label) {
            bail!("duplicate trust label `{}`", label);
        }
    }
    Ok(())
}

fn peer_trust_status_name(status: PeerTrustStatus) -> &'static str {
    match status {
        PeerTrustStatus::Trusted => "trusted",
        PeerTrustStatus::Quarantined => "quarantined",
        PeerTrustStatus::Revoked => "revoked",
    }
}

const PEER_INVITE_SCHEMA_VERSION: u8 = 1;
const PEER_INVITE_SIGNATURE_DOMAIN: &[u8] = b"ZAP-PEER-INVITE-v1";

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PeerInvitePayload {
    schema_version: u8,
    node_id: Uuid,
    addr: String,
    public_key: String,
    transport_key: String,
    transport_key_epoch: u64,
    transport_key_rotated_at_micros: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    expires_at_micros: Option<u64>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    labels: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SignedPeerInvite {
    schema_version: u8,
    payload: PeerInvitePayload,
    signature: String,
}

#[derive(Debug, Serialize)]
struct PeerAcceptReport {
    peer: Uuid,
    addr: String,
    out: Option<String>,
    config: Option<String>,
    peer_block: Option<String>,
}

#[derive(Debug, Serialize)]
struct PeerMutationReport {
    peer: Uuid,
    out: Option<String>,
    config: String,
    transport_key_epoch: Option<u64>,
    transport_key_rotated_at_micros: Option<u64>,
    status: &'static str,
    updated_config: Option<String>,
}

fn peer(command: PeerCommand) -> Result<()> {
    match command {
        PeerCommand::Invite {
            config,
            addr,
            transport_key,
            transport_key_epoch,
            transport_key_rotated_at_micros,
            expires_at_micros,
            labels,
            out,
            force,
        } => peer_invite(PeerInviteOptions {
            config_path: &config,
            addr,
            transport_key,
            transport_key_epoch,
            transport_key_rotated_at_micros,
            expires_at_micros,
            labels,
            out,
            force,
        }),
        PeerCommand::Accept {
            invite,
            config,
            out,
            force,
            json,
        } => peer_accept(&invite, config.as_deref(), out.as_deref(), force, json),
        PeerCommand::Rotate {
            config,
            node_id,
            transport_key,
            transport_key_epoch,
            transport_key_rotated_at_micros,
            out,
            force,
            json,
        } => peer_rotate(PeerRotateOptions {
            config_path: &config,
            node_id,
            transport_key,
            transport_key_epoch,
            transport_key_rotated_at_micros,
            out,
            force,
            json,
        }),
        PeerCommand::Revoke {
            config,
            node_id,
            out,
            force,
            json,
        } => peer_revoke(&config, node_id, out.as_deref(), force, json),
    }
}

struct PeerInviteOptions<'a> {
    config_path: &'a Path,
    addr: Option<String>,
    transport_key: Option<String>,
    transport_key_epoch: Option<u64>,
    transport_key_rotated_at_micros: Option<u64>,
    expires_at_micros: Option<u64>,
    labels: Vec<String>,
    out: Option<PathBuf>,
    force: bool,
}

fn peer_invite(options: PeerInviteOptions<'_>) -> Result<()> {
    let config = ZapNodeConfig::from_path(options.config_path)?;
    let keypair = load_keypair(&config.key_file)?;
    let public_key = STANDARD_NO_PAD.encode(keypair.verifying_key().to_bytes());
    let node_id = keypair.node_id();
    let addr = options.addr.unwrap_or(config.bind);
    let transport_key = match options.transport_key {
        Some(key) => {
            validate_transport_key_hex(&key)?;
            key
        }
        None => random_transport_key_hex(),
    };
    let transport_key_epoch = options.transport_key_epoch.unwrap_or(1);
    if transport_key_epoch == 0 {
        bail!("transport_key_epoch must be greater than zero");
    }
    let transport_key_rotated_at_micros = options
        .transport_key_rotated_at_micros
        .unwrap_or(now_micros()?);
    validate_peer_expiry(options.expires_at_micros)?;
    validate_trust_labels(&options.labels)?;

    let payload = PeerInvitePayload {
        schema_version: PEER_INVITE_SCHEMA_VERSION,
        node_id,
        addr,
        public_key,
        transport_key,
        transport_key_epoch,
        transport_key_rotated_at_micros,
        expires_at_micros: options.expires_at_micros,
        labels: options.labels,
    };
    let payload_bytes = canonical_peer_invite_payload(&payload)?;
    let signature = keypair.sign_domain_message(PEER_INVITE_SIGNATURE_DOMAIN, &payload_bytes);
    let invite = SignedPeerInvite {
        schema_version: PEER_INVITE_SCHEMA_VERSION,
        payload,
        signature: STANDARD_NO_PAD.encode(signature),
    };
    let output = format!("{}\n", serde_json::to_string_pretty(&invite)?);
    if let Some(out) = options.out {
        write_text_file(&out, &output, options.force)?;
        println!("peer_invite={}", out.display());
    } else {
        print!("{output}");
    }
    Ok(())
}

fn peer_accept(
    invite_path: &Path,
    config_path: Option<&Path>,
    out: Option<&Path>,
    force: bool,
    json: bool,
) -> Result<()> {
    let invite = load_signed_peer_invite(invite_path)?;
    let peer = peer_config_from_invite(&invite)?;
    let mut peer_block = None;
    let mut output_config = None;
    let output = if let Some(config_path) = config_path {
        let mut config = load_raw_node_config(config_path)?;
        ensure_peer_absent(&config, peer.node_id)?;
        config.peers.push(peer.clone());
        let encoded = toml::to_string_pretty(&config)?;
        output_config = Some(config_path.display().to_string());
        encoded
    } else {
        let encoded = toml::to_string_pretty(&TrustEnrollDocument {
            peers: vec![peer.clone()],
        })?;
        peer_block = Some(encoded.clone());
        encoded
    };

    if let Some(out) = out {
        write_text_file(out, &output, force)?;
    } else if !json {
        print!("{output}");
    }

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&PeerAcceptReport {
                peer: peer.node_id,
                addr: peer.addr,
                out: out.map(|path| path.display().to_string()),
                config: output_config,
                peer_block,
            })?
        );
    } else if let Some(out) = out {
        println!("peer={}", peer.node_id);
        println!("out={}", out.display());
    }
    Ok(())
}

struct PeerRotateOptions<'a> {
    config_path: &'a Path,
    node_id: Uuid,
    transport_key: Option<String>,
    transport_key_epoch: Option<u64>,
    transport_key_rotated_at_micros: Option<u64>,
    out: Option<PathBuf>,
    force: bool,
    json: bool,
}

fn peer_rotate(options: PeerRotateOptions<'_>) -> Result<()> {
    let mut config = load_raw_node_config(options.config_path)?;
    let peer = config
        .peers
        .iter_mut()
        .find(|peer| peer.node_id == options.node_id)
        .with_context(|| format!("peer {} not found", options.node_id))?;
    let transport_key = match options.transport_key {
        Some(key) => {
            validate_transport_key_hex(&key)?;
            key
        }
        None => random_transport_key_hex(),
    };
    let transport_key_epoch = match options.transport_key_epoch {
        Some(0) => bail!("transport_key_epoch must be greater than zero"),
        Some(epoch) => epoch,
        None => peer
            .transport_key_epoch
            .unwrap_or(0)
            .saturating_add(1)
            .max(1),
    };
    let rotated_at = options
        .transport_key_rotated_at_micros
        .unwrap_or(now_micros()?);
    peer.transport_key = transport_key;
    peer.transport_key_epoch = Some(transport_key_epoch);
    peer.transport_key_rotated_at_micros = Some(rotated_at);
    let output = toml::to_string_pretty(&config)?;
    write_peer_config_mutation(
        &output,
        options.config_path,
        options.out.as_deref(),
        options.force,
        options.json,
        PeerMutationReport {
            peer: options.node_id,
            out: options.out.as_ref().map(|path| path.display().to_string()),
            config: options.config_path.display().to_string(),
            transport_key_epoch: Some(transport_key_epoch),
            transport_key_rotated_at_micros: Some(rotated_at),
            status: "trusted",
            updated_config: None,
        },
    )
}

fn peer_revoke(
    config_path: &Path,
    node_id: Uuid,
    out: Option<&Path>,
    force: bool,
    json: bool,
) -> Result<()> {
    let mut config = load_raw_node_config(config_path)?;
    let peer = config
        .peers
        .iter_mut()
        .find(|peer| peer.node_id == node_id)
        .with_context(|| format!("peer {} not found", node_id))?;
    peer.trust.status = PeerTrustStatus::Revoked;
    peer.trust.allow_send = false;
    peer.trust.allow_receive = false;
    peer.trust.allow_forward = false;
    peer.trust.allow_poa_attestation = false;
    let transport_key_epoch = peer.transport_key_epoch;
    let transport_key_rotated_at_micros = peer.transport_key_rotated_at_micros;
    let status = peer_trust_status_name(peer.trust.status);
    let output = toml::to_string_pretty(&config)?;
    write_peer_config_mutation(
        &output,
        config_path,
        out,
        force,
        json,
        PeerMutationReport {
            peer: node_id,
            out: out.map(|path| path.display().to_string()),
            config: config_path.display().to_string(),
            transport_key_epoch,
            transport_key_rotated_at_micros,
            status,
            updated_config: None,
        },
    )
}

fn write_peer_config_mutation(
    output: &str,
    config_path: &Path,
    out: Option<&Path>,
    force: bool,
    json: bool,
    mut report: PeerMutationReport,
) -> Result<()> {
    if let Some(out) = out {
        write_text_file(out, output, force)?;
    } else if json {
        report.updated_config = Some(output.to_string());
    } else {
        print!("{output}");
    }

    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else if let Some(out) = out {
        println!("peer={}", report.peer);
        println!("config={}", config_path.display());
        println!("out={}", out.display());
        println!("status={}", report.status);
    }
    Ok(())
}

fn load_signed_peer_invite(path: &Path) -> Result<SignedPeerInvite> {
    let input = fs::read_to_string(path)
        .with_context(|| format!("failed to read peer invite {}", path.display()))?;
    let invite: SignedPeerInvite = serde_json::from_str(&input)
        .with_context(|| format!("failed to parse peer invite {}", path.display()))?;
    verify_signed_peer_invite(&invite)?;
    Ok(invite)
}

fn verify_signed_peer_invite(invite: &SignedPeerInvite) -> Result<()> {
    if invite.schema_version != PEER_INVITE_SCHEMA_VERSION {
        bail!(
            "peer invite schema version {} is unsupported",
            invite.schema_version
        );
    }
    validate_peer_invite_payload(&invite.payload)?;
    let public_key = decode_public_key(&invite.payload.public_key)?;
    let signature = decode_signature(&invite.signature)?;
    public_key.verify_domain_message(
        PEER_INVITE_SIGNATURE_DOMAIN,
        &canonical_peer_invite_payload(&invite.payload)?,
        &signature,
    )?;
    Ok(())
}

fn validate_peer_invite_payload(payload: &PeerInvitePayload) -> Result<()> {
    if payload.schema_version != PEER_INVITE_SCHEMA_VERSION {
        bail!(
            "peer invite payload schema version {} is unsupported",
            payload.schema_version
        );
    }
    let public_key = decode_public_key(&payload.public_key)?;
    if public_key.node_id() != payload.node_id {
        bail!(
            "peer invite public_key derives node_id {}, but payload declares {}",
            public_key.node_id(),
            payload.node_id
        );
    }
    validate_transport_key_hex(&payload.transport_key)?;
    if payload.transport_key_epoch == 0 {
        bail!("transport_key_epoch must be greater than zero");
    }
    validate_peer_expiry(payload.expires_at_micros)?;
    validate_trust_labels(&payload.labels)?;
    Ok(())
}

fn peer_config_from_invite(invite: &SignedPeerInvite) -> Result<PeerConfig> {
    verify_signed_peer_invite(invite)?;
    Ok(PeerConfig {
        node_id: invite.payload.node_id,
        addr: invite.payload.addr.clone(),
        public_key: invite.payload.public_key.clone(),
        transport_key: invite.payload.transport_key.clone(),
        transport_key_epoch: Some(invite.payload.transport_key_epoch),
        transport_key_rotated_at_micros: Some(invite.payload.transport_key_rotated_at_micros),
        trust: PeerTrustConfig {
            expires_at_micros: invite.payload.expires_at_micros,
            labels: invite.payload.labels.clone(),
            ..PeerTrustConfig::default()
        },
    })
}

fn canonical_peer_invite_payload(payload: &PeerInvitePayload) -> Result<Vec<u8>> {
    Ok(serde_json::to_vec(payload)?)
}

fn validate_transport_key_hex(encoded: &str) -> Result<()> {
    let transport_key = TransportKey::from_hex(encoded)?;
    if transport_key.0 == [0_u8; 32] {
        bail!("transport_key must not be all zeros");
    }
    Ok(())
}

fn random_transport_key_hex() -> String {
    let mut key = [0_u8; 32];
    OsRng.fill_bytes(&mut key);
    hex::encode(key)
}

fn validate_peer_expiry(expires_at_micros: Option<u64>) -> Result<()> {
    if let Some(expires_at_micros) = expires_at_micros
        && expires_at_micros <= now_micros()?
    {
        bail!("peer invite expires_at_micros must be in the future");
    }
    Ok(())
}

fn load_raw_node_config(path: &Path) -> Result<ZapNodeConfig> {
    let input = fs::read_to_string(path)
        .with_context(|| format!("failed to read node config {}", path.display()))?;
    ZapNodeConfig::from_toml_str(&input)
        .with_context(|| format!("failed to parse node config {}", path.display()))
}

fn ensure_peer_absent(config: &ZapNodeConfig, node_id: Uuid) -> Result<()> {
    if config.peers.iter().any(|peer| peer.node_id == node_id) {
        bail!("peer {} already exists in config", node_id);
    }
    Ok(())
}

fn decode_signature(encoded: &str) -> Result<[u8; ED25519_SIGNATURE_LEN]> {
    let bytes = STANDARD_NO_PAD.decode(encoded)?;
    if bytes.len() != ED25519_SIGNATURE_LEN {
        bail!(
            "invalid signature length: expected {}, got {}",
            ED25519_SIGNATURE_LEN,
            bytes.len()
        );
    }
    Ok(bytes.try_into().unwrap())
}

fn schema(command: SchemaCommand) -> Result<()> {
    match command {
        SchemaCommand::Validate {
            contract,
            envelope,
            json,
        } => schema_validate(&contract, &envelope, json),
        SchemaCommand::Inspect { contract, json } => schema_inspect(&contract, json),
    }
}

fn schema_validate(contract_path: &Path, envelope_path: &Path, json: bool) -> Result<()> {
    let contract = load_message_contract(contract_path)?;
    let envelope_bytes = fs::read(envelope_path)
        .with_context(|| format!("failed to read envelope {}", envelope_path.display()))?;
    let envelope = ZapEnvelopeRef::parse(&envelope_bytes).context("invalid ZENV envelope")?;
    contract.validate_message(&MessageParts {
        kind: envelope.kind().as_str(),
        subject: envelope.subject(),
        content_type: Some(envelope.content_type()),
        metadata: envelope.metadata(),
        body: envelope.body(),
    })?;
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "contract": contract.contract_name(),
                "envelope": envelope_path.display().to_string(),
                "kind": envelope.kind().as_str(),
                "subject": envelope.subject(),
                "content_type": envelope.content_type(),
                "valid": true
            }))?
        );
    } else {
        println!("contract={}", contract.contract_name());
        println!("envelope={}", envelope_path.display());
        println!("kind={}", envelope.kind());
        println!("subject={}", envelope.subject());
        println!("valid=true");
    }
    Ok(())
}

fn schema_inspect(contract_path: &Path, json: bool) -> Result<()> {
    let contract = load_message_contract(contract_path)?;
    if json {
        println!("{}", serde_json::to_string_pretty(&contract)?);
    } else {
        println!("contract={}", contract.contract_name());
        println!("kind={}", contract.kind);
        println!("subject={}", contract.subject);
        println!(
            "content_type={}",
            contract.content_type.as_deref().unwrap_or("*")
        );
        println!("body_format={}", contract.body.format);
    }
    Ok(())
}

fn load_message_contract(path: &Path) -> Result<MessageContract> {
    let input = fs::read_to_string(path)
        .with_context(|| format!("failed to read message contract {}", path.display()))?;
    match path.extension().and_then(|extension| extension.to_str()) {
        Some(extension) if extension.eq_ignore_ascii_case("json") => {
            MessageContract::from_json_str(&input).map_err(Into::into)
        }
        _ => MessageContract::from_toml_str(&input).map_err(Into::into),
    }
}

fn policy(command: PolicyCommand) -> Result<()> {
    match command {
        PolicyCommand::Evaluate {
            policy,
            kind,
            subject,
            source_node,
            target_node,
            content_type,
            requires_consensus,
            grants,
            human_approved,
            simulation_passed,
            strict,
            json,
        } => policy_evaluate(PolicyEvaluateOptions {
            policy_path: &policy,
            kind: &kind,
            subject: &subject,
            source_node,
            target_node,
            content_type: content_type.as_deref(),
            requires_consensus,
            grants,
            human_approved,
            simulation_passed,
            strict,
            json,
        }),
    }
}

struct PolicyEvaluateOptions<'a> {
    policy_path: &'a Path,
    kind: &'a str,
    subject: &'a str,
    source_node: Option<Uuid>,
    target_node: Option<Uuid>,
    content_type: Option<&'a str>,
    requires_consensus: bool,
    grants: Vec<String>,
    human_approved: bool,
    simulation_passed: bool,
    strict: bool,
    json: bool,
}

fn policy_evaluate(options: PolicyEvaluateOptions<'_>) -> Result<()> {
    let input = fs::read_to_string(options.policy_path)
        .with_context(|| format!("failed to read policy {}", options.policy_path.display()))?;
    let policy = PolicySet::from_toml_str(&input)?;
    let granted_capabilities = options
        .grants
        .iter()
        .map(|grant| grant.parse::<CapabilityId>())
        .collect::<std::result::Result<BTreeSet<_>, _>>()?;
    let evaluation = policy.evaluate(&PolicyInput {
        kind: options.kind,
        subject: options.subject,
        source_node: options.source_node,
        target_node: options.target_node,
        content_type: options.content_type,
        consensus_protected: options.requires_consensus,
        granted_capabilities: &granted_capabilities,
        human_approved: options.human_approved,
        simulation_passed: options.simulation_passed,
    });
    if options.json {
        println!("{}", serde_json::to_string_pretty(&evaluation)?);
    } else {
        println!("decision={:?}", evaluation.decision);
        println!("allowed={}", evaluation.allowed);
        println!("reason={}", evaluation.reason);
        if let Some(capability) = &evaluation.required_capability {
            println!("required_capability={capability}");
        }
        println!("required_poa={}", evaluation.required_poa);
        println!(
            "human_approval_required={}",
            evaluation.human_approval_required
        );
        println!("simulation_required={}", evaluation.simulation_required);
    }
    if options.strict && !evaluation.allowed {
        bail!(
            "policy strict gate denied {} {}: {}",
            options.kind,
            options.subject,
            evaluation.reason
        );
    }
    Ok(())
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
            allow_emit_event,
            allow_memory_read,
            allow_memory_write,
            allow_device_call,
            max_host_call_bytes,
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
                emit_event: allow_emit_event,
                memory_read: allow_memory_read,
                memory_write: allow_memory_write,
                device_call: allow_device_call,
                max_host_call_bytes,
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

async fn registry(command: RegistryCommand) -> Result<()> {
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
        RegistryCommand::Pull {
            config,
            target,
            out,
            require_signature,
            operator_public_key,
            timeout_ms,
            force,
            json,
        } => {
            pull_registry(RegistryPullOptions {
                config_path: &config,
                target,
                out: &out,
                require_signature,
                operator_public_key,
                timeout_ms,
                force,
                json,
            })
            .await
        }
        RegistryCommand::Mirror {
            config,
            peers,
            out,
            require_signature,
            operator_public_key,
            timeout_ms,
            allow_partial,
            force,
            json,
        } => {
            mirror_registry(RegistryMirrorOptions {
                config_path: &config,
                peers,
                out: &out,
                require_signature,
                operator_public_key,
                timeout_ms,
                allow_partial,
                force,
                json,
            })
            .await
        }
        RegistryCommand::Publication { command } => registry_publication(command),
        RegistryCommand::Bundle { command } => registry_bundle(command),
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

struct RegistryPullOptions<'a> {
    config_path: &'a Path,
    target: Uuid,
    out: &'a Path,
    require_signature: bool,
    operator_public_key: Option<String>,
    timeout_ms: u64,
    force: bool,
    json: bool,
}

#[derive(Debug, Serialize)]
struct RegistryPullReport {
    peer: Uuid,
    out: String,
    entries: usize,
    signed: bool,
    operator_node_id: Option<Uuid>,
}

struct RegistryMirrorOptions<'a> {
    config_path: &'a Path,
    peers: Vec<Uuid>,
    out: &'a Path,
    require_signature: bool,
    operator_public_key: Option<String>,
    timeout_ms: u64,
    allow_partial: bool,
    force: bool,
    json: bool,
}

struct RegistryFetchOptions<'a> {
    target: Uuid,
    require_signature: bool,
    operator_public_key: Option<&'a str>,
    timeout_ms: u64,
    operation: &'a str,
}

struct RegistryPublicationCreateOptions<'a> {
    registry_path: &'a Path,
    publisher_key_path: &'a Path,
    out: &'a Path,
    published_at_micros: Option<u64>,
    channel: Option<String>,
    labels: Vec<String>,
    force: bool,
    json: bool,
}

struct RegistryBundleExportOptions<'a> {
    registry_path: &'a Path,
    publication_path: Option<&'a Path>,
    out: &'a Path,
    base_dir: Option<&'a Path>,
    drivers: Vec<String>,
    force: bool,
    json: bool,
}

struct RegistryBundleVerifyOptions<'a> {
    bundle: &'a Path,
    publisher_public_key: Option<&'a str>,
    require_drivers: bool,
}

struct RegistryBundleImportOptions<'a> {
    bundle: &'a Path,
    out: &'a Path,
    publisher_public_key: Option<&'a str>,
    require_drivers: bool,
    force: bool,
    json: bool,
}

#[derive(Debug, Serialize)]
struct RegistryMirrorReport {
    out: String,
    requested_peers: usize,
    mirrored_peers: usize,
    failed_peers: usize,
    entries: usize,
    added: usize,
    unchanged: usize,
    revoked_overrides: usize,
    requires_resign: bool,
    results: Vec<RegistryMirrorPeerReport>,
}

#[derive(Debug, Serialize)]
struct RegistryMirrorPeerReport {
    peer: Uuid,
    status: String,
    entries: Option<usize>,
    added: Option<usize>,
    unchanged: Option<usize>,
    revoked_overrides: Option<usize>,
    signed: Option<bool>,
    operator_node_id: Option<Uuid>,
    error: Option<String>,
}

#[derive(Debug, Serialize)]
struct RegistryPublicationReport {
    registry: String,
    publication: String,
    verified: bool,
    registry_hash: String,
    registry_entries: usize,
    registry_operator_node_id: Option<Uuid>,
    publisher_node_id: Uuid,
    published_at_micros: u64,
    channel: Option<String>,
    labels: Vec<String>,
}

#[derive(Debug, Serialize)]
struct RegistryBundleReport {
    bundle: String,
    verified: bool,
    registry_hash: String,
    publication_hash: Option<String>,
    entries: usize,
    manifests: usize,
    drivers: usize,
    imported_to: Option<String>,
}

async fn pull_registry(options: RegistryPullOptions<'_>) -> Result<()> {
    let RegistryPullOptions {
        config_path,
        target,
        out,
        require_signature,
        operator_public_key,
        timeout_ms,
        force,
        json,
    } = options;
    let config = ZapNodeConfig::from_path(config_path)?;
    config.validate()?;
    let keypair = load_keypair(&config.key_file)?;
    let endpoint = build_capability_endpoint(&config, &keypair).await?;
    let registry = fetch_registry_index_from_peer(
        &config,
        &endpoint,
        &keypair,
        RegistryFetchOptions {
            target,
            require_signature,
            operator_public_key: operator_public_key.as_deref(),
            timeout_ms,
            operation: "registry pull",
        },
    )
    .await?;
    let output = registry.to_toml_string()?;
    write_text_file(out, &output, force)?;
    let report = RegistryPullReport {
        peer: target,
        out: out.display().to_string(),
        entries: registry.entries.len(),
        signed: registry.signature.is_some(),
        operator_node_id: registry.operator_node_id,
    };
    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        println!("peer={}", report.peer);
        println!("out={}", report.out);
        println!("entries={}", report.entries);
        println!("signed={}", report.signed);
        println!(
            "operator_node_id={}",
            report
                .operator_node_id
                .map(|value| value.to_string())
                .unwrap_or_else(|| "none".to_string())
        );
    }
    Ok(())
}

async fn mirror_registry(options: RegistryMirrorOptions<'_>) -> Result<()> {
    let RegistryMirrorOptions {
        config_path,
        peers,
        out,
        require_signature,
        operator_public_key,
        timeout_ms,
        allow_partial,
        force,
        json,
    } = options;
    let config = ZapNodeConfig::from_path(config_path)?;
    config.validate()?;
    let keypair = load_keypair(&config.key_file)?;
    let targets = registry_mirror_targets(&config, &peers)?;
    let endpoint = build_capability_endpoint(&config, &keypair).await?;
    let mut merged = DriverRegistry::empty(Some(format!(
        "zap-cli {} registry mirror",
        env!("CARGO_PKG_VERSION")
    )));
    let mut total = DriverRegistryMergeReport::default();
    let mut results = Vec::with_capacity(targets.len());
    for target in &targets {
        let fetch_result = fetch_registry_index_from_peer(
            &config,
            &endpoint,
            &keypair,
            RegistryFetchOptions {
                target: *target,
                require_signature,
                operator_public_key: operator_public_key.as_deref(),
                timeout_ms,
                operation: "registry mirror",
            },
        )
        .await;
        match fetch_result {
            Ok(registry) => {
                let entries = registry.entries.len();
                let signed = registry.signature.is_some();
                let operator_node_id = registry.operator_node_id;
                match merged.merge_from(&registry) {
                    Ok(report) => {
                        total.added += report.added;
                        total.unchanged += report.unchanged;
                        total.revoked_overrides += report.revoked_overrides;
                        results.push(RegistryMirrorPeerReport {
                            peer: *target,
                            status: "ok".to_string(),
                            entries: Some(entries),
                            added: Some(report.added),
                            unchanged: Some(report.unchanged),
                            revoked_overrides: Some(report.revoked_overrides),
                            signed: Some(signed),
                            operator_node_id,
                            error: None,
                        });
                    }
                    Err(error) if allow_partial => {
                        results.push(RegistryMirrorPeerReport {
                            peer: *target,
                            status: "failed".to_string(),
                            entries: Some(entries),
                            added: None,
                            unchanged: None,
                            revoked_overrides: None,
                            signed: Some(signed),
                            operator_node_id,
                            error: Some(format!("failed to merge registry: {error:#}")),
                        });
                    }
                    Err(error) => {
                        return Err(error)
                            .with_context(|| format!("failed to merge registry from {}", target));
                    }
                }
            }
            Err(error) if allow_partial => {
                results.push(RegistryMirrorPeerReport {
                    peer: *target,
                    status: "failed".to_string(),
                    entries: None,
                    added: None,
                    unchanged: None,
                    revoked_overrides: None,
                    signed: None,
                    operator_node_id: None,
                    error: Some(format!("{error:#}")),
                });
            }
            Err(error) => {
                return Err(error)
                    .with_context(|| format!("failed to mirror registry from {}", target));
            }
        }
    }
    let mirrored_peers = results
        .iter()
        .filter(|result| result.status == "ok")
        .count();
    if mirrored_peers == 0 {
        bail!("no registry indexes mirrored successfully");
    }
    let output = merged.to_toml_string()?;
    write_text_file(out, &output, force)?;
    let report = RegistryMirrorReport {
        out: out.display().to_string(),
        requested_peers: targets.len(),
        mirrored_peers,
        failed_peers: results.len() - mirrored_peers,
        entries: merged.entries.len(),
        added: total.added,
        unchanged: total.unchanged,
        revoked_overrides: total.revoked_overrides,
        requires_resign: true,
        results,
    };
    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        println!("out={}", report.out);
        println!("requested_peers={}", report.requested_peers);
        println!("mirrored_peers={}", report.mirrored_peers);
        println!("failed_peers={}", report.failed_peers);
        println!("entries={}", report.entries);
        println!("added={}", report.added);
        println!("unchanged={}", report.unchanged);
        println!("revoked_overrides={}", report.revoked_overrides);
        println!("requires_resign={}", report.requires_resign);
        for result in &report.results {
            println!("peer={} status={}", result.peer, result.status);
        }
    }
    Ok(())
}

async fn fetch_registry_index_from_peer(
    config: &ZapNodeConfig,
    endpoint: &ZapEndpoint,
    keypair: &Keypair,
    options: RegistryFetchOptions<'_>,
) -> Result<DriverRegistry> {
    let RegistryFetchOptions {
        target,
        require_signature,
        operator_public_key,
        timeout_ms,
        operation,
    } = options;
    let target_peer = config
        .peers
        .iter()
        .find(|peer| peer.node_id == target)
        .with_context(|| format!("target {} not found in node config", target))?;
    if !target_peer.trust.allows_send() {
        bail!(
            "target {} is not permitted by local peer trust policy for {}",
            target,
            operation
        );
    }
    let request = RegistryIndexRequest {
        schema_version: zap_store::REGISTRY_INDEX_SYNC_SCHEMA_VERSION,
        require_signature: require_signature || operator_public_key.is_some(),
    };
    request.validate()?;

    let envelope = ZapEnvelope::new(
        ZapMessageKind::Control,
        REGISTRY_INDEX_REQUEST_SUBJECT,
        REGISTRY_INDEX_CONTENT_TYPE,
        Bytes::from(serde_json::to_vec(&request)?),
    )?;
    let frame = ZapFrame::new(
        keypair.node_id(),
        target,
        ZapFlags::ENCRYPTED,
        envelope.encode(),
    )?;
    let frame = sign_frame(keypair, &frame)?;
    endpoint.send_frame(target, &frame).await?;

    let public_key = decode_public_key(&target_peer.public_key)?;
    let deadline = Instant::now() + Duration::from_millis(timeout_ms);
    let response = loop {
        let now = Instant::now();
        if now >= deadline {
            bail!(
                "timed out waiting for registry index response from {}",
                target
            );
        }
        let remaining = deadline.saturating_duration_since(now);
        let inbound = tokio::time::timeout(remaining, endpoint.recv()).await??;
        if inbound.peer.node_id != target {
            continue;
        }
        if config.require_signed {
            verify_frame(&public_key, &inbound.frame)?;
        }
        let envelope = ZapEnvelopeRef::parse(&inbound.frame.payload)
            .context("invalid registry index response envelope")?;
        if envelope.kind() != ZapMessageKind::Control
            || envelope.subject() != REGISTRY_INDEX_RESPONSE_SUBJECT
        {
            continue;
        }
        let response: RegistryIndexResponse = serde_json::from_slice(envelope.body())
            .context("invalid registry index response body")?;
        if response.node_id != target {
            bail!(
                "registry index response from {} advertised node_id {}",
                target,
                response.node_id
            );
        }
        response
            .verify(request.require_signature, operator_public_key)
            .context("invalid registry index response")?;
        break response;
    };
    let RegistryIndexResponse {
        registry,
        unavailable_reason,
        ..
    } = response;
    registry.with_context(|| {
        format!(
            "peer {} did not return a registry index: {}",
            target,
            unavailable_reason.unwrap_or_else(|| "unavailable".to_string())
        )
    })
}

fn registry_mirror_targets(config: &ZapNodeConfig, requested: &[Uuid]) -> Result<Vec<Uuid>> {
    if requested.is_empty() {
        let peers: Vec<Uuid> = config
            .peers
            .iter()
            .filter(|peer| peer.trust.allows_send())
            .map(|peer| peer.node_id)
            .collect();
        if peers.is_empty() {
            bail!("node config has no send-allowed peers for registry mirror");
        }
        return Ok(peers);
    }
    let mut targets = Vec::with_capacity(requested.len());
    let mut seen = BTreeSet::new();
    for target in requested {
        if !seen.insert(*target) {
            bail!("duplicate registry mirror peer {}", target);
        }
        let peer = config
            .peers
            .iter()
            .find(|peer| peer.node_id == *target)
            .with_context(|| format!("target {} not found in node config", target))?;
        if !peer.trust.allows_send() {
            bail!(
                "target {} is not permitted by local peer trust policy for registry mirror",
                target
            );
        }
        targets.push(*target);
    }
    Ok(targets)
}

fn registry_publication(command: RegistryPublicationCommand) -> Result<()> {
    match command {
        RegistryPublicationCommand::Create {
            registry,
            publisher_key,
            out,
            published_at_micros,
            channel,
            labels,
            force,
            json,
        } => create_registry_publication(RegistryPublicationCreateOptions {
            registry_path: &registry,
            publisher_key_path: &publisher_key,
            out: &out,
            published_at_micros,
            channel,
            labels,
            force,
            json,
        }),
        RegistryPublicationCommand::Verify {
            registry,
            publication,
            publisher_public_key,
            json,
        } => verify_registry_publication(
            &registry,
            &publication,
            publisher_public_key.as_deref(),
            json,
        ),
    }
}

fn create_registry_publication(options: RegistryPublicationCreateOptions<'_>) -> Result<()> {
    let RegistryPublicationCreateOptions {
        registry_path,
        publisher_key_path,
        out,
        published_at_micros,
        channel,
        labels,
        force,
        json,
    } = options;
    let registry = load_driver_registry(registry_path)?;
    let publisher = load_keypair(publisher_key_path)?;
    let published_at_micros = match published_at_micros {
        Some(value) => value,
        None => now_micros()?,
    };
    let publication =
        RegistryPublication::new(&registry, &publisher, published_at_micros, channel, labels)?;
    publication.verify_for_registry(&registry, None)?;
    write_text_file(out, &format!("{}\n", publication.to_json_string()?), force)?;
    let report = registry_publication_report(registry_path, out, true, &publication);
    print_registry_publication_report(&report, json)
}

fn verify_registry_publication(
    registry_path: &Path,
    publication_path: &Path,
    publisher_public_key: Option<&str>,
    json: bool,
) -> Result<()> {
    let registry = load_driver_registry(registry_path)?;
    let publication = load_registry_publication(publication_path)?;
    publication.verify_for_registry(&registry, publisher_public_key)?;
    let report = registry_publication_report(registry_path, publication_path, true, &publication);
    print_registry_publication_report(&report, json)
}

fn registry_publication_report(
    registry_path: &Path,
    publication_path: &Path,
    verified: bool,
    publication: &RegistryPublication,
) -> RegistryPublicationReport {
    RegistryPublicationReport {
        registry: registry_path.display().to_string(),
        publication: publication_path.display().to_string(),
        verified,
        registry_hash: publication.registry_hash.clone(),
        registry_entries: publication.registry_entries,
        registry_operator_node_id: publication.registry_operator_node_id,
        publisher_node_id: publication.publisher_node_id,
        published_at_micros: publication.published_at_micros,
        channel: publication.channel.clone(),
        labels: publication.labels.clone(),
    }
}

fn print_registry_publication_report(report: &RegistryPublicationReport, json: bool) -> Result<()> {
    if json {
        println!("{}", serde_json::to_string_pretty(report)?);
    } else {
        println!("registry={}", report.registry);
        println!("publication={}", report.publication);
        println!("verified={}", report.verified);
        println!("registry_hash={}", report.registry_hash);
        println!("registry_entries={}", report.registry_entries);
        println!(
            "registry_operator_node_id={}",
            report
                .registry_operator_node_id
                .map(|value| value.to_string())
                .unwrap_or_else(|| "none".to_string())
        );
        println!("publisher_node_id={}", report.publisher_node_id);
        println!("published_at_micros={}", report.published_at_micros);
        println!("channel={}", report.channel.as_deref().unwrap_or("none"));
        println!(
            "labels={}",
            if report.labels.is_empty() {
                "none".to_string()
            } else {
                report.labels.join(",")
            }
        );
    }
    Ok(())
}

fn registry_bundle(command: RegistryBundleCommand) -> Result<()> {
    match command {
        RegistryBundleCommand::Export {
            registry,
            publication,
            out,
            base_dir,
            drivers,
            force,
            json,
        } => export_registry_bundle(RegistryBundleExportOptions {
            registry_path: &registry,
            publication_path: publication.as_deref(),
            out: &out,
            base_dir: base_dir.as_deref(),
            drivers,
            force,
            json,
        }),
        RegistryBundleCommand::Verify {
            bundle,
            publisher_public_key,
            require_drivers,
            json,
        } => {
            let report = verify_registry_bundle(RegistryBundleVerifyOptions {
                bundle: &bundle,
                publisher_public_key: publisher_public_key.as_deref(),
                require_drivers,
            })?;
            print_registry_bundle_report(&report, json)
        }
        RegistryBundleCommand::Import {
            bundle,
            out,
            publisher_public_key,
            require_drivers,
            force,
            json,
        } => import_registry_bundle(RegistryBundleImportOptions {
            bundle: &bundle,
            out: &out,
            publisher_public_key: publisher_public_key.as_deref(),
            require_drivers,
            force,
            json,
        }),
    }
}

fn export_registry_bundle(options: RegistryBundleExportOptions<'_>) -> Result<()> {
    let RegistryBundleExportOptions {
        registry_path,
        publication_path,
        out,
        base_dir,
        drivers,
        force,
        json,
    } = options;
    let registry = load_driver_registry(registry_path)?;
    registry.verify_signature()?;
    let registry_hash = zap_store::registry_hash(&registry)?;
    let registry_bytes = fs::read(registry_path)
        .with_context(|| format!("failed to read registry {}", registry_path.display()))?;
    let base_dir = base_dir.map(Path::to_path_buf).unwrap_or_else(|| {
        registry_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .to_path_buf()
    });
    let mut driver_map = parse_bundle_driver_specs(drivers)?;
    fs::create_dir_all(out)
        .with_context(|| format!("failed to create bundle directory {}", out.display()))?;

    let registry_rel = "registry.index.toml".to_string();
    write_bytes_file(&out.join(&registry_rel), &registry_bytes, force)?;

    let (publication_rel, publication_hash) = if let Some(path) = publication_path {
        let publication = load_registry_publication(path)?;
        publication.verify_for_registry(&registry, None)?;
        let bytes = fs::read(path)
            .with_context(|| format!("failed to read registry publication {}", path.display()))?;
        let rel = "registry.publication.json".to_string();
        write_bytes_file(&out.join(&rel), &bytes, force)?;
        (Some(rel), Some(artifact_hash(&bytes)))
    } else {
        (None, None)
    };

    let mut entries = Vec::with_capacity(registry.entries.len());
    for registry_entry in &registry.entries {
        let key = bundle_entry_key(&registry_entry.action, &registry_entry.version);
        let mut bundle_entry = RegistryBundleEntry::from_registry_entry(registry_entry);
        if let Some(manifest_path) = registry_entry.manifest_path.as_deref() {
            let manifest_source = resolve_source_path(&base_dir, Path::new(manifest_path));
            let manifest_bytes = fs::read(&manifest_source).with_context(|| {
                format!("failed to read manifest {}", manifest_source.display())
            })?;
            let manifest = DriverManifest::from_toml_str(
                std::str::from_utf8(&manifest_bytes).with_context(|| {
                    format!("manifest {} is not UTF-8", manifest_source.display())
                })?,
            )
            .with_context(|| format!("failed to parse manifest {}", manifest_source.display()))?;
            manifest.verify_static_and_signature().with_context(|| {
                format!(
                    "invalid signed driver manifest {}",
                    manifest_source.display()
                )
            })?;
            verify_manifest_matches_registry_entry(registry_entry, &manifest)?;
            let rel = format!(
                "manifests/{}-{}.manifest.toml",
                safe_bundle_component(&key),
                short_hash(&artifact_hash(&manifest_bytes))
            );
            write_bytes_file(&out.join(&rel), &manifest_bytes, force)?;
            bundle_entry.manifest_path = Some(rel);
            bundle_entry.manifest_hash = Some(artifact_hash(&manifest_bytes));
        }
        if let Some(driver_path) = driver_map.remove(&key) {
            let driver_bytes = fs::read(&driver_path)
                .with_context(|| format!("failed to read driver {}", driver_path.display()))?;
            let driver_hash = artifact_hash(&driver_bytes);
            if driver_hash != registry_entry.wasm_hash {
                bail!(
                    "driver {} hash {} does not match registry hash {} for {}",
                    driver_path.display(),
                    driver_hash,
                    registry_entry.wasm_hash,
                    key
                );
            }
            let extension = driver_path
                .extension()
                .and_then(|value| value.to_str())
                .filter(|value| !value.trim().is_empty())
                .unwrap_or("wasm");
            let rel = format!(
                "drivers/{}-{}.{}",
                safe_bundle_component(&key),
                short_hash(&driver_hash),
                safe_bundle_component(extension)
            );
            write_bytes_file(&out.join(&rel), &driver_bytes, force)?;
            bundle_entry.driver_path = Some(rel);
            bundle_entry.driver_hash = Some(driver_hash);
        }
        bundle_entry.validate()?;
        entries.push(bundle_entry);
    }
    if !driver_map.is_empty() {
        let extras = driver_map.keys().cloned().collect::<Vec<_>>().join(",");
        bail!("driver specs did not match registry entries: {extras}");
    }
    let manifest = RegistryBundleManifest::new(
        Some(format!("zap-cli {}", env!("CARGO_PKG_VERSION"))),
        registry_rel,
        registry_hash,
        publication_rel,
        publication_hash,
        entries,
    );
    manifest.validate()?;
    let manifest_rel = "zapstore.bundle.json";
    write_text_file(
        &out.join(manifest_rel),
        &format!("{}\n", manifest.to_json_string()?),
        force,
    )?;
    let report = verify_registry_bundle(RegistryBundleVerifyOptions {
        bundle: out,
        publisher_public_key: None,
        require_drivers: false,
    })?;
    print_registry_bundle_report(&report, json)
}

fn verify_registry_bundle(
    options: RegistryBundleVerifyOptions<'_>,
) -> Result<RegistryBundleReport> {
    let RegistryBundleVerifyOptions {
        bundle,
        publisher_public_key,
        require_drivers,
    } = options;
    let manifest = load_registry_bundle_manifest(bundle)?;
    let registry_path = resolve_bundle_path(bundle, &manifest.registry_path)?;
    let registry = load_driver_registry(&registry_path)?;
    registry.verify_signature()?;
    let actual_registry_hash = zap_store::registry_hash(&registry)?;
    if actual_registry_hash != manifest.registry_hash {
        bail!(
            "bundle registry hash mismatch: manifest {}, actual {}",
            manifest.registry_hash,
            actual_registry_hash
        );
    }
    if let Some(publication_rel) = manifest.publication_path.as_deref() {
        let publication_path = resolve_bundle_path(bundle, publication_rel)?;
        let publication_bytes = fs::read(&publication_path).with_context(|| {
            format!(
                "failed to read registry publication {}",
                publication_path.display()
            )
        })?;
        if let Some(expected_hash) = manifest.publication_hash.as_deref() {
            let actual_hash = artifact_hash(&publication_bytes);
            if actual_hash != expected_hash {
                bail!(
                    "bundle publication hash mismatch: manifest {}, actual {}",
                    expected_hash,
                    actual_hash
                );
            }
        }
        let publication = RegistryPublication::from_json_str(
            std::str::from_utf8(&publication_bytes).with_context(|| {
                format!("publication {} is not UTF-8", publication_path.display())
            })?,
        )
        .with_context(|| {
            format!(
                "failed to parse registry publication {}",
                publication_path.display()
            )
        })?;
        publication.verify_for_registry(&registry, publisher_public_key)?;
    }
    let mut manifest_count = 0;
    let mut driver_count = 0;
    for bundle_entry in &manifest.entries {
        bundle_entry.validate()?;
        let registry_entry = registry
            .entries
            .iter()
            .find(|entry| {
                entry.action == bundle_entry.action && entry.version == bundle_entry.version
            })
            .with_context(|| {
                format!(
                    "bundle entry {}@{} is missing from registry",
                    bundle_entry.action, bundle_entry.version
                )
            })?;
        verify_bundle_entry_matches_registry(bundle_entry, registry_entry)?;
        if let Some(manifest_rel) = bundle_entry.manifest_path.as_deref() {
            let path = resolve_bundle_path(bundle, manifest_rel)?;
            let bytes = fs::read(&path)
                .with_context(|| format!("failed to read manifest {}", path.display()))?;
            if let Some(expected_hash) = bundle_entry.manifest_hash.as_deref() {
                let actual_hash = artifact_hash(&bytes);
                if actual_hash != expected_hash {
                    bail!(
                        "bundle manifest hash mismatch for {}@{}: manifest {}, actual {}",
                        bundle_entry.action,
                        bundle_entry.version,
                        expected_hash,
                        actual_hash
                    );
                }
            }
            let driver_manifest = DriverManifest::from_toml_str(
                std::str::from_utf8(&bytes)
                    .with_context(|| format!("manifest {} is not UTF-8", path.display()))?,
            )
            .with_context(|| format!("failed to parse manifest {}", path.display()))?;
            driver_manifest
                .verify_static_and_signature()
                .with_context(|| format!("invalid signed driver manifest {}", path.display()))?;
            verify_manifest_matches_registry_entry(registry_entry, &driver_manifest)?;
            manifest_count += 1;
        }
        if let Some(driver_rel) = bundle_entry.driver_path.as_deref() {
            let path = resolve_bundle_path(bundle, driver_rel)?;
            let bytes = fs::read(&path)
                .with_context(|| format!("failed to read driver {}", path.display()))?;
            let actual_hash = artifact_hash(&bytes);
            if Some(actual_hash.as_str()) != bundle_entry.driver_hash.as_deref() {
                bail!(
                    "bundle driver hash mismatch for {}@{}",
                    bundle_entry.action,
                    bundle_entry.version
                );
            }
            if actual_hash != registry_entry.wasm_hash {
                bail!(
                    "bundle driver hash does not match registry for {}@{}",
                    bundle_entry.action,
                    bundle_entry.version
                );
            }
            driver_count += 1;
        } else if require_drivers {
            bail!(
                "bundle entry {}@{} has no driver artifact",
                bundle_entry.action,
                bundle_entry.version
            );
        }
    }
    Ok(RegistryBundleReport {
        bundle: bundle.display().to_string(),
        verified: true,
        registry_hash: manifest.registry_hash,
        publication_hash: manifest.publication_hash,
        entries: manifest.entries.len(),
        manifests: manifest_count,
        drivers: driver_count,
        imported_to: None,
    })
}

fn import_registry_bundle(options: RegistryBundleImportOptions<'_>) -> Result<()> {
    let RegistryBundleImportOptions {
        bundle,
        out,
        publisher_public_key,
        require_drivers,
        force,
        json,
    } = options;
    let mut report = verify_registry_bundle(RegistryBundleVerifyOptions {
        bundle,
        publisher_public_key,
        require_drivers,
    })?;
    let manifest = load_registry_bundle_manifest(bundle)?;
    copy_bundle_file(bundle, &manifest.registry_path, out, force)?;
    if let Some(publication_path) = manifest.publication_path.as_deref() {
        copy_bundle_file(bundle, publication_path, out, force)?;
    }
    copy_bundle_file(bundle, "zapstore.bundle.json", out, force)?;
    for entry in &manifest.entries {
        if let Some(manifest_path) = entry.manifest_path.as_deref() {
            copy_bundle_file(bundle, manifest_path, out, force)?;
        }
        if let Some(driver_path) = entry.driver_path.as_deref() {
            copy_bundle_file(bundle, driver_path, out, force)?;
        }
    }
    report.imported_to = Some(out.display().to_string());
    print_registry_bundle_report(&report, json)
}

fn print_registry_bundle_report(report: &RegistryBundleReport, json: bool) -> Result<()> {
    if json {
        println!("{}", serde_json::to_string_pretty(report)?);
    } else {
        println!("bundle={}", report.bundle);
        println!("verified={}", report.verified);
        println!("registry_hash={}", report.registry_hash);
        println!(
            "publication_hash={}",
            report.publication_hash.as_deref().unwrap_or("none")
        );
        println!("entries={}", report.entries);
        println!("manifests={}", report.manifests);
        println!("drivers={}", report.drivers);
        if let Some(imported_to) = &report.imported_to {
            println!("imported_to={imported_to}");
        }
    }
    Ok(())
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

fn load_registry_bundle_manifest(bundle: &Path) -> Result<RegistryBundleManifest> {
    let path = bundle.join("zapstore.bundle.json");
    let manifest = RegistryBundleManifest::from_json_str(
        &fs::read_to_string(&path)
            .with_context(|| format!("failed to read bundle manifest {}", path.display()))?,
    )
    .with_context(|| format!("failed to parse bundle manifest {}", path.display()))?;
    manifest
        .validate()
        .with_context(|| format!("invalid bundle manifest {}", path.display()))?;
    Ok(manifest)
}

fn parse_bundle_driver_specs(specs: Vec<String>) -> Result<BTreeMap<String, PathBuf>> {
    let mut drivers = BTreeMap::new();
    for spec in specs {
        let (key, path) = spec
            .split_once('=')
            .with_context(|| format!("invalid --driver `{spec}`; expected action@version=path"))?;
        let (action, version) = key
            .split_once('@')
            .with_context(|| format!("invalid --driver `{spec}`; expected action@version=path"))?;
        if action.trim().is_empty() || version.trim().is_empty() || path.trim().is_empty() {
            bail!("invalid --driver `{spec}`; action, version, and path are required");
        }
        let key = bundle_entry_key(action, version);
        if drivers.insert(key.clone(), PathBuf::from(path)).is_some() {
            bail!("duplicate --driver for {key}");
        }
    }
    Ok(drivers)
}

fn bundle_entry_key(action: &str, version: &str) -> String {
    format!("{action}@{version}")
}

fn safe_bundle_component(value: &str) -> String {
    let mut output = String::with_capacity(value.len());
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_') {
            output.push(byte as char);
        } else {
            output.push('_');
        }
    }
    if output.is_empty() {
        "artifact".to_string()
    } else {
        output
    }
}

fn short_hash(hash: &str) -> String {
    hash.strip_prefix("blake3:")
        .unwrap_or(hash)
        .chars()
        .take(12)
        .collect()
}

fn resolve_source_path(base_dir: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        base_dir.join(path)
    }
}

fn resolve_bundle_path(bundle: &Path, rel: &str) -> Result<PathBuf> {
    ensure_safe_relative_path(rel)?;
    Ok(bundle.join(rel))
}

fn ensure_safe_relative_path(rel: &str) -> Result<()> {
    let path = Path::new(rel);
    if path.is_absolute() || path.as_os_str().is_empty() {
        bail!("bundle path `{rel}` is not a safe relative path");
    }
    for component in path.components() {
        match component {
            std::path::Component::Normal(_) => {}
            _ => bail!("bundle path `{rel}` is not a safe relative path"),
        }
    }
    Ok(())
}

fn copy_bundle_file(bundle: &Path, rel: &str, out: &Path, force: bool) -> Result<()> {
    let source = resolve_bundle_path(bundle, rel)?;
    let bytes =
        fs::read(&source).with_context(|| format!("failed to read {}", source.display()))?;
    write_bytes_file(&out.join(rel), &bytes, force)
}

fn verify_manifest_matches_registry_entry(
    entry: &zap_store::DriverRegistryEntry,
    manifest: &DriverManifest,
) -> Result<()> {
    if entry.action != manifest.action
        || entry.version != manifest.version
        || entry.name != manifest.name
        || entry.abi_version != manifest.abi_version
        || entry.wasm_hash != manifest.wasm_hash
        || entry.author_node_id != manifest.author_node_id
    {
        bail!(
            "manifest for {}@{} does not match registry entry",
            entry.action,
            entry.version
        );
    }
    Ok(())
}

fn verify_bundle_entry_matches_registry(
    bundle_entry: &RegistryBundleEntry,
    registry_entry: &zap_store::DriverRegistryEntry,
) -> Result<()> {
    if bundle_entry.name != registry_entry.name
        || bundle_entry.abi_version != registry_entry.abi_version
        || bundle_entry.wasm_hash != registry_entry.wasm_hash
        || bundle_entry.author_node_id != registry_entry.author_node_id
        || bundle_entry.status != registry_entry.status
    {
        bail!(
            "bundle entry {}@{} does not match registry metadata",
            bundle_entry.action,
            bundle_entry.version
        );
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

fn load_registry_publication(path: &Path) -> Result<RegistryPublication> {
    RegistryPublication::from_json_str(
        &fs::read_to_string(path)
            .with_context(|| format!("failed to read registry publication {}", path.display()))?,
    )
    .with_context(|| format!("failed to parse registry publication {}", path.display()))
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
    write_bytes_file(out, contents.as_bytes(), force)
}

fn write_bytes_file(out: &Path, contents: &[u8], force: bool) -> Result<()> {
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
    file.write_all(contents)
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
