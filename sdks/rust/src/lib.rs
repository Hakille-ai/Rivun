//! External ZAP SDK helpers.
//!
//! This crate keeps a small, application-friendly surface around the canonical
//! ZAP crates. It is intentionally network-free: callers can build and parse
//! `ZENV` control payloads, then hand the bytes to their chosen transport.

use bytes::Bytes;
use serde::{Serialize, de::DeserializeOwned};
use std::{error::Error, fmt};
use uuid::Uuid;

pub use zap_core::{
    AUTH_TRAILER_LEN, AuthTrailer, ED25519_SIGNATURE_LEN, HEADER_LEN as WIRE_HEADER_LEN,
    MAX_PAYLOAD_LEN, PoaAttestation, PoaTrailer, SIGNING_PREFIX_LEN, SignatureAlgorithm, ZapFlags,
    ZapFrame, ZapHeader, now_micros,
};
pub use zap_envelope::{
    DEFAULT_CONTENT_TYPE, HEADER_LEN as ENVELOPE_HEADER_LEN, ZapEnvelope, ZapEnvelopeRef,
    ZapMessageKind,
};
pub use zap_pact::{
    PACT_BUNDLE_SUBJECT, PACT_CONTENT_TYPE, PACT_RECORD_SUBJECT, PACT_REVOKE_SUBJECT,
    PACT_SCHEMA_VERSION, PACT_VERIFY_SUBJECT, ZapPact, ZapPactBundle, ZapPactProof,
    ZapPactRevocation, ZapPactStatus, ZapPactVerification,
};
pub use zap_ledger::{ReceiptJournalStore, SignedActionReceipt};
pub use zap_store::{
    DRIVER_ABI_VERSION, DRIVER_HASH_PREFIX, DriverAbiRequirement, DriverManifest, DriverRegistry,
    DriverRegistryEntry, DriverRegistryMigration, DriverRegistryStatus,
    REGISTRY_BUNDLE_MANIFEST_CONTENT_TYPE, REGISTRY_BUNDLE_MANIFEST_REQUEST_SUBJECT,
    REGISTRY_BUNDLE_MANIFEST_RESPONSE_SUBJECT, REGISTRY_BUNDLE_SCHEMA_VERSION,
    REGISTRY_INDEX_CONTENT_TYPE, REGISTRY_INDEX_REQUEST_SUBJECT, REGISTRY_INDEX_RESPONSE_SUBJECT,
    REGISTRY_INDEX_SYNC_SCHEMA_VERSION, REGISTRY_INSTALL_PLAN_SCHEMA_VERSION, RegistryBundleEntry,
    RegistryBundleManifest, RegistryBundleManifestRequest, RegistryBundleManifestResponse,
    RegistryIndexRequest, RegistryIndexResponse, RegistryInstallPlan, RegistryInstallPlanEntry,
    RegistryInstallPlanRequest, RegistryPublication, artifact_hash, driver_hash, registry_hash,
};

pub type Result<T> = std::result::Result<T, SdkError>;

#[derive(Debug)]
pub enum SdkError {
    Envelope(zap_envelope::ZapEnvelopeError),
    Json(serde_json::Error),
    ExpectedControl { actual: ZapMessageKind },
}

impl fmt::Display for SdkError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Envelope(error) => write!(f, "{error}"),
            Self::Json(error) => write!(f, "{error}"),
            Self::ExpectedControl { actual } => {
                write!(f, "expected control envelope, got {}", actual.as_str())
            }
        }
    }
}

impl Error for SdkError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Envelope(error) => Some(error),
            Self::Json(error) => Some(error),
            Self::ExpectedControl { .. } => None,
        }
    }
}

impl From<zap_envelope::ZapEnvelopeError> for SdkError {
    fn from(value: zap_envelope::ZapEnvelopeError) -> Self {
        Self::Envelope(value)
    }
}

