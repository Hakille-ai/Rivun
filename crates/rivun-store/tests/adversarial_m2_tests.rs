use std::fs;
use tempfile::tempdir;
use rivun_store::{
    DomainPackCompatibility, DomainPackRegistry, DomainPackRegistryEntry, DomainPackRisk,
    DomainPackStatus,
    bundle::{DomainPackBundle, DomainPackBundleManifest},
    resolver::{DomainPackDependencyResolver, DomainPackDependencySpec, matches_version_req},
    validator::DomainPackPolicyValidator,
};

#[test]
fn test_path_traversal_zip_slip_vulnerability() {
    let tmp = tempdir().unwrap();
    let target_dir = tmp.path().join("sandbox");
    let outside_file = tmp.path().join("escaped.txt");

    let manifest = DomainPackBundleManifest {
        schema_version: 1,
        pack_id: "com.rivun.malicious".to_string(),
        name: "Malicious Pack".to_string(),
        version: "1.0.0".to_string(),
        status: DomainPackStatus::Active,
        created_at_micros: 0,
        artifacts: vec![],
    };

    let mut files = std::collections::BTreeMap::new();
    // Path traversal payload
    files.insert("../escaped.txt".to_string(), b"PWNED".to_vec());

    let bundle = DomainPackBundle {
        manifest,
        raw_bytes: vec![],
        bundle_sha256: "00".to_string(),
        files,
    };

    // Extract bundle
    let res = bundle.extract_to_dir(&target_dir);
    assert!(res.is_err(), "extract_to_dir must reject path traversal!");

    // Check if file escaped sandbox
    assert!(
        !outside_file.exists(),
        "CRITICAL VULNERABILITY: extract_to_dir allowed path traversal outside target_dir!"
    );
}

#[test]
fn test_version_req_semver_and_invalid_inputs() {
    // Caret for 0.x versions: ^0.1.0 should NOT match 0.2.0 in SemVer rules
    let matches_caret_breaking = matches_version_req("0.2.0", "^0.1.0");
    assert!(
        !matches_caret_breaking,
        "BUG: ^0.1.0 matched 0.2.0 which violates SemVer 0.x breaking rules!"
    );

    // Invalid requirement string: should return false
    let matches_invalid = matches_version_req("1.0.0", "INVALID_SPEC_123");
    assert!(
        !matches_invalid,
        "BUG: matches_version_req returned true for invalid requirement string!"
    );
}

#[test]
fn test_policy_validator_ignores_non_keyword_policy_files() {
    let tmp = tempdir().unwrap();
    let pack_dir = tmp.path().join("policy_pack");
    fs::create_dir_all(&pack_dir).unwrap();

    // Invalid TOML policy content
    let invalid_policy = r#"THIS IS NOT VALID TOML policy = {"#;

    // File named custom_rules.toml (not containing "policy" in filename)
    fs::write(pack_dir.join("custom_rules.toml"), invalid_policy).unwrap();
    fs::write(
        pack_dir.join("pack.toml"),
        r#"
schema_version = 1
id = "com.rivun.custompolicy"
name = "Custom Policy Pack"
version = "1.0.0"
status = "active"

[[policies]]
path = "custom_rules.toml"
"#,
    )
    .unwrap();

    let bundle = DomainPackBundle::build_from_dir(&pack_dir).unwrap();
    let res = DomainPackPolicyValidator::validate_bundle_policies(&bundle);

    // If validator checked custom_rules.toml, valid will be false!
    assert!(
        !res.valid,
        "BUG: DomainPackPolicyValidator ignored custom_rules.toml declared in [[policies]]!"
    );
}

#[test]
fn test_transitive_dependency_resolution() {
    // Registry entries for A -> B -> C
    let entry_c = DomainPackRegistryEntry {
        id: "com.rivun.c".to_string(),
        name: "Pack C".to_string(),
        version: "1.0.0".to_string(),
        status: DomainPackStatus::Active,
        risk: DomainPackRisk::Low,
        description: None,
        deprecated_reason: None,
        revoked_reason: None,
        author_node_id: uuid::Uuid::nil(),
        compatibility: DomainPackCompatibility::default(),
        manifest: rivun_store::DomainPackArtifact {
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

    let entry_b = DomainPackRegistryEntry {
        id: "com.rivun.b".to_string(),
        name: "Pack B".to_string(),
        version: "1.0.0".to_string(),
        status: DomainPackStatus::Active,
        risk: DomainPackRisk::Low,
        description: None,
        deprecated_reason: None,
        revoked_reason: None,
        author_node_id: uuid::Uuid::nil(),
        compatibility: DomainPackCompatibility::default(),
        manifest: rivun_store::DomainPackArtifact {
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
        dependencies: vec![DomainPackDependencySpec {
            pack_id: "com.rivun.c".to_string(),
            version_req: "^1.0.0".to_string(),
            optional: false,
        }],
        labels: vec![],
    };

    let registry = DomainPackRegistry {
        schema_version: 1,
        generated_by: None,
        channel: None,
        operator_node_id: None,
        operator_public_key: None,
        signature: None,
        entries: vec![entry_b, entry_c],
    };

    let resolver = DomainPackDependencyResolver::new(&registry);
    let deps_a = vec![DomainPackDependencySpec {
        pack_id: "com.rivun.b".to_string(),
        version_req: "^1.0.0".to_string(),
        optional: false,
    }];

    let plan = resolver.resolve("com.rivun.a", "1.0.0", &deps_a).unwrap();
    // B depends on C, so install_order should contain B and C.
    let resolved_ids: Vec<String> = plan.install_order.iter().map(|e| e.id.clone()).collect();
    assert!(
        resolved_ids.contains(&"com.rivun.c".to_string()),
        "BUG: Transitive dependency C was not resolved when resolving A -> B -> C!"
    );
}
