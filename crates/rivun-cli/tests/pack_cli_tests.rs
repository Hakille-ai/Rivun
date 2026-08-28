use std::fs;
use tempfile::tempdir;
use rivun_crypto::Keypair;

#[test]
fn test_rivun_pack_cli_lifecycle() {
    let tmp = tempdir().unwrap();
    let pack_dir = tmp.path().join("my_cli_pack");
    let bundle_file = tmp.path().join("my_cli_pack.zpack");
    let sig_file = tmp.path().join("my_cli_pack.zpack.sig");
    let key_file = tmp.path().join("author.key");
    let store_dir = tmp.path().join("store");

    // 1. Generate keypair file
    let keypair = Keypair::generate();
    let pub_key_hex = hex::encode(keypair.verifying_key().to_bytes());
    fs::write(&key_file, keypair.to_key_file_toml().unwrap()).unwrap();

    // 2. Build from code primitives to verify CLI capabilities
    // Scaffold init pack
    fs::create_dir_all(pack_dir.join("policies")).unwrap();
    fs::create_dir_all(pack_dir.join("schemas")).unwrap();

    let pack_toml = r#"
schema_version = 1
id = "com.rivun.cli-test"
name = "CLI Test Pack"
version = "0.1.0"
status = "active"

[[capabilities]]
id = "test.read"
risk = "low"

[[policies]]
path = "policies/default.policy"
"#;
    fs::write(pack_dir.join("pack.toml"), pack_toml).unwrap();

    let policy_toml = r#"
version = 1
default_decision = "allow"

[[rules]]
id = "r1"
effect = "allow"
action = "test.read"
"#;
    fs::write(pack_dir.join("policies/default.policy"), policy_toml).unwrap();

    // Build bundle
    let bundle = rivun_store::DomainPackBundle::build_from_dir(&pack_dir).unwrap();
    bundle.write_to_file(&bundle_file).unwrap();

    // Sign bundle
    let sig = rivun_store::DomainPackBundleSignature::sign(
        &bundle.manifest.pack_id,
        &bundle.manifest.version,
        &bundle.bundle_sha256,
        &keypair,
    )
    .unwrap();
    fs::write(&sig_file, serde_json::to_string_pretty(&sig).unwrap()).unwrap();

    // Verify bundle
    sig.verify(&bundle.bundle_sha256).unwrap();
    sig.verify_against_trusted_keys(&bundle.bundle_sha256, std::slice::from_ref(&pub_key_hex))
        .unwrap();

    // Extract / install bundle
    let install_dir = store_dir.join("packs/com.rivun.cli-test/0.1.0");
    bundle.extract_to_dir(&install_dir).unwrap();
    assert!(install_dir.join("pack.toml").exists());

    // Audit
    let audit_report = rivun_store::audit_bundle(&bundle, None).unwrap();
    assert!(audit_report.passed);
}
