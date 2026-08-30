//! Local verification and atomic staging of policy bundles.

use base64::{Engine as _, engine::general_purpose::STANDARD_NO_PAD};
use std::fs;
use std::path::Path;
use thiserror::Error;
use rivun_crypto::{PublicKey, RivunCryptoError};
use rivun_policy::PolicySet;

use crate::models::{POLICY_BUNDLE_SIGNATURE_DOMAIN, PolicyBundle};

#[derive(Debug, Error)]
pub enum PolicyVerificationError {
    #[error("untrusted operator public key: {0}")]
    UntrustedOperator(String),
    #[error("invalid Ed25519 public key format: {0}")]
    InvalidPublicKey(String),
    #[error("invalid Ed25519 signature format: {0}")]
    InvalidSignature(String),
    #[error("signature verification failed for operator {0}")]
    SignatureFailed(String),
    #[error("policy TOML validation failed: {0}")]
    InvalidPolicyContent(String),
    #[error("I/O error writing local policy file: {0}")]
    Io(#[from] std::io::Error),
    #[error("crypto error: {0}")]
    Crypto(#[from] RivunCryptoError),
}

pub struct PolicyVerifier;

impl PolicyVerifier {
    /// Verifies the cryptographic signature on a policy bundle and verifies that the policy is valid TOML.
    pub fn verify_bundle(
        bundle: &PolicyBundle,
        authorized_operators: &[String],
    ) -> Result<PolicySet, PolicyVerificationError> {
        // 1. Check if the signer public key is in the authorized list (if whitelist is non-empty)
        if !authorized_operators.is_empty()
            && !authorized_operators.iter().any(|op| op == &bundle.signed_by_pubkey)
        {
            return Err(PolicyVerificationError::UntrustedOperator(
                bundle.signed_by_pubkey.clone(),
            ));
        }

        // 2. Decode the signer public key (supports base64 or hex)
        let pubkey = Self::parse_public_key(&bundle.signed_by_pubkey)?;

        // 3. Decode signature (supports base64 or hex, 64 bytes)
        let signature = Self::parse_signature(&bundle.signature)?;

        // 4. Construct canonical signing message: version + name + body_toml
        let signing_message = Self::compute_signing_message(bundle);

        // 5. Verify Ed25519 signature over the domain message
        pubkey
            .verify_domain_message(POLICY_BUNDLE_SIGNATURE_DOMAIN, &signing_message, &signature)
            .map_err(|_| PolicyVerificationError::SignatureFailed(bundle.signed_by_pubkey.clone()))?;

        // 6. Validate policy TOML syntax and invariants
        let policy_set = PolicySet::from_toml_str(&bundle.body_toml)
            .map_err(|e| PolicyVerificationError::InvalidPolicyContent(e.to_string()))?;

        Ok(policy_set)
    }

    /// Atomically applies the policy to disk.
    pub fn apply_bundle_to_path(
        bundle: &PolicyBundle,
        target_path: impl AsRef<Path>,
        authorized_operators: &[String],
    ) -> Result<PolicySet, PolicyVerificationError> {
        let policy_set = Self::verify_bundle(bundle, authorized_operators)?;
        let target_path = target_path.as_ref();

        // Atomic write: write to temp file then rename
        let tmp_path = target_path.with_extension("tmp.toml");
        fs::write(&tmp_path, &bundle.body_toml)?;
        fs::rename(&tmp_path, target_path)?;

        Ok(policy_set)
    }

    pub fn compute_signing_message(bundle: &PolicyBundle) -> Vec<u8> {
        let mut msg = Vec::new();
        msg.extend_from_slice(bundle.org_id.as_bytes());
        msg.push(b':');
        msg.extend_from_slice(bundle.name.as_bytes());
        msg.push(b':');
        msg.extend_from_slice(&bundle.version.to_be_bytes());
        msg.push(b':');
        msg.extend_from_slice(bundle.body_toml.as_bytes());
        msg
    }

    pub fn parse_public_key(input: &str) -> Result<PublicKey, PolicyVerificationError> {
        let raw_bytes = if let Ok(bytes) = STANDARD_NO_PAD.decode(input.trim()) {
            bytes
        } else if let Ok(bytes) = hex::decode(input.trim()) {
            bytes
        } else {
            return Err(PolicyVerificationError::InvalidPublicKey(input.to_string()));
        };

        if raw_bytes.len() != 32 {
            return Err(PolicyVerificationError::InvalidPublicKey(format!(
                "expected 32 bytes, got {}",
                raw_bytes.len()
            )));
        }

        let mut arr = [0u8; 32];
        arr.copy_from_slice(&raw_bytes);
        PublicKey::from_bytes(arr)
            .map_err(|e| PolicyVerificationError::InvalidPublicKey(e.to_string()))
    }

    pub fn parse_signature(input: &str) -> Result<[u8; 64], PolicyVerificationError> {
        let raw_bytes = if let Ok(bytes) = STANDARD_NO_PAD.decode(input.trim()) {
            bytes
        } else if let Ok(bytes) = hex::decode(input.trim()) {
            bytes
        } else {
            return Err(PolicyVerificationError::InvalidSignature(input.to_string()));
        };

        if raw_bytes.len() != 64 {
            return Err(PolicyVerificationError::InvalidSignature(format!(
                "expected 64 bytes, got {}",
                raw_bytes.len()
            )));
        }

        let mut arr = [0u8; 64];
        arr.copy_from_slice(&raw_bytes);
        Ok(arr)
    }
}
