//! Local Secure Key Vault for Operator Station.
//!
//! Stores private Ed25519 keys locally on the operator's filesystem / OS Keychain.
//! Invariant: Keys NEVER leave this machine and are never transmitted over the network.

use base64::{Engine as _, engine::general_purpose::STANDARD_NO_PAD};
use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;
use uuid::Uuid;
use rivun_crypto::Keypair;

#[derive(Debug, Error)]
pub enum VaultError {
    #[error("I/O error in local vault: {0}")]
    Io(#[from] std::io::Error),
    #[error("Crypto error: {0}")]
    Crypto(#[from] rivun_crypto::RivunCryptoError),
    #[error("Key not found in vault for node ID: {0}")]
    KeyNotFound(Uuid),
}

pub struct KeyVault {
    vault_dir: PathBuf,
}

impl KeyVault {
    pub fn new(vault_dir: impl AsRef<Path>) -> Result<Self, VaultError> {
        let path = vault_dir.as_ref().to_path_buf();
        if !path.exists() {
            fs::create_dir_all(&path)?;
        }
        Ok(Self { vault_dir: path })
    }

    pub fn default_path() -> PathBuf {
        let home = std::env::var("USERPROFILE")
            .or_else(|_| std::env::var("HOME"))
            .unwrap_or_else(|_| ".".to_string());
        PathBuf::from(home).join(".rivun").join("operator_keys")
    }

    /// Generates a new Ed25519 identity keypair and stores it securely in the local vault.
    pub fn generate_and_save_key(&self, label: Option<&str>) -> Result<(Uuid, String), VaultError> {
        let keypair = Keypair::generate();
        let node_id = keypair.node_id();
        let pubkey_b64 = STANDARD_NO_PAD.encode(keypair.verifying_key().to_bytes());

        let key_file = keypair.to_key_file();
        let toml_str = toml::to_string_pretty(&key_file).map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

        let file_name = if let Some(l) = label {
            format!("{}_{}.toml", l, node_id)
        } else {
            format!("{}.toml", node_id)
        };

        fs::write(self.vault_dir.join(file_name), toml_str)?;
        Ok((node_id, pubkey_b64))
    }

    /// Loads a local keypair for signing.
    pub fn load_keypair(&self, node_id: Uuid) -> Result<Keypair, VaultError> {
        for entry in fs::read_dir(&self.vault_dir)?.flatten() {
            let path = entry.path();
            if path.is_file() && path.extension().is_some_and(|e| e == "toml") {
                if let Ok(content) = fs::read_to_string(&path) {
                    if let Ok(keypair) = Keypair::from_key_file_toml(&content) {
                        if keypair.node_id() == node_id {
                            return Ok(keypair);
                        }
                    }
                }
            }
        }
        Err(VaultError::KeyNotFound(node_id))
    }

    /// Lists all local operator public keys and node IDs.
    pub fn list_identities(&self) -> Result<Vec<(Uuid, String, PathBuf)>, VaultError> {
        let mut results = Vec::new();
        for entry in fs::read_dir(&self.vault_dir)?.flatten() {
            let path = entry.path();
            if path.is_file() && path.extension().is_some_and(|e| e == "toml") {
                if let Ok(content) = fs::read_to_string(&path) {
                    if let Ok(keypair) = Keypair::from_key_file_toml(&content) {
                        let pubkey_b64 = STANDARD_NO_PAD.encode(keypair.verifying_key().to_bytes());
                        results.push((keypair.node_id(), pubkey_b64, path));
                    }
                }
            }
        }
        Ok(results)
    }
}
