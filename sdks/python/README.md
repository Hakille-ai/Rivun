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

## Test

```bash
python -m unittest discover -s sdks/python/tests
```
