//! Core Rivun-Wire protocol types.
//!
//! The PDF specification fixes the first 64 bytes of every Rivun discharge.
//! This crate keeps that header strict, big-endian, and allocation-free to
//! parse. Payload and authentication trailers are layered after the header.

use bitflags::bitflags;
use bytes::{Bytes, BytesMut};
use std::time::{SystemTime, UNIX_EPOCH};
use thiserror::Error;
use uuid::Uuid;

pub const MAGIC_NUMBER: u32 = 0x5A41_505F;
pub const MAGIC_BYTES: [u8; 4] = *b"ZAP_";
pub const VERSION: u16 = 0x0001;
pub const HEADER_LEN: usize = 64;
pub const SIGNING_PREFIX_LEN: usize = 56;
pub const MAX_PAYLOAD_LEN: u64 = 16 * 1024 * 1024;

pub const MAGIC_OFFSET: usize = 0;
pub const VERSION_OFFSET: usize = 4;
pub const FLAGS_OFFSET: usize = 6;
pub const SOURCE_NODE_OFFSET: usize = 8;
pub const TARGET_NODE_OFFSET: usize = 24;
pub const TIMESTAMP_OFFSET: usize = 40;
pub const ZAP_LEN_OFFSET: usize = 48;
pub const ZAP_SIGN_OFFSET: usize = 56;

pub const AUTH_TRAILER_MAGIC: [u8; 4] = *b"ZSIG";
pub const AUTH_TRAILER_LEN: usize = 72;
pub const ED25519_SIGNATURE_LEN: usize = 64;
pub const POA_TRAILER_MAGIC: [u8; 4] = *b"ZPOA";
pub const POA_TRAILER_VERSION: u16 = 1;
pub const POA_TRAILER_HEADER_LEN: usize = 44;
pub const POA_ATTESTATION_LEN: usize = 80;
pub const MAX_POA_ATTESTATIONS: usize = 64;

