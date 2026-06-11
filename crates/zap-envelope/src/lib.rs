//! Binary universal envelopes for ZAP payloads.
//!
//! `zap-envelope` is intentionally layered inside the ZAP-Wire payload. It
//! does not alter or reinterpret the fixed 64-byte wire header owned by
//! `zap-core`.

use bytes::{Bytes, BytesMut};
use std::{fmt, str::FromStr};
use thiserror::Error;
use uuid::Uuid;

pub const MAGIC_BYTES: [u8; 4] = *b"ZENV";
pub const MAGIC_NUMBER: u32 = 0x5A45_4E56;
pub const VERSION: u16 = 1;
pub const HEADER_LEN: usize = 74;
pub const MAX_SUBJECT_LEN: usize = 512;
pub const MAX_CONTENT_TYPE_LEN: usize = 128;
pub const MAX_METADATA_LEN: usize = 64 * 1024;
pub const DEFAULT_CONTENT_TYPE: &str = "application/octet-stream";

const MAGIC_OFFSET: usize = 0;
const VERSION_OFFSET: usize = 4;
const KIND_OFFSET: usize = 6;
const RESERVED_OFFSET: usize = 8;
const ID_OFFSET: usize = 10;
const CORRELATION_ID_OFFSET: usize = 26;
const CAUSATION_ID_OFFSET: usize = 42;
const SUBJECT_LEN_OFFSET: usize = 58;
const CONTENT_TYPE_LEN_OFFSET: usize = 60;
const METADATA_LEN_OFFSET: usize = 62;
const BODY_LEN_OFFSET: usize = 66;

#[repr(u16)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ZapMessageKind {
    Data = 1,
    Event = 2,
    Command = 3,
    Query = 4,
    Response = 5,
    StreamChunk = 6,
    Action = 7,
    Control = 8,
}

impl ZapMessageKind {
    pub const fn as_u16(self) -> u16 {
        self as u16
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Data => "data",
            Self::Event => "event",
            Self::Command => "command",
            Self::Query => "query",
            Self::Response => "response",
            Self::StreamChunk => "stream_chunk",
            Self::Action => "action",
            Self::Control => "control",
        }
    }

    pub const fn requires_subject(self) -> bool {
        !matches!(self, Self::Data)
    }
}

impl fmt::Display for ZapMessageKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl TryFrom<u16> for ZapMessageKind {
    type Error = ZapEnvelopeError;

    fn try_from(value: u16) -> Result<Self> {
        match value {
            1 => Ok(Self::Data),
            2 => Ok(Self::Event),
            3 => Ok(Self::Command),
            4 => Ok(Self::Query),
            5 => Ok(Self::Response),
            6 => Ok(Self::StreamChunk),
            7 => Ok(Self::Action),
            8 => Ok(Self::Control),
            other => Err(ZapEnvelopeError::UnknownKind(other)),
        }
    }
}

impl FromStr for ZapMessageKind {
    type Err = ZapEnvelopeError;

