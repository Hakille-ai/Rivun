//! Adversarial Test Suite for Milestone 2 (Signed Domain Pack Lifecycle & Marketplace)
//! Location: .agents/challenger_m2_2/m2_adversarial_tests.rs

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use tempfile::tempdir;
use ed25519_dalek::{Signer, SigningKey};
use zap_crypto::Keypair;
use zap_store::{
    audit_bundle, audit_pack_dir, matches_version_req, DomainPackArtifactDigest,
    DomainPackBundle, DomainPackBundleManifest, DomainPackBundleSignature,
    DomainPackCompatibility, DomainPackDependencyResolver, DomainPackDependencySpec,
    DomainPackPolicyValidator, DomainPackRegistry, DomainPackRegistryEntry, DomainPackRisk,
    DomainPackStatus, ZapStoreError,
};

/// 1. Test corrupt bundle detection
#[test]
fn test_corrupt_bundle_magic_header_mismatch() {
    let mut bad_bytes = b"BADPACK1".to_vec();
    bad_bytes.extend_from_slice(&[0u8; 32]);
    let res = DomainPackBundle::decode_bytes(&bad_bytes);
    assert!(matches!(res, Err(ZapStoreError::InvalidDomainPackBundleFormat(msg)) if msg.contains("invalid ZPACK magic header")));
}

#[test]
fn test_corrupt_bundle_truncated_header() {
    let truncated_bytes = b"ZPACK001".to_vec();
    let res = DomainPackBundle::decode_bytes(&truncated_bytes);
    assert!(matches!(res, Err(ZapStoreError::InvalidDomainPackBundleFormat(msg)) if msg.contains("truncated bundle header")));
}

#[test]
fn test_corrupt_bundle_payload_tampering() {
    let tmp = tempdir().unwrap();
    let pack_dir = tmp.path().join("orig_pack");
    fs::create_dir_all(&pack_dir).unwrap();
    fs::write(
        pack_dir.join("pack.toml"),
        r#"
schema_version = 1
id = "com.zap.tamper"
name = "Tamper Pack"
version = "1.0.0"
status = "active"
"#,
    )
    .unwrap();

    let bundle = DomainPackBundle::build_from_dir(&pack_dir).unwrap();
    let mut encoded = bundle.encode_bytes();

    // Tamper with content bytes at the end of the file
    let len = encoded.len();
    encoded[len - 1] ^= 0xFF;

    let res = DomainPackBundle::decode_bytes(&encoded);
    assert!(res.is_err(), "Decoding tampered bundle payload must fail integrity check");
}

#[test]
fn test_corrupt_bundle_invalid_signature_digest_mismatch() {
    let keypair = Keypair::generate();
    let sig = DomainPackBundleSignature::sign("com.zap.test", "1.0.0", "digest_a", &keypair).unwrap();

    // Verification against different bundle digest must fail
    let res = sig.verify("digest_b");
    assert!(matches!(res, Err(ZapStoreError::DomainPackBundleDigestMismatch { .. })));
}

#[test]
fn test_corrupt_bundle_untrusted_signer() {
    let keypair = Keypair::generate();
    let pubkey_hex = hex::encode(keypair.public_key());
    let sig = DomainPackBundleSignature::sign("com.zap.test", "1.0.0", "digest_a", &keypair).unwrap();

    let untrusted_key = hex::encode(Keypair::generate().public_key());
    let res = sig.verify_against_trusted_keys("digest_a", &[untrusted_key]);
    assert!(matches!(res, Err(ZapStoreError::UntrustedDomainPackSigner { .. })));
}

/// 2. Test dependency resolution edge cases
#[test]
fn test_dependency_circular_self_reference() {
    let registry = DomainPackRegistry {
        schema_version: 1,
        operator_node_id: None,
        operator_public_key: None,
        signature: None,
        entries: vec![],
    };

    let resolver = DomainPackDependencyResolver::new(&registry);
    let self_dep = DomainPackDependencySpec {
        pack_id: "com.zap.self".to_string(),
        version_req: "^1.0.0".to_string(),
        optional: false,
    };

    let res = resolver.resolve("com.zap.self", "1.0.0", &[self_dep]);
    assert!(matches!(res, Err(ZapStoreError::CircularDomainPackDependency(id)) if id == "com.zap.self"));
}

#[test]
fn test_dependency_version_mismatch() {
    let entry = DomainPackRegistryEntry {
        id: "com.zap.dep".to_string(),
        name: "Dep Pack".to_string(),
        version: "1.5.0".to_string(),
        status: DomainPackStatus::Active,
        risk: DomainPackRisk::Low,
        author_node_id: uuid::Uuid::nil(),
        compatibility: DomainPackCompatibility::default(),
        manifest: zap_store::DomainPackArtifact {
            relative_path: "pack.toml".to_string(),
            sha256_hex: "00".to_string(),
            size_bytes: 10,
            content_type: "application/toml".to_string(),
        },
        archive: None,
        policies: vec![],
        schemas: vec![],
        drivers: vec![],
        metadata: BTreeMap::new(),
    };

    let registry = DomainPackRegistry {
        schema_version: 1,
        operator_node_id: None,
        operator_public_key: None,
        signature: None,
        entries: vec![entry],
    };

    let resolver = DomainPackDependencyResolver::new(&registry);
    let dep_spec = DomainPackDependencySpec {
        pack_id: "com.zap.dep".to_string(),
        version_req: "^2.0.0".to_string(),
        optional: false,
    };

    let res = resolver.resolve("com.zap.app", "1.0.0", &[dep_spec]);
    assert!(matches!(res, Err(ZapStoreError::UnsatisfiedDomainPackDependency { pack_id, requirement })
        if pack_id == "com.zap.dep" && requirement == "^2.0.0"));
}

/// 3. Test security policy risk auditing
#[test]
fn test_security_audit_exceeding_max_risk() {
    let tmp = tempdir().unwrap();
    let pack_dir = tmp.path().join("high_risk_pack");
    fs::create_dir_all(&pack_dir).unwrap();
    fs::write(
        pack_dir.join("pack.toml"),
        r#"
schema_version = 1
id = "com.zap.highrisk"
name = "High Risk Pack"
version = "1.0.0"
status = "active"

[[capabilities]]
id = "sys.raw_io"
risk = "high"
"#,
    )
    .unwrap();

    let report = audit_pack_dir(&pack_dir, Some(DomainPackRisk::Medium)).unwrap();
    assert_eq!(report.overall_risk, DomainPackRisk::High);
    assert!(!report.passed, "Audit must fail when overall risk High exceeds max risk Medium");
}

#[test]
fn test_audit_bundle_revoked_status() {
    let tmp = tempdir().unwrap();
    let pack_dir = tmp.path().join("revoked_pack");
    fs::create_dir_all(&pack_dir).unwrap();
    fs::write(
        pack_dir.join("pack.toml"),
        r#"
schema_version = 1
id = "com.zap.revoked"
name = "Revoked Pack"
version = "1.0.0"
status = "revoked"
"#,
    )
    .unwrap();

    let bundle = DomainPackBundle::build_from_dir(&pack_dir).unwrap();
    let report = audit_bundle(&bundle, Some(DomainPackRisk::High)).unwrap();

    assert_eq!(report.overall_risk, DomainPackRisk::Critical);
    assert!(!report.passed, "Audit bundle must fail for revoked pack when max risk is High");
}
