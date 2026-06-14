from __future__ import annotations

import json
import struct
from dataclasses import dataclass, field
from enum import IntEnum
from typing import Any
from uuid import UUID, uuid4

MAGIC = b"ZENV"
VERSION = 1
HEADER_LEN = 74
MAX_SUBJECT_LEN = 512
MAX_CONTENT_TYPE_LEN = 128
MAX_METADATA_LEN = 64 * 1024
MAX_BODY_LEN = 16 * 1024 * 1024
DEFAULT_CONTENT_TYPE = "application/octet-stream"

REGISTRY_INDEX_CONTENT_TYPE = "application/zap-registry-index+json"
REGISTRY_BUNDLE_MANIFEST_CONTENT_TYPE = "application/zap-registry-bundle-manifest+json"
REGISTRY_INDEX_REQUEST_SUBJECT = "zap.registry.index.request"
REGISTRY_INDEX_RESPONSE_SUBJECT = "zap.registry.index.response"
REGISTRY_BUNDLE_MANIFEST_REQUEST_SUBJECT = "zap.registry.bundle.manifest.request"
REGISTRY_BUNDLE_MANIFEST_RESPONSE_SUBJECT = "zap.registry.bundle.manifest.response"

NIL_UUID = UUID(int=0)


class ZapMessageKind(IntEnum):
    DATA = 1
    EVENT = 2
    COMMAND = 3
    QUERY = 4
    RESPONSE = 5
    STREAM_CHUNK = 6
    ACTION = 7
    CONTROL = 8

    @property
    def protocol_name(self) -> str:
        return {
            ZapMessageKind.DATA: "data",
            ZapMessageKind.EVENT: "event",
            ZapMessageKind.COMMAND: "command",
            ZapMessageKind.QUERY: "query",
            ZapMessageKind.RESPONSE: "response",
            ZapMessageKind.STREAM_CHUNK: "stream_chunk",
            ZapMessageKind.ACTION: "action",
            ZapMessageKind.CONTROL: "control",
        }[self]

    def requires_subject(self) -> bool:
        return self is not ZapMessageKind.DATA


@dataclass(frozen=True)
class ZapEnvelope:
    kind: ZapMessageKind
    subject: str
    content_type: str
    body: bytes = b""
    metadata: bytes = b""
    id: UUID = field(default_factory=uuid4)
    correlation_id: UUID | None = None
    causation_id: UUID | None = None

    def __post_init__(self) -> None:
        _validate_parts(
            self.kind,
            self.subject.encode("utf-8"),
            self.content_type.encode("utf-8"),
            len(self.metadata),
            len(self.body),
        )

    def encode(self) -> bytes:
        subject = self.subject.encode("utf-8")
        content_type = self.content_type.encode("utf-8")
        header = bytearray()
        header.extend(MAGIC)
        header.extend(struct.pack(">HHH", VERSION, int(self.kind), 0))
        header.extend(self.id.bytes)
        header.extend((self.correlation_id or NIL_UUID).bytes)
        header.extend((self.causation_id or NIL_UUID).bytes)
        header.extend(struct.pack(">HHIQ", len(subject), len(content_type), len(self.metadata), len(self.body)))
        return bytes(header) + subject + content_type + self.metadata + self.body

    @classmethod
    def decode(cls, payload: bytes) -> "ZapEnvelope":
        if len(payload) < HEADER_LEN:
            raise ValueError(f"envelope too short: expected at least {HEADER_LEN}, got {len(payload)}")
        if payload[0:4] != MAGIC:
            raise ValueError("invalid envelope magic")
        version, kind_value, reserved = struct.unpack(">HHH", payload[4:10])
        if version != VERSION:
            raise ValueError(f"unsupported envelope version {version}")
        if reserved != 0:
            raise ValueError(f"reserved envelope field must be zero, got {reserved}")
        try:
            kind = ZapMessageKind(kind_value)
        except ValueError as exc:
            raise ValueError(f"unknown envelope kind {kind_value}") from exc

        envelope_id = UUID(bytes=payload[10:26])
        correlation_id = _optional_uuid(payload[26:42])
        causation_id = _optional_uuid(payload[42:58])
        subject_len, content_type_len, metadata_len, body_len = struct.unpack(">HHIQ", payload[58:74])
        _validate_parts(kind, b"x" * subject_len, b"x" * content_type_len, metadata_len, body_len)

        expected = HEADER_LEN + subject_len + content_type_len + metadata_len + body_len
        if len(payload) != expected:
            raise ValueError(f"envelope length mismatch: expected {expected}, got {len(payload)}")

        subject_start = HEADER_LEN
        content_type_start = subject_start + subject_len
        metadata_start = content_type_start + content_type_len
        body_start = metadata_start + metadata_len
        subject = payload[subject_start:content_type_start].decode("utf-8")
        content_type = payload[content_type_start:metadata_start].decode("utf-8")
        metadata = payload[metadata_start:body_start]
        body = payload[body_start:]
        return cls(
            kind=kind,
            subject=subject,
            content_type=content_type,
            body=body,
            metadata=metadata,
            id=envelope_id,
            correlation_id=correlation_id,
            causation_id=causation_id,
        )


