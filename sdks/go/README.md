# ZAP Go SDK

Go helpers for ZAP `ZENV` control envelopes, local UDP requests, and ZapStore
registry payloads.

The SDK prepares bytes and JSON that can be handed to any ZAP transport, and it
includes `UDPClient` for loopback/dev peer integration.

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
`ArtifactHash` computes canonical BLAKE3 values through
`lukechampine.com/blake3`.

`VerifyEd25519Signature` verifies base64/base64-no-pad Ed25519 signatures with
Go's standard `crypto/ed25519` package.

## Test

```bash
go test ./sdks/go/...
```