bitflags! {
    /// Rivun-Wire frame flags. Unknown bits are rejected in v1 so malformed or
    /// future frames cannot silently downgrade behavior.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct RivunFlags: u16 {
        const ENCRYPTED = 1 << 0;
        const PRIORITY = 1 << 1;
        const REQUIRES_CONSENSUS = 1 << 2;
        const SIGNED = 1 << 3;
        const BROADCAST = 1 << 4;
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum RivunError {
    #[error("frame too short: expected at least {expected} bytes, got {actual}")]
    Truncated { expected: usize, actual: usize },
    #[error("invalid magic number 0x{0:08X}")]
    InvalidMagic(u32),
    #[error("unsupported Rivun version {0}")]
    UnsupportedVersion(u16),
    #[error("unknown Rivun flag bits 0x{0:04X}")]
    UnknownFlags(u16),
    #[error("payload length {0} exceeds maximum {MAX_PAYLOAD_LEN}")]
    PayloadTooLarge(u64),
    #[error("frame length mismatch: expected {expected} bytes, got {actual}")]
    LengthMismatch { expected: usize, actual: usize },
    #[error("invalid auth trailer magic")]
    InvalidAuthTrailerMagic,
    #[error("unsupported signature algorithm {0}")]
    UnsupportedSignatureAlgorithm(u16),
    #[error("invalid signature length {0}")]
    InvalidSignatureLength(u16),
    #[error("signed frame is missing an Ed25519 auth trailer")]
    MissingAuthTrailer,
    #[error("auth trailer present on an unsigned frame")]
    UnexpectedAuthTrailer,
    #[error("invalid PoA trailer magic")]
    InvalidPoaTrailerMagic,
    #[error("unsupported PoA trailer version {0}")]
    UnsupportedPoaTrailerVersion(u16),
    #[error("invalid PoA threshold {0}")]
    InvalidPoaThreshold(u16),
    #[error("PoA attestation count {0} exceeds maximum {MAX_POA_ATTESTATIONS}")]
    TooManyPoaAttestations(u16),
    #[error("PoA trailer length mismatch: expected {expected} bytes, got {actual}")]
    PoaTrailerLengthMismatch { expected: usize, actual: usize },
    #[error("PoA trailer present on a frame that does not require consensus")]
    UnexpectedPoaTrailer,
    #[error("system clock is before Unix epoch")]
    ClockBeforeUnixEpoch,
}

pub type Result<T> = std::result::Result<T, RivunError>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignatureAlgorithm {
    Ed25519 = 1,
}

impl TryFrom<u16> for SignatureAlgorithm {
    type Error = RivunError;

    fn try_from(value: u16) -> Result<Self> {
        match value {
            1 => Ok(Self::Ed25519),
            other => Err(RivunError::UnsupportedSignatureAlgorithm(other)),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AuthTrailer {
    pub algorithm: SignatureAlgorithm,
    pub signature: [u8; ED25519_SIGNATURE_LEN],
}

impl AuthTrailer {
    pub fn ed25519(signature: [u8; ED25519_SIGNATURE_LEN]) -> Self {
        Self {
            algorithm: SignatureAlgorithm::Ed25519,
            signature,
        }
    }

    pub fn parse(input: &[u8]) -> Result<Self> {
        if input.len() < AUTH_TRAILER_LEN {
            return Err(RivunError::Truncated {
                expected: AUTH_TRAILER_LEN,
                actual: input.len(),
            });
        }
        if input[0..4] != AUTH_TRAILER_MAGIC {
            return Err(RivunError::InvalidAuthTrailerMagic);
        }

        let algorithm = SignatureAlgorithm::try_from(u16::from_be_bytes([input[4], input[5]]))?;
        let signature_len = u16::from_be_bytes([input[6], input[7]]);
        if signature_len as usize != ED25519_SIGNATURE_LEN {
            return Err(RivunError::InvalidSignatureLength(signature_len));
        }

        let mut signature = [0_u8; ED25519_SIGNATURE_LEN];
        signature.copy_from_slice(&input[8..AUTH_TRAILER_LEN]);
        Ok(Self {
            algorithm,
            signature,
        })
    }

    pub fn write_to(&self, out: &mut [u8; AUTH_TRAILER_LEN]) {
        out[0..4].copy_from_slice(&AUTH_TRAILER_MAGIC);
        out[4..6].copy_from_slice(&(self.algorithm as u16).to_be_bytes());
        out[6..8].copy_from_slice(&(ED25519_SIGNATURE_LEN as u16).to_be_bytes());
        out[8..AUTH_TRAILER_LEN].copy_from_slice(&self.signature);
    }

    pub fn to_bytes(&self) -> [u8; AUTH_TRAILER_LEN] {
        let mut out = [0_u8; AUTH_TRAILER_LEN];
        self.write_to(&mut out);
        out
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PoaAttestation {
    pub validator_node: Uuid,
    pub signature: [u8; ED25519_SIGNATURE_LEN],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PoaTrailer {
    pub threshold: u16,
    pub frame_digest: [u8; 32],
    pub attestations: Vec<PoaAttestation>,
}

impl PoaTrailer {
    pub fn new(
        threshold: u16,
        frame_digest: [u8; 32],
        attestations: Vec<PoaAttestation>,
    ) -> Result<Self> {
        if threshold == 0 {
            return Err(RivunError::InvalidPoaThreshold(threshold));
        }
        if attestations.len() > MAX_POA_ATTESTATIONS {
            return Err(RivunError::TooManyPoaAttestations(attestations.len() as u16));
        }
        Ok(Self {
            threshold,
            frame_digest,
            attestations,
        })
    }

    pub fn parse(input: &[u8]) -> Result<Self> {
        if input.len() < POA_TRAILER_HEADER_LEN {
            return Err(RivunError::Truncated {
                expected: POA_TRAILER_HEADER_LEN,
                actual: input.len(),
            });
        }
        if input[0..4] != POA_TRAILER_MAGIC {
            return Err(RivunError::InvalidPoaTrailerMagic);
        }

        let version = u16::from_be_bytes([input[4], input[5]]);
        if version != POA_TRAILER_VERSION {
            return Err(RivunError::UnsupportedPoaTrailerVersion(version));
        }
        let threshold = u16::from_be_bytes([input[6], input[7]]);
        if threshold == 0 {
            return Err(RivunError::InvalidPoaThreshold(threshold));
        }
        let attestation_count = u16::from_be_bytes([input[8], input[9]]);
        if attestation_count as usize > MAX_POA_ATTESTATIONS {
            return Err(RivunError::TooManyPoaAttestations(attestation_count));
        }

        let expected = POA_TRAILER_HEADER_LEN
            .checked_add(attestation_count as usize * POA_ATTESTATION_LEN)
            .ok_or(RivunError::TooManyPoaAttestations(attestation_count))?;
        if input.len() != expected {
            return Err(RivunError::PoaTrailerLengthMismatch {
                expected,
                actual: input.len(),
            });
        }

        let mut frame_digest = [0_u8; 32];
        frame_digest.copy_from_slice(&input[12..44]);
        let mut attestations = Vec::with_capacity(attestation_count as usize);
        let mut offset = POA_TRAILER_HEADER_LEN;
        for _ in 0..attestation_count {
            let validator_node = Uuid::from_bytes(input[offset..offset + 16].try_into().unwrap());
            offset += 16;
            let mut signature = [0_u8; ED25519_SIGNATURE_LEN];
            signature.copy_from_slice(&input[offset..offset + ED25519_SIGNATURE_LEN]);
            offset += ED25519_SIGNATURE_LEN;
            attestations.push(PoaAttestation {
                validator_node,
                signature,
            });
        }

        Ok(Self {
            threshold,
            frame_digest,
            attestations,
        })
    }

    pub fn encoded_len(&self) -> usize {
        POA_TRAILER_HEADER_LEN + self.attestations.len() * POA_ATTESTATION_LEN
    }

    pub fn write_to(&self, out: &mut BytesMut) {
        out.extend_from_slice(&POA_TRAILER_MAGIC);
        out.extend_from_slice(&POA_TRAILER_VERSION.to_be_bytes());
        out.extend_from_slice(&self.threshold.to_be_bytes());
        out.extend_from_slice(&(self.attestations.len() as u16).to_be_bytes());
        out.extend_from_slice(&0_u16.to_be_bytes());
        out.extend_from_slice(&self.frame_digest);
        for attestation in &self.attestations {
            out.extend_from_slice(attestation.validator_node.as_bytes());
            out.extend_from_slice(&attestation.signature);
        }
    }

    pub fn to_bytes(&self) -> Bytes {
        let mut out = BytesMut::with_capacity(self.encoded_len());
        self.write_to(&mut out);
        out.freeze()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RivunHeader {
    pub version: u16,
    pub flags: RivunFlags,
    pub source_node: Uuid,
    pub target_node: Uuid,
    pub timestamp_micros: u64,
    pub rivun_len: u64,
    pub rivun_sign: [u8; 8],
}

impl RivunHeader {
    pub fn new(
        flags: RivunFlags,
        source_node: Uuid,
        target_node: Uuid,
        timestamp_micros: u64,
        rivun_len: u64,
    ) -> Result<Self> {
        if rivun_len > MAX_PAYLOAD_LEN {
            return Err(RivunError::PayloadTooLarge(rivun_len));
        }

        Ok(Self {
            version: VERSION,
            flags,
            source_node,
            target_node,
            timestamp_micros,
            rivun_len,
            rivun_sign: [0_u8; 8],
        })
    }

    pub fn parse(input: &[u8]) -> Result<Self> {
        if input.len() < HEADER_LEN {
            return Err(RivunError::Truncated {
                expected: HEADER_LEN,
                actual: input.len(),
            });
        }

        let magic = u32::from_be_bytes(input[MAGIC_OFFSET..VERSION_OFFSET].try_into().unwrap());
        if magic != MAGIC_NUMBER {
            return Err(RivunError::InvalidMagic(magic));
        }

        let version = u16::from_be_bytes(input[VERSION_OFFSET..FLAGS_OFFSET].try_into().unwrap());
        if version != VERSION {
            return Err(RivunError::UnsupportedVersion(version));
        }

        let flags_bits =
            u16::from_be_bytes(input[FLAGS_OFFSET..SOURCE_NODE_OFFSET].try_into().unwrap());
        let flags = RivunFlags::from_bits(flags_bits).ok_or_else(|| {
            let unknown = flags_bits & !RivunFlags::all().bits();
            RivunError::UnknownFlags(unknown)
        })?;

        let source_node = Uuid::from_bytes(
            input[SOURCE_NODE_OFFSET..TARGET_NODE_OFFSET]
                .try_into()
                .unwrap(),
        );
        let target_node = Uuid::from_bytes(
            input[TARGET_NODE_OFFSET..TIMESTAMP_OFFSET]
                .try_into()
                .unwrap(),
        );
        let timestamp_micros =
            u64::from_be_bytes(input[TIMESTAMP_OFFSET..ZAP_LEN_OFFSET].try_into().unwrap());
        let rivun_len =
            u64::from_be_bytes(input[ZAP_LEN_OFFSET..ZAP_SIGN_OFFSET].try_into().unwrap());
        if rivun_len > MAX_PAYLOAD_LEN {
            return Err(RivunError::PayloadTooLarge(rivun_len));
        }

        let mut rivun_sign = [0_u8; 8];
        rivun_sign.copy_from_slice(&input[ZAP_SIGN_OFFSET..HEADER_LEN]);

        Ok(Self {
            version,
            flags,
            source_node,
            target_node,
            timestamp_micros,
            rivun_len,
            rivun_sign,
        })
    }

    pub fn write_to(&self, out: &mut [u8; HEADER_LEN]) {
        out[MAGIC_OFFSET..VERSION_OFFSET].copy_from_slice(&MAGIC_NUMBER.to_be_bytes());
        out[VERSION_OFFSET..FLAGS_OFFSET].copy_from_slice(&self.version.to_be_bytes());
        out[FLAGS_OFFSET..SOURCE_NODE_OFFSET].copy_from_slice(&self.flags.bits().to_be_bytes());
        out[SOURCE_NODE_OFFSET..TARGET_NODE_OFFSET].copy_from_slice(self.source_node.as_bytes());
        out[TARGET_NODE_OFFSET..TIMESTAMP_OFFSET].copy_from_slice(self.target_node.as_bytes());
        out[TIMESTAMP_OFFSET..ZAP_LEN_OFFSET].copy_from_slice(&self.timestamp_micros.to_be_bytes());
        out[ZAP_LEN_OFFSET..ZAP_SIGN_OFFSET].copy_from_slice(&self.rivun_len.to_be_bytes());
        out[ZAP_SIGN_OFFSET..HEADER_LEN].copy_from_slice(&self.rivun_sign);
    }

    pub fn to_bytes(&self) -> [u8; HEADER_LEN] {
        let mut out = [0_u8; HEADER_LEN];
        self.write_to(&mut out);
        out
    }

    pub fn signing_prefix(&self) -> [u8; SIGNING_PREFIX_LEN] {
        let bytes = self.to_bytes();
        bytes[0..SIGNING_PREFIX_LEN].try_into().unwrap()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RivunFrame {
    pub header: RivunHeader,
    pub payload: Bytes,
    pub auth: Option<AuthTrailer>,
    pub poa: Option<PoaTrailer>,
}

impl RivunFrame {
    pub fn new(
        source_node: Uuid,
        target_node: Uuid,
        flags: RivunFlags,
        payload: Bytes,
    ) -> Result<Self> {
        Self::with_timestamp(source_node, target_node, flags, now_micros()?, payload)
    }

    pub fn with_timestamp(
        source_node: Uuid,
        target_node: Uuid,
        flags: RivunFlags,
        timestamp_micros: u64,
        payload: Bytes,
    ) -> Result<Self> {
        let header = RivunHeader::new(
            flags,
            source_node,
            target_node,
            timestamp_micros,
            payload.len() as u64,
        )?;
        Ok(Self {
            header,
            payload,
            auth: None,
            poa: None,
        })
    }

    pub fn decode(input: &[u8]) -> Result<Self> {
        let header = RivunHeader::parse(input)?;
        let payload_len = usize::try_from(header.rivun_len)
            .map_err(|_| RivunError::PayloadTooLarge(header.rivun_len))?;
        let payload_end = HEADER_LEN
            .checked_add(payload_len)
            .ok_or(RivunError::PayloadTooLarge(header.rivun_len))?;

        if input.len() < payload_end {
            return Err(RivunError::Truncated {
                expected: payload_end,
                actual: input.len(),
            });
        }

        let payload = Bytes::copy_from_slice(&input[HEADER_LEN..payload_end]);
        let mut offset = payload_end;
        let auth = if header.flags.contains(RivunFlags::SIGNED) {
            let auth_end = offset
                .checked_add(AUTH_TRAILER_LEN)
                .ok_or(RivunError::PayloadTooLarge(header.rivun_len))?;
            if input.len() < auth_end {
                return Err(RivunError::MissingAuthTrailer);
            }
            let auth = AuthTrailer::parse(&input[offset..auth_end])?;
            offset = auth_end;
            Some(auth)
        } else if input.len() >= offset + AUTH_TRAILER_LEN
            && input[offset..offset + 4] == AUTH_TRAILER_MAGIC
        {
            return Err(RivunError::UnexpectedAuthTrailer);
        } else {
            None
        };

        let poa = if input.len() > offset {
            if !header.flags.contains(RivunFlags::REQUIRES_CONSENSUS) {
                return Err(RivunError::UnexpectedPoaTrailer);
            }
            Some(PoaTrailer::parse(&input[offset..])?)
        } else {
            None
        };

        match (header.flags.contains(RivunFlags::SIGNED), auth.is_some()) {
            (true, false) => Err(RivunError::MissingAuthTrailer),
            (false, true) => Err(RivunError::UnexpectedAuthTrailer),
            _ => Ok(Self {
                header,
                payload,
                auth,
                poa,
            }),
        }
    }

    pub fn encode(&self) -> Bytes {
        self.encode_with_poa(true)
    }

    pub fn encode_without_poa(&self) -> Bytes {
        self.encode_with_poa(false)
    }

    fn encode_with_poa(&self, include_poa: bool) -> Bytes {
        let auth_len = self.auth.map(|_| AUTH_TRAILER_LEN).unwrap_or(0);
        let poa_len = if include_poa {
            self.poa.as_ref().map(PoaTrailer::encoded_len).unwrap_or(0)
        } else {
            0
        };
        let mut out = BytesMut::with_capacity(HEADER_LEN + self.payload.len() + auth_len + poa_len);
        out.extend_from_slice(&self.header.to_bytes());
        out.extend_from_slice(&self.payload);
        if let Some(auth) = &self.auth {
            out.extend_from_slice(&auth.to_bytes());
        }
        if include_poa && let Some(poa) = &self.poa {
            poa.write_to(&mut out);
        }
        out.freeze()
    }

    pub fn signing_transcript(&self) -> Bytes {
        let mut out = BytesMut::with_capacity(SIGNING_PREFIX_LEN + self.payload.len());
        out.extend_from_slice(&self.header.signing_prefix());
        out.extend_from_slice(&self.payload);
        out.freeze()
    }

    pub fn set_auth(&mut self, signature: [u8; ED25519_SIGNATURE_LEN], hint: [u8; 8]) {
        self.header.flags |= RivunFlags::SIGNED;
        self.header.rivun_sign = hint;
        self.auth = Some(AuthTrailer::ed25519(signature));
    }

    pub fn set_poa(&mut self, poa: PoaTrailer) {
        self.header.flags |= RivunFlags::REQUIRES_CONSENSUS;
        self.poa = Some(poa);
    }
}

pub fn now_micros() -> Result<u64> {
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| RivunError::ClockBeforeUnixEpoch)?;
    Ok(duration.as_micros().min(u128::from(u64::MAX)) as u64)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn source() -> Uuid {
        Uuid::from_bytes([1_u8; 16])
    }

    fn target() -> Uuid {
        Uuid::from_bytes([2_u8; 16])
    }

    #[test]
    fn header_offsets_are_big_endian_and_stable() {
        let mut header = RivunHeader::new(
            RivunFlags::ENCRYPTED | RivunFlags::PRIORITY,
            source(),
            target(),
            0x0102_0304_0506_0708,
            0x0000_0000_0000_0018,
        )
        .unwrap();
        header.rivun_sign = [0xAA; 8];

        let bytes = header.to_bytes();
        assert_eq!(&bytes[0..4], b"ZAP_");
        assert_eq!(&bytes[4..6], &VERSION.to_be_bytes());
        assert_eq!(&bytes[6..8], &(0b11_u16).to_be_bytes());
        assert_eq!(&bytes[8..24], &[1_u8; 16]);
        assert_eq!(&bytes[24..40], &[2_u8; 16]);
        assert_eq!(&bytes[40..48], &0x0102_0304_0506_0708_u64.to_be_bytes());
        assert_eq!(&bytes[48..56], &0x0000_0000_0000_0018_u64.to_be_bytes());
        assert_eq!(&bytes[56..64], &[0xAA; 8]);
        assert_eq!(RivunHeader::parse(&bytes).unwrap(), header);
    }

    #[test]
    fn rejects_invalid_magic_version_flags_and_payload_len() {
        let mut bytes = RivunHeader::new(RivunFlags::empty(), source(), target(), 1, 0)
            .unwrap()
            .to_bytes();

        bytes[0] = 0;
        assert!(matches!(
            RivunHeader::parse(&bytes),
            Err(RivunError::InvalidMagic(_))
        ));

        let mut bytes = RivunHeader::new(RivunFlags::empty(), source(), target(), 1, 0)
            .unwrap()
            .to_bytes();
        bytes[5] = 2;
        assert_eq!(
            RivunHeader::parse(&bytes),
            Err(RivunError::UnsupportedVersion(2))
        );

        let mut bytes = RivunHeader::new(RivunFlags::empty(), source(), target(), 1, 0)
            .unwrap()
            .to_bytes();
        bytes[6..8].copy_from_slice(&0x8000_u16.to_be_bytes());
        assert_eq!(
            RivunHeader::parse(&bytes),
            Err(RivunError::UnknownFlags(0x8000))
        );

        let mut bytes = RivunHeader::new(RivunFlags::empty(), source(), target(), 1, 0)
            .unwrap()
            .to_bytes();
        bytes[48..56].copy_from_slice(&(MAX_PAYLOAD_LEN + 1).to_be_bytes());
        assert_eq!(
            RivunHeader::parse(&bytes),
            Err(RivunError::PayloadTooLarge(MAX_PAYLOAD_LEN + 1))
        );
    }

    #[test]
    fn frame_round_trip_without_auth() {
        let frame = RivunFrame::with_timestamp(
            source(),
            target(),
            RivunFlags::PRIORITY,
            42,
            Bytes::from_static(b"open"),
        )
        .unwrap();

        let encoded = frame.encode();
        let decoded = RivunFrame::decode(&encoded).unwrap();
        assert_eq!(decoded, frame);
    }

    #[test]
    fn signed_flag_requires_auth_trailer() {
        let frame = RivunFrame::with_timestamp(
            source(),
            target(),
            RivunFlags::SIGNED,
            42,
            Bytes::from_static(b"open"),
        )
        .unwrap();

        assert_eq!(
            RivunFrame::decode(&frame.encode()),
            Err(RivunError::MissingAuthTrailer)
        );
    }

    #[test]
    fn auth_trailer_requires_signed_flag() {
        let mut frame = RivunFrame::with_timestamp(
            source(),
            target(),
            RivunFlags::empty(),
            42,
            Bytes::from_static(b"open"),
        )
        .unwrap();
        frame.auth = Some(AuthTrailer::ed25519([3_u8; 64]));

        assert_eq!(
            RivunFrame::decode(&frame.encode()),
            Err(RivunError::UnexpectedAuthTrailer)
        );
    }

    #[test]
    fn poa_trailer_round_trips() {
        let attestation = PoaAttestation {
            validator_node: Uuid::from_bytes([9_u8; 16]),
            signature: [0xAB; ED25519_SIGNATURE_LEN],
        };
        let trailer = PoaTrailer::new(1, [0xCD; 32], vec![attestation]).unwrap();

        let encoded = trailer.to_bytes();
        assert_eq!(encoded.len(), POA_TRAILER_HEADER_LEN + POA_ATTESTATION_LEN);
        assert_eq!(PoaTrailer::parse(&encoded).unwrap(), trailer);
    }

    #[test]
    fn consensus_frame_round_trips_with_poa_trailer() {
        let mut frame = RivunFrame::with_timestamp(
            source(),
            target(),
            RivunFlags::REQUIRES_CONSENSUS,
            42,
            Bytes::from_static(b"critical"),
        )
        .unwrap();
        frame.set_poa(
            PoaTrailer::new(
                1,
                [0xCD; 32],
                vec![PoaAttestation {
                    validator_node: Uuid::from_bytes([9_u8; 16]),
                    signature: [0xAB; ED25519_SIGNATURE_LEN],
                }],
            )
            .unwrap(),
        );

        let encoded = frame.encode();
        let decoded = RivunFrame::decode(&encoded).unwrap();
        assert_eq!(decoded, frame);
        assert_eq!(
            frame.encode_without_poa().len(),
            HEADER_LEN + b"critical".len()
        );
    }

    #[test]
    fn poa_trailer_requires_consensus_flag() {
        let mut frame = RivunFrame::with_timestamp(
            source(),
            target(),
            RivunFlags::empty(),
            42,
            Bytes::from_static(b"open"),
        )
        .unwrap();
        frame.poa = Some(
            PoaTrailer::new(
                1,
                [0xCD; 32],
                vec![PoaAttestation {
                    validator_node: Uuid::from_bytes([9_u8; 16]),
                    signature: [0xAB; ED25519_SIGNATURE_LEN],
                }],
            )
            .unwrap(),
        );

        assert_eq!(
            RivunFrame::decode(&frame.encode()),
            Err(RivunError::UnexpectedPoaTrailer)
        );
    }

    #[test]
    fn signing_transcript_excludes_signature_hint() {
        let mut frame = RivunFrame::with_timestamp(
            source(),
            target(),
            RivunFlags::SIGNED,
            42,
            Bytes::from_static(b"open"),
        )
        .unwrap();
        let before = frame.signing_transcript();
        frame.header.rivun_sign = [9; 8];
        assert_eq!(frame.signing_transcript(), before);
    }

    #[test]
    fn unsigned_frame_golden_vector_is_stable() {
        let mut frame = RivunFrame::with_timestamp(
            source(),
            target(),
            RivunFlags::ENCRYPTED | RivunFlags::PRIORITY,
            0x0102_0304_0506_0708,
            Bytes::from_static(b"hello"),
        )
        .unwrap();
        frame.header.rivun_sign = [0xAA; 8];

        let expected =
            hex::decode(include_str!("../tests/golden/rivun_frame_v1_unsigned.hex").trim()).unwrap();
        let encoded = frame.encode();
        assert_eq!(encoded.as_ref(), expected.as_slice());
        assert_eq!(RivunFrame::decode(&expected).unwrap(), frame);
    }
}
