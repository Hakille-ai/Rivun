# Rivun Go SDK

Go helpers for Rivun `ZENV` control envelopes, local UDP requests, and RivunStore
registry payloads.

The SDK prepares bytes and JSON that can be handed to any Rivun transport, and it
includes `UDPClient` for loopback/dev peer integration.

## Build a registry bundle manifest request

```go
package main

import (
	"fmt"

	rivun "github.com/rivun-protocol/rivun-sdk-go"
)

func main() {
	frame, err := rivun.RegistryBundleManifestRequestFrame(true, true)
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

## Shared fixtures and conformance

The Go tests read the shared protocol fixtures from the repository-level
`fixtures/` directory. They currently assert:

- `ZENV-control-registry-bundle-manifest-request.json` matches the Go RivunStore
  request helper and round-trips through `ControlFrame`.
- `control-subjects-v1.json` includes the Go registry subjects and expected
  media types.

To add a fixture, create a small deterministic JSON file in `fixtures/`, then
add or extend a test in `sdks/go/protocol_test.go` that loads it with
`loadRootFixture` and checks the schema version, subject, media type, and body
fields that define the contract.

## Test

```bash
go test ./sdks/go/...
```

This command requires a local Go toolchain. CI installs Go before running the
SDK workflow; local machines without `go` installed cannot execute these tests.

