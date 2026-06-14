# ZAP Go SDK

Pure-standard-library Go helpers for ZAP `ZENV` control envelopes and ZapStore
registry payloads.

The SDK is network-free: it prepares bytes and JSON that can be handed to a ZAP
transport, CLI bridge, or integration test.

## Build a registry bundle manifest request

```go
package main

import (
	"fmt"

	zap "github.com/zap-protocol/zap-sdk-go"
)

func main() {
	frame, err := zap.RegistryBundleManifestRequestFrame(true, true)
	if err != nil {
		panic(err)
	}
	payload, err := frame.Encode()
	if err != nil {
		panic(err)
	}
	fmt.Println(frame.Subject, len(payload))
}
```

## Integrity helpers

`ValidateArtifactHash` checks the canonical `blake3:<64 hex chars>` shape.
Go's standard library does not provide BLAKE3, so `ArtifactHash` returns an
explicit error. Use `zap-cli`, the Rust SDK, or inject a vetted BLAKE3
implementation in the application layer.

Signature verification is represented by `SignatureVerificationPlaceholder`
until this package grows a vetted Ed25519 implementation over the exact ZAP
domain-separated payloads.

## Test

```bash
go test ./sdks/go/...
```
