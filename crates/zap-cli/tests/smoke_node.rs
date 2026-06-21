use base64::{Engine as _, engine::general_purpose::STANDARD_NO_PAD};
use std::{
    net::UdpSocket,
    process::{Child, Command, Stdio},
    thread,
    time::{Duration, Instant},
};
use tempfile::tempdir;
use zap_crypto::Keypair;

fn free_udp_addr() -> String {
    let socket = UdpSocket::bind("127.0.0.1:0").unwrap();
    socket.local_addr().unwrap().to_string()
}

fn public_key_string(keypair: &Keypair) -> String {
    STANDARD_NO_PAD.encode(keypair.verifying_key().to_bytes())
}

fn toml_path(path: &std::path::Path) -> String {
    path.display().to_string()
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

struct ChildGuard {
    child: Child,
}

impl ChildGuard {
    fn new(child: Child) -> Self {
        Self { child }
    }

    fn try_wait(&mut self) -> Option<std::process::ExitStatus> {
        self.child.try_wait().unwrap()
    }
}

impl Drop for ChildGuard {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[test]
fn zap_binary_launches_node_sends_action_and_writes_receipt() {
    let bin = env!("CARGO_BIN_EXE_zap");
    let temp = tempdir().unwrap();
    let receiver_key = Keypair::generate();
    let sender_key = Keypair::generate();
    let receiver_addr = free_udp_addr();
    let sender_addr = free_udp_addr();
    let transport_key = "5151515151515151515151515151515151515151515151515151515151515151";

    let receiver_key_path = temp.path().join("receiver.key");
    let sender_key_path = temp.path().join("sender.key");
    let driver_path = temp.path().join("echo.wat");
    let receiver_config_path = temp.path().join("receiver.toml");
    let sender_config_path = temp.path().join("sender.toml");
    let receipt_path = temp.path().join("receipts").join("actions");

    std::fs::write(&receiver_key_path, receiver_key.to_key_file_toml().unwrap()).unwrap();
    std::fs::write(&sender_key_path, sender_key.to_key_file_toml().unwrap()).unwrap();
    std::fs::write(&driver_path, echo_driver_wat()).unwrap();

    std::fs::write(
        &receiver_config_path,
        format!(
            r#"
bind = '{}'
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

[receipts]
dir = '{}'
"#,
            receiver_addr,
            toml_path(&receiver_key_path),
            sender_key.node_id(),
            sender_addr,
            public_key_string(&sender_key),
            transport_key,
            toml_path(&driver_path),
            toml_path(&receipt_path),
        ),
    )
    .unwrap();

    std::fs::write(
        &sender_config_path,
        format!(
            r#"
bind = '{}'
key_file = '{}'
require_signed = true

[[peers]]
node_id = '{}'
addr = '{}'
public_key = '{}'
transport_key = '{}'
"#,
            sender_addr,
            toml_path(&sender_key_path),
            receiver_key.node_id(),
            receiver_addr,
            public_key_string(&receiver_key),
            transport_key,
        ),
    )
    .unwrap();

    for config in [&receiver_config_path, &sender_config_path] {
        let output = Command::new(bin)
            .args(["check-config", "--config", config.to_str().unwrap()])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "check-config failed for {}: {}{}",
            config.display(),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let child = Command::new(bin)
        .args(["run", "--config", receiver_config_path.to_str().unwrap()])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let mut child = ChildGuard::new(child);

    let deadline = Instant::now() + Duration::from_secs(15);
    let mut last_send_output = None;
    let mut last_verify_output = None;
    let mut verify_output = None;

    while Instant::now() < deadline {
        if let Some(status) = child.try_wait() {
            panic!("zap run exited before smoke send completed: {status}");
        }
        let output = Command::new(bin)
            .args([
                "send",
                "--config",
                sender_config_path.to_str().unwrap(),
                "--target",
                &receiver_key.node_id().to_string(),
                "--action",
                "echo",
                "--payload",
                "smoke-payload",
            ])
            .output()
            .unwrap();

        if output.status.success() && receipt_path.exists() {
            let verify = Command::new(bin)
                .args([
                    "receipts",
                    "verify",
                    "--dir",
                    receipt_path.to_str().unwrap(),
                    "--json",
                ])
                .output()
                .unwrap();
            if verify.status.success() {
                verify_output = Some(verify);
                break;
            }
            last_verify_output = Some(verify);
        }
        last_send_output = Some(output);
        thread::sleep(Duration::from_millis(200));
    }

    let output = match verify_output {
        Some(output) => output,
        None => {
            let send_details = last_send_output
                .as_ref()
                .map(|output| {
                    format!(
                        "last send output: {}{}",
                        String::from_utf8_lossy(&output.stdout),
                        String::from_utf8_lossy(&output.stderr)
                    )
                })
                .unwrap_or_else(|| "send command was never attempted".to_string());
            let verify_details = last_verify_output
                .as_ref()
                .map(|output| {
                    format!(
                        "last verify output: {}{}",
                        String::from_utf8_lossy(&output.stdout),
                        String::from_utf8_lossy(&output.stderr)
                    )
                })
                .unwrap_or_else(|| "receipt verification was never attempted".to_string());
            panic!("receipt log was not written and verified\n{send_details}\n{verify_details}");
        }
    };

    assert!(
        output.status.success(),
        "receipt verification failed: {}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["verified"], true, "{stdout}");
    assert!(
        report["receipts"].as_u64().unwrap_or_default() >= 1,
        "{stdout}"
    );
}
