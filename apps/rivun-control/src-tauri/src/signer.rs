//! Local Policy and Validator Set Signing Station.

use base64::{Engine as _, engine::general_purpose::STANDARD_NO_PAD};
use uuid::Uuid;
use rivun_crypto::Keypair;

pub const POLICY_BUNDLE_SIGNATURE_DOMAIN: &[u8] = b"Rivun-POLICY-BUNDLE-v1";
pub const POA_VALIDATOR_SET_SIGNATURE_DOMAIN: &[u8] = b"Rivun-POA-VALIDATOR-SET-v1";

pub struct OperatorSigner;

impl OperatorSigner {
    /// Signs a staged policy bundle locally with the operator's private Ed25519 key.
    pub fn sign_policy_bundle(
        keypair: &Keypair,
        org_id: &str,
        name: &str,
        version: u32,
        body_toml: &str,
    ) -> (String, String) {
        let mut msg = Vec::new();
        msg.extend_from_slice(org_id.as_bytes());
        msg.push(b':');
        msg.extend_from_slice(name.as_bytes());
        msg.push(b':');
        msg.extend_from_slice(&version.to_be_bytes());
        msg.push(b':');
        msg.extend_from_slice(body_toml.as_bytes());

        let sig_bytes = keypair.sign_domain_message(POLICY_BUNDLE_SIGNATURE_DOMAIN, &msg);
        let pubkey_b64 = STANDARD_NO_PAD.encode(keypair.verifying_key().to_bytes());
        let sig_b64 = STANDARD_NO_PAD.encode(sig_bytes);

        (pubkey_b64, sig_b64)
    }

    /// Signs a proposed validator set rotation locally.
    pub fn sign_validator_rotation(
        authority_keypair: &Keypair,
        set_id: Uuid,
        epoch: u64,
        threshold: u16,
        validator_descriptors: &[(Uuid, String)],
    ) -> (String, String) {
        let mut msg = Vec::new();
        msg.extend_from_slice(set_id.as_bytes());
        msg.extend_from_slice(&epoch.to_be_bytes());
        msg.extend_from_slice(&threshold.to_be_bytes());
        for (vid, pk) in validator_descriptors {
            msg.extend_from_slice(vid.as_bytes());
            msg.extend_from_slice(pk.as_bytes());
        }

        let sig_bytes = authority_keypair.sign_domain_message(POA_VALIDATOR_SET_SIGNATURE_DOMAIN, &msg);
        let pubkey_b64 = STANDARD_NO_PAD.encode(authority_keypair.verifying_key().to_bytes());
        let sig_b64 = STANDARD_NO_PAD.encode(sig_bytes);

        (pubkey_b64, sig_b64)
    }
}
