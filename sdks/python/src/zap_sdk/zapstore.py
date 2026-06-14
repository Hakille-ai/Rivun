from __future__ import annotations

import json
import re
from dataclasses import dataclass, fields, is_dataclass
from typing import Any
from uuid import UUID

from .protocol import (
    REGISTRY_BUNDLE_MANIFEST_CONTENT_TYPE,
    REGISTRY_BUNDLE_MANIFEST_REQUEST_SUBJECT,
    REGISTRY_INDEX_CONTENT_TYPE,
    REGISTRY_INDEX_REQUEST_SUBJECT,
    ControlFrame,
)

REGISTRY_INDEX_SYNC_SCHEMA_VERSION = 1
REGISTRY_BUNDLE_SCHEMA_VERSION = 1
REGISTRY_INSTALL_PLAN_SCHEMA_VERSION = 1
DRIVER_ABI_VERSION = 1
DRIVER_HASH_PREFIX = "blake3:"

_HASH_RE = re.compile(r"^blake3:[0-9a-f]{64}$")


class MissingCryptoBackend(RuntimeError):
    pass


@dataclass(frozen=True)
class SignatureVerificationStatus:
    supported: bool
    reason: str


@dataclass(frozen=True)
class RegistryIndexRequest:
    schema_version: int = REGISTRY_INDEX_SYNC_SCHEMA_VERSION
    require_signature: bool = False

    def to_dict(self) -> dict[str, Any]:
        return _to_plain(self)


@dataclass(frozen=True)
class ZapStoreClient:
    def registry_index_request(self, require_signature: bool = False) -> ControlFrame:
        return registry_index_request_frame(require_signature=require_signature)

    def registry_bundle_manifest_request(
        self, *, require_publication: bool = False, require_drivers: bool = False
    ) -> ControlFrame:
        return registry_bundle_manifest_request_frame(
            require_publication=require_publication,
            require_drivers=require_drivers,
        )


@dataclass(frozen=True)
class RegistryIndexResponse:
    schema_version: int
    node_id: UUID
    registry: "DriverRegistry | None" = None
    unavailable_reason: str | None = None

    @classmethod
    def from_dict(cls, data: dict[str, Any]) -> "RegistryIndexResponse":
        return cls(
            schema_version=data["schema_version"],
            node_id=UUID(str(data["node_id"])),
            registry=DriverRegistry.from_dict(data["registry"]) if data.get("registry") else None,
            unavailable_reason=data.get("unavailable_reason"),
        )

    def to_dict(self) -> dict[str, Any]:
        return _to_plain(self)


@dataclass(frozen=True)
class RegistryBundleManifestRequest:
    schema_version: int = REGISTRY_BUNDLE_SCHEMA_VERSION
    require_publication: bool = False
    require_drivers: bool = False

    def to_dict(self) -> dict[str, Any]:
        return _to_plain(self)


@dataclass(frozen=True)
class RegistryBundleManifestResponse:
    schema_version: int
    node_id: UUID
    manifest: "RegistryBundleManifest | None" = None
    unavailable_reason: str | None = None

    @classmethod
    def from_dict(cls, data: dict[str, Any]) -> "RegistryBundleManifestResponse":
        return cls(
            schema_version=data["schema_version"],
            node_id=UUID(str(data["node_id"])),
            manifest=RegistryBundleManifest.from_dict(data["manifest"]) if data.get("manifest") else None,
            unavailable_reason=data.get("unavailable_reason"),
        )

    def to_dict(self) -> dict[str, Any]:
        return _to_plain(self)

    def verify_shape(self, request: RegistryBundleManifestRequest) -> None:
        if self.schema_version != REGISTRY_BUNDLE_SCHEMA_VERSION:
            raise ValueError(f"unsupported registry bundle schema version {self.schema_version}")
        if request.schema_version != REGISTRY_BUNDLE_SCHEMA_VERSION:
            raise ValueError(f"unsupported registry bundle request schema version {request.schema_version}")
        if self.manifest is None:
            return
        self.manifest.validate_shape()
        if request.require_publication and (
            self.manifest.publication_path is None or self.manifest.publication_hash is None
        ):
            raise ValueError("registry bundle publication path/hash metadata is incomplete")
        if request.require_drivers:
            for entry in self.manifest.entries:
                if entry.driver_path is None or entry.driver_hash is None:
                    raise ValueError(f"registry bundle entry {entry.action}@{entry.version} lacks driver metadata")