    fn from_str(input: &str) -> Result<Self> {
        let normalized = input
            .chars()
            .filter(|ch| *ch != '-' && *ch != '_')
            .flat_map(char::to_lowercase)
            .collect::<String>();
        match normalized.as_str() {
            "data" => Ok(Self::Data),
            "event" => Ok(Self::Event),
            "command" => Ok(Self::Command),
            "query" => Ok(Self::Query),
            "response" => Ok(Self::Response),
            "streamchunk" => Ok(Self::StreamChunk),
            "action" => Ok(Self::Action),
            "control" => Ok(Self::Control),
            _ => Err(ZapEnvelopeError::UnknownKindName(input.to_string())),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZapEnvelopeField {
    Subject,
    ContentType,
}

impl fmt::Display for ZapEnvelopeField {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Subject => f.write_str("subject"),
            Self::ContentType => f.write_str("content_type"),
        }
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ZapEnvelopeError {
    #[error("envelope too short: expected at least {expected} bytes, got {actual}")]
    Truncated { expected: usize, actual: usize },
    #[error("invalid envelope magic 0x{0:08X}")]
    InvalidMagic(u32),
    #[error("unsupported envelope version {0}")]
    UnsupportedVersion(u16),
    #[error("unknown envelope kind {0}")]
    UnknownKind(u16),
    #[error("unknown envelope kind `{0}`")]
    UnknownKindName(String),
    #[error("reserved envelope field must be zero, got {0}")]
    NonzeroReserved(u16),
    #[error("subject is required for {kind:?} envelopes")]
    MissingSubject { kind: ZapMessageKind },
    #[error("subject length {actual} exceeds maximum {max}")]
    SubjectTooLong { max: usize, actual: usize },
    #[error("content_type length {actual} exceeds maximum {max}")]
    ContentTypeTooLong { max: usize, actual: usize },
    #[error("metadata length {actual} exceeds maximum {max}")]
    MetadataTooLarge { max: usize, actual: usize },
    #[error("body length {actual} exceeds maximum {max}")]
    BodyTooLarge { max: u64, actual: u64 },
    #[error("invalid UTF-8 in {field}")]
    InvalidUtf8 { field: ZapEnvelopeField },
    #[error("envelope length overflow")]
    LengthOverflow,
    #[error("envelope length mismatch: expected {expected} bytes, got {actual}")]
    LengthMismatch { expected: usize, actual: usize },
}

pub type Result<T> = std::result::Result<T, ZapEnvelopeError>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ZapEnvelope {
    kind: ZapMessageKind,
    id: Uuid,
    correlation_id: Option<Uuid>,
    causation_id: Option<Uuid>,
    subject: String,
    content_type: String,
    metadata: Bytes,
    body: Bytes,
}

impl ZapEnvelope {
    pub fn new(
        kind: ZapMessageKind,
        subject: impl Into<String>,
        content_type: impl Into<String>,
        body: impl Into<Bytes>,
    ) -> Result<Self> {
        let subject = subject.into();
        let content_type = content_type.into();
        let body = body.into();
        validate_parts(
            kind,
            subject.as_bytes(),
            content_type.as_bytes(),
            0,
            body.len() as u64,
        )?;

        Ok(Self {
            kind,
            id: Uuid::new_v4(),
            correlation_id: None,
            causation_id: None,
            subject,
            content_type,
            metadata: Bytes::new(),
            body,
        })
    }

    pub fn action(subject: impl Into<String>, body: impl Into<Bytes>) -> Result<Self> {
        Self::new(ZapMessageKind::Action, subject, DEFAULT_CONTENT_TYPE, body)
    }

    pub fn event(subject: impl Into<String>, body: impl Into<Bytes>) -> Result<Self> {
        Self::new(ZapMessageKind::Event, subject, DEFAULT_CONTENT_TYPE, body)
    }

    pub fn data(subject: impl Into<String>, body: impl Into<Bytes>) -> Result<Self> {
        Self::new(ZapMessageKind::Data, subject, DEFAULT_CONTENT_TYPE, body)
    }

    pub fn command(subject: impl Into<String>, body: impl Into<Bytes>) -> Result<Self> {
        Self::new(ZapMessageKind::Command, subject, DEFAULT_CONTENT_TYPE, body)
    }

    pub fn query(subject: impl Into<String>, body: impl Into<Bytes>) -> Result<Self> {
        Self::new(ZapMessageKind::Query, subject, DEFAULT_CONTENT_TYPE, body)
    }

    pub fn response(subject: impl Into<String>, body: impl Into<Bytes>) -> Result<Self> {
        Self::new(
            ZapMessageKind::Response,
            subject,
            DEFAULT_CONTENT_TYPE,
            body,
        )
    }

    pub fn stream_chunk(subject: impl Into<String>, body: impl Into<Bytes>) -> Result<Self> {
        Self::new(
            ZapMessageKind::StreamChunk,
            subject,
            DEFAULT_CONTENT_TYPE,
            body,
        )
    }

    pub fn control(subject: impl Into<String>, body: impl Into<Bytes>) -> Result<Self> {
        Self::new(ZapMessageKind::Control, subject, DEFAULT_CONTENT_TYPE, body)
    }

    pub fn with_content_type(mut self, content_type: impl Into<String>) -> Result<Self> {
        let content_type = content_type.into();
        validate_content_type_len(content_type.len())?;
        self.content_type = content_type;
        Ok(self)
    }

    pub fn with_id(mut self, id: Uuid) -> Self {
        self.id = id;
        self
    }

    pub fn with_correlation_id(mut self, correlation_id: Uuid) -> Self {
        self.correlation_id = Some(correlation_id);
        self
    }

    pub fn without_correlation_id(mut self) -> Self {
        self.correlation_id = None;
        self
    }

    pub fn with_causation_id(mut self, causation_id: Uuid) -> Self {
        self.causation_id = Some(causation_id);
        self
    }

    pub fn without_causation_id(mut self) -> Self {
        self.causation_id = None;
        self
    }

    pub fn with_metadata(mut self, metadata: impl Into<Bytes>) -> Result<Self> {
        let metadata = metadata.into();
        validate_metadata_len(metadata.len())?;
        self.metadata = metadata;
        Ok(self)
    }

    pub fn kind(&self) -> ZapMessageKind {
        self.kind
    }

    pub fn id(&self) -> Uuid {
        self.id
    }

    pub fn correlation_id(&self) -> Option<Uuid> {
        self.correlation_id
    }

    pub fn causation_id(&self) -> Option<Uuid> {
        self.causation_id
    }

    pub fn subject(&self) -> &str {
        &self.subject
    }

    pub fn content_type(&self) -> &str {
        &self.content_type
    }

    pub fn metadata(&self) -> &[u8] {
        &self.metadata
    }

    pub fn body(&self) -> &[u8] {
        &self.body
    }

    pub fn encoded_len(&self) -> usize {
        HEADER_LEN
            + self.subject.len()
            + self.content_type.len()
            + self.metadata.len()
            + self.body.len()
    }

    pub fn encode(&self) -> Bytes {
        let mut out = BytesMut::with_capacity(self.encoded_len());
        out.extend_from_slice(&MAGIC_BYTES);
        out.extend_from_slice(&VERSION.to_be_bytes());
        out.extend_from_slice(&self.kind.as_u16().to_be_bytes());
        out.extend_from_slice(&0_u16.to_be_bytes());
        out.extend_from_slice(self.id.as_bytes());
        match self.correlation_id {
            Some(correlation_id) => out.extend_from_slice(correlation_id.as_bytes()),
            None => out.extend_from_slice(Uuid::nil().as_bytes()),
        }
        match self.causation_id {
            Some(causation_id) => out.extend_from_slice(causation_id.as_bytes()),
            None => out.extend_from_slice(Uuid::nil().as_bytes()),
        }
        out.extend_from_slice(&(self.subject.len() as u16).to_be_bytes());
        out.extend_from_slice(&(self.content_type.len() as u16).to_be_bytes());
        out.extend_from_slice(&(self.metadata.len() as u32).to_be_bytes());
        out.extend_from_slice(&(self.body.len() as u64).to_be_bytes());
        out.extend_from_slice(self.subject.as_bytes());
        out.extend_from_slice(self.content_type.as_bytes());
        out.extend_from_slice(&self.metadata);
        out.extend_from_slice(&self.body);
        out.freeze()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ZapEnvelopeRef<'a> {
    kind: ZapMessageKind,
    id: Uuid,
    correlation_id: Option<Uuid>,
    causation_id: Option<Uuid>,
    subject: &'a str,
    content_type: &'a str,
    metadata: &'a [u8],
    body: &'a [u8],
}

impl<'a> ZapEnvelopeRef<'a> {
    pub fn parse(input: &'a [u8]) -> Result<Self> {
        if input.len() < HEADER_LEN {
            return Err(ZapEnvelopeError::Truncated {
                expected: HEADER_LEN,
                actual: input.len(),
            });
        }

        let magic = u32::from_be_bytes(input[MAGIC_OFFSET..VERSION_OFFSET].try_into().unwrap());
        if magic != MAGIC_NUMBER {
            return Err(ZapEnvelopeError::InvalidMagic(magic));
        }

        let version = u16::from_be_bytes(input[VERSION_OFFSET..KIND_OFFSET].try_into().unwrap());
        if version != VERSION {
            return Err(ZapEnvelopeError::UnsupportedVersion(version));
        }

        let kind_bits = u16::from_be_bytes(input[KIND_OFFSET..RESERVED_OFFSET].try_into().unwrap());
        let kind = ZapMessageKind::try_from(kind_bits)?;

        let reserved = u16::from_be_bytes(input[RESERVED_OFFSET..ID_OFFSET].try_into().unwrap());
        if reserved != 0 {
            return Err(ZapEnvelopeError::NonzeroReserved(reserved));
        }

        let id = Uuid::from_bytes(input[ID_OFFSET..CORRELATION_ID_OFFSET].try_into().unwrap());
        let correlation_id = optional_uuid(
            input[CORRELATION_ID_OFFSET..CAUSATION_ID_OFFSET]
                .try_into()
                .unwrap(),
        );
        let causation_id = optional_uuid(
            input[CAUSATION_ID_OFFSET..SUBJECT_LEN_OFFSET]
                .try_into()
                .unwrap(),
        );

        let subject_len = u16::from_be_bytes(
            input[SUBJECT_LEN_OFFSET..CONTENT_TYPE_LEN_OFFSET]
                .try_into()
                .unwrap(),
        ) as usize;
        let content_type_len = u16::from_be_bytes(
            input[CONTENT_TYPE_LEN_OFFSET..METADATA_LEN_OFFSET]
                .try_into()
                .unwrap(),
        ) as usize;
        let metadata_len = u32::from_be_bytes(
            input[METADATA_LEN_OFFSET..BODY_LEN_OFFSET]
                .try_into()
                .unwrap(),
        ) as usize;
        let body_len = u64::from_be_bytes(input[BODY_LEN_OFFSET..HEADER_LEN].try_into().unwrap());

        validate_part_lens(kind, subject_len, content_type_len, metadata_len, body_len)?;

        let body_len_usize =
            usize::try_from(body_len).map_err(|_| ZapEnvelopeError::BodyTooLarge {
                max: zap_core::MAX_PAYLOAD_LEN,
                actual: body_len,
            })?;
        let expected =
            checked_total_len(subject_len, content_type_len, metadata_len, body_len_usize)?;

        match input.len().cmp(&expected) {
            std::cmp::Ordering::Less => {
                return Err(ZapEnvelopeError::Truncated {
                    expected,
                    actual: input.len(),
                });
            }
            std::cmp::Ordering::Greater => {
                return Err(ZapEnvelopeError::LengthMismatch {
                    expected,
                    actual: input.len(),
                });
            }
            std::cmp::Ordering::Equal => {}
        }

        let subject_start = HEADER_LEN;
        let content_type_start = subject_start + subject_len;
        let metadata_start = content_type_start + content_type_len;
        let body_start = metadata_start + metadata_len;

        let subject =
            std::str::from_utf8(&input[subject_start..content_type_start]).map_err(|_| {
                ZapEnvelopeError::InvalidUtf8 {
                    field: ZapEnvelopeField::Subject,
                }
            })?;
        if kind.requires_subject() && subject.is_empty() {
            return Err(ZapEnvelopeError::MissingSubject { kind });
        }

        let content_type = std::str::from_utf8(&input[content_type_start..metadata_start])
            .map_err(|_| ZapEnvelopeError::InvalidUtf8 {
                field: ZapEnvelopeField::ContentType,
            })?;

        Ok(Self {
            kind,
            id,
            correlation_id,
            causation_id,
            subject,
            content_type,
            metadata: &input[metadata_start..body_start],
            body: &input[body_start..],
        })
    }

    pub fn kind(&self) -> ZapMessageKind {
        self.kind
    }

    pub fn id(&self) -> Uuid {
        self.id
    }

    pub fn correlation_id(&self) -> Option<Uuid> {
        self.correlation_id
    }

    pub fn causation_id(&self) -> Option<Uuid> {
        self.causation_id
    }

    pub fn subject(&self) -> &'a str {
        self.subject
    }

    pub fn content_type(&self) -> &'a str {
        self.content_type
    }

    pub fn metadata(&self) -> &'a [u8] {
        self.metadata
    }

    pub fn body(&self) -> &'a [u8] {
        self.body
    }
}

