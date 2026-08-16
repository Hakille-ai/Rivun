//! Signed Domain Pack lifecycle, packaging, verification, and offline resolution.

pub use zap_store::ZapStoreError as ZapPackError;
pub use zap_store::audit::*;
pub use zap_store::bundle::*;
pub use zap_store::resolver::*;
pub use zap_store::validator::*;

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use zap_crypto::Keypair;

    #[test]
    fn test_pack_bundle_lifecycle() {
        let dir = tempdir().unwrap();
        let pack_dir = dir.path().join("my_pack");
        std::fs::create_dir_all(&pack_dir).unwrap();

        let manifest_content = r#"
schema_version = 1
id = "com.example.testpack"
name = "Test Pack"
version = "1.0.0"
status = "active"

[[capabilities]]
id = "test.read"
risk = "low"
"#;
        std::fs::write(pack_dir.join("pack.toml"), manifest_content).unwrap();

        // Build
        let bundle = DomainPackBundle::build_from_dir(&pack_dir).unwrap();
        assert_eq!(bundle.manifest.pack_id, "com.example.testpack");
        assert_eq!(bundle.manifest.version, "1.0.0");

        // Sign
        let key = Keypair::generate();
        let sig = DomainPackBundleSignature::sign(
            &bundle.manifest.pack_id,
            &bundle.manifest.version,
            &bundle.bundle_sha256,
            &key,
        )
        .unwrap();

        // Verify
        sig.verify(&bundle.bundle_sha256).unwrap();

        // Audit
        let report = audit_bundle(&bundle, None).unwrap();
        assert!(report.passed);
    }
}
