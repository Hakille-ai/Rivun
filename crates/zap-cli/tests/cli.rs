use base64::{Engine as _, engine::general_purpose::STANDARD_NO_PAD};
use bytes::Bytes;
use std::{net::UdpSocket, process::Command};
use tempfile::tempdir;
use tokio::time::{Duration, timeout};
use uuid::Uuid;
use zap_core::{ZapFlags, ZapFrame};
use zap_crypto::{Keypair, sign_frame, verify_frame, verify_poa_certificate};
use zap_envelope::{ZapEnvelope, ZapEnvelopeRef, ZapMessageKind};
use zap_ledger::SignedActionReceipt;
use zap_net::{Peer, ZapEndpoint, ZapEndpointConfig};
use zap_node::{ZapNode, ZapNodeConfig};
use zap_store::{DriverManifest, DriverRegistry, DriverRegistryStatus};

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

fn free_udp_addr() -> String {
    let socket = UdpSocket::bind("127.0.0.1:0").unwrap();
    socket.local_addr().unwrap().to_string()
}

fn public_key_string(keypair: &Keypair) -> String {
    STANDARD_NO_PAD.encode(keypair.verifying_key().to_bytes())
}

fn echo_driver_wat() -> &'static str {
    r#"
    (module
      (memory (export "memory") 1)
      (global $heap (mut i32) (i32.const 1024))
      (func (export "zap_alloc") (param $len i32) (result i32)
        global.get $heap
        global.get $heap
        local.get $len
        i32.add
        global.set $heap)
      (func (export "zap_dealloc") (param i32 i32))
      (func (export "zap_execute")
        (param $action_ptr i32) (param $action_len i32)
        (param $payload_ptr i32) (param $payload_len i32)
        (result i64)
        local.get $payload_ptr
        i64.extend_i32_u
        i64.const 32
        i64.shl
        local.get $payload_len
        i64.extend_i32_u
        i64.or))
    "#
}

fn missing_execute_driver_wat() -> &'static str {
    r#"
    (module
      (memory (export "memory") 1)
      (func (export "zap_alloc") (param i32) (result i32) i32.const 0)
      (func (export "zap_dealloc") (param i32 i32)))
    "#
}

fn write_config(
    dir: &tempfile::TempDir,
    local_key: &Keypair,
    peer_key: &Keypair,
    peer_public_key: String,
) -> std::path::PathBuf {
    let key_path = dir.path().join("node.key");
    let driver_path = dir.path().join("echo.wat");
    let config_path = dir.path().join("node.toml");
    std::fs::write(&key_path, local_key.to_key_file_toml().unwrap()).unwrap();
    std::fs::write(&driver_path, echo_driver_wat()).unwrap();
    std::fs::write(
        &config_path,
        format!(
            r#"
bind = '127.0.0.1:0'
key_file = '{}'
require_signed = true

[[peers]]
node_id = '{}'
addr = '127.0.0.1:9'
public_key = '{}'
transport_key = '4242424242424242424242424242424242424242424242424242424242424242'

[[drivers]]
action = 'echo'
path = '{}'
"#,
            key_path.display(),
            peer_key.node_id(),
            peer_public_key,
            driver_path.display(),
        ),
    )
    .unwrap();
    config_path
}

#[tokio::test]
async fn send_uses_configured_bind_address_and_sends_signed_frame() {
    let sender = Keypair::generate();
    let receiver = Keypair::generate();
    let transport_key = [0x42_u8; 32];
    let receiver_endpoint = ZapEndpoint::bind(ZapEndpointConfig::new(
        "127.0.0.1:0".parse().unwrap(),
        receiver.node_id(),
    ))
    .await
    .unwrap();
    let sender_addr = free_udp_addr();
    receiver_endpoint
        .add_peer(Peer::new(
            sender.node_id(),
            sender_addr.parse().unwrap(),
            transport_key,
        ))
        .await;

    let dir = tempdir().unwrap();
    let key_path = dir.path().join("sender.key");
    std::fs::write(&key_path, sender.to_key_file_toml().unwrap()).unwrap();
    let config_path = dir.path().join("sender.toml");
    std::fs::write(
        &config_path,
        format!(
            r#"
bind = '{sender_addr}'
key_file = '{}'
require_signed = true

[[peers]]
node_id = '{}'
addr = '{}'
public_key = '{}'
transport_key = '{}'
"#,
            key_path.display(),
            receiver.node_id(),
            receiver_endpoint.local_addr().unwrap(),
            public_key_string(&receiver),
            hex_transport_key(transport_key),
        ),
    )
    .unwrap();

    let binary = env!("CARGO_BIN_EXE_zap");
    let config_arg = config_path.clone();
    let target = receiver.node_id().to_string();
    let output = tokio::task::spawn_blocking(move || {
        Command::new(binary)
            .args([
                "send",
                "--config",
                config_arg.to_str().unwrap(),
                "--target",
                &target,
                "--payload",
                "hello-from-cli",
            ])
            .output()
    })
    .await
    .unwrap()
    .unwrap();

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let inbound = timeout(Duration::from_secs(2), receiver_endpoint.recv())
        .await
        .unwrap()
        .unwrap();
    verify_frame(&sender.verifying_key(), &inbound.frame).unwrap();
    assert_eq!(inbound.from_addr.to_string(), sender_addr);
    assert_eq!(inbound.frame.payload.as_ref(), b"hello-from-cli");
}

