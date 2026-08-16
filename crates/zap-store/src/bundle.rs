use base64::{Engine as _, engine::general_purpose::STANDARD_NO_PAD};
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use uuid::Uuid;
use zap_crypto::{Keypair, node_id_from_public_key};

use crate::{DomainPackStatus, ZapStoreError};

pub const DOMAIN_PACK_BUNDLE_SIGNATURE_DOMAIN: &[u8] = b"ZAP-DOMAIN-PACK-BUNDLE-v1";
const ZPACK_MAGIC: &[u8; 8] = b"ZPACK001";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DomainPackArtifactDigest {
    pub relative_path: String,
    pub sha256_hex: String,
    pub size_bytes: u64,
    pub content_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DomainPackBundleManifest {
    pub schema_version: u8,
    pub pack_id: String,
    pub name: String,
    pub version: String,
    pub status: DomainPackStatus,
    pub created_at_micros: u64,
    pub artifacts: Vec<DomainPackArtifactDigest>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DomainPackBundleSignature {
    pub schema_version: u8,
    pub pack_id: String,
    pub pack_version: String,
    pub bundle_sha256: String,
    pub signer_node_id: Uuid,
    pub signer_public_key: String, // Hex or Base64
    pub signature: String,         // Base64
    pub signed_at_micros: u64,
}

#[derive(Serialize)]
struct BundleSigningPayload<'a> {
    domain: &'static str,
    pack_id: &'a str,
    pack_version: &'a str,
    bundle_sha256: &'a str,
    signer_node_id: Uuid,
    signer_public_key: &'a str,
    signed_at_micros: u64,
}

impl DomainPackBundleSignature {
    pub fn sign(
        pack_id: &str,
        pack_version: &str,
        bundle_sha256: &str,
        keypair: &Keypair,
    ) -> Result<Self, ZapStoreError> {
        let public_key = keypair.verifying_key().to_bytes();
        let pub_key_hex = hex::encode(public_key);
        let signer_node_id = node_id_from_public_key(&public_key);
        let signed_at_micros = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_micros() as u64;

        let payload = BundleSigningPayload {
            domain: "ZAP-DOMAIN-PACK-BUNDLE-v1",
            pack_id,
            pack_version,
            bundle_sha256,
            signer_node_id,
            signer_public_key: &pub_key_hex,
            signed_at_micros,
        };

        let payload_bytes = serde_json::to_vec(&payload)?;
        let signing_key = SigningKey::from_bytes(&keypair.secret_bytes());
        let signature = signing_key.sign(&payload_bytes);
        let sig_b64 = STANDARD_NO_PAD.encode(signature.to_bytes());

        Ok(DomainPackBundleSignature {
            schema_version: 1,
            pack_id: pack_id.to_string(),
            pack_version: pack_version.to_string(),
            bundle_sha256: bundle_sha256.to_string(),
            signer_node_id,
            signer_public_key: pub_key_hex,
            signature: sig_b64,
            signed_at_micros,
        })
    }

    pub fn verify(&self, expected_bundle_sha256: &str) -> Result<(), ZapStoreError> {
        if self.bundle_sha256 != expected_bundle_sha256 {
            return Err(ZapStoreError::DomainPackBundleDigestMismatch {
                path: "bundle.zpack".to_string(),
                expected: expected_bundle_sha256.to_string(),
                actual: self.bundle_sha256.clone(),
            });
        }

        let pub_key_bytes = parse_public_key_str(&self.signer_public_key)?;
        let derived_node_id = node_id_from_public_key(&pub_key_bytes);
        if derived_node_id != self.signer_node_id {
            return Err(ZapStoreError::DomainPackRegistryOperatorNodeMismatch {
                declared: self.signer_node_id,
                derived: derived_node_id,
            });
        }

        let verifying_key = VerifyingKey::from_bytes(&pub_key_bytes)
            .map_err(|_| ZapStoreError::InvalidDomainPackBundleSignature)?;

        let sig_bytes = STANDARD_NO_PAD
            .decode(&self.signature)
            .map_err(|_| ZapStoreError::InvalidDomainPackBundleSignature)?;

        let sig_arr: [u8; 64] = sig_bytes
            .try_into()
            .map_err(|_| ZapStoreError::InvalidDomainPackBundleSignature)?;
        let signature = Signature::from_bytes(&sig_arr);

        let payload = BundleSigningPayload {
            domain: "ZAP-DOMAIN-PACK-BUNDLE-v1",
            pack_id: &self.pack_id,
            pack_version: &self.pack_version,
            bundle_sha256: &self.bundle_sha256,
            signer_node_id: self.signer_node_id,
            signer_public_key: &self.signer_public_key,
            signed_at_micros: self.signed_at_micros,
        };

        let payload_bytes = serde_json::to_vec(&payload)?;

        verifying_key
            .verify(&payload_bytes, &signature)
            .map_err(|_| ZapStoreError::InvalidDomainPackBundleSignature)?;

        Ok(())
    }

    pub fn verify_against_trusted_keys(
        &self,
        expected_bundle_sha256: &str,
        trusted_public_keys: &[String],
    ) -> Result<(), ZapStoreError> {
        self.verify(expected_bundle_sha256)?;

        if !trusted_public_keys.is_empty() {
            let signer_key_bytes = parse_public_key_str(&self.signer_public_key).ok();
            let mut matched = false;
            for trusted in trusted_public_keys {
                let cleaned = trusted.trim().to_lowercase();
                if cleaned == self.signer_public_key.to_lowercase() {
                    matched = true;
                    break;
                }
                if let (Some(sig_bytes), Ok(trust_bytes)) =
                    (signer_key_bytes, parse_public_key_str(&cleaned))
                    && sig_bytes == trust_bytes
                {
                    matched = true;
                    break;
                }
            }
            if !matched {
                return Err(ZapStoreError::UntrustedDomainPackSigner {
                    signer: self.signer_public_key.clone(),
                });
            }
        }

        Ok(())
    }
}

fn parse_public_key_str(s: &str) -> Result<[u8; 32], ZapStoreError> {
    let s = s.trim();
    if let Ok(bytes) = hex::decode(s)
        && bytes.len() == 32
    {
        let mut arr = [0u8; 32];
        arr.copy_from_slice(&bytes);
        return Ok(arr);
    }
    if let Ok(bytes) = STANDARD_NO_PAD.decode(s)
        && bytes.len() == 32
    {
        let mut arr = [0u8; 32];
        arr.copy_from_slice(&bytes);
        return Ok(arr);
    }
    Err(ZapStoreError::InvalidDomainPackBundleSignature)
}

#[derive(Debug, Clone)]
pub struct DomainPackBundle {
    pub manifest: DomainPackBundleManifest,
    pub raw_bytes: Vec<u8>,
    pub bundle_sha256: String,
    pub files: BTreeMap<String, Vec<u8>>,
}

fn compute_sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

impl DomainPackBundle {
    pub fn build_from_dir(pack_dir: &Path) -> Result<Self, ZapStoreError> {
        let manifest_path = pack_dir.join("pack.toml");
        if !manifest_path.exists() {
            return Err(ZapStoreError::InvalidDomainPackBundleFormat(format!(
                "missing pack.toml in {}",
                pack_dir.display()
            )));
        }

        let manifest_toml_str = fs::read_to_string(&manifest_path)
            .map_err(|e| ZapStoreError::IoError(e.to_string()))?;
        let pack_toml: serde_json::Value = toml::from_str(&manifest_toml_str)
            .map_err(|e| ZapStoreError::InvalidDomainPackBundleFormat(e.to_string()))?;

        let pack_id = pack_toml
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ZapStoreError::EmptyDomainPackId)?
            .to_string();

        let name = pack_toml
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or(&pack_id)
            .to_string();

        let version = pack_toml
            .get("version")
            .and_then(|v| v.as_str())
            .unwrap_or("0.1.0")
            .to_string();

        let status_str = pack_toml
            .get("status")
            .and_then(|v| v.as_str())
            .unwrap_or("active");
        let status = match status_str.to_lowercase().as_str() {
            "deprecated" => DomainPackStatus::Deprecated,
            "revoked" => DomainPackStatus::Revoked,
            "draft" => DomainPackStatus::Draft,
            _ => DomainPackStatus::Active,
        };

        let mut files = BTreeMap::new();
        let mut artifacts = Vec::new();

        fn walk_dir(
            base_dir: &Path,
            current_dir: &Path,
            files: &mut BTreeMap<String, Vec<u8>>,
            artifacts: &mut Vec<DomainPackArtifactDigest>,
        ) -> Result<(), ZapStoreError> {
            let entries =
                fs::read_dir(current_dir).map_err(|e| ZapStoreError::IoError(e.to_string()))?;
            for entry in entries {
                let entry = entry.map_err(|e| ZapStoreError::IoError(e.to_string()))?;
                let path = entry.path();
                let file_name = path.file_name().unwrap_or_default().to_string_lossy();

                if file_name.starts_with('.')
                    || file_name == "target"
                    || file_name == "node_modules"
                    || file_name.ends_with(".zpack")
                    || file_name.ends_with(".sig")
                {
                    continue;
                }

                if path.is_dir() {
                    walk_dir(base_dir, &path, files, artifacts)?;
                } else if path.is_file() {
                    let rel_path = path
                        .strip_prefix(base_dir)
                        .map_err(|_| {
                            ZapStoreError::InvalidDomainPackArtifactPath(path.display().to_string())
                        })?
                        .to_string_lossy()
                        .replace('\\', "/");

                    let content =
                        fs::read(&path).map_err(|e| ZapStoreError::IoError(e.to_string()))?;
                    let sha256_hex = compute_sha256_hex(&content);
                    let size_bytes = content.len() as u64;

                    let content_type = if rel_path.ends_with(".toml") {
                        "application/toml"
                    } else if rel_path.ends_with(".json") {
                        "application/json"
                    } else if rel_path.ends_with(".wasm") {
                        "application/wasm"
                    } else if rel_path.ends_with(".md") {
                        "text/markdown"
                    } else {
                        "application/octet-stream"
                    }
                    .to_string();

                    artifacts.push(DomainPackArtifactDigest {
                        relative_path: rel_path.clone(),
                        sha256_hex,
                        size_bytes,
                        content_type,
                    });

                    files.insert(rel_path, content);
                }
            }
            Ok(())
        }

        walk_dir(pack_dir, pack_dir, &mut files, &mut artifacts)?;

        let created_at_micros = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_micros() as u64;

        let manifest = DomainPackBundleManifest {
            schema_version: 1,
            pack_id,
            name,
            version,
            status,
            created_at_micros,
            artifacts,
        };

        let mut bundle = DomainPackBundle {
            manifest,
            raw_bytes: Vec::new(),
            bundle_sha256: String::new(),
            files,
        };

        bundle.raw_bytes = bundle.encode_bytes();
        bundle.bundle_sha256 = compute_sha256_hex(&bundle.raw_bytes);

        Ok(bundle)
    }

    pub fn encode_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(ZPACK_MAGIC);

        let manifest_json = serde_json::to_vec(&self.manifest).unwrap_or_default();
        let manifest_len = manifest_json.len() as u32;
        buf.extend_from_slice(&manifest_len.to_be_bytes());
        buf.extend_from_slice(&manifest_json);

        let files_count = self.files.len() as u32;
        buf.extend_from_slice(&files_count.to_be_bytes());

        for (rel_path, content) in &self.files {
            let path_bytes = rel_path.as_bytes();
            let path_len = path_bytes.len() as u16;
            buf.extend_from_slice(&path_len.to_be_bytes());
            buf.extend_from_slice(path_bytes);

            let content_len = content.len() as u64;
            buf.extend_from_slice(&content_len.to_be_bytes());
            buf.extend_from_slice(content);
        }

        buf
    }

    pub fn decode_bytes(bytes: &[u8]) -> Result<Self, ZapStoreError> {
        if bytes.len() < 16 || &bytes[0..8] != ZPACK_MAGIC {
            return Err(ZapStoreError::InvalidDomainPackBundleFormat(
                "invalid ZPACK magic header".to_string(),
            ));
        }

        let mut offset = 8;

        if offset + 4 > bytes.len() {
            return Err(ZapStoreError::InvalidDomainPackBundleFormat(
                "truncated bundle header".to_string(),
            ));
        }
        let manifest_len =
            u32::from_be_bytes(bytes[offset..offset + 4].try_into().unwrap()) as usize;
        offset += 4;

        if offset + manifest_len > bytes.len() {
            return Err(ZapStoreError::InvalidDomainPackBundleFormat(
                "truncated manifest payload".to_string(),
            ));
        }
        let manifest_bytes = &bytes[offset..offset + manifest_len];
        offset += manifest_len;

        let manifest: DomainPackBundleManifest = serde_json::from_slice(manifest_bytes)
            .map_err(|e| ZapStoreError::InvalidDomainPackBundleFormat(e.to_string()))?;

        if offset + 4 > bytes.len() {
            return Err(ZapStoreError::InvalidDomainPackBundleFormat(
                "truncated files header".to_string(),
            ));
        }
        let files_count =
            u32::from_be_bytes(bytes[offset..offset + 4].try_into().unwrap()) as usize;
        offset += 4;

        let mut files = BTreeMap::new();

        for _ in 0..files_count {
            if offset + 2 > bytes.len() {
                return Err(ZapStoreError::InvalidDomainPackBundleFormat(
                    "truncated file path len".to_string(),
                ));
            }
            let path_len =
                u16::from_be_bytes(bytes[offset..offset + 2].try_into().unwrap()) as usize;
            offset += 2;

            if offset + path_len > bytes.len() {
                return Err(ZapStoreError::InvalidDomainPackBundleFormat(
                    "truncated file path".to_string(),
                ));
            }
            let rel_path = String::from_utf8(bytes[offset..offset + path_len].to_vec())
                .map_err(|e| ZapStoreError::InvalidDomainPackBundleFormat(e.to_string()))?;
            offset += path_len;

            let rel_path_buf = PathBuf::from(&rel_path);
            for component in rel_path_buf.components() {
                match component {
                    std::path::Component::ParentDir
                    | std::path::Component::RootDir
                    | std::path::Component::Prefix(_) => {
                        return Err(ZapStoreError::InvalidDomainPackArtifactPath(format!(
                            "path traversal in bundle file path: {}",
                            rel_path
                        )));
                    }
                    _ => {}
                }
            }

            if offset + 8 > bytes.len() {
                return Err(ZapStoreError::InvalidDomainPackBundleFormat(
                    "truncated file content len".to_string(),
                ));
            }
            let content_len =
                u64::from_be_bytes(bytes[offset..offset + 8].try_into().unwrap()) as usize;
            offset += 8;

            if offset + content_len > bytes.len() {
                return Err(ZapStoreError::InvalidDomainPackBundleFormat(
                    "truncated file content".to_string(),
                ));
            }
            let content = bytes[offset..offset + content_len].to_vec();
            offset += content_len;

            files.insert(rel_path, content);
        }

        let bundle_sha256 = compute_sha256_hex(bytes);

        let bundle = DomainPackBundle {
            manifest,
            raw_bytes: bytes.to_vec(),
            bundle_sha256,
            files,
        };

        bundle.verify_integrity()?;

        Ok(bundle)
    }

    pub fn open_from_file(bundle_path: &Path) -> Result<Self, ZapStoreError> {
        let bytes = fs::read(bundle_path).map_err(|e| {
            ZapStoreError::IoError(format!(
                "failed to read bundle file {}: {}",
                bundle_path.display(),
                e
            ))
        })?;
        Self::decode_bytes(&bytes)
    }

    pub fn write_to_file(&self, output_path: &Path) -> Result<(), ZapStoreError> {
        if let Some(parent) = output_path.parent() {
            fs::create_dir_all(parent).map_err(|e| ZapStoreError::IoError(e.to_string()))?;
        }
        fs::write(output_path, &self.raw_bytes).map_err(|e| {
            ZapStoreError::IoError(format!(
                "failed to write bundle file {}: {}",
                output_path.display(),
                e
            ))
        })
    }

    pub fn verify_integrity(&self) -> Result<(), ZapStoreError> {
        for artifact in &self.manifest.artifacts {
            let Some(content) = self.files.get(&artifact.relative_path) else {
                return Err(ZapStoreError::InvalidDomainPackBundleFormat(format!(
                    "missing artifact {}",
                    artifact.relative_path
                )));
            };

            if content.len() as u64 != artifact.size_bytes {
                return Err(ZapStoreError::DomainPackArtifactHashMismatch {
                    path: artifact.relative_path.clone(),
                    expected: format!("size {}", artifact.size_bytes),
                    actual: format!("size {}", content.len()),
                });
            }

            let actual_sha256 = compute_sha256_hex(content);
            if actual_sha256 != artifact.sha256_hex {
                return Err(ZapStoreError::DomainPackArtifactHashMismatch {
                    path: artifact.relative_path.clone(),
                    expected: artifact.sha256_hex.clone(),
                    actual: actual_sha256,
                });
            }
        }
        Ok(())
    }

    pub fn extract_to_dir(&self, target_dir: &Path) -> Result<(), ZapStoreError> {
        let canonical_target = target_dir
            .canonicalize()
            .or_else(|_| {
                fs::create_dir_all(target_dir)?;
                target_dir.canonicalize()
            })
            .map_err(|e| ZapStoreError::IoError(e.to_string()))?;

        for (rel_path, content) in &self.files {
            let rel_path_buf = PathBuf::from(rel_path);
            for component in rel_path_buf.components() {
                match component {
                    std::path::Component::ParentDir => {
                        return Err(ZapStoreError::InvalidDomainPackArtifactPath(format!(
                            "path traversal detected in artifact path: {}",
                            rel_path
                        )));
                    }
                    std::path::Component::RootDir | std::path::Component::Prefix(_) => {
                        return Err(ZapStoreError::InvalidDomainPackArtifactPath(format!(
                            "absolute path detected in artifact path: {}",
                            rel_path
                        )));
                    }
                    _ => {}
                }
            }

            let out_path = target_dir.join(&rel_path_buf);
            if let Some(parent) = out_path.parent() {
                fs::create_dir_all(parent).map_err(|e| ZapStoreError::IoError(e.to_string()))?;
            }

            let canonical_parent = out_path
                .parent()
                .ok_or_else(|| ZapStoreError::InvalidDomainPackArtifactPath(rel_path.clone()))?
                .canonicalize()
                .map_err(|e| ZapStoreError::IoError(e.to_string()))?;

            if !canonical_parent.starts_with(&canonical_target) {
                return Err(ZapStoreError::InvalidDomainPackArtifactPath(format!(
                    "path traversal outside target directory: {}",
                    rel_path
                )));
            }

            fs::write(&out_path, content).map_err(|e| {
                ZapStoreError::IoError(format!(
                    "failed to write extracted file {}: {}",
                    out_path.display(),
                    e
                ))
            })?;
        }

        Ok(())
    }
}