@dataclass(frozen=True)
class DriverRegistryEntry:
    name: str
    version: str
    action: str
    abi_version: int
    wasm_hash: str
    author_node_id: UUID
    manifest_path: str | None = None
    status: str = "active"
    revoked_reason: str | None = None
    deprecated_reason: str | None = None

    @classmethod
    def from_dict(cls, data: dict[str, Any]) -> "DriverRegistryEntry":
        return cls(
            name=data["name"],
            version=data["version"],
            action=data["action"],
            abi_version=data["abi_version"],
            wasm_hash=data["wasm_hash"],
            manifest_path=data.get("manifest_path"),
            author_node_id=UUID(str(data["author_node_id"])),
            status=data.get("status", "active"),
            revoked_reason=data.get("revoked_reason"),
            deprecated_reason=data.get("deprecated_reason"),
        )


@dataclass(frozen=True)
class DriverRegistry:
    schema_version: int
    entries: list[DriverRegistryEntry]
    generated_by: str | None = None
    operator_node_id: UUID | None = None
    operator_public_key: str | None = None
    signature: str | None = None

    @classmethod
    def from_dict(cls, data: dict[str, Any]) -> "DriverRegistry":
        return cls(
            schema_version=data["schema_version"],
            generated_by=data.get("generated_by"),
            operator_node_id=UUID(str(data["operator_node_id"])) if data.get("operator_node_id") else None,
            operator_public_key=data.get("operator_public_key"),
            signature=data.get("signature"),
            entries=[DriverRegistryEntry.from_dict(entry) for entry in data.get("entries", [])],
        )

    def to_dict(self) -> dict[str, Any]:
        return _to_plain(self)


@dataclass(frozen=True)
class RegistryBundleManifest:
    schema_version: int
    registry_path: str
    registry_hash: str
    entries: list["RegistryBundleEntry"]
    generated_by: str | None = None
    publication_path: str | None = None
    publication_hash: str | None = None

    @classmethod
    def from_dict(cls, data: dict[str, Any]) -> "RegistryBundleManifest":
        return cls(
            schema_version=data["schema_version"],
            generated_by=data.get("generated_by"),
            registry_path=data["registry_path"],
            registry_hash=data["registry_hash"],
            publication_path=data.get("publication_path"),
            publication_hash=data.get("publication_hash"),
            entries=[RegistryBundleEntry.from_dict(entry) for entry in data.get("entries", [])],
        )

    def to_dict(self) -> dict[str, Any]:
        return _to_plain(self)

    def validate_shape(self) -> None:
        if self.schema_version != REGISTRY_BUNDLE_SCHEMA_VERSION:
            raise ValueError(f"unsupported registry bundle schema version {self.schema_version}")
        _validate_relative_path(self.registry_path)
        if not validate_artifact_hash(self.registry_hash):
            raise ValueError(f"invalid registry hash {self.registry_hash!r}")
        if (self.publication_path is None) != (self.publication_hash is None):
            raise ValueError("registry bundle publication path/hash metadata is incomplete")
        if self.publication_path is not None:
            _validate_relative_path(self.publication_path)
        if self.publication_hash is not None and not validate_artifact_hash(self.publication_hash):
            raise ValueError(f"invalid publication hash {self.publication_hash!r}")
        seen: set[tuple[str, str]] = set()
        for entry in self.entries:
            key = (entry.action, entry.version)
            if key in seen:
                raise ValueError(f"duplicate registry bundle entry {entry.action}@{entry.version}")
            seen.add(key)
            entry.validate_shape()


@dataclass(frozen=True)
class RegistryBundleEntry:
    action: str
    version: str
    name: str
    abi_version: int
    wasm_hash: str
    author_node_id: UUID
    status: str = "active"
    manifest_path: str | None = None
    manifest_hash: str | None = None
    driver_path: str | None = None
    driver_hash: str | None = None

    @classmethod
    def from_dict(cls, data: dict[str, Any]) -> "RegistryBundleEntry":
        return cls(
            action=data["action"],
            version=data["version"],
            name=data["name"],
            abi_version=data["abi_version"],
            wasm_hash=data["wasm_hash"],
            author_node_id=UUID(str(data["author_node_id"])),
            status=data.get("status", "active"),
            manifest_path=data.get("manifest_path"),
            manifest_hash=data.get("manifest_hash"),
            driver_path=data.get("driver_path"),
            driver_hash=data.get("driver_hash"),
        )

    def validate_shape(self) -> None:
        if not self.action.strip():
            raise ValueError("driver action must not be empty")
        if not validate_artifact_hash(self.wasm_hash):
            raise ValueError(f"invalid wasm hash {self.wasm_hash!r}")
        if (self.manifest_path is None) != (self.manifest_hash is None):
            raise ValueError(f"registry bundle entry {self.action}@{self.version} has incomplete manifest metadata")
        if self.manifest_path is not None:
            _validate_relative_path(self.manifest_path)
        if self.manifest_hash is not None and not validate_artifact_hash(self.manifest_hash):
            raise ValueError(f"invalid manifest hash {self.manifest_hash!r}")
        if (self.driver_path is None) != (self.driver_hash is None):
            raise ValueError(f"registry bundle entry {self.action}@{self.version} has incomplete driver metadata")
        if self.driver_path is not None:
            _validate_relative_path(self.driver_path)
        if self.driver_hash is not None:
            if not validate_artifact_hash(self.driver_hash):
                raise ValueError(f"invalid driver hash {self.driver_hash!r}")
            if self.driver_hash != self.wasm_hash:
                raise ValueError(f"driver hash does not match wasm hash for {self.action}@{self.version}")


