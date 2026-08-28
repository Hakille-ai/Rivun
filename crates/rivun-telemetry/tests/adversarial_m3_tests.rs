use flate2::read::GzDecoder;
use std::io::Read;
use uuid::Uuid;
use rivun_telemetry::{IncidentCapturer, ProcessState, SecretRedactor, SocketState, TarBuilder};

#[test]
fn test_adversarial_secret_redactor_leaks() {
    // 1. Transport Keys (hex key attached to name without spaces)
    let config_transport_key = r#"
[transport]
transport_key=0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef
"#;
    let redacted_transport = SecretRedactor::redact_text(config_transport_key);
    let leaks_transport_key = redacted_transport
        .contains("0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef");
    assert!(!leaks_transport_key, "transport_key must be redacted!");
    assert!(redacted_transport.contains("[REDACTED]"));

    // 2. PEM Block Private Key
    let pem_key = r#"
-----BEGIN PRIVATE KEY-----
MC4CAQAwBQYDK2VwBCIEINTx1234567890abcdef1234567890abcdef12345678
-----END PRIVATE KEY-----
"#;
    let redacted_pem = SecretRedactor::redact_text(pem_key);
    let leaks_pem_key =
        redacted_pem.contains("MC4CAQAwBQYDK2VwBCIEINTx1234567890abcdef1234567890abcdef12345678");
    assert!(!leaks_pem_key, "PEM Private Key must be redacted!");
    assert!(redacted_pem.contains("[REDACTED_PEM_KEY]"));

    // 3. API Key / Access Token / Bearer without colon
    let stripe_like_key = format!("sk_{}{}", "live_", "999888777666555444333222111");
    let tokens_config = format!(
        "api_key = \"{stripe_like_key}\"\naccess_token = \"at_secret_token_val_123\"\nbearer_token = \"bearer_secret_abc_987\""
    );
    let redacted_tokens = SecretRedactor::redact_text(&tokens_config);
    let leaks_api_key = redacted_tokens.contains(&stripe_like_key);
    let leaks_access_token = redacted_tokens.contains("at_secret_token_val_123");
    let leaks_bearer = redacted_tokens.contains("bearer_secret_abc_987");
    assert!(!leaks_api_key, "api_key must be redacted!");
    assert!(!leaks_access_token, "access_token must be redacted!");
    assert!(!leaks_bearer, "bearer_token must be redacted!");

    // 4. JSON line corruption
    let json_config = r#"{"secret_key": "my_secret_val", "node_id": "node_101", "port": 9090}"#;
    let redacted_json = SecretRedactor::redact_text(json_config);
    assert!(
        !redacted_json.contains("my_secret_val"),
        "secret_key value must be redacted"
    );
    assert!(
        redacted_json.trim().ends_with('}'),
        "JSON line must preserve trailing closing brace"
    );
    assert!(
        redacted_json.contains("node_101"),
        "JSON line must preserve non-sensitive fields"
    );
}

#[test]
fn test_adversarial_tar_builder_unpacking_and_gzip() {
    let mut builder = TarBuilder::new();
    builder.add_file("test1.txt", b"Hello Tar World!");
    let archive_bytes = builder.finish();

    // Verify 512-byte block alignment for raw tar
    assert_eq!(
        archive_bytes.len() % 512,
        0,
        "Tar archive size must be 512-byte aligned"
    );

    // Test gzip compressed incident archive
    let node_id = Uuid::new_v4();
    let snapshot = IncidentCapturer::capture(node_id, "test_metrics 1\n", None);
    let gz_bytes = IncidentCapturer::build_tar_gz_archive(&snapshot).unwrap();

    // Verify gzip magic header 0x1f, 0x8b
    assert!(gz_bytes.len() >= 2);
    assert_eq!(gz_bytes[0], 0x1f, "Gzip header magic byte 1");
    assert_eq!(gz_bytes[1], 0x8b, "Gzip header magic byte 2");

    // Decompress and verify inner tar archive
    let mut decoder = GzDecoder::new(&gz_bytes[..]);
    let mut decompressed_tar = Vec::new();
    decoder.read_to_end(&mut decompressed_tar).unwrap();

    assert_eq!(
        decompressed_tar.len() % 512,
        0,
        "Decompressed tar must be 512-byte aligned"
    );
    assert!(
        decompressed_tar.len() > 1024,
        "Decompressed tar must contain headers and content"
    );
}

#[test]
fn test_adversarial_process_and_socket_state_hardcoding() {
    let node_id = Uuid::new_v4();
    let snapshot1 = IncidentCapturer::capture(node_id, "metric 1", None);

    // Verify live process collection queries real OS PID
    assert_eq!(
        snapshot1.process.pid,
        std::process::id(),
        "Process PID must match live process ID"
    );
    assert!(
        snapshot1.process.rss_bytes > 0,
        "Process RSS must be non-zero"
    );
    assert!(
        snapshot1.process.vms_bytes > 0,
        "Process VMS must be non-zero"
    );
    assert!(
        snapshot1.process.thread_count >= 1,
        "Thread count must be at least 1"
    );

    // Verify socket state collection
    let sockets = SocketState::collect();
    assert!(
        !sockets.listening_ports.is_empty(),
        "Socket state must have listening ports"
    );
    assert!(
        !sockets.active_sockets.is_empty(),
        "Socket state must have active socket descriptions"
    );

    let process = ProcessState::collect();
    assert_eq!(process.pid, std::process::id());
}