@dataclass(frozen=True)
class ControlFrame:
    subject: str
    content_type: str
    body: bytes
    metadata: bytes = b""
    id: UUID = field(default_factory=uuid4)
    correlation_id: UUID | None = None
    causation_id: UUID | None = None

    @classmethod
    def json(
        cls,
        subject: str,
        content_type: str,
        payload: Any,
        *,
        id: UUID | None = None,
        correlation_id: UUID | None = None,
        causation_id: UUID | None = None,
    ) -> "ControlFrame":
        body = json.dumps(payload, separators=(",", ":"), ensure_ascii=False).encode("utf-8")
        return cls(
            subject=subject,
            content_type=content_type,
            body=body,
            id=id or uuid4(),
            correlation_id=correlation_id,
            causation_id=causation_id,
        )

    def to_envelope(self) -> ZapEnvelope:
        return ZapEnvelope(
            kind=ZapMessageKind.CONTROL,
            subject=self.subject,
            content_type=self.content_type,
            body=self.body,
            metadata=self.metadata,
            id=self.id,
            correlation_id=self.correlation_id,
            causation_id=self.causation_id,
        )

    def encode(self) -> bytes:
        return self.to_envelope().encode()

    def json_body(self) -> Any:
        return json.loads(self.body.decode("utf-8"))

    @classmethod
    def decode(cls, payload: bytes) -> "ControlFrame":
        envelope = ZapEnvelope.decode(payload)
        if envelope.kind is not ZapMessageKind.CONTROL:
            raise ValueError(f"expected control envelope, got {envelope.kind.protocol_name}")
        return cls(
            subject=envelope.subject,
            content_type=envelope.content_type,
            body=envelope.body,
            metadata=envelope.metadata,
            id=envelope.id,
            correlation_id=envelope.correlation_id,
            causation_id=envelope.causation_id,
        )


def _optional_uuid(raw: bytes) -> UUID | None:
    value = UUID(bytes=raw)
    return None if value == NIL_UUID else value


def _validate_parts(
    kind: ZapMessageKind,
    subject: bytes,
    content_type: bytes,
    metadata_len: int,
    body_len: int,
) -> None:
    if len(subject) > MAX_SUBJECT_LEN:
        raise ValueError(f"subject length exceeds maximum {MAX_SUBJECT_LEN}")
    if len(content_type) > MAX_CONTENT_TYPE_LEN:
        raise ValueError(f"content_type length exceeds maximum {MAX_CONTENT_TYPE_LEN}")
    if metadata_len > MAX_METADATA_LEN:
        raise ValueError(f"metadata length exceeds maximum {MAX_METADATA_LEN}")
    if body_len > MAX_BODY_LEN:
        raise ValueError(f"body length exceeds maximum {MAX_BODY_LEN}")
    if kind.requires_subject() and len(subject) == 0:
        raise ValueError(f"subject is required for {kind.protocol_name} envelopes")
