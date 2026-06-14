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
	parsed, err := zap.DecodeControlFrame(payload)
	if err != nil {
		panic(err)
	}
	fmt.Printf("built %s (%d bytes)\n", parsed.Subject, len(payload))
}
