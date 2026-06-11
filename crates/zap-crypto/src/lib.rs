//! ZAP identity and signature primitives.
//!
//! The 64-byte ZAP-Wire header keeps an 8-byte `ZAP_SIGN` field for fast
//! synchronous filtering. This crate treats that field as a hint only. The
//! complete Ed25519 signature is stored in the authenticated trailer defined by
//! `zap-core`.

use base64::{Engine as _, engine::general_purpose::STANDARD_NO_PAD};
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use rand_core::OsRng;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use thiserror::Error;
use uuid::Uuid;
use zap_core::{
    ED25519_SIGNATURE_LEN, PoaAttestation, PoaTrailer, ZapError as CoreError, ZapFlags, ZapFrame,
};

const KEY_FILE_SCHEMA_VERSION: u8 = 1;
const SECRET_KEY_LEN: usize = 32;
const PUBLIC_KEY_LEN: usize = 32;
const NODE_ID_DOMAIN: &[u8] = b"ZAP-NODE-ID-v1";
const SIGN_HINT_DOMAIN: &[u8] = b"ZAP-SIGN-HINT-v1";
const POA_DIGEST_DOMAIN: &[u8] = b"ZAP-POA-DIGEST-v1";
const POA_SIGNATURE_DOMAIN: &[u8] = b"ZAP-POA-SIGNATURE-v1";
pub const POA_ATTESTATION_SCHEMA_VERSION: u8 = 1;
pub const POA_ATTESTATION_CONTENT_TYPE: &str = "application/json";
pub const POA_ATTESTATION_REQUEST_SUBJECT: &str = "poa.attestation_request";
pub const POA_ATTESTATION_RESPONSE_SUBJECT: &str = "poa.attestation_response";