#[test]
fn send_rejects_invalid_config_before_network_send() {
    let dir = tempdir().unwrap();
    let local = Keypair::generate();
    let peer = Keypair::generate();
    let wrong_peer = Keypair::generate();
    let config_path = write_config(&dir, &local, &peer, public_key_string(&wrong_peer));
    let output = Command::new(env!("CARGO_BIN_EXE_zap"))
        .args([
            "send",
            "--config",
            config_path.to_str().unwrap(),
            "--target",
            &peer.node_id().to_string(),
            "--payload",
            "hello",
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("peer public_key derives node_id"),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn send_rejects_target_when_peer_trust_disallows_send() {
    let dir = tempdir().unwrap();
    let local = Keypair::generate();
    let peer = Keypair::generate();
    let config_path = write_config(&dir, &local, &peer, public_key_string(&peer));
    let mut config_text = std::fs::read_to_string(&config_path).unwrap();
    config_text = config_text.replace(
        "\n[[drivers]]",
        r#"
[peers.trust]
allow_send = false

[[drivers]]"#,
    );
    std::fs::write(&config_path, config_text).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_zap"))
        .args([
            "send",
            "--config",
            config_path.to_str().unwrap(),
            "--target",
            &peer.node_id().to_string(),
            "--payload",
            "hello",
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("peer trust policy"),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn schema_validate_accepts_matching_universal_envelope() {
    let dir = tempdir().unwrap();
    let contract_path = dir.path().join("echo.contract.toml");
    let envelope_path = dir.path().join("echo.zenv");
    std::fs::write(
        &contract_path,
        r#"
schema_version = 1
name = "echo contract"
kind = "action"
subject = "echo"
content_type = "application/json"

[body]
format = "json_object"
required_json_fields = ["message"]
"#,
    )
    .unwrap();
    let envelope = ZapEnvelope::new(
        ZapMessageKind::Action,
        "echo",
        "application/json",
        Bytes::from_static(br#"{"message":"hello"}"#),
    )
    .unwrap()
    .encode();
    std::fs::write(&envelope_path, envelope).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_zap"))
        .args([
            "schema",
            "validate",
            "--contract",
            contract_path.to_str().unwrap(),
            "--envelope",
            envelope_path.to_str().unwrap(),
            "--json",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["valid"], true);
    assert_eq!(json["subject"], "echo");
}

#[test]
fn policy_evaluate_strict_enforces_required_poa() {
    let dir = tempdir().unwrap();
    let policy_path = dir.path().join("policy.toml");
    std::fs::write(
        &policy_path,
        r#"
[[rules]]
name = "safety quorum"
kind = "action"
subject = "safety.*"
decision = "require_poa"
reason = "safety actions require quorum"
"#,
    )
    .unwrap();

    let denied = Command::new(env!("CARGO_BIN_EXE_zap"))
        .args([
            "policy",
            "evaluate",
            "--policy",
            policy_path.to_str().unwrap(),
            "--kind",
            "action",
            "--subject",
            "safety.emergency_stop",
            "--strict",
        ])
        .output()
        .unwrap();
    assert!(!denied.status.success());
    assert!(
        String::from_utf8_lossy(&denied.stderr).contains("policy strict gate denied"),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&denied.stdout),
        String::from_utf8_lossy(&denied.stderr)
    );

    let allowed = Command::new(env!("CARGO_BIN_EXE_zap"))
        .args([
            "policy",
            "evaluate",
            "--policy",
            policy_path.to_str().unwrap(),
            "--kind",
            "action",
            "--subject",
            "safety.emergency_stop",
            "--requires-consensus",
            "--strict",
            "--json",
        ])
        .output()
        .unwrap();
    assert!(
        allowed.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&allowed.stdout),
        String::from_utf8_lossy(&allowed.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&allowed.stdout).unwrap();
    assert_eq!(json["allowed"], true);
    assert_eq!(json["required_poa"], true);
}

fn hex_transport_key(key: [u8; 32]) -> String {
    key.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn write_sender_config(
    dir: &tempfile::TempDir,
    sender: &Keypair,
    receiver: &Keypair,
    receiver_addr: std::net::SocketAddr,
    sender_addr: &str,
    transport_key: [u8; 32],
) -> std::path::PathBuf {
    let key_path = dir.path().join("sender.key");
    let config_path = dir.path().join("sender.toml");
    std::fs::write(&key_path, sender.to_key_file_toml().unwrap()).unwrap();
    std::fs::write(
        &config_path,
        format!(
            r#"
bind = '{sender_addr}'
key_file = '{}'
require_signed = true

[[peers]]
node_id = '{}'
addr = '{}'
public_key = '{}'
transport_key = '{}'
"#,
            key_path.display(),
            receiver.node_id(),
            receiver_addr,
            public_key_string(receiver),
            hex_transport_key(transport_key),
        ),
    )
    .unwrap();
    config_path
}

#[test]
fn keygen_writes_parseable_key_file() {
    let dir = tempdir().unwrap();
    let key_path = dir.path().join("node.key");
    let output = Command::new(env!("CARGO_BIN_EXE_zap"))
        .args(["keygen", "--out", key_path.to_str().unwrap()])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let key = std::fs::read_to_string(key_path).unwrap();
    let keypair = Keypair::from_key_file_toml(&key).unwrap();
    assert_ne!(keypair.node_id(), Uuid::nil());
}

#[cfg(unix)]
#[test]
fn keygen_writes_owner_only_key_file_permissions() {
    let dir = tempdir().unwrap();
    let key_path = dir.path().join("node.key");
    let output = Command::new(env!("CARGO_BIN_EXE_zap"))
        .args(["keygen", "--out", key_path.to_str().unwrap()])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let mode = std::fs::metadata(&key_path).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o600);
}

#[test]
fn keygen_refuses_to_overwrite_existing_key_without_force() {
    let dir = tempdir().unwrap();
    let key_path = dir.path().join("node.key");
    std::fs::write(&key_path, "existing-key").unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_zap"))
        .args(["keygen", "--out", key_path.to_str().unwrap()])
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert_eq!(std::fs::read_to_string(&key_path).unwrap(), "existing-key");
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("refusing to overwrite existing key file"),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn keygen_force_overwrites_existing_key() {
    let dir = tempdir().unwrap();
    let key_path = dir.path().join("node.key");
    std::fs::write(&key_path, "existing-key").unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_zap"))
        .args(["keygen", "--out", key_path.to_str().unwrap(), "--force"])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let key = std::fs::read_to_string(key_path).unwrap();
    let keypair = Keypair::from_key_file_toml(&key).unwrap();
    assert_ne!(keypair.node_id(), Uuid::nil());
}

#[test]
fn driver_manifest_create_and_verify_round_trip() {
    let dir = tempdir().unwrap();
    let author = Keypair::generate();
    let author_key_path = dir.path().join("author.key");
    let driver_path = dir.path().join("echo.wat");
    let manifest_path = dir.path().join("echo.manifest.toml");
    std::fs::write(&author_key_path, author.to_key_file_toml().unwrap()).unwrap();
    std::fs::write(&driver_path, echo_driver_wat()).unwrap();

    let create = Command::new(env!("CARGO_BIN_EXE_zap"))
        .args([
            "driver-manifest",
            "create",
            "--driver",
            driver_path.to_str().unwrap(),
            "--action",
            "echo",
            "--author-key",
            author_key_path.to_str().unwrap(),
            "--out",
            manifest_path.to_str().unwrap(),
            "--allow-emit-event",
            "--allow-memory-write",
            "--allow-device-call",
            "--max-host-call-bytes",
            "8192",
        ])
        .output()
        .unwrap();

    assert!(
        create.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&create.stdout),
        String::from_utf8_lossy(&create.stderr)
    );
    let manifest =
        DriverManifest::from_toml_str(&std::fs::read_to_string(&manifest_path).unwrap()).unwrap();
    manifest
        .verify_for_driver("echo", echo_driver_wat().as_bytes())
        .unwrap();
    assert_eq!(manifest.author_node_id, author.node_id());
    assert!(manifest.permissions.emit_event);
    assert!(manifest.permissions.memory_write);
    assert!(manifest.permissions.device_call);
    assert_eq!(manifest.permissions.max_host_call_bytes, 8192);

    let verify = Command::new(env!("CARGO_BIN_EXE_zap"))
        .args([
            "driver-manifest",
            "verify",
            "--driver",
            driver_path.to_str().unwrap(),
            "--manifest",
            manifest_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert!(
        verify.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&verify.stdout),
        String::from_utf8_lossy(&verify.stderr)
    );
    assert!(String::from_utf8_lossy(&verify.stdout).contains("ok"));
}

#[test]
fn registry_init_add_list_and_verify_round_trip() {
    let dir = tempdir().unwrap();
    let author = Keypair::generate();
    let manifest_path = dir.path().join("echo.manifest.toml");
    let registry_path = dir.path().join("registry.index.toml");
    let manifest = DriverManifest::new(
        "echo-driver",
        "0.1.0",
        "echo",
        echo_driver_wat().as_bytes(),
        zap_runtime::DriverPermissions::none(),
        None,
        &author,
    )
    .unwrap();
    std::fs::write(&manifest_path, manifest.to_toml_string().unwrap()).unwrap();

    let init = Command::new(env!("CARGO_BIN_EXE_zap"))
        .args(["registry", "init", "--out", registry_path.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(
        init.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&init.stdout),
        String::from_utf8_lossy(&init.stderr)
    );

    let add = Command::new(env!("CARGO_BIN_EXE_zap"))
        .args([
            "registry",
            "add",
            "--registry",
            registry_path.to_str().unwrap(),
            "--manifest",
            manifest_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        add.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&add.stdout),
        String::from_utf8_lossy(&add.stderr)
    );

    let list = Command::new(env!("CARGO_BIN_EXE_zap"))
        .args([
            "registry",
            "list",
            "--registry",
            registry_path.to_str().unwrap(),
            "--json",
        ])
        .output()
        .unwrap();
    assert!(
        list.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&list.stdout),
        String::from_utf8_lossy(&list.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&list.stdout).unwrap();
    assert_eq!(json["entries"][0]["action"], "echo");

    let verify = Command::new(env!("CARGO_BIN_EXE_zap"))
        .args([
            "registry",
            "verify",
            "--registry",
            registry_path.to_str().unwrap(),
            "--manifest",
            manifest_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        verify.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&verify.stdout),
        String::from_utf8_lossy(&verify.stderr)
    );
    assert!(String::from_utf8_lossy(&verify.stdout).contains("ok"));
}

#[test]
fn driver_manifest_verify_rejects_tampered_driver() {
    let dir = tempdir().unwrap();
    let author = Keypair::generate();
    let author_key_path = dir.path().join("author.key");
    let driver_path = dir.path().join("echo.wat");
    let manifest_path = dir.path().join("echo.manifest.toml");
    std::fs::write(&author_key_path, author.to_key_file_toml().unwrap()).unwrap();
    std::fs::write(&driver_path, echo_driver_wat()).unwrap();
    let create = Command::new(env!("CARGO_BIN_EXE_zap"))
        .args([
            "driver-manifest",
            "create",
            "--driver",
            driver_path.to_str().unwrap(),
            "--action",
            "echo",
            "--author-key",
            author_key_path.to_str().unwrap(),
            "--out",
            manifest_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(create.status.success());
    std::fs::write(
        &driver_path,
        format!(
            "{}\n;; modified after manifest signing\n",
            echo_driver_wat()
        ),
    )
    .unwrap();

    let verify = Command::new(env!("CARGO_BIN_EXE_zap"))
        .args([
            "driver-manifest",
            "verify",
            "--driver",
            driver_path.to_str().unwrap(),
            "--manifest",
            manifest_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert!(!verify.status.success());
    assert!(
        String::from_utf8_lossy(&verify.stderr).contains("driver hash mismatch"),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&verify.stdout),
        String::from_utf8_lossy(&verify.stderr)
    );
}

#[test]
fn registry_commands_manage_signed_manifest_index() {
    let dir = tempdir().unwrap();
    let author = Keypair::generate();
    let operator = Keypair::generate();
    let manifest_path = dir.path().join("echo.manifest.toml");
    let registry_path = dir.path().join("registry.index.toml");
    let operator_key_path = dir.path().join("operator.key");
    let manifest = DriverManifest::new(
        "echo-driver",
        "0.1.0",
        "echo",
        echo_driver_wat().as_bytes(),
        Default::default(),
        None,
        &author,
    )
    .unwrap();
    std::fs::write(&manifest_path, manifest.to_toml_string().unwrap()).unwrap();
    std::fs::write(&operator_key_path, operator.to_key_file_toml().unwrap()).unwrap();

    let init = Command::new(env!("CARGO_BIN_EXE_zap"))
        .args(["registry", "init", "--out", registry_path.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(init.status.success());

    let add = Command::new(env!("CARGO_BIN_EXE_zap"))
        .args([
            "registry",
            "add",
            "--registry",
            registry_path.to_str().unwrap(),
            "--manifest",
            manifest_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        add.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&add.stdout),
        String::from_utf8_lossy(&add.stderr)
    );

    let verify = Command::new(env!("CARGO_BIN_EXE_zap"))
        .args([
            "registry",
            "verify",
            "--registry",
            registry_path.to_str().unwrap(),
            "--manifest",
            manifest_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(verify.status.success());

    let sign = Command::new(env!("CARGO_BIN_EXE_zap"))
        .args([
            "registry",
            "sign",
            "--registry",
            registry_path.to_str().unwrap(),
            "--operator-key",
            operator_key_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        sign.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&sign.stdout),
        String::from_utf8_lossy(&sign.stderr)
    );

    let verify_signature = Command::new(env!("CARGO_BIN_EXE_zap"))
        .args([
            "registry",
            "verify-signature",
            "--registry",
            registry_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        verify_signature.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&verify_signature.stdout),
        String::from_utf8_lossy(&verify_signature.stderr)
    );

    let list = Command::new(env!("CARGO_BIN_EXE_zap"))
        .args([
            "registry",
            "list",
            "--registry",
            registry_path.to_str().unwrap(),
            "--json",
        ])
        .output()
        .unwrap();
    assert!(list.status.success());
    let registry =
        DriverRegistry::from_toml_str(&std::fs::read_to_string(&registry_path).unwrap()).unwrap();
    registry.verify_signature().unwrap();
    assert_eq!(registry.entries.len(), 1);
    let json: serde_json::Value = serde_json::from_slice(&list.stdout).unwrap();
    assert_eq!(json["entries"][0]["action"], "echo");
    assert_eq!(
        json["operator_node_id"],
        serde_json::Value::String(operator.node_id().to_string())
    );
}

#[test]
fn registry_revoke_marks_entry_and_clears_signature() {
    let dir = tempdir().unwrap();
    let author = Keypair::generate();
    let operator = Keypair::generate();
    let manifest_path = dir.path().join("echo.manifest.toml");
    let registry_path = dir.path().join("registry.index.toml");
    let operator_key_path = dir.path().join("operator.key");
    let manifest = DriverManifest::new(
        "echo-driver",
        "0.1.0",
        "echo",
        echo_driver_wat().as_bytes(),
        Default::default(),
        None,
        &author,
    )
    .unwrap();
    std::fs::write(&manifest_path, manifest.to_toml_string().unwrap()).unwrap();
    std::fs::write(&operator_key_path, operator.to_key_file_toml().unwrap()).unwrap();

    assert!(
        Command::new(env!("CARGO_BIN_EXE_zap"))
            .args(["registry", "init", "--out", registry_path.to_str().unwrap()])
            .output()
            .unwrap()
            .status
            .success()
    );
    assert!(
        Command::new(env!("CARGO_BIN_EXE_zap"))
            .args([
                "registry",
                "add",
                "--registry",
                registry_path.to_str().unwrap(),
                "--manifest",
                manifest_path.to_str().unwrap(),
            ])
            .output()
            .unwrap()
            .status
            .success()
    );
    assert!(
        Command::new(env!("CARGO_BIN_EXE_zap"))
            .args([
                "registry",
                "sign",
                "--registry",
                registry_path.to_str().unwrap(),
                "--operator-key",
                operator_key_path.to_str().unwrap(),
            ])
            .output()
            .unwrap()
            .status
            .success()
    );

    let revoke = Command::new(env!("CARGO_BIN_EXE_zap"))
        .args([
            "registry",
            "revoke",
            "--registry",
            registry_path.to_str().unwrap(),
            "--action",
            "echo",
            "--version",
            "0.1.0",
            "--reason",
            "bad release",
        ])
        .output()
        .unwrap();
    assert!(
        revoke.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&revoke.stdout),
        String::from_utf8_lossy(&revoke.stderr)
    );

    let registry =
        DriverRegistry::from_toml_str(&std::fs::read_to_string(&registry_path).unwrap()).unwrap();
    assert_eq!(registry.entries[0].status, DriverRegistryStatus::Revoked);
    assert_eq!(
        registry.entries[0].revoked_reason.as_deref(),
        Some("bad release")
    );
    assert!(registry.signature.is_none());

    let verify = Command::new(env!("CARGO_BIN_EXE_zap"))
        .args([
            "registry",
            "verify",
            "--registry",
            registry_path.to_str().unwrap(),
            "--manifest",
            manifest_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(!verify.status.success());
    assert!(String::from_utf8_lossy(&verify.stderr).contains("revoked"));

    let verify_signature = Command::new(env!("CARGO_BIN_EXE_zap"))
        .args([
            "registry",
            "verify-signature",
            "--registry",
            registry_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(!verify_signature.status.success());
    assert!(String::from_utf8_lossy(&verify_signature.stderr).contains("not signed"));
}

#[test]
fn receipts_verify_checks_signed_jsonl_logs() {
    let dir = tempdir().unwrap();
    let node = Keypair::generate();
    let source = Keypair::generate();
    let frame = ZapFrame::with_timestamp(
        source.node_id(),
        node.node_id(),
        ZapFlags::SIGNED,
        123,
        Bytes::from_static(b"payload"),
    )
    .unwrap();
    let signed = sign_frame(&source, &frame).unwrap();
    let receipt = SignedActionReceipt::new(&node, &signed, "echo", Some(b"ok"), 456, None).unwrap();
    let receipt_path = dir.path().join("receipts.jsonl");
    std::fs::write(
        &receipt_path,
        format!("\n{}", receipt.to_json_line().unwrap()),
    )
    .unwrap();

    let verify = Command::new(env!("CARGO_BIN_EXE_zap"))
        .args([
            "receipts",
            "verify",
            "--path",
            receipt_path.to_str().unwrap(),
            "--json",
        ])
        .output()
        .unwrap();
    assert!(
        verify.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&verify.stdout),
        String::from_utf8_lossy(&verify.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&verify.stdout).unwrap();
    assert_eq!(json["receipts"], 1);
    assert_eq!(json["verified"], true);

    let mut tampered = receipt.clone();
    tampered.receipt.subject = "tampered".to_string();
    std::fs::write(&receipt_path, tampered.to_json_line().unwrap()).unwrap();
    let failed = Command::new(env!("CARGO_BIN_EXE_zap"))
        .args([
            "receipts",
            "verify",
            "--path",
            receipt_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(!failed.status.success());
    let stderr = String::from_utf8_lossy(&failed.stderr);
    assert!(stderr.contains("invalid receipt signature"));
    assert!(stderr.contains("line 1"));
}

#[test]
fn receipts_prune_writes_verified_retention_output() {
    let dir = tempdir().unwrap();
    let node = Keypair::generate();
    let source = Keypair::generate();
    let frame = ZapFrame::with_timestamp(
        source.node_id(),
        node.node_id(),
        ZapFlags::SIGNED,
        123,
        Bytes::from_static(b"payload"),
    )
    .unwrap();
    let signed = sign_frame(&source, &frame).unwrap();
    let old_receipt =
        SignedActionReceipt::new(&node, &signed, "echo.old", Some(b"old"), 100, None).unwrap();
    let new_receipt =
        SignedActionReceipt::new(&node, &signed, "echo.new", Some(b"new"), 200, None).unwrap();
    let receipt_path = dir.path().join("receipts.jsonl");
    let pruned_path = dir.path().join("retained.jsonl");
    std::fs::write(
        &receipt_path,
        format!(
            "{}{}",
            old_receipt.to_json_line().unwrap(),
            new_receipt.to_json_line().unwrap()
        ),
    )
    .unwrap();

    let prune = Command::new(env!("CARGO_BIN_EXE_zap"))
        .args([
            "receipts",
            "prune",
            "--path",
            receipt_path.to_str().unwrap(),
            "--before-processed-at-micros",
            "150",
            "--out",
            pruned_path.to_str().unwrap(),
            "--json",
        ])
        .output()
        .unwrap();
    assert!(
        prune.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&prune.stdout),
        String::from_utf8_lossy(&prune.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&prune.stdout).unwrap();
    assert_eq!(json["input_receipts"], 2);
    assert_eq!(json["retained_receipts"], 1);
    assert_eq!(json["pruned_receipts"], 1);
    assert_eq!(json["verified"], true);

    let retained = std::fs::read_to_string(&pruned_path).unwrap();
    let retained_lines = retained.lines().collect::<Vec<_>>();
    assert_eq!(retained_lines.len(), 1);
    let retained_receipt = SignedActionReceipt::from_json_str(retained_lines[0]).unwrap();
    retained_receipt.verify().unwrap();
    assert_eq!(retained_receipt.receipt.subject, "echo.new");
    assert_eq!(retained_receipt.receipt.processed_at_micros, 200);

    let overwrite = Command::new(env!("CARGO_BIN_EXE_zap"))
        .args([
            "receipts",
            "prune",
            "--path",
            receipt_path.to_str().unwrap(),
            "--before-processed-at-micros",
            "150",
            "--out",
            pruned_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(!overwrite.status.success());
    assert!(String::from_utf8_lossy(&overwrite.stderr).contains("refusing to overwrite"));

    let destructive = Command::new(env!("CARGO_BIN_EXE_zap"))
        .args([
            "receipts",
            "prune",
            "--path",
            receipt_path.to_str().unwrap(),
            "--before-processed-at-micros",
            "150",
            "--out",
            receipt_path.to_str().unwrap(),
            "--force",
        ])
        .output()
        .unwrap();
    assert!(!destructive.status.success());
    assert!(
        String::from_utf8_lossy(&destructive.stderr)
            .contains("must not point at an input receipt log")
    );
}

#[test]
fn receipts_merge_deduplicates_verified_logs() {
    let dir = tempdir().unwrap();
    let node = Keypair::generate();
    let source = Keypair::generate();
    let frame = ZapFrame::with_timestamp(
        source.node_id(),
        node.node_id(),
        ZapFlags::SIGNED,
        123,
        Bytes::from_static(b"payload"),
    )
    .unwrap();
    let signed = sign_frame(&source, &frame).unwrap();
    let first =
        SignedActionReceipt::new(&node, &signed, "echo.first", Some(b"first"), 100, None).unwrap();
    let duplicate =
        SignedActionReceipt::new(&node, &signed, "echo.shared", Some(b"shared"), 200, None)
            .unwrap();
    let last =
        SignedActionReceipt::new(&node, &signed, "echo.last", Some(b"last"), 300, None).unwrap();
    let left_path = dir.path().join("left.jsonl");
    let right_path = dir.path().join("right.jsonl");
    let merged_path = dir.path().join("merged.jsonl");
    std::fs::write(
        &left_path,
        format!(
            "{}{}",
            first.to_json_line().unwrap(),
            duplicate.to_json_line().unwrap()
        ),
    )
    .unwrap();
    std::fs::write(
        &right_path,
        format!(
            "{}{}",
            duplicate.to_json_line().unwrap(),
            last.to_json_line().unwrap()
        ),
    )
    .unwrap();

    let merge = Command::new(env!("CARGO_BIN_EXE_zap"))
        .args([
            "receipts",
            "merge",
            left_path.to_str().unwrap(),
            right_path.to_str().unwrap(),
            "--out",
            merged_path.to_str().unwrap(),
            "--json",
        ])
        .output()
        .unwrap();
    assert!(
        merge.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&merge.stdout),
        String::from_utf8_lossy(&merge.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&merge.stdout).unwrap();
    assert_eq!(json["input_logs"], 2);
    assert_eq!(json["input_receipts"], 4);
    assert_eq!(json["written_receipts"], 3);
    assert_eq!(json["duplicate_receipts"], 1);
    assert_eq!(json["verified"], true);

    let merged = std::fs::read_to_string(&merged_path).unwrap();
    let subjects = merged
        .lines()
        .map(|line| {
            let receipt = SignedActionReceipt::from_json_str(line).unwrap();
            receipt.verify().unwrap();
            receipt.receipt.subject
        })
        .collect::<Vec<_>>();
    assert_eq!(subjects, vec!["echo.first", "echo.shared", "echo.last"]);

    let destructive = Command::new(env!("CARGO_BIN_EXE_zap"))
        .args([
            "receipts",
            "merge",
            left_path.to_str().unwrap(),
            right_path.to_str().unwrap(),
            "--out",
            left_path.to_str().unwrap(),
            "--force",
        ])
        .output()
        .unwrap();
    assert!(!destructive.status.success());
    assert!(
        String::from_utf8_lossy(&destructive.stderr)
            .contains("must not point at an input receipt log")
    );
}

#[test]
fn poa_request_and_attest_commands_exchange_json() {
    let dir = tempdir().unwrap();
    let source = Keypair::generate();
    let target = Keypair::generate();
    let validator = Keypair::generate();
    let source_key_path = dir.path().join("source.key");
    let validator_key_path = dir.path().join("validator.key");
    let frame_path = dir.path().join("frame.bin");
    let request_path = dir.path().join("poa-request.json");
    std::fs::write(&source_key_path, source.to_key_file_toml().unwrap()).unwrap();
    std::fs::write(&validator_key_path, validator.to_key_file_toml().unwrap()).unwrap();
    let frame = ZapFrame::with_timestamp(
        source.node_id(),
        target.node_id(),
        ZapFlags::REQUIRES_CONSENSUS,
        123,
        Bytes::from_static(b"critical"),
    )
    .unwrap();
    let signed = sign_frame(&source, &frame).unwrap();
    std::fs::write(&frame_path, signed.encode()).unwrap();

    let request = Command::new(env!("CARGO_BIN_EXE_zap"))
        .args([
            "poa",
            "request",
            "--frame",
            frame_path.to_str().unwrap(),
            "--requester-key",
            source_key_path.to_str().unwrap(),
            "--threshold",
            "1",
        ])
        .output()
        .unwrap();
    assert!(
        request.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&request.stdout),
        String::from_utf8_lossy(&request.stderr)
    );
    let request_json: serde_json::Value = serde_json::from_slice(&request.stdout).unwrap();
    assert_eq!(request_json["threshold"], 1);
    std::fs::write(&request_path, &request.stdout).unwrap();

    let response = Command::new(env!("CARGO_BIN_EXE_zap"))
        .args([
            "poa",
            "attest",
            "--request",
            request_path.to_str().unwrap(),
            "--validator-key",
            validator_key_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        response.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&response.stdout),
        String::from_utf8_lossy(&response.stderr)
    );
    let response_json: serde_json::Value = serde_json::from_slice(&response.stdout).unwrap();
    assert_eq!(
        response_json["validator_node"],
        validator.node_id().to_string()
    );
    assert_eq!(response_json["frame_digest"], request_json["frame_digest"]);
}

#[tokio::test]
async fn send_with_action_builds_universal_action_envelope() {
    let sender = Keypair::generate();
    let receiver = Keypair::generate();
    let transport_key = [0x42_u8; 32];
    let receiver_endpoint = ZapEndpoint::bind(ZapEndpointConfig::new(
        "127.0.0.1:0".parse().unwrap(),
        receiver.node_id(),
    ))
    .await
    .unwrap();
    let sender_addr = free_udp_addr();
    receiver_endpoint
        .add_peer(Peer::new(
            sender.node_id(),
            sender_addr.parse().unwrap(),
            transport_key,
        ))
        .await;

    let dir = tempdir().unwrap();
    let config_path = write_sender_config(
        &dir,
        &sender,
        &receiver,
        receiver_endpoint.local_addr().unwrap(),
        &sender_addr,
        transport_key,
    );
    let target = receiver.node_id().to_string();
    let output = tokio::task::spawn_blocking(move || {
        Command::new(env!("CARGO_BIN_EXE_zap"))
            .args([
                "send",
                "--config",
                config_path.to_str().unwrap(),
                "--target",
                &target,
                "--action",
                "echo",
                "--payload",
                "hello-action",
            ])
            .output()
    })
    .await
    .unwrap()
    .unwrap();

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let inbound = timeout(Duration::from_secs(2), receiver_endpoint.recv())
        .await
        .unwrap()
        .unwrap();
    verify_frame(&sender.verifying_key(), &inbound.frame).unwrap();
    let envelope = ZapEnvelopeRef::parse(&inbound.frame.payload).unwrap();
    assert_eq!(envelope.kind(), ZapMessageKind::Action);
    assert_eq!(envelope.subject(), "echo");
    assert_eq!(envelope.content_type(), "text/plain");
    assert_eq!(envelope.body(), b"hello-action");
}

#[tokio::test]
async fn send_with_kind_event_builds_universal_envelope() {
    let sender = Keypair::generate();
    let receiver = Keypair::generate();
    let transport_key = [0x42_u8; 32];
    let receiver_endpoint = ZapEndpoint::bind(ZapEndpointConfig::new(
        "127.0.0.1:0".parse().unwrap(),
        receiver.node_id(),
    ))
    .await
    .unwrap();
    let sender_addr = free_udp_addr();
    receiver_endpoint
        .add_peer(Peer::new(
            sender.node_id(),
            sender_addr.parse().unwrap(),
            transport_key,
        ))
        .await;

    let dir = tempdir().unwrap();
    let config_path = write_sender_config(
        &dir,
        &sender,
        &receiver,
        receiver_endpoint.local_addr().unwrap(),
        &sender_addr,
        transport_key,
    );
    let target = receiver.node_id().to_string();
    let output = tokio::task::spawn_blocking(move || {
        Command::new(env!("CARGO_BIN_EXE_zap"))
            .args([
                "send",
                "--config",
                config_path.to_str().unwrap(),
                "--target",
                &target,
                "--kind",
                "event",
                "--subject",
                "sensor.temperature",
                "--payload",
                r#"{"c":21.5}"#,
                "--content-type",
                "application/json",
                "--metadata",
                r#"{"source":"sim"}"#,
            ])
            .output()
    })
    .await
    .unwrap()
    .unwrap();

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let inbound = timeout(Duration::from_secs(2), receiver_endpoint.recv())
        .await
        .unwrap()
        .unwrap();
    verify_frame(&sender.verifying_key(), &inbound.frame).unwrap();
    let envelope = ZapEnvelopeRef::parse(&inbound.frame.payload).unwrap();
    assert_eq!(envelope.kind(), ZapMessageKind::Event);
    assert_eq!(envelope.subject(), "sensor.temperature");
    assert_eq!(envelope.content_type(), "application/json");
    assert_eq!(envelope.metadata(), br#"{"source":"sim"}"#);
    assert_eq!(envelope.body(), br#"{"c":21.5}"#);
}

#[test]
fn inspect_decodes_universal_envelope_fields() {
    let dir = tempdir().unwrap();
    let sender = Keypair::generate();
    let receiver = Keypair::generate();
    let payload = ZapEnvelope::new(
        ZapMessageKind::Event,
        "sensor.temperature",
        "application/octet-stream",
        Bytes::from_static(br#"{"c":21.5}"#),
    )
    .unwrap()
    .with_metadata(Bytes::from_static(br#"{"source":"sim"}"#))
    .unwrap()
    .encode();
    let frame = ZapFrame::with_timestamp(
        sender.node_id(),
        receiver.node_id(),
        ZapFlags::empty(),
        42,
        payload,
    )
    .unwrap();
    let frame_path = dir.path().join("frame.bin");
    std::fs::write(&frame_path, frame.encode()).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_zap"))
        .args(["inspect", frame_path.to_str().unwrap()])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["envelope_kind"], "event");
    assert_eq!(json["subject"], "sensor.temperature");
    assert_eq!(json["content_type"], "application/octet-stream");
    assert_eq!(json["metadata_len"], 16);
    assert_eq!(json["body_len"], 10);
}

#[test]
fn inspect_verifies_with_public_key_without_private_key_file() {
    let dir = tempdir().unwrap();
    let sender = Keypair::generate();
    let receiver = Keypair::generate();
    let frame = ZapFrame::with_timestamp(
        sender.node_id(),
        receiver.node_id(),
        ZapFlags::ENCRYPTED,
        42,
        Bytes::from_static(b"hello"),
    )
    .unwrap();
    let signed = sign_frame(&sender, &frame).unwrap();
    let frame_path = dir.path().join("signed-frame.bin");
    std::fs::write(&frame_path, signed.encode()).unwrap();
    let public_key = public_key_string(&sender);

    let output = Command::new(env!("CARGO_BIN_EXE_zap"))
        .args([
            "inspect",
            frame_path.to_str().unwrap(),
            "--verify-with-public-key",
            &public_key,
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["verified"], true);
    assert_eq!(json["has_auth_trailer"], true);
}

#[tokio::test]
async fn send_requires_consensus_attaches_poa_certificate() {
    let sender = Keypair::generate();
    let receiver = Keypair::generate();
    let validator = Keypair::generate();
    let transport_key = [0x42_u8; 32];
    let receiver_endpoint = ZapEndpoint::bind(ZapEndpointConfig::new(
        "127.0.0.1:0".parse().unwrap(),
        receiver.node_id(),
    ))
    .await
    .unwrap();
    let sender_addr = free_udp_addr();
    receiver_endpoint
        .add_peer(Peer::new(
            sender.node_id(),
            sender_addr.parse().unwrap(),
            transport_key,
        ))
        .await;

    let dir = tempdir().unwrap();
    let validator_key_path = dir.path().join("validator.key");
    std::fs::write(&validator_key_path, validator.to_key_file_toml().unwrap()).unwrap();
    let config_path = write_sender_config(
        &dir,
        &sender,
        &receiver,
        receiver_endpoint.local_addr().unwrap(),
        &sender_addr,
        transport_key,
    );
    let target = receiver.node_id().to_string();
    let output = tokio::task::spawn_blocking(move || {
        Command::new(env!("CARGO_BIN_EXE_zap"))
            .args([
                "send",
                "--config",
                config_path.to_str().unwrap(),
                "--target",
                &target,
                "--kind",
                "action",
                "--subject",
                "safety.emergency_stop",
                "--payload",
                r#"{"reason":"operator_request"}"#,
                "--content-type",
                "application/json",
                "--requires-consensus",
                "--poa-validator-key",
                validator_key_path.to_str().unwrap(),
            ])
            .output()
    })
    .await
    .unwrap()
    .unwrap();

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let safety = timeout(Duration::from_secs(2), receiver_endpoint.recv())
        .await
        .unwrap()
        .unwrap();
    verify_frame(&sender.verifying_key(), &safety.frame).unwrap();
    let envelope = ZapEnvelopeRef::parse(&safety.frame.payload).unwrap();
    assert_eq!(envelope.kind(), ZapMessageKind::Action);
    assert_eq!(envelope.subject(), "safety.emergency_stop");
    assert_eq!(envelope.content_type(), "application/json");
    let payload: serde_json::Value = serde_json::from_slice(envelope.body()).unwrap();
    assert_eq!(payload["reason"], "operator_request");
    assert!(
        safety
            .frame
            .header
            .flags
            .contains(ZapFlags::REQUIRES_CONSENSUS)
    );
    assert!(safety.frame.poa.is_some());
    verify_poa_certificate(
        &safety.frame,
        &[(validator.node_id(), validator.verifying_key())],
        1,
    )
    .unwrap();
}

#[tokio::test]
async fn send_with_requires_consensus_requires_poa_validator_key() {
    let sender = Keypair::generate();
    let receiver = Keypair::generate();
    let transport_key = [0x42_u8; 32];
    let receiver_endpoint = ZapEndpoint::bind(ZapEndpointConfig::new(
        "127.0.0.1:0".parse().unwrap(),
        receiver.node_id(),
    ))
    .await
    .unwrap();
    let sender_addr = free_udp_addr();
    receiver_endpoint
        .add_peer(Peer::new(
            sender.node_id(),
            sender_addr.parse().unwrap(),
            transport_key,
        ))
        .await;

    let dir = tempdir().unwrap();
    let config_path = write_sender_config(
        &dir,
        &sender,
        &receiver,
        receiver_endpoint.local_addr().unwrap(),
        &sender_addr,
        transport_key,
    );
    let target = receiver.node_id().to_string();
    let output = tokio::task::spawn_blocking(move || {
        Command::new(env!("CARGO_BIN_EXE_zap"))
            .args([
                "send",
                "--config",
                config_path.to_str().unwrap(),
                "--target",
                &target,
                "--kind",
                "action",
                "--subject",
                "safety.emergency_stop",
                "--payload",
                r#"{"reason":"operator_request"}"#,
                "--content-type",
                "application/json",
                "--requires-consensus",
            ])
            .output()
    })
    .await
    .unwrap()
    .unwrap();

    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("--poa-validator-key"),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[tokio::test]
async fn send_with_network_poa_honors_timeout_option() {
    let sender = Keypair::generate();
    let receiver = Keypair::generate();
    let validator = Keypair::generate();
    let sender_addr = free_udp_addr();
    let _validator_socket = UdpSocket::bind("127.0.0.1:0").unwrap();
    let validator_addr = _validator_socket.local_addr().unwrap();
    let dir = tempdir().unwrap();
    let sender_key_path = dir.path().join("sender.key");
    let sender_config_path = dir.path().join("sender.toml");
    std::fs::write(&sender_key_path, sender.to_key_file_toml().unwrap()).unwrap();
    std::fs::write(
        &sender_config_path,
        format!(
            r#"
bind = '{sender_addr}'
key_file = '{}'
require_signed = true

[[peers]]
node_id = '{}'
addr = '127.0.0.1:9'
public_key = '{}'
transport_key = '4242424242424242424242424242424242424242424242424242424242424242'

[[peers]]
node_id = '{}'
addr = '{validator_addr}'
public_key = '{}'
transport_key = '4343434343434343434343434343434343434343434343434343434343434343'

[poa]
required_threshold = 1

[[poa.validators]]
node_id = '{}'
public_key = '{}'
"#,
            sender_key_path.display(),
            receiver.node_id(),
            public_key_string(&receiver),
            validator.node_id(),
            public_key_string(&validator),
            validator.node_id(),
            public_key_string(&validator),
        ),
    )
    .unwrap();

    let target = receiver.node_id().to_string();
    let output = tokio::task::spawn_blocking(move || {
        Command::new(env!("CARGO_BIN_EXE_zap"))
            .args([
                "send",
                "--config",
                sender_config_path.to_str().unwrap(),
                "--target",
                &target,
                "--kind",
                "action",
                "--subject",
                "safety.emergency_stop",
                "--payload",
                r#"{"reason":"operator_request"}"#,
                "--content-type",
                "application/json",
                "--requires-consensus",
                "--poa-network",
                "--poa-timeout-ms",
                "1",
            ])
            .output()
    })
    .await
    .unwrap()
    .unwrap();

    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("PoA network threshold not met"),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[tokio::test]
async fn send_with_network_poa_collects_validator_attestation() {
    let sender = Keypair::generate();
    let receiver = Keypair::generate();
    let validator = Keypair::generate();
    let sender_addr = free_udp_addr();
    let sender_receiver_key = [0x42_u8; 32];
    let sender_validator_key = [0x43_u8; 32];

    let receiver_endpoint = ZapEndpoint::bind(ZapEndpointConfig::new(
        "127.0.0.1:0".parse().unwrap(),
        receiver.node_id(),
    ))
    .await
    .unwrap();
    receiver_endpoint
        .add_peer(Peer::new(
            sender.node_id(),
            sender_addr.parse().unwrap(),
            sender_receiver_key,
        ))
        .await;

    let dir = tempdir().unwrap();
    let validator_key_path = dir.path().join("validator.key");
    let validator_config_path = dir.path().join("validator.toml");
    std::fs::write(&validator_key_path, validator.to_key_file_toml().unwrap()).unwrap();
    std::fs::write(
        &validator_config_path,
        format!(
            r#"
bind = '127.0.0.1:0'
key_file = '{}'
require_signed = true

[[peers]]
node_id = '{}'
addr = '{}'
public_key = '{}'
transport_key = '{}'
"#,
            validator_key_path.display(),
            sender.node_id(),
            sender_addr,
            public_key_string(&sender),
            hex_transport_key(sender_validator_key),
        ),
    )
    .unwrap();
    let validator_node =
        ZapNode::from_config(ZapNodeConfig::from_path(&validator_config_path).unwrap())
            .await
            .unwrap();
    let validator_addr = validator_node.local_addr().unwrap();

    let sender_key_path = dir.path().join("sender.key");
    let sender_config_path = dir.path().join("sender.toml");
    std::fs::write(&sender_key_path, sender.to_key_file_toml().unwrap()).unwrap();
    std::fs::write(
        &sender_config_path,
        format!(
            r#"
bind = '{sender_addr}'
key_file = '{}'
require_signed = true

[[peers]]
node_id = '{}'
addr = '{}'
public_key = '{}'
transport_key = '{}'

[[peers]]
node_id = '{}'
addr = '{}'
public_key = '{}'
transport_key = '{}'

[poa]
required_threshold = 1

[[poa.validators]]
node_id = '{}'
public_key = '{}'
"#,
            sender_key_path.display(),
            receiver.node_id(),
            receiver_endpoint.local_addr().unwrap(),
            public_key_string(&receiver),
            hex_transport_key(sender_receiver_key),
            validator.node_id(),
            validator_addr,
            public_key_string(&validator),
            hex_transport_key(sender_validator_key),
            validator.node_id(),
            public_key_string(&validator),
        ),
    )
    .unwrap();

    let validator_task = tokio::spawn(async move { validator_node.handle_once().await });
    let target = receiver.node_id().to_string();
    let output = tokio::task::spawn_blocking(move || {
        Command::new(env!("CARGO_BIN_EXE_zap"))
            .args([
                "send",
                "--config",
                sender_config_path.to_str().unwrap(),
                "--target",
                &target,
                "--kind",
                "action",
                "--subject",
                "safety.emergency_stop",
                "--payload",
                r#"{"reason":"operator_request"}"#,
                "--content-type",
                "application/json",
                "--requires-consensus",
                "--poa-network",
            ])
            .output()
            .unwrap()
    })
    .await
    .unwrap();

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    timeout(Duration::from_secs(2), validator_task)
        .await
        .unwrap()
        .unwrap()
        .unwrap();

    let inbound = timeout(Duration::from_secs(2), receiver_endpoint.recv())
        .await
        .unwrap()
        .unwrap();
    verify_frame(&sender.verifying_key(), &inbound.frame).unwrap();
    verify_poa_certificate(
        &inbound.frame,
        &[(validator.node_id(), validator.verifying_key())],
        1,
    )
    .unwrap();
    let envelope = ZapEnvelopeRef::parse(&inbound.frame.payload).unwrap();
    assert_eq!(envelope.subject(), "safety.emergency_stop");
    assert!(inbound.frame.poa.is_some());
}

#[tokio::test]
async fn send_with_binary_payload_file_builds_universal_action_envelope() {
    let sender = Keypair::generate();
    let receiver = Keypair::generate();
    let transport_key = [0x42_u8; 32];
    let receiver_endpoint = ZapEndpoint::bind(ZapEndpointConfig::new(
        "127.0.0.1:0".parse().unwrap(),
        receiver.node_id(),
    ))
    .await
    .unwrap();
    let sender_addr = free_udp_addr();
    receiver_endpoint
        .add_peer(Peer::new(
            sender.node_id(),
            sender_addr.parse().unwrap(),
            transport_key,
        ))
        .await;

    let dir = tempdir().unwrap();
    let payload_path = dir.path().join("payload.bin");
    std::fs::write(&payload_path, [0, 1, 2, 250, 255]).unwrap();
    let config_path = write_sender_config(
        &dir,
        &sender,
        &receiver,
        receiver_endpoint.local_addr().unwrap(),
        &sender_addr,
        transport_key,
    );
    let target = receiver.node_id().to_string();
    let output = tokio::task::spawn_blocking(move || {
        Command::new(env!("CARGO_BIN_EXE_zap"))
            .args([
                "send",
                "--config",
                config_path.to_str().unwrap(),
                "--target",
                &target,
                "--action",
                "upload",
                "--payload-file",
                payload_path.to_str().unwrap(),
                "--binary-payload",
            ])
            .output()
    })
    .await
    .unwrap()
    .unwrap();

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let inbound = timeout(Duration::from_secs(2), receiver_endpoint.recv())
        .await
        .unwrap()
        .unwrap();
    verify_frame(&sender.verifying_key(), &inbound.frame).unwrap();
    let envelope = ZapEnvelopeRef::parse(&inbound.frame.payload).unwrap();
    assert_eq!(envelope.kind(), ZapMessageKind::Action);
    assert_eq!(envelope.subject(), "upload");
    assert_eq!(envelope.content_type(), "application/octet-stream");
    assert_eq!(envelope.body(), &[0, 1, 2, 250, 255]);
}

#[test]
fn check_config_accepts_valid_config_and_prints_json_report() {
    let dir = tempdir().unwrap();
    let local = Keypair::generate();
    let peer = Keypair::generate();
    let config_path = write_config(&dir, &local, &peer, public_key_string(&peer));
    let output = Command::new(env!("CARGO_BIN_EXE_zap"))
        .args([
            "check-config",
            "--config",
            config_path.to_str().unwrap(),
            "--json",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["node_id"], local.node_id().to_string());
    assert_eq!(json["peer_count"], 1);
    assert_eq!(json["trusted_peer_count"], 1);
    assert_eq!(json["restricted_peer_count"], 0);
    assert_eq!(json["peer_send_enabled_count"], 1);
    assert_eq!(json["peer_receive_enabled_count"], 1);
    assert_eq!(json["peer_forward_enabled_count"], 1);
    assert_eq!(json["driver_count"], 1);
    assert_eq!(json["signed_driver_count"], 0);
    assert_eq!(json["route_count"], 0);
    assert_eq!(json["memory_enabled"], false);
    assert_eq!(json["capability_count"], 1);
    assert_eq!(json["capability_grant_count"], 0);
    assert_eq!(json["capability_requirement_count"], 0);
    assert_eq!(json["ungranted_capability_count"], 1);
    assert_eq!(json["capability_cache_enabled"], false);
    assert_eq!(json["peer_grant_route_count"], 0);
}

#[test]
fn trust_enroll_outputs_verified_peer_toml() {
    let peer = Keypair::generate();
    let output = Command::new(env!("CARGO_BIN_EXE_zap"))
        .args([
            "trust",
            "enroll",
            "--node-id",
            &peer.node_id().to_string(),
            "--addr",
            "127.0.0.1:9000",
            "--public-key",
            &public_key_string(&peer),
            "--transport-key",
            "4242424242424242424242424242424242424242424242424242424242424242",
            "--transport-key-epoch",
            "2",
            "--label",
            "edge",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("[[peers]]"));
    assert!(stdout.contains(&format!("node_id = \"{}\"", peer.node_id())));
    assert!(stdout.contains("transport_key_epoch = 2"));
    assert!(stdout.contains("[[peers.trust.labels]]") || stdout.contains("labels = [\"edge\"]"));
}

#[test]
fn trust_inspect_reports_restricted_peer() {
    let dir = tempdir().unwrap();
    let local = Keypair::generate();
    let peer = Keypair::generate();
    let config_path = write_config(&dir, &local, &peer, public_key_string(&peer));
    let mut config_text = std::fs::read_to_string(&config_path).unwrap();
    config_text = config_text.replace(
        "\n[[drivers]]",
        r#"
[peers.trust]
allow_send = false
labels = ["lab"]

[[drivers]]"#,
    );
    std::fs::write(&config_path, config_text).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_zap"))
        .args([
            "trust",
            "inspect",
            "--config",
            config_path.to_str().unwrap(),
            "--json",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["trusted_peer_count"], 1);
    assert_eq!(json["restricted_peer_count"], 1);
    assert_eq!(json["peer_send_enabled_count"], 0);
    assert_eq!(json["peers"][0]["allow_send"], false);
    assert_eq!(json["peers"][0]["labels"][0], "lab");
}

#[test]
fn doctor_reports_readiness_json() {
    let dir = tempdir().unwrap();
    let local = Keypair::generate();
    let peer = Keypair::generate();
    let config_path = write_config(&dir, &local, &peer, public_key_string(&peer));
    let output = Command::new(env!("CARGO_BIN_EXE_zap"))
        .args([
            "doctor",
            "--config",
            config_path.to_str().unwrap(),
            "--json",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["config"], config_path.display().to_string());
    assert_eq!(json["status"], "needs_attention");
    assert!(json["score"].as_u64().unwrap() < 100);
    assert!(
        json["checks"]
            .as_array()
            .unwrap()
            .iter()
            .any(|check| check["name"] == "identity" && check["status"] == "pass")
    );
    assert!(
        json["warnings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|warning| warning.as_str().unwrap().contains("receipt audit"))
    );
}

#[test]
fn check_config_and_doctor_report_capability_policy() {
    let dir = tempdir().unwrap();
    let local = Keypair::generate();
    let peer = Keypair::generate();
    let config_path = write_config(&dir, &local, &peer, public_key_string(&peer));
    let mut config = std::fs::read_to_string(&config_path).unwrap();
    config.push_str(
        r#"
[capability_policy]
require_grants_for_advertised = true

[[capability_policy.grants]]
capability = 'driver.execute:echo'
reason = 'operator-approved local echo driver'

[[capability_policy.requirements]]
capability = 'poa.validator'
required = true
reason = 'critical frames require validator quorum'
"#,
    );
    std::fs::write(&config_path, config).unwrap();

    let check = Command::new(env!("CARGO_BIN_EXE_zap"))
        .args([
            "check-config",
            "--config",
            config_path.to_str().unwrap(),
            "--json",
        ])
        .output()
        .unwrap();
    assert!(
        check.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&check.stdout),
        String::from_utf8_lossy(&check.stderr)
    );
    let check_json: serde_json::Value = serde_json::from_slice(&check.stdout).unwrap();
    assert_eq!(check_json["capability_grant_count"], 1);
    assert_eq!(check_json["capability_requirement_count"], 1);
    assert_eq!(check_json["ungranted_capability_count"], 0);

    let doctor = Command::new(env!("CARGO_BIN_EXE_zap"))
        .args([
            "doctor",
            "--config",
            config_path.to_str().unwrap(),
            "--json",
        ])
        .output()
        .unwrap();
    assert!(doctor.status.success());
    let doctor_json: serde_json::Value = serde_json::from_slice(&doctor.stdout).unwrap();
    assert!(
        doctor_json["checks"]
            .as_array()
            .unwrap()
            .iter()
            .any(|check| check["name"] == "capability policy" && check["status"] == "pass")
    );
}

#[test]
fn doctor_strict_rejects_readiness_warnings() {
    let dir = tempdir().unwrap();
    let local = Keypair::generate();
    let peer = Keypair::generate();
    let config_path = write_config(&dir, &local, &peer, public_key_string(&peer));
    let output = Command::new(env!("CARGO_BIN_EXE_zap"))
        .args([
            "doctor",
            "--config",
            config_path.to_str().unwrap(),
            "--json",
            "--strict",
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["status"], "needs_attention");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("doctor strict gate failed"),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        stderr
    );
}

#[test]
fn doctor_reports_validation_errors_as_json() {
    let dir = tempdir().unwrap();
    let local = Keypair::generate();
    let peer = Keypair::generate();
    let wrong_peer = Keypair::generate();
    let config_path = write_config(&dir, &local, &peer, public_key_string(&wrong_peer));
    let output = Command::new(env!("CARGO_BIN_EXE_zap"))
        .args([
            "doctor",
            "--config",
            config_path.to_str().unwrap(),
            "--json",
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["status"], "failed");
    assert_eq!(json["score"], 0);
    assert!(
        json["error"]
            .as_str()
            .unwrap()
            .contains("peer public_key derives node_id")
    );
}

#[test]
fn capability_list_prints_local_driver_capability() {
    let dir = tempdir().unwrap();
    let local = Keypair::generate();
    let peer = Keypair::generate();
    let config_path = write_config(&dir, &local, &peer, public_key_string(&peer));
    let output = Command::new(env!("CARGO_BIN_EXE_zap"))
        .args([
            "capability",
            "list",
            "--config",
            config_path.to_str().unwrap(),
            "--json",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["node_id"], local.node_id().to_string());
    assert!(
        json["capabilities"]["capabilities"]
            .as_array()
            .unwrap()
            .iter()
            .any(|value| value == "driver.execute:echo")
    );
}

#[tokio::test]
async fn capability_query_can_cache_and_verify_peer_advertisement() {
    let dir = tempdir().unwrap();
    let receiver = Keypair::generate();
    let sender = Keypair::generate();
    let transport_key = [0x63_u8; 32];
    let sender_addr = free_udp_addr();
    let receiver_key_path = dir.path().join("receiver.key");
    let receiver_driver_path = dir.path().join("receiver-echo.wat");
    let receiver_config_path = dir.path().join("receiver.toml");
    std::fs::write(&receiver_key_path, receiver.to_key_file_toml().unwrap()).unwrap();
    std::fs::write(&receiver_driver_path, echo_driver_wat()).unwrap();
    std::fs::write(
        &receiver_config_path,
        format!(
            r#"
bind = '127.0.0.1:0'
key_file = '{}'
require_signed = true

[[peers]]
node_id = '{}'
addr = '{}'
public_key = '{}'
transport_key = '{}'

[[drivers]]
action = 'echo'
path = '{}'

[capability_policy]
require_grants_for_advertised = true

[[capability_policy.grants]]
capability = 'driver.execute:echo'
reason = 'operator-approved test driver'
"#,
            receiver_key_path.display(),
            sender.node_id(),
            sender_addr,
            public_key_string(&sender),
            hex_transport_key(transport_key),
            receiver_driver_path.display(),
        ),
    )
    .unwrap();
    let receiver_config = ZapNodeConfig::from_path(&receiver_config_path).unwrap();
    let receiver_node = ZapNode::from_config(receiver_config).await.unwrap();
    let sender_config = write_sender_config(
        &dir,
        &sender,
        &receiver,
        receiver_node.local_addr().unwrap(),
        &sender_addr,
        transport_key,
    );
    let cache_path = dir.path().join("capabilities.jsonl");
    let target = receiver.node_id().to_string();
    let config_arg = sender_config.clone();
    let cache_arg = cache_path.clone();

    let command = tokio::task::spawn_blocking(move || {
        Command::new(env!("CARGO_BIN_EXE_zap"))
            .args([
                "capability",
                "query",
                "--config",
                config_arg.to_str().unwrap(),
                "--target",
                &target,
                "--cache",
                cache_arg.to_str().unwrap(),
                "--json",
            ])
            .output()
    });
    let handled = receiver_node.handle_once();
    let (event, output) = timeout(Duration::from_secs(5), async {
        tokio::join!(handled, command)
    })
    .await
    .unwrap();
    event.unwrap();
    let output = output.unwrap().unwrap();
    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let response: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(
        response["response"]["advertisement"]["node_id"],
        receiver.node_id().to_string()
    );
    assert_eq!(
        response["cached_entry"]["peer_node_id"],
        receiver.node_id().to_string()
    );
    assert_eq!(
        response["response"]["advertisement"]["grants"][0]["capability"],
        "driver.execute:echo"
    );

    let verify = Command::new(env!("CARGO_BIN_EXE_zap"))
        .args([
            "capability",
            "cache",
            "verify",
            "--path",
            cache_path.to_str().unwrap(),
            "--json",
        ])
        .output()
        .unwrap();
    assert!(verify.status.success());
    let verify_json: serde_json::Value = serde_json::from_slice(&verify.stdout).unwrap();
    assert_eq!(verify_json["verified"], true);
    assert_eq!(verify_json["entries"], 1);

    let list = Command::new(env!("CARGO_BIN_EXE_zap"))
        .args([
            "capability",
            "cache",
            "list",
            "--path",
            cache_path.to_str().unwrap(),
            "--peer",
            &receiver.node_id().to_string(),
            "--json",
        ])
        .output()
        .unwrap();
    assert!(list.status.success());
    let list_json: serde_json::Value = serde_json::from_slice(&list.stdout).unwrap();
    assert_eq!(list_json.as_array().unwrap().len(), 1);
    assert!(
        list_json[0]["advertisement"]["capabilities"]["capabilities"]
            .as_array()
            .unwrap()
            .iter()
            .any(|capability| capability == "driver.execute:echo")
    );
}

#[tokio::test]
async fn capability_cache_refresh_queries_configured_peer() {
    let dir = tempdir().unwrap();
    let receiver = Keypair::generate();
    let sender = Keypair::generate();
    let transport_key = [0x64_u8; 32];
    let sender_addr = free_udp_addr();
    let receiver_key_path = dir.path().join("receiver.key");
    let receiver_driver_path = dir.path().join("receiver-echo.wat");
    let receiver_config_path = dir.path().join("receiver.toml");
    std::fs::write(&receiver_key_path, receiver.to_key_file_toml().unwrap()).unwrap();
    std::fs::write(&receiver_driver_path, echo_driver_wat()).unwrap();
    std::fs::write(
        &receiver_config_path,
        format!(
            r#"
bind = '127.0.0.1:0'
key_file = '{}'
require_signed = true

[[peers]]
node_id = '{}'
addr = '{}'
public_key = '{}'
transport_key = '{}'

[[drivers]]
action = 'echo'
path = '{}'

[capability_policy]
require_grants_for_advertised = true

[[capability_policy.grants]]
capability = 'driver.execute:echo'
reason = 'operator-approved test driver'
"#,
            receiver_key_path.display(),
            sender.node_id(),
            sender_addr,
            public_key_string(&sender),
            hex_transport_key(transport_key),
            receiver_driver_path.display(),
        ),
    )
    .unwrap();
    let receiver_config = ZapNodeConfig::from_path(&receiver_config_path).unwrap();
    let receiver_node = ZapNode::from_config(receiver_config).await.unwrap();
    let sender_config = write_sender_config(
        &dir,
        &sender,
        &receiver,
        receiver_node.local_addr().unwrap(),
        &sender_addr,
        transport_key,
    );
    let cache_path = dir.path().join("refreshed-capabilities.jsonl");
    let mut sender_config_text = std::fs::read_to_string(&sender_config).unwrap();
    sender_config_text.push_str(&format!(
        r#"
[capability_cache]
path = '{}'
"#,
        cache_path.display()
    ));
    std::fs::write(&sender_config, sender_config_text).unwrap();

    let config_arg = sender_config.clone();
    let command = tokio::task::spawn_blocking(move || {
        Command::new(env!("CARGO_BIN_EXE_zap"))
            .args([
                "capability",
                "cache",
                "refresh",
                "--config",
                config_arg.to_str().unwrap(),
                "--json",
                "--strict",
            ])
            .output()
    });
    let handled = receiver_node.handle_once();
    let (event, output) = timeout(Duration::from_secs(5), async {
        tokio::join!(handled, command)
    })
    .await
    .unwrap();
    event.unwrap();
    let output = output.unwrap().unwrap();
    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["requested_peer_count"], 1);
    assert_eq!(report["refreshed"], 1);
    assert_eq!(report["skipped"], 0);
    assert_eq!(report["failed"], 0);
    assert_eq!(report["results"][0]["peer"], receiver.node_id().to_string());
    assert_eq!(report["results"][0]["status"], "ok");
    assert_eq!(report["results"][0]["capabilities"], 1);
    assert_eq!(report["results"][0]["grants"], 1);

    let verify = Command::new(env!("CARGO_BIN_EXE_zap"))
        .args([
            "capability",
            "cache",
            "verify",
            "--path",
            cache_path.to_str().unwrap(),
            "--json",
        ])
        .output()
        .unwrap();
    assert!(verify.status.success());
    let verify_json: serde_json::Value = serde_json::from_slice(&verify.stdout).unwrap();
    assert_eq!(verify_json["verified"], true);
    assert_eq!(verify_json["entries"], 1);
}

#[test]
fn capability_inspect_manifest_prints_capabilities() {
    let dir = tempdir().unwrap();
    let author = Keypair::generate();
    let manifest_path = dir.path().join("echo.manifest.toml");
    let manifest = DriverManifest::new(
        "echo-driver",
        "0.1.0",
        "echo",
        echo_driver_wat().as_bytes(),
        zap_runtime::DriverPermissions::none(),
        None,
        &author,
    )
    .unwrap();
    std::fs::write(&manifest_path, manifest.to_toml_string().unwrap()).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_zap"))
        .args([
            "capability",
            "inspect-manifest",
            "--manifest",
            manifest_path.to_str().unwrap(),
            "--json",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["action"], "echo");
    assert!(
        json["capabilities"]["capabilities"]
            .as_array()
            .unwrap()
            .iter()
            .any(|value| value == "driver.execute:echo")
    );
}

#[test]
fn memory_put_query_and_verify_round_trip() {
    let dir = tempdir().unwrap();
    let memory_path = dir.path().join("memory.jsonl");
    let put = Command::new(env!("CARGO_BIN_EXE_zap"))
        .args([
            "memory",
            "put",
            "--path",
            memory_path.to_str().unwrap(),
            "--subject",
            "note",
            "--payload",
            "hello",
            "--json",
        ])
        .output()
        .unwrap();
    assert!(
        put.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&put.stdout),
        String::from_utf8_lossy(&put.stderr)
    );
    let record: serde_json::Value = serde_json::from_slice(&put.stdout).unwrap();
    assert_eq!(record["subject"], "note");

    let query = Command::new(env!("CARGO_BIN_EXE_zap"))
        .args([
            "memory",
            "query",
            "--path",
            memory_path.to_str().unwrap(),
            "--subject",
            "note",
            "--json",
        ])
        .output()
        .unwrap();
    assert!(query.status.success());
    let records: serde_json::Value = serde_json::from_slice(&query.stdout).unwrap();
    assert_eq!(records.as_array().unwrap().len(), 1);

    let verify = Command::new(env!("CARGO_BIN_EXE_zap"))
        .args([
            "memory",
            "verify",
            "--path",
            memory_path.to_str().unwrap(),
            "--json",
        ])
        .output()
        .unwrap();
    assert!(verify.status.success());
    let report: serde_json::Value = serde_json::from_slice(&verify.stdout).unwrap();
    assert_eq!(report["verified"], true);
    assert_eq!(report["records"], 1);
}

#[test]
fn route_explain_uses_default_local_driver_route() {
    let dir = tempdir().unwrap();
    let local = Keypair::generate();
    let peer = Keypair::generate();
    let config_path = write_config(&dir, &local, &peer, public_key_string(&peer));
    let output = Command::new(env!("CARGO_BIN_EXE_zap"))
        .args([
            "route",
            "explain",
            "--config",
            config_path.to_str().unwrap(),
            "--kind",
            "action",
            "--subject",
            "echo",
            "--json",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["decision"]["target"]["local_driver"], "echo");
    assert_eq!(json["decision"]["reason"], "default route");
}

#[test]
fn check_config_strict_rejects_validation_warnings() {
    let dir = tempdir().unwrap();
    let local = Keypair::generate();
    let peer = Keypair::generate();
    let config_path = write_config(&dir, &local, &peer, public_key_string(&peer));
    let output = Command::new(env!("CARGO_BIN_EXE_zap"))
        .args([
            "check-config",
            "--config",
            config_path.to_str().unwrap(),
            "--strict",
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("strict config validation failed"),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        stderr
    );
}

#[test]
fn check_config_resolves_relative_paths_from_config_directory() {
    let dir = tempdir().unwrap();
    let config_dir = dir.path().join("conf");
    let key_dir = config_dir.join("keys");
    let driver_dir = config_dir.join("drivers");
    let other_dir = dir.path().join("elsewhere");
    std::fs::create_dir_all(&key_dir).unwrap();
    std::fs::create_dir_all(&driver_dir).unwrap();
    std::fs::create_dir_all(&other_dir).unwrap();
    let local = Keypair::generate();
    let peer = Keypair::generate();
    std::fs::write(key_dir.join("node.key"), local.to_key_file_toml().unwrap()).unwrap();
    std::fs::write(driver_dir.join("echo.wat"), echo_driver_wat()).unwrap();
    let config_path = config_dir.join("node.toml");
    std::fs::write(
        &config_path,
        format!(
            r#"
bind = '127.0.0.1:0'
key_file = 'keys/node.key'
require_signed = true

[[peers]]
node_id = '{}'
addr = '127.0.0.1:9'
public_key = '{}'
transport_key = '4242424242424242424242424242424242424242424242424242424242424242'

[[drivers]]
action = 'echo'
path = 'drivers/echo.wat'
"#,
            peer.node_id(),
            public_key_string(&peer),
        ),
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_zap"))
        .current_dir(other_dir)
        .args([
            "check-config",
            "--config",
            config_path.to_str().unwrap(),
            "--json",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["node_id"], local.node_id().to_string());
    assert_eq!(json["driver_count"], 1);
}

#[test]
fn check_config_rejects_peer_public_key_mismatch() {
    let dir = tempdir().unwrap();
    let local = Keypair::generate();
    let peer = Keypair::generate();
    let wrong_peer = Keypair::generate();
    let config_path = write_config(&dir, &local, &peer, public_key_string(&wrong_peer));
    let output = Command::new(env!("CARGO_BIN_EXE_zap"))
        .args(["check-config", "--config", config_path.to_str().unwrap()])
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("peer public_key derives node_id"),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn check_config_rejects_invalid_driver_abi() {
    let dir = tempdir().unwrap();
    let local = Keypair::generate();
    let peer = Keypair::generate();
    let config_path = write_config(&dir, &local, &peer, public_key_string(&peer));
    std::fs::write(dir.path().join("echo.wat"), missing_execute_driver_wat()).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_zap"))
        .args(["check-config", "--config", config_path.to_str().unwrap()])
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("invalid driver ABI"),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        stderr
    );
    assert!(
        stderr.contains("missing required export `zap_execute`"),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        stderr
    );
}