fn optional_uuid(bytes: [u8; 16]) -> Option<Uuid> {
    let uuid = Uuid::from_bytes(bytes);
    (uuid != Uuid::nil()).then_some(uuid)
}

fn validate_parts(
    kind: ZapMessageKind,
    subject: &[u8],
    content_type: &[u8],
    metadata_len: usize,
    body_len: u64,
) -> Result<()> {
    validate_part_lens(
        kind,
        subject.len(),
        content_type.len(),
        metadata_len,
        body_len,
    )
}

fn validate_part_lens(
    kind: ZapMessageKind,
    subject_len: usize,
    content_type_len: usize,
    metadata_len: usize,
    body_len: u64,
) -> Result<()> {
    validate_subject_len(subject_len)?;
    validate_content_type_len(content_type_len)?;
    validate_metadata_len(metadata_len)?;
    validate_body_len(body_len)?;

    if kind.requires_subject() && subject_len == 0 {
        return Err(ZapEnvelopeError::MissingSubject { kind });
    }

    Ok(())
}

fn validate_subject_len(actual: usize) -> Result<()> {
    if actual > MAX_SUBJECT_LEN {
        return Err(ZapEnvelopeError::SubjectTooLong {
            max: MAX_SUBJECT_LEN,
            actual,
        });
    }
    Ok(())
}

