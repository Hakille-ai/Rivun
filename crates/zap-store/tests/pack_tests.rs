use std::fs;
use tempfile::tempdir;
use zap_crypto::Keypair;
use zap_store::{
    DomainPackBundle, DomainPackBundleSignature, DomainPackCompatibility,
    DomainPackDependencyResolver, DomainPackDependencySpec, DomainPackPolicyValidator,
    DomainPackRegistry, DomainPackRegistryEntry, DomainPackRisk, DomainPackStatus, audit_pack_dir,
};

#[test]
fn test_bundle_creation_extraction_and_signing() {
    let tmp = tempdir().unwrap();
    let pack_dir = tmp.path().join("test_pack");
    fs::create_dir_all(pack_dir.join("policies")).unwrap();

    let pack_toml = r#"
schema_version = 1
id = "com.zap.finance"
name = "Finance Pack"
version = "1.2.3"
status = "active"

[[capabilities]]
id = "finance.transfer"
risk = "high"

[[policies]]
path = "policies/transfer.policy"
"#;
    fs::write(pack_dir.join("pack.toml"), pack_toml).unwrap();

    let policy_toml = r#"
version = 1
default_decision = "deny"

[[rules]]
id = "allow_transfer"
effect = "allow"
action = "finance.transfer"
"#;
    fs::write(pack_dir.join("policies/transfer.policy"), policy_toml).unwrap();

    // 1. Build bundle
    let bundle = DomainPackBundle::build_from_dir(&pack_dir).unwrap();
    assert_eq!(bundle.manifest.pack_id, "com.zap.finance");
    assert_eq!(bundle.manifest.version, "1.2.3");
    assert_eq!(bundle.manifest.artifacts.len(), 2);

    // 2. Sign bundle
    let keypair = Keypair::generate();
    let sig = DomainPackBundleSignature::sign(
        &bundle.manifest.pack_id,
        &bundle.manifest.version,
        &bundle.bundle_sha256,
        &keypair,
    )
    .unwrap();

    // 3. Verify signature
    sig.verify(&bundle.bundle_sha256).unwrap();

    let pub_key_hex = hex::encode(keypair.verifying_key().to_bytes());
    sig.verify_against_trusted_keys(&bundle.bundle_sha256, std::slice::from_ref(&pub_key_hex))
        .unwrap();

    // 4. Untrusted signer fails
    let wrong_key = hex::encode(Keypair::generate().verifying_key().to_bytes());
    assert!(
        sig.verify_against_trusted_keys(&bundle.bundle_sha256, &[wrong_key])
            .is_err()
    );

    // 5. Test Roundtrip encode/decode
    let zpack_file = tmp.path().join("finance-1.2.3.zpack");
    bundle.write_to_file(&zpack_file).unwrap();

    let loaded_bundle = DomainPackBundle::open_from_file(&zpack_file).unwrap();
    assert_eq!(loaded_bundle.manifest.pack_id, "com.zap.finance");
    assert_eq!(loaded_bundle.bundle_sha256, bundle.bundle_sha256);

    // 6. Test Extraction
    let extract_dir = tmp.path().join("extracted");
    loaded_bundle.extract_to_dir(&extract_dir).unwrap();
    assert!(extract_dir.join("pack.toml").exists());
    assert!(extract_dir.join("policies/transfer.policy").exists());
}

#[test]
fn test_policy_validator() {
    let tmp = tempdir().unwrap();
    let pack_dir = tmp.path().join("policy_pack");
    fs::create_dir_all(pack_dir.join("policies")).unwrap();

    fs::write(
        pack_dir.join("pack.toml"),
        r#"
schema_version = 1
id = "com.zap.policy"
name = "Policy Pack"
version = "0.1.0"
status = "active"
"#,
    )
    .unwrap();

    let valid_policy = r#"
version = 1
default_decision = "allow"

[[rules]]
id = "r1"
effect = "deny"
action = "system.shutdown"
"#;
    fs::write(pack_dir.join("policies/p1.policy"), valid_policy).unwrap();

    let bundle = DomainPackBundle::build_from_dir(&pack_dir).unwrap();
    let res = DomainPackPolicyValidator::validate_bundle_policies(&bundle);
    assert!(res.valid);
    assert_eq!(res.policy_rule_count, 1);
}

#[test]
fn test_dependency_resolver() {
    let entry_a = DomainPackRegistryEntry {
        id: "com.zap.core".to_string(),
        name: "Core Pack".to_string(),
        version: "1.0.0".to_string(),
        status: DomainPackStatus::Active,
        risk: DomainPackRisk::Low,
        description: None,
        deprecated_reason: None,
        revoked_reason: None,
        author_node_id: uuid::Uuid::nil(),
        compatibility: DomainPackCompatibility {
            min_zap_version: None,
            max_zap_version: None,
            runtimes: vec![],
            abi_versions: vec![],
            zap_version_req: ">=0.1.0".to_string(),
            abi_version_req: ">=1".to_string(),
            capabilities_required: vec![],
            capabilities_provided: vec!["core.init".to_string()],
        },
        manifest: zap_store::DomainPackArtifact {
            path: "pack.toml".to_string(),
            hash: "00".to_string(),
            content_type: Some("application/toml".to_string()),
            size_bytes: Some(10),
            relative_path: Some("pack.toml".to_string()),
            sha256_hex: Some("00".to_string()),
        },
        archive: None,
        policies: vec![],
        schemas: vec![],
        drivers: vec![],
        metadata: std::collections::BTreeMap::new(),
        dependencies: vec![],
        labels: vec![],
    };

    let registry = DomainPackRegistry {
        schema_version: 1,
        generated_by: None,
        channel: None,
        operator_node_id: None,
        operator_public_key: None,
        signature: None,
        entries: vec![entry_a],
    };

    let resolver = DomainPackDependencyResolver::new(&registry);
    let deps = vec![DomainPackDependencySpec {
        pack_id: "com.zap.core".to_string(),
        version_req: "^1.0.0".to_string(),
        optional: false,
    }];

    let plan = resolver.resolve("com.zap.app", "0.1.0", &deps).unwrap();
    assert_eq!(plan.install_order.len(), 1);
    assert_eq!(plan.install_order[0].id, "com.zap.core");
    assert_eq!(plan.provided_capabilities, vec!["core.init"]);
}

#[test]
fn test_security_audit() {
    let tmp = tempdir().unwrap();
    let pack_dir = tmp.path().join("audit_pack");
    fs::create_dir_all(&pack_dir).unwrap();

    fs::write(
        pack_dir.join("pack.toml"),
        r#"
schema_version = 1
id = "com.zap.audit"
name = "Audit Pack"
version = "0.1.0"
status = "active"

[[capabilities]]
id = "root.execute"
risk = "critical"
"#,
    )
    .unwrap();

    let report = audit_pack_dir(&pack_dir, Some(DomainPackRisk::Medium)).unwrap();
    assert_eq!(report.overall_risk, DomainPackRisk::Critical);
    assert!(!report.passed);
    assert_eq!(report.issues.len(), 1);
}
