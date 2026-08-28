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
	parsed, err := rivun.DecodeControlFrame(payload)
	if err != nil {
		panic(err)
	}
	fmt.Printf("built %s (%d bytes)\n", parsed.Subject, len(payload))
}