impl From<serde_json::Error> for SdkError {
    fn from(value: serde_json::Error) -> Self {
        Self::Json(value)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ControlFrame {
    envelope: ZapEnvelope,
}

#[derive(Debug, Default, Clone, Copy)]
pub struct ZapStoreClient;

impl ZapStoreClient {
    pub fn registry_index_request(&self, require_signature: bool) -> Result<ControlFrame> {
        registry_index_request_frame(require_signature)
    }

    pub fn registry_bundle_manifest_request(
        &self,
        require_publication: bool,
        require_drivers: bool,
    ) -> Result<ControlFrame> {
        registry_bundle_manifest_request_frame(require_publication, require_drivers)
    }
}

impl ControlFrame {
    pub fn new(
        subject: impl Into<String>,
        content_type: impl Into<String>,
        body: impl Into<Bytes>,
    ) -> Result<Self> {
        let envelope =
            ZapEnvelope::new(ZapMessageKind::Control, subject, content_type, body.into())?;
        Ok(Self { envelope })
    }

    pub fn json<T: Serialize>(
        subject: impl Into<String>,
        content_type: impl Into<String>,
        body: &T,
    ) -> Result<Self> {
        Self::new(
            subject,
            content_type,
            Bytes::from(serde_json::to_vec(body)?),
        )
    }

    pub fn decode(input: &[u8]) -> Result<Self> {
        let envelope = ZapEnvelopeRef::parse(input)?;
        if envelope.kind() != ZapMessageKind::Control {
            return Err(SdkError::ExpectedControl {
                actual: envelope.kind(),
            });
        }
        let owned = ZapEnvelope::new(
            envelope.kind(),
            envelope.subject(),
            envelope.content_type(),
            Bytes::copy_from_slice(envelope.body()),
        )?
        .with_id(envelope.id())
        .with_metadata(Bytes::copy_from_slice(envelope.metadata()))?;
        let owned = match envelope.correlation_id() {
            Some(correlation_id) => owned.with_correlation_id(correlation_id),
            None => owned,
        };
        let owned = match envelope.causation_id() {
            Some(causation_id) => owned.with_causation_id(causation_id),
            None => owned,
        };
        Ok(Self { envelope: owned })
    }

    pub fn encode(&self) -> Bytes {
        self.envelope.encode()
    }

    pub fn subject(&self) -> &str {
        self.envelope.subject()
    }

    pub fn content_type(&self) -> &str {
        self.envelope.content_type()
    }

    pub fn body(&self) -> &[u8] {
        self.envelope.body()
    }

    pub fn id(&self) -> Uuid {
        self.envelope.id()
    }

    pub fn json_body<T: DeserializeOwned>(&self) -> Result<T> {
        Ok(serde_json::from_slice(self.body())?)
    }

    pub fn into_envelope(self) -> ZapEnvelope {
        self.envelope
    }
}

pub fn registry_index_request_frame(require_signature: bool) -> Result<ControlFrame> {
    let request = RegistryIndexRequest {
        schema_version: REGISTRY_INDEX_SYNC_SCHEMA_VERSION,
        require_signature,
    };
    ControlFrame::json(
        REGISTRY_INDEX_REQUEST_SUBJECT,
        REGISTRY_INDEX_CONTENT_TYPE,
        &request,
    )
}

pub fn registry_bundle_manifest_request_frame(
    require_publication: bool,
    require_drivers: bool,
) -> Result<ControlFrame> {
    let request = RegistryBundleManifestRequest {
        schema_version: REGISTRY_BUNDLE_SCHEMA_VERSION,
        require_publication,
        require_drivers,
    };
    ControlFrame::json(
        REGISTRY_BUNDLE_MANIFEST_REQUEST_SUBJECT,
        REGISTRY_BUNDLE_MANIFEST_CONTENT_TYPE,
        &request,
    )
}

pub fn decode_registry_index_response(input: &[u8]) -> Result<RegistryIndexResponse> {
    let frame = ControlFrame::decode(input)?;
    Ok(serde_json::from_slice(frame.body())?)
}

pub fn decode_registry_bundle_manifest_response(
    input: &[u8],
) -> Result<RegistryBundleManifestResponse> {
    let frame = ControlFrame::decode(input)?;
    Ok(serde_json::from_slice(frame.body())?)
}

pub fn verify_registry_signature(registry: &DriverRegistry) -> zap_store::Result<()> {
    registry.verify_signature()
}

#[derive(Debug)]
pub struct ZapUdpClient {
    socket: std::net::UdpSocket,
}

impl ZapUdpClient {
    pub fn bind(addr: impl std::net::ToSocketAddrs) -> Result<Self> {
        let socket = std::net::UdpSocket::bind(addr).map_err(|e| {
            SdkError::Envelope(zap_envelope::ZapEnvelopeError::InvalidHeader(e.to_string()))
        })?;
        Ok(Self { socket })
    }

    pub fn send_envelope(
        &self,
        envelope: &ZapEnvelope,
        target: impl std::net::ToSocketAddrs,
    ) -> Result<usize> {
        let bytes = envelope.encode();
        self.socket.send_to(&bytes, target).map_err(|e| {
            SdkError::Envelope(zap_envelope::ZapEnvelopeError::InvalidHeader(e.to_string()))
        })
    }

    pub fn send_control(
        &self,
        frame: &ControlFrame,
        target: impl std::net::ToSocketAddrs,
    ) -> Result<usize> {
        let bytes = frame.encode();
        self.socket.send_to(&bytes, target).map_err(|e| {
            SdkError::Envelope(zap_envelope::ZapEnvelopeError::InvalidHeader(e.to_string()))
        })
    }

    pub fn recv_envelope(
        &self,
        timeout: Option<std::time::Duration>,
    ) -> Result<(ZapEnvelope, std::net::SocketAddr)> {
        self.socket.set_read_timeout(timeout).ok();
        let mut buf = [0u8; 65535];
        let (n, addr) = self.socket.recv_from(&mut buf).map_err(|e| {
            SdkError::Envelope(zap_envelope::ZapEnvelopeError::InvalidHeader(e.to_string()))
        })?;
        let env_ref = ZapEnvelopeRef::parse(&buf[..n])?;
        let owned = ZapEnvelope::new(
            env_ref.kind(),
            env_ref.subject(),
            env_ref.content_type(),
            Bytes::copy_from_slice(env_ref.body()),
        )?
        .with_id(env_ref.id())
        .with_metadata(Bytes::copy_from_slice(env_ref.metadata()))?;
        Ok((owned, addr))
    }

    pub fn request_control(
        &self,
        frame: &ControlFrame,
        target: impl std::net::ToSocketAddrs,
        timeout: std::time::Duration,
    ) -> Result<ControlFrame> {
        self.send_control(frame, target)?;
        let (env, _) = self.recv_envelope(Some(timeout))?;
        if env.kind() != ZapMessageKind::Control {
            return Err(SdkError::ExpectedControl {
                actual: env.kind(),
            });
        }
        let encoded = env.encode();
        ControlFrame::decode(&encoded)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HASH: &str = "blake3:0000000000000000000000000000000000000000000000000000000000000000";

    #[test]
    fn registry_bundle_manifest_request_control_frame_round_trips() {
        let frame = ZapStoreClient
            .registry_bundle_manifest_request(true, true)
            .unwrap();
        let encoded = frame.encode();
        let decoded = ControlFrame::decode(&encoded).unwrap();

        assert_eq!(decoded.subject(), REGISTRY_BUNDLE_MANIFEST_REQUEST_SUBJECT);
        assert_eq!(
            decoded.content_type(),
            REGISTRY_BUNDLE_MANIFEST_CONTENT_TYPE
        );
        let body: RegistryBundleManifestRequest = decoded.json_body().unwrap();
        assert_eq!(body.schema_version, REGISTRY_BUNDLE_SCHEMA_VERSION);
        assert!(body.require_publication);
        assert!(body.require_drivers);
    }

    #[test]
    fn bundle_manifest_response_verification_honors_required_driver_metadata() {
        let manifest = RegistryBundleManifest::new(
            None,
            "registry.index.toml".to_string(),
            HASH.to_string(),
            None,
            None,
            vec![RegistryBundleEntry {
                action: "echo".to_string(),
                version: "0.1.0".to_string(),
                name: "echo-driver".to_string(),
                abi_version: DRIVER_ABI_VERSION,
                wasm_hash: HASH.to_string(),
                author_node_id: Uuid::nil(),
                status: DriverRegistryStatus::Active,
                manifest_path: Some("manifests/echo.toml".to_string()),
                manifest_hash: Some(HASH.to_string()),
                driver_path: None,
                driver_hash: None,
            }],
        );
        let response = RegistryBundleManifestResponse::new(Uuid::nil(), Some(manifest), None);
        let request = RegistryBundleManifestRequest {
            schema_version: REGISTRY_BUNDLE_SCHEMA_VERSION,
            require_publication: false,
            require_drivers: true,
        };

        assert!(response.verify(&request).is_err());
    }

    #[test]
    fn artifact_hash_uses_canonical_zap_store_blake3() {
        assert_eq!(
            artifact_hash(b"driver"),
            "blake3:bb6f2f5117d7690122f64d2950ca874cd26fbe808e2e28dc9b914ebd22d7800b"
        );
    }

    #[test]
    fn protocol_security_fixtures_are_readable_by_rust_sdk_tests() {
        let signed: serde_json::Value =
            serde_json::from_str(include_str!("../../../fixtures/protocol/signed-control-frame-v1.json"))
                .unwrap();
        let poa: serde_json::Value =
            serde_json::from_str(include_str!("../../../fixtures/protocol/poa-control-frame-v1.json"))
                .unwrap();
        let capability: serde_json::Value = serde_json::from_str(include_str!(
            "../../../fixtures/protocol/capability-response-v1.json"
        ))
        .unwrap();
        let datagram: serde_json::Value = serde_json::from_str(include_str!(
            "../../../fixtures/protocol/encrypted-datagram-v1.json"
        ))
        .unwrap();

        assert_eq!(signed["security"]["signed"], true);
        assert_eq!(signed["security"]["auth_trailer"]["algorithm"], "ed25519");
        assert_eq!(poa["security"]["poa_trailer"]["threshold"], 1);
        assert_eq!(capability["subject"], "zap.capability.response");
        assert_eq!(datagram["cipher"], "ChaCha20-Poly1305");
        assert_eq!(datagram["nonce_hex"].as_str().unwrap().len(), 24);
    }

    #[test]
    fn pact_fixture_verifies_with_rust_sdk() {
        let fixture: serde_json::Value =
            serde_json::from_str(include_str!("../../../fixtures/pact-record-v1.json")).unwrap();
        assert_eq!(fixture["subject"], PACT_RECORD_SUBJECT);
        assert_eq!(fixture["content_type"], PACT_CONTENT_TYPE);
        let pact: ZapPact = serde_json::from_value(fixture["body_json"].clone()).unwrap();
        let verification = pact.verify(None).unwrap();
        assert!(verification.valid);
        assert_eq!(verification.hash, pact.hash.unwrap());
    }

    #[test]
    fn udp_client_send_and_receive_envelope() {
        let server = ZapUdpClient::bind("127.0.0.1:0").unwrap();
        let server_addr = server.socket.local_addr().unwrap();
        let client = ZapUdpClient::bind("127.0.0.1:0").unwrap();

        let frame = ControlFrame::new("zap.test.ping", "text/plain", Bytes::from_static(b"ping")).unwrap();
        let bytes_sent = client.send_control(&frame, server_addr).unwrap();
        assert!(bytes_sent > 0);

        let (recv_env, from_addr) = server.recv_envelope(Some(std::time::Duration::from_millis(500))).unwrap();
        assert_eq!(recv_env.subject(), "zap.test.ping");
        assert_eq!(recv_env.body(), b"ping");
        assert_eq!(from_addr, client.socket.local_addr().unwrap());
    }
}