fn validate_content_type_len(actual: usize) -> Result<()> {
    if actual > MAX_CONTENT_TYPE_LEN {
        return Err(ZapEnvelopeError::ContentTypeTooLong {
            max: MAX_CONTENT_TYPE_LEN,
            actual,
        });
    }
    Ok(())
}

fn validate_metadata_len(actual: usize) -> Result<()> {
    if actual > MAX_METADATA_LEN {
        return Err(ZapEnvelopeError::MetadataTooLarge {
            max: MAX_METADATA_LEN,
            actual,
        });
    }
    Ok(())
}

fn validate_body_len(actual: u64) -> Result<()> {
    if actual > zap_core::MAX_PAYLOAD_LEN {
        return Err(ZapEnvelopeError::BodyTooLarge {
            max: zap_core::MAX_PAYLOAD_LEN,
            actual,
        });
    }
    Ok(())
}

fn checked_total_len(
    subject_len: usize,
    content_type_len: usize,
    metadata_len: usize,
    body_len: usize,
) -> Result<usize> {
    HEADER_LEN
        .checked_add(subject_len)
        .and_then(|len| len.checked_add(content_type_len))
        .and_then(|len| len.checked_add(metadata_len))
        .and_then(|len| len.checked_add(body_len))
        .ok_or(ZapEnvelopeError::LengthOverflow)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(byte: u8) -> Uuid {
        Uuid::from_bytes([byte; 16])
    }

    fn set_u16(bytes: &mut [u8], offset: usize, value: u16) {
        bytes[offset..offset + 2].copy_from_slice(&value.to_be_bytes());
    }

    fn set_u32(bytes: &mut [u8], offset: usize, value: u32) {
        bytes[offset..offset + 4].copy_from_slice(&value.to_be_bytes());
    }

    fn set_u64(bytes: &mut [u8], offset: usize, value: u64) {
        bytes[offset..offset + 8].copy_from_slice(&value.to_be_bytes());
    }

    #[test]
    fn encodes_binary_layout_and_parses_borrowed() {
        let envelope = ZapEnvelope::new(
            ZapMessageKind::Action,
            "thermostat.set",
            "application/zap-action",
            "heat",
        )
        .unwrap()
        .with_id(id(1))
        .with_correlation_id(id(2))
        .with_causation_id(id(3))
        .with_metadata(Bytes::from_static(b"trace=abc"))
        .unwrap();

        let encoded = envelope.encode();
        assert_eq!(&encoded[MAGIC_OFFSET..VERSION_OFFSET], b"ZENV");
        assert_eq!(&encoded[VERSION_OFFSET..KIND_OFFSET], &1_u16.to_be_bytes());
        assert_eq!(
            &encoded[KIND_OFFSET..RESERVED_OFFSET],
            &ZapMessageKind::Action.as_u16().to_be_bytes()
        );
        assert_eq!(&encoded[RESERVED_OFFSET..ID_OFFSET], &0_u16.to_be_bytes());
        assert_eq!(&encoded[ID_OFFSET..CORRELATION_ID_OFFSET], id(1).as_bytes());
        assert_eq!(
            &encoded[CORRELATION_ID_OFFSET..CAUSATION_ID_OFFSET],
            id(2).as_bytes()
        );
        assert_eq!(
            &encoded[CAUSATION_ID_OFFSET..SUBJECT_LEN_OFFSET],
            id(3).as_bytes()
        );
        assert_eq!(
            &encoded[SUBJECT_LEN_OFFSET..CONTENT_TYPE_LEN_OFFSET],
            &(b"thermostat.set".len() as u16).to_be_bytes()
        );
        assert_eq!(
            &encoded[CONTENT_TYPE_LEN_OFFSET..METADATA_LEN_OFFSET],
            &(b"application/zap-action".len() as u16).to_be_bytes()
        );
        assert_eq!(
            &encoded[METADATA_LEN_OFFSET..BODY_LEN_OFFSET],
            &(b"trace=abc".len() as u32).to_be_bytes()
        );
        assert_eq!(
            &encoded[BODY_LEN_OFFSET..HEADER_LEN],
            &(b"heat".len() as u64).to_be_bytes()
        );

        let parsed = ZapEnvelopeRef::parse(&encoded).unwrap();
        assert_eq!(parsed.kind(), ZapMessageKind::Action);
        assert_eq!(parsed.id(), id(1));
        assert_eq!(parsed.correlation_id(), Some(id(2)));
        assert_eq!(parsed.causation_id(), Some(id(3)));
        assert_eq!(parsed.subject(), "thermostat.set");
        assert_eq!(parsed.content_type(), "application/zap-action");
        assert_eq!(parsed.metadata(), b"trace=abc");
        assert_eq!(parsed.body(), b"heat");
    }

    #[test]
    fn data_can_omit_subject_and_absent_ids_are_nil_on_wire() {
        let envelope = ZapEnvelope::data("", Bytes::from_static(b"raw"))
            .unwrap()
            .with_id(id(9));

        let encoded = envelope.encode();
        assert_eq!(
            &encoded[CORRELATION_ID_OFFSET..CAUSATION_ID_OFFSET],
            Uuid::nil().as_bytes()
        );
        assert_eq!(
            &encoded[CAUSATION_ID_OFFSET..SUBJECT_LEN_OFFSET],
            Uuid::nil().as_bytes()
        );

        let parsed = ZapEnvelopeRef::parse(&encoded).unwrap();
        assert_eq!(parsed.kind(), ZapMessageKind::Data);
        assert_eq!(parsed.subject(), "");
        assert_eq!(parsed.correlation_id(), None);
        assert_eq!(parsed.causation_id(), None);
        assert_eq!(parsed.body(), b"raw");
    }

    #[test]
    fn helpers_create_expected_kinds() {
        assert_eq!(
            ZapEnvelope::event("sensor.changed", Bytes::new())
                .unwrap()
                .kind(),
            ZapMessageKind::Event
        );
        assert_eq!(
            ZapEnvelope::command("device.open", Bytes::new())
                .unwrap()
                .kind(),
            ZapMessageKind::Command
        );
        assert_eq!(
            ZapEnvelope::query("sensor.read", Bytes::new())
                .unwrap()
                .kind(),
            ZapMessageKind::Query
        );
        assert_eq!(
            ZapEnvelope::response("sensor.read", Bytes::new())
                .unwrap()
                .kind(),
            ZapMessageKind::Response
        );
        assert_eq!(
            ZapEnvelope::stream_chunk("stream.audio", Bytes::new())
                .unwrap()
                .kind(),
            ZapMessageKind::StreamChunk
        );
        assert_eq!(
            ZapEnvelope::control("node.pause", Bytes::new())
                .unwrap()
                .kind(),
            ZapMessageKind::Control
        );
    }

    #[test]
    fn rejects_missing_subject_for_non_data_kinds() {
        assert_eq!(
            ZapEnvelope::new(ZapMessageKind::Command, "", "", Bytes::new()).unwrap_err(),
            ZapEnvelopeError::MissingSubject {
                kind: ZapMessageKind::Command
            }
        );

        let mut encoded = ZapEnvelope::data("", Bytes::new())
            .unwrap()
            .encode()
            .to_vec();
        set_u16(&mut encoded, KIND_OFFSET, ZapMessageKind::Event.as_u16());
        assert_eq!(
            ZapEnvelopeRef::parse(&encoded).unwrap_err(),
            ZapEnvelopeError::MissingSubject {
                kind: ZapMessageKind::Event
            }
        );
    }

    #[test]
    fn accepts_declared_size_boundaries() {
        let subject = "s".repeat(MAX_SUBJECT_LEN);
        let content_type = "c".repeat(MAX_CONTENT_TYPE_LEN);
        let metadata = Bytes::from(vec![0xAB; MAX_METADATA_LEN]);
        let envelope = ZapEnvelope::new(
            ZapMessageKind::Event,
            subject.clone(),
            content_type.clone(),
            Bytes::new(),
        )
        .unwrap()
        .with_metadata(metadata.clone())
        .unwrap();

        let encoded = envelope.encode();
        let parsed = ZapEnvelopeRef::parse(&encoded).unwrap();
        assert_eq!(parsed.subject(), subject);
        assert_eq!(parsed.content_type(), content_type);
        assert_eq!(parsed.metadata(), metadata.as_ref());
    }

    #[test]
    fn rejects_size_limits_from_owned_api() {
        assert!(matches!(
            ZapEnvelope::event("s".repeat(MAX_SUBJECT_LEN + 1), Bytes::new()),
            Err(ZapEnvelopeError::SubjectTooLong { .. })
        ));
        assert!(matches!(
            ZapEnvelope::new(
                ZapMessageKind::Event,
                "s",
                "c".repeat(MAX_CONTENT_TYPE_LEN + 1),
                Bytes::new()
            ),
            Err(ZapEnvelopeError::ContentTypeTooLong { .. })
        ));
        assert!(matches!(
            ZapEnvelope::data("", Bytes::new())
                .unwrap()
                .with_metadata(vec![0; MAX_METADATA_LEN + 1]),
            Err(ZapEnvelopeError::MetadataTooLarge { .. })
        ));
    }

    #[test]
    fn rejects_invalid_header_fields() {
        let encoded = ZapEnvelope::event("subject", Bytes::new())
            .unwrap()
            .encode();

        let mut invalid = encoded.to_vec();
        invalid[MAGIC_OFFSET] = 0;
        assert!(matches!(
            ZapEnvelopeRef::parse(&invalid),
            Err(ZapEnvelopeError::InvalidMagic(_))
        ));

        let mut invalid = encoded.to_vec();
        set_u16(&mut invalid, VERSION_OFFSET, 2);
        assert_eq!(
            ZapEnvelopeRef::parse(&invalid),
            Err(ZapEnvelopeError::UnsupportedVersion(2))
        );

        let mut invalid = encoded.to_vec();
        set_u16(&mut invalid, KIND_OFFSET, 9);
        assert_eq!(
            ZapEnvelopeRef::parse(&invalid),
            Err(ZapEnvelopeError::UnknownKind(9))
        );

        let mut invalid = encoded.to_vec();
        set_u16(&mut invalid, RESERVED_OFFSET, 1);
        assert_eq!(
            ZapEnvelopeRef::parse(&invalid),
            Err(ZapEnvelopeError::NonzeroReserved(1))
        );
    }

    #[test]
    fn rejects_declared_size_limits_from_wire() {
        let encoded = ZapEnvelope::data("", Bytes::new()).unwrap().encode();

        let mut invalid = encoded.to_vec();
        set_u16(
            &mut invalid,
            SUBJECT_LEN_OFFSET,
            (MAX_SUBJECT_LEN + 1) as u16,
        );
        assert_eq!(
            ZapEnvelopeRef::parse(&invalid),
            Err(ZapEnvelopeError::SubjectTooLong {
                max: MAX_SUBJECT_LEN,
                actual: MAX_SUBJECT_LEN + 1
            })
        );

        let mut invalid = encoded.to_vec();
        set_u16(
            &mut invalid,
            CONTENT_TYPE_LEN_OFFSET,
            (MAX_CONTENT_TYPE_LEN + 1) as u16,
        );
        assert_eq!(
            ZapEnvelopeRef::parse(&invalid),
            Err(ZapEnvelopeError::ContentTypeTooLong {
                max: MAX_CONTENT_TYPE_LEN,
                actual: MAX_CONTENT_TYPE_LEN + 1
            })
        );

        let mut invalid = encoded.to_vec();
        set_u32(
            &mut invalid,
            METADATA_LEN_OFFSET,
            (MAX_METADATA_LEN + 1) as u32,
        );
        assert_eq!(
            ZapEnvelopeRef::parse(&invalid),
            Err(ZapEnvelopeError::MetadataTooLarge {
                max: MAX_METADATA_LEN,
                actual: MAX_METADATA_LEN + 1
            })
        );

        let mut invalid = encoded.to_vec();
        set_u64(&mut invalid, BODY_LEN_OFFSET, zap_core::MAX_PAYLOAD_LEN + 1);
        assert_eq!(
            ZapEnvelopeRef::parse(&invalid),
            Err(ZapEnvelopeError::BodyTooLarge {
                max: zap_core::MAX_PAYLOAD_LEN,
                actual: zap_core::MAX_PAYLOAD_LEN + 1
            })
        );
    }

    #[test]
    fn rejects_truncated_and_extra_bytes() {
        let encoded = ZapEnvelope::event("subject", "body").unwrap().encode();

        let truncated_header = &encoded[..HEADER_LEN - 1];
        assert_eq!(
            ZapEnvelopeRef::parse(truncated_header),
            Err(ZapEnvelopeError::Truncated {
                expected: HEADER_LEN,
                actual: HEADER_LEN - 1
            })
        );

        let truncated_body = &encoded[..encoded.len() - 1];
        assert_eq!(
            ZapEnvelopeRef::parse(truncated_body),
            Err(ZapEnvelopeError::Truncated {
                expected: encoded.len(),
                actual: encoded.len() - 1
            })
        );

        let mut extra = BytesMut::from(encoded.as_ref());
        extra.extend_from_slice(b"x");
        assert_eq!(
            ZapEnvelopeRef::parse(&extra),
            Err(ZapEnvelopeError::LengthMismatch {
                expected: encoded.len(),
                actual: encoded.len() + 1
            })
        );
    }

    #[test]
    fn rejects_invalid_utf8_subject_and_content_type() {
        let mut subject = ZapEnvelope::new(ZapMessageKind::Data, "", "", Bytes::new())
            .unwrap()
            .encode()
            .to_vec();
        set_u16(&mut subject, KIND_OFFSET, ZapMessageKind::Data.as_u16());
        set_u16(&mut subject, SUBJECT_LEN_OFFSET, 1);
        subject.extend_from_slice(&[0xFF]);
        assert_eq!(
            ZapEnvelopeRef::parse(&subject),
            Err(ZapEnvelopeError::InvalidUtf8 {
                field: ZapEnvelopeField::Subject
            })
        );

        let mut content_type = ZapEnvelope::new(ZapMessageKind::Data, "", "", Bytes::new())
            .unwrap()
            .encode()
            .to_vec();
        set_u16(&mut content_type, CONTENT_TYPE_LEN_OFFSET, 1);
        content_type.extend_from_slice(&[0xFF]);
        assert_eq!(
            ZapEnvelopeRef::parse(&content_type),
            Err(ZapEnvelopeError::InvalidUtf8 {
                field: ZapEnvelopeField::ContentType
            })
        );
    }
}