#[derive(Debug, Error)]
pub enum ZapCryptoError {
    #[error(transparent)]
    Core(#[from] CoreError),
    #[error("frame source node {frame_node} does not match signing key node {key_node}")]
    SourceNodeMismatch { frame_node: Uuid, key_node: Uuid },
    #[error("frame source node {frame_node} does not match verifying key node {key_node}")]
    VerifyingNodeMismatch { frame_node: Uuid, key_node: Uuid },
    #[error("frame is not signed")]
    MissingSignature,
    #[error("signature hint mismatch")]
    SignatureHintMismatch,
    #[error("Ed25519 signature verification failed")]
    InvalidSignature,
    #[error("frame is missing a Proof-of-Action certificate")]
    MissingPoaCertificate,
    #[error("Proof-of-Action certificate can only be attached to frames marked REQUIRES_CONSENSUS")]
    PoaNotRequired,
    #[error("Proof-of-Action frame digest mismatch")]
    PoaDigestMismatch,
    #[error("Proof-of-Action threshold not met: required {required}, got {actual}")]
    PoaThresholdNotMet { required: u16, actual: u16 },
    #[error("unknown Proof-of-Action validator {0}")]
    UnknownPoaValidator(Uuid),
    #[error("duplicate Proof-of-Action validator {0}")]
    DuplicatePoaValidator(Uuid),
    #[error("Proof-of-Action validator signature failed for {0}")]
    InvalidPoaSignature(Uuid),
    #[error("Proof-of-Action attestation schema version {0} is unsupported")]
    UnsupportedPoaAttestationVersion(u8),
    #[error("Proof-of-Action digest length is invalid: expected 32, got {0}")]
    InvalidPoaDigestLength(usize),
    #[error("Proof-of-Action response digest does not match requested digest")]
    PoaResponseDigestMismatch,
    #[error("invalid key material length for {kind}: expected {expected}, got {actual}")]
    InvalidKeyLength {
        kind: &'static str,
        expected: usize,
        actual: usize,
    },
    #[error("key file schema version {0} is unsupported")]
    UnsupportedKeyFileVersion(u8),
    #[error("public key does not match the private key")]
    PublicKeyMismatch,
    #[error("node id in key file does not match public key")]
    NodeIdMismatch,
    #[error("failed to decode base64 key material: {0}")]
    Base64(#[from] base64::DecodeError),
    #[error("failed to parse Ed25519 key material: {0}")]
    Ed25519(#[from] ed25519_dalek::SignatureError),
    #[error("failed to parse TOML key file: {0}")]
    TomlDecode(#[from] toml::de::Error),
    #[error("failed to serialize TOML key file: {0}")]
    TomlEncode(#[from] toml::ser::Error),
}

pub type Result<T> = std::result::Result<T, ZapCryptoError>;

#[derive(Debug, Clone)]
pub struct Keypair {
    signing_key: SigningKey,
}

impl Keypair {
    pub fn generate() -> Self {
        Self {
            signing_key: SigningKey::generate(&mut OsRng),
        }
    }

    pub fn from_secret_bytes(bytes: [u8; SECRET_KEY_LEN]) -> Self {
        Self {
            signing_key: SigningKey::from_bytes(&bytes),
        }
    }

    pub fn verifying_key(&self) -> PublicKey {
        PublicKey {
            verifying_key: self.signing_key.verifying_key(),
        }
    }

    pub fn node_id(&self) -> Uuid {
        self.verifying_key().node_id()
    }

    pub fn secret_bytes(&self) -> [u8; SECRET_KEY_LEN] {
        self.signing_key.to_bytes()
    }

    pub fn to_key_file(&self) -> ZapKeyFile {
        let public_key = self.verifying_key();
        ZapKeyFile {
            schema_version: KEY_FILE_SCHEMA_VERSION,
            node_id: public_key.node_id(),
            public_key: STANDARD_NO_PAD.encode(public_key.to_bytes()),
            secret_key: STANDARD_NO_PAD.encode(self.secret_bytes()),
        }
    }

    pub fn to_key_file_toml(&self) -> Result<String> {
        Ok(toml::to_string_pretty(&self.to_key_file())?)
    }

    pub fn from_key_file(file: &ZapKeyFile) -> Result<Self> {
        if file.schema_version != KEY_FILE_SCHEMA_VERSION {
            return Err(ZapCryptoError::UnsupportedKeyFileVersion(
                file.schema_version,
            ));
        }

        let secret = decode_fixed::<SECRET_KEY_LEN>(&file.secret_key, "secret_key")?;
        let public = decode_fixed::<PUBLIC_KEY_LEN>(&file.public_key, "public_key")?;
        let keypair = Self::from_secret_bytes(secret);
        let verifying = keypair.verifying_key();

        if verifying.to_bytes() != public {
            return Err(ZapCryptoError::PublicKeyMismatch);
        }
        if verifying.node_id() != file.node_id {
            return Err(ZapCryptoError::NodeIdMismatch);
        }

        Ok(keypair)
    }

    pub fn from_key_file_toml(input: &str) -> Result<Self> {
        let file: ZapKeyFile = toml::from_str(input)?;
        Self::from_key_file(&file)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct PublicKey {
    verifying_key: VerifyingKey,
}

impl PublicKey {
    pub fn from_bytes(bytes: [u8; PUBLIC_KEY_LEN]) -> Result<Self> {
        Ok(Self {
            verifying_key: VerifyingKey::from_bytes(&bytes)?,
        })
    }

    pub fn to_bytes(&self) -> [u8; PUBLIC_KEY_LEN] {
        self.verifying_key.to_bytes()
    }

    pub fn node_id(&self) -> Uuid {
        node_id_from_public_key(&self.to_bytes())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ZapKeyFile {
    pub schema_version: u8,
    pub node_id: Uuid,
    pub public_key: String,
    pub secret_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PoaAttestationRequest {
    pub schema_version: u8,
    pub requester_node: Uuid,
    pub frame_digest: String,
    pub threshold: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PoaAttestationResponse {
    pub schema_version: u8,
    pub validator_node: Uuid,
    pub frame_digest: String,
    pub signature: String,
}

pub fn sign_frame(keypair: &Keypair, frame: &ZapFrame) -> Result<ZapFrame> {
    let key_node = keypair.node_id();
    if frame.header.source_node != key_node {
        return Err(ZapCryptoError::SourceNodeMismatch {
            frame_node: frame.header.source_node,
            key_node,
        });
    }

    let mut signed = frame.clone();
    signed.header.flags |= ZapFlags::SIGNED;
    signed.header.zap_sign = [0_u8; 8];
    signed.auth = None;
    signed.poa = None;

    let signature: Signature = keypair.signing_key.sign(&signed.signing_transcript());
    let signature_bytes = signature.to_bytes();
    let hint = signature_hint(&signature_bytes);
    signed.set_auth(signature_bytes, hint);
    Ok(signed)
}

pub fn verify_frame(public_key: &PublicKey, frame: &ZapFrame) -> Result<()> {
    let key_node = public_key.node_id();
    if frame.header.source_node != key_node {
        return Err(ZapCryptoError::VerifyingNodeMismatch {
            frame_node: frame.header.source_node,
            key_node,
        });
    }
    if !frame.header.flags.contains(ZapFlags::SIGNED) {
        return Err(ZapCryptoError::MissingSignature);
    }

    let auth = frame.auth.ok_or(ZapCryptoError::MissingSignature)?;
    if signature_hint(&auth.signature) != frame.header.zap_sign {
        return Err(ZapCryptoError::SignatureHintMismatch);
    }

    let signature = Signature::from_bytes(&auth.signature);
    public_key
        .verifying_key
        .verify(&frame.signing_transcript(), &signature)
        .map_err(|_| ZapCryptoError::InvalidSignature)
}

pub fn certify_frame(frame: &ZapFrame, threshold: u16, validators: &[Keypair]) -> Result<ZapFrame> {
    if !frame.header.flags.contains(ZapFlags::SIGNED) {
        return Err(ZapCryptoError::MissingSignature);
    }
    if !frame.header.flags.contains(ZapFlags::REQUIRES_CONSENSUS) {
        return Err(ZapCryptoError::PoaNotRequired);
    }
    if validators.len() < threshold as usize {
        return Err(ZapCryptoError::PoaThresholdNotMet {
            required: threshold,
            actual: validators.len() as u16,
        });
    }

    let digest = poa_frame_digest(frame);
    let message = poa_signing_message(&digest);
    let mut seen = HashSet::with_capacity(validators.len());
    let mut attestations = Vec::with_capacity(validators.len());
    for validator in validators {
        let validator_node = validator.node_id();
        if !seen.insert(validator_node) {
            return Err(ZapCryptoError::DuplicatePoaValidator(validator_node));
        }
        let signature: Signature = validator.signing_key.sign(&message);
        attestations.push(PoaAttestation {
            validator_node,
            signature: signature.to_bytes(),
        });
    }

    let mut certified = frame.clone();
    certified.set_poa(PoaTrailer::new(threshold, digest, attestations)?);
    Ok(certified)
}

pub fn poa_attestation_request(
    frame: &ZapFrame,
    requester_node: Uuid,
    threshold: u16,
) -> Result<PoaAttestationRequest> {
    if !frame.header.flags.contains(ZapFlags::SIGNED) {
        return Err(ZapCryptoError::MissingSignature);
    }
    if !frame.header.flags.contains(ZapFlags::REQUIRES_CONSENSUS) {
        return Err(ZapCryptoError::PoaNotRequired);
    }
    if threshold == 0 {
        return Err(CoreError::InvalidPoaThreshold(threshold).into());
    }
    Ok(PoaAttestationRequest {
        schema_version: POA_ATTESTATION_SCHEMA_VERSION,
        requester_node,
        frame_digest: STANDARD_NO_PAD.encode(poa_frame_digest(frame)),
        threshold,
    })
}

pub fn sign_poa_attestation_request(
    validator: &Keypair,
    request: &PoaAttestationRequest,
) -> Result<PoaAttestationResponse> {
    if request.schema_version != POA_ATTESTATION_SCHEMA_VERSION {
        return Err(ZapCryptoError::UnsupportedPoaAttestationVersion(
            request.schema_version,
        ));
    }
    if request.threshold == 0 {
        return Err(CoreError::InvalidPoaThreshold(request.threshold).into());
    }
    let digest = decode_poa_digest(&request.frame_digest)?;
    let signature: Signature = validator.signing_key.sign(&poa_signing_message(&digest));
    Ok(PoaAttestationResponse {
        schema_version: POA_ATTESTATION_SCHEMA_VERSION,
        validator_node: validator.node_id(),
        frame_digest: request.frame_digest.clone(),
        signature: STANDARD_NO_PAD.encode(signature.to_bytes()),
    })
}

pub fn verify_poa_attestation_response(
    response: &PoaAttestationResponse,
    validator: &PublicKey,
    expected_digest: &[u8; 32],
) -> Result<PoaAttestation> {
    if response.schema_version != POA_ATTESTATION_SCHEMA_VERSION {
        return Err(ZapCryptoError::UnsupportedPoaAttestationVersion(
            response.schema_version,
        ));
    }
    if response.validator_node != validator.node_id() {
        return Err(ZapCryptoError::UnknownPoaValidator(response.validator_node));
    }
    let digest = decode_poa_digest(&response.frame_digest)?;
    if &digest != expected_digest {
        return Err(ZapCryptoError::PoaResponseDigestMismatch);
    }
    let signature = decode_fixed::<ED25519_SIGNATURE_LEN>(&response.signature, "signature")?;
    let parsed = Signature::from_bytes(&signature);
    validator
        .verifying_key
        .verify(&poa_signing_message(&digest), &parsed)
        .map_err(|_| ZapCryptoError::InvalidPoaSignature(response.validator_node))?;
    Ok(PoaAttestation {
        validator_node: response.validator_node,
        signature,
    })
}

pub fn certify_frame_with_attestations(
    frame: &ZapFrame,
    threshold: u16,
    attestations: Vec<PoaAttestation>,
) -> Result<ZapFrame> {
    if !frame.header.flags.contains(ZapFlags::SIGNED) {
        return Err(ZapCryptoError::MissingSignature);
    }
    if !frame.header.flags.contains(ZapFlags::REQUIRES_CONSENSUS) {
        return Err(ZapCryptoError::PoaNotRequired);
    }
    if attestations.len() < threshold as usize {
        return Err(ZapCryptoError::PoaThresholdNotMet {
            required: threshold,
            actual: attestations.len() as u16,
        });
    }
    let mut certified = frame.clone();
    certified.set_poa(PoaTrailer::new(
        threshold,
        poa_frame_digest(frame),
        attestations,
    )?);
    Ok(certified)
}

pub fn verify_poa_certificate(
    frame: &ZapFrame,
    validators: &[(Uuid, PublicKey)],
    required_threshold: u16,
) -> Result<()> {
    let poa = frame
        .poa
        .as_ref()
        .ok_or(ZapCryptoError::MissingPoaCertificate)?;
    if poa.threshold < required_threshold {
        return Err(ZapCryptoError::PoaThresholdNotMet {
            required: required_threshold,
            actual: poa.threshold,
        });
    }
    if poa.frame_digest != poa_frame_digest(frame) {
        return Err(ZapCryptoError::PoaDigestMismatch);
    }

    let message = poa_signing_message(&poa.frame_digest);
    let mut seen = HashSet::with_capacity(poa.attestations.len());
    let mut valid_count = 0_u16;
    for attestation in &poa.attestations {
        if !seen.insert(attestation.validator_node) {
            return Err(ZapCryptoError::DuplicatePoaValidator(
                attestation.validator_node,
            ));
        }
        let public_key = validators
            .iter()
            .find_map(|(node_id, public_key)| {
                (*node_id == attestation.validator_node).then_some(*public_key)
            })
            .ok_or(ZapCryptoError::UnknownPoaValidator(
                attestation.validator_node,
            ))?;
        let signature = Signature::from_bytes(&attestation.signature);
        public_key
            .verifying_key
            .verify(&message, &signature)
            .map_err(|_| ZapCryptoError::InvalidPoaSignature(attestation.validator_node))?;
        valid_count = valid_count.saturating_add(1);
    }

    let required = poa.threshold.max(required_threshold);
    if valid_count < required {
        return Err(ZapCryptoError::PoaThresholdNotMet {
            required,
            actual: valid_count,
        });
    }
    Ok(())
}

pub fn poa_frame_digest(frame: &ZapFrame) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(POA_DIGEST_DOMAIN);
    hasher.update(&frame.encode_without_poa());
    hasher.finalize().into()
}

pub fn node_id_from_public_key(public_key: &[u8; PUBLIC_KEY_LEN]) -> Uuid {
    let mut hasher = blake3::Hasher::new();
    hasher.update(NODE_ID_DOMAIN);
    hasher.update(public_key);
    let hash = hasher.finalize();
    let mut bytes = [0_u8; 16];
    bytes.copy_from_slice(&hash.as_bytes()[..16]);
    bytes[6] = (bytes[6] & 0x0F) | 0x80; // UUID version 8: application-defined.
    bytes[8] = (bytes[8] & 0x3F) | 0x80; // RFC 9562 variant.
    Uuid::from_bytes(bytes)
}

pub fn signature_hint(signature: &[u8; ED25519_SIGNATURE_LEN]) -> [u8; 8] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(SIGN_HINT_DOMAIN);
    hasher.update(signature);
    let hash = hasher.finalize();
    hash.as_bytes()[..8].try_into().unwrap()
}

fn poa_signing_message(digest: &[u8; 32]) -> Vec<u8> {
    let mut message = Vec::with_capacity(POA_SIGNATURE_DOMAIN.len() + digest.len());
    message.extend_from_slice(POA_SIGNATURE_DOMAIN);
    message.extend_from_slice(digest);
    message
}

fn decode_fixed<const N: usize>(encoded: &str, kind: &'static str) -> Result<[u8; N]> {
    let decoded = STANDARD_NO_PAD.decode(encoded)?;
    if decoded.len() != N {
        return Err(ZapCryptoError::InvalidKeyLength {
            kind,
            expected: N,
            actual: decoded.len(),
        });
    }
    Ok(decoded.try_into().unwrap())
}

fn decode_poa_digest(encoded: &str) -> Result<[u8; 32]> {
    let decoded = STANDARD_NO_PAD.decode(encoded)?;
    if decoded.len() != 32 {
        return Err(ZapCryptoError::InvalidPoaDigestLength(decoded.len()));
    }
    Ok(decoded.try_into().unwrap())
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::Bytes;
    use zap_core::{ZapFlags, ZapFrame};

    fn unsigned_frame(keypair: &Keypair) -> ZapFrame {
        ZapFrame::with_timestamp(
            keypair.node_id(),
            Uuid::from_bytes([7_u8; 16]),
            ZapFlags::PRIORITY,
            123,
            Bytes::from_static(b"act"),
        )
        .unwrap()
    }

    fn unsigned_consensus_frame(keypair: &Keypair) -> ZapFrame {
        ZapFrame::with_timestamp(
            keypair.node_id(),
            Uuid::from_bytes([7_u8; 16]),
            ZapFlags::REQUIRES_CONSENSUS,
            123,
            Bytes::from_static(b"critical"),
        )
        .unwrap()
    }

    #[test]
    fn signed_frame_verifies() {
        let keypair = Keypair::generate();
        let frame = sign_frame(&keypair, &unsigned_frame(&keypair)).unwrap();
        verify_frame(&keypair.verifying_key(), &frame).unwrap();
        assert!(frame.header.flags.contains(ZapFlags::SIGNED));
        assert!(frame.auth.is_some());
    }

    #[test]
    fn payload_tampering_fails() {
        let keypair = Keypair::generate();
        let mut frame = sign_frame(&keypair, &unsigned_frame(&keypair)).unwrap();
        frame.payload = Bytes::from_static(b"evil");

        assert!(matches!(
            verify_frame(&keypair.verifying_key(), &frame),
            Err(ZapCryptoError::InvalidSignature)
        ));
    }

    #[test]
    fn signature_hint_tampering_fails_fast() {
        let keypair = Keypair::generate();
        let mut frame = sign_frame(&keypair, &unsigned_frame(&keypair)).unwrap();
        frame.header.zap_sign[0] ^= 0xFF;

        assert!(matches!(
            verify_frame(&keypair.verifying_key(), &frame),
            Err(ZapCryptoError::SignatureHintMismatch)
        ));
    }

    #[test]
    fn wrong_public_key_fails_on_source_node() {
        let sender = Keypair::generate();
        let other = Keypair::generate();
        let frame = sign_frame(&sender, &unsigned_frame(&sender)).unwrap();

        assert!(matches!(
            verify_frame(&other.verifying_key(), &frame),
            Err(ZapCryptoError::VerifyingNodeMismatch { .. })
        ));
    }

    #[test]
    fn key_file_round_trips() {
        let keypair = Keypair::generate();
        let encoded = keypair.to_key_file_toml().unwrap();
        let decoded = Keypair::from_key_file_toml(&encoded).unwrap();

        assert_eq!(decoded.secret_bytes(), keypair.secret_bytes());
        assert_eq!(decoded.node_id(), keypair.node_id());
    }

    #[test]
    fn refuses_to_sign_for_a_different_source_node() {
        let keypair = Keypair::generate();
        let mut frame = unsigned_frame(&keypair);
        frame.header.source_node = Uuid::from_bytes([9_u8; 16]);

        assert!(matches!(
            sign_frame(&keypair, &frame),
            Err(ZapCryptoError::SourceNodeMismatch { .. })
        ));
    }

    #[test]
    fn poa_certificate_verifies_for_threshold() {
        let source = Keypair::generate();
        let validator_a = Keypair::generate();
        let validator_b = Keypair::generate();
        let signed = sign_frame(&source, &unsigned_consensus_frame(&source)).unwrap();
        let certified =
            certify_frame(&signed, 2, &[validator_a.clone(), validator_b.clone()]).unwrap();

        verify_frame(&source.verifying_key(), &certified).unwrap();
        verify_poa_certificate(
            &certified,
            &[
                (validator_a.node_id(), validator_a.verifying_key()),
                (validator_b.node_id(), validator_b.verifying_key()),
            ],
            2,
        )
        .unwrap();
    }

    #[test]
    fn poa_attestation_request_response_builds_certificate() {
        let source = Keypair::generate();
        let validator = Keypair::generate();
        let signed = sign_frame(&source, &unsigned_consensus_frame(&source)).unwrap();
        let request = poa_attestation_request(&signed, source.node_id(), 1).unwrap();
        let response = sign_poa_attestation_request(&validator, &request).unwrap();
        let digest = poa_frame_digest(&signed);
        let attestation =
            verify_poa_attestation_response(&response, &validator.verifying_key(), &digest)
                .unwrap();
        let certified = certify_frame_with_attestations(&signed, 1, vec![attestation]).unwrap();

        verify_poa_certificate(
            &certified,
            &[(validator.node_id(), validator.verifying_key())],
            1,
        )
        .unwrap();
    }

    #[test]
    fn poa_attestation_response_rejects_wrong_digest() {
        let source = Keypair::generate();
        let validator = Keypair::generate();
        let signed = sign_frame(&source, &unsigned_consensus_frame(&source)).unwrap();
        let request = poa_attestation_request(&signed, source.node_id(), 1).unwrap();
        let response = sign_poa_attestation_request(&validator, &request).unwrap();

        assert!(matches!(
            verify_poa_attestation_response(&response, &validator.verifying_key(), &[0xAA; 32]),
            Err(ZapCryptoError::PoaResponseDigestMismatch)
        ));
    }

    #[test]
    fn poa_certificate_detects_payload_tampering() {
        let source = Keypair::generate();
        let validator = Keypair::generate();
        let signed = sign_frame(&source, &unsigned_consensus_frame(&source)).unwrap();
        let mut certified = certify_frame(&signed, 1, std::slice::from_ref(&validator)).unwrap();
        certified.payload = Bytes::from_static(b"tampered");

        assert!(matches!(
            verify_poa_certificate(
                &certified,
                &[(validator.node_id(), validator.verifying_key())],
                1,
            ),
            Err(ZapCryptoError::PoaDigestMismatch)
        ));
    }

    #[test]
    fn poa_certificate_rejects_unknown_validator() {
        let source = Keypair::generate();
        let validator = Keypair::generate();
        let signed = sign_frame(&source, &unsigned_consensus_frame(&source)).unwrap();
        let certified = certify_frame(&signed, 1, std::slice::from_ref(&validator)).unwrap();

        assert!(matches!(
            verify_poa_certificate(&certified, &[], 1),
            Err(ZapCryptoError::UnknownPoaValidator(_))
        ));
    }

    #[test]
    fn poa_certificate_enforces_required_threshold() {
        let source = Keypair::generate();
        let validator = Keypair::generate();
        let signed = sign_frame(&source, &unsigned_consensus_frame(&source)).unwrap();
        let certified = certify_frame(&signed, 1, std::slice::from_ref(&validator)).unwrap();

        assert!(matches!(
            verify_poa_certificate(
                &certified,
                &[(validator.node_id(), validator.verifying_key())],
                2,
            ),
            Err(ZapCryptoError::PoaThresholdNotMet {
                required: 2,
                actual: 1
            })
        ));
    }
}