@dataclass(frozen=True)
class RegistryInstallPlanRequest:
    action: str
    requirement: str
    abi_version: int | None = None

    def to_dict(self) -> dict[str, Any]:
        return _to_plain(self)


@dataclass(frozen=True)
class RegistryInstallPlanEntry:
    action: str
    requirement: str
    selected_version: str
    name: str
    abi_version: int
    wasm_hash: str
    author_node_id: UUID
    requested_abi_version: int | None = None
    manifest_path: str | None = None


@dataclass(frozen=True)
class RegistryInstallPlan:
    schema_version: int
    registry_hash: str
    registry_entries: int
    requested_at_micros: int
    entries: list[RegistryInstallPlanEntry]
    planner_node_id: UUID
    planner_public_key: str
    signature: str
    registry_operator_node_id: UUID | None = None
    publication_hash: str | None = None
    target: str | None = None
    labels: list[str] | None = None

    def to_dict(self) -> dict[str, Any]:
        return _to_plain(self)


def registry_index_request_frame(require_signature: bool = False) -> ControlFrame:
    request = RegistryIndexRequest(require_signature=require_signature)
    return ControlFrame.json(REGISTRY_INDEX_REQUEST_SUBJECT, REGISTRY_INDEX_CONTENT_TYPE, request.to_dict())


def registry_bundle_manifest_request_frame(
    *, require_publication: bool = False, require_drivers: bool = False
) -> ControlFrame:
    request = RegistryBundleManifestRequest(
        require_publication=require_publication,
        require_drivers=require_drivers,
    )
    return ControlFrame.json(
        REGISTRY_BUNDLE_MANIFEST_REQUEST_SUBJECT,
        REGISTRY_BUNDLE_MANIFEST_CONTENT_TYPE,
        request.to_dict(),
    )


def validate_artifact_hash(value: str) -> bool:
    return bool(_HASH_RE.fullmatch(value))


def artifact_hash(data: bytes) -> str:
    try:
        import blake3  # type: ignore[import-not-found]
    except ImportError as exc:
        raise MissingCryptoBackend(
            "canonical ZAP artifact hashes use BLAKE3; install the optional 'blake3' Python package "
            "or verify hashes with zap-cli"
        ) from exc
    return f"{DRIVER_HASH_PREFIX}{blake3.blake3(data).hexdigest()}"


def registry_hash(registry: DriverRegistry) -> str:
    encoded = json.dumps(registry.to_dict(), separators=(",", ":"), ensure_ascii=False).encode("utf-8")
    return artifact_hash(encoded)


def verify_signature_placeholder(kind: str) -> SignatureVerificationStatus:
    return SignatureVerificationStatus(
        supported=False,
        reason=(
            f"{kind} signatures are Ed25519 signatures over ZAP domain-separated payloads. "
            "This dependency-free Python SDK does not verify them yet; use zap-cli or the Rust SDK."
        ),
    )


def _to_plain(value: Any) -> Any:
    if isinstance(value, UUID):
        return str(value)
    if is_dataclass(value):
        output: dict[str, Any] = {}
        for item in fields(value):
            raw = getattr(value, item.name)
            if raw is None:
                continue
            output[item.name] = _to_plain(raw)
        return output
    if isinstance(value, list):
        return [_to_plain(item) for item in value]
    if isinstance(value, dict):
        return {key: _to_plain(item) for key, item in value.items() if item is not None}
    return value


def _validate_relative_path(path: str) -> None:
    if not path or path.startswith(("/", "\\")):
        raise ValueError(f"bundle path {path!r} is not a safe relative path")
    parts = path.replace("\\", "/").split("/")
    if any(part in ("", ".", "..") for part in parts):
        raise ValueError(f"bundle path {path!r} is not a safe relative path")
