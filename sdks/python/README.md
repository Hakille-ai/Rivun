# ZAP Python SDK

Lightweight Python helpers for building ZAP control envelopes, local UDP
requests, and ZapStore request/response payloads.

The SDK prepares bytes and JSON that can be handed to any ZAP transport, and it
includes `ZapUdpClient` for loopback/dev peer integration.

## Install locally

```bash
python -m pip install -e sdks/python
```

## Build a registry bundle manifest request

```python
from zap_sdk import registry_bundle_manifest_request_frame

frame = registry_bundle_manifest_request_frame(
    require_publication=True,
    require_drivers=True,
)

wire_payload = frame.encode()
assert frame.subject == "zap.registry.bundle.manifest.request"
```

## Parse a control envelope

```python
from zap_sdk import ControlFrame

parsed = ControlFrame.decode(wire_payload)
print(parsed.json_body())
```

## Integrity helpers

`validate_artifact_hash()` checks the canonical `blake3:<64 hex chars>` shape.
`artifact_hash()` computes the canonical hash only when the optional `blake3`
Python package is installed. Without it, the SDK raises `MissingCryptoBackend`
instead of returning a non-ZAP checksum.

`verify_ed25519_signature()` verifies signatures when the optional `PyNaCl`
package is installed. Install both optional crypto backends with:

```bash
python -m pip install -e "sdks/python[crypto]"
```

## Shared fixtures and conformance

The Python tests read the shared protocol fixtures from the repository-level
`fixtures/` directory. They currently assert:

- `zenv-control-registry-bundle-manifest-request.json` matches the Python
  ZapStore request helper and round-trips through `ControlFrame`.
- `control-subjects-v1.json` stays aligned with the registry control subjects
  exposed by the SDK.
- `agent-intent-message-v1.json` can be carried as an
  `application/zap-agent+json` control envelope.

To add a fixture, create a small deterministic JSON file in `fixtures/`, then
add or extend a test in `sdks/python/tests/test_protocol.py` that loads it via
the shared fixture helper and checks the subject, media type, schema version,
and body fields that must remain stable.

## Test

```bash
python -m unittest discover -s sdks/python/tests
```

Install optional crypto dependencies before running tests that need canonical
BLAKE3 hashing or Ed25519 signature verification:

```bash
python -m pip install -e "sdks/python[crypto]"
```
