package zap

import (
	"encoding/json"
	"strings"
	"testing"
)

func TestRegistryBundleManifestControlFrameRoundTrips(t *testing.T) {
	frame, err := (ZapStoreClient{}).RegistryBundleManifestRequest(true, true)
	if err != nil {
		t.Fatal(err)
	}
	encoded, err := frame.Encode()
	if err != nil {
		t.Fatal(err)
	}
	decoded, err := DecodeControlFrame(encoded)
	if err != nil {
		t.Fatal(err)
	}
	if decoded.Subject != BundleManifestRequestSubject {
		t.Fatalf("subject = %q", decoded.Subject)
	}
	if decoded.ContentType != RegistryBundleContentType {
		t.Fatalf("content type = %q", decoded.ContentType)
	}
	if string(encoded[0:4]) != EnvelopeMagic {
		t.Fatalf("magic = %q", string(encoded[0:4]))
	}
	if kind := readU16(encoded[6:8]); kind != uint16(KindControl) {
		t.Fatalf("kind = %d", kind)
	}

	var body RegistryBundleManifestRequest
	if err := decoded.JSONBody(&body); err != nil {
		t.Fatal(err)
	}
	if !body.RequirePublication || !body.RequireDrivers {
		raw, _ := json.Marshal(body)
		t.Fatalf("body = %s", raw)
	}
}

func TestRegistryBundleManifestResponseRequiresDriverMetadata(t *testing.T) {
	hash := "blake3:" + strings.Repeat("0", 64)

	response := RegistryBundleManifestResponse{
		SchemaVersion: RegistryBundleSchemaVersion,
		NodeID:        UUID{1},
		Manifest: &RegistryBundleManifest{
			SchemaVersion: RegistryBundleSchemaVersion,
			RegistryPath:  "registry.index.toml",
			RegistryHash:  hash,
			Entries: []RegistryBundleEntry{
				{
					Action:       "echo",
					Version:      "0.1.0",
					Name:         "echo-driver",
					ABIVersion:   DriverABIVersion,
					WASMHash:     hash,
					AuthorNodeID: UUID{},
					Status:       DriverRegistryStatusActive,
					ManifestPath: "manifests/echo.toml",
					ManifestHash: hash,
				},
			},
		},
	}

	err := response.VerifyShape(RegistryBundleManifestRequest{
		SchemaVersion:  RegistryBundleSchemaVersion,
		RequireDrivers: true,
	})
	if err == nil {
		t.Fatal("expected missing driver metadata error")
	}
}

func TestHashAndSignatureHelpersAreExplicit(t *testing.T) {
	valid := "blake3:0000000000000000000000000000000000000000000000000000000000000000"
	if !ValidateArtifactHash(valid) {
		t.Fatal("expected valid artifact hash")
	}
	if ValidateArtifactHash("sha256:0000000000000000000000000000000000000000000000000000000000000000") {
		t.Fatal("sha256 hash should not be accepted")
	}
	if _, err := ArtifactHash([]byte("driver")); err == nil {
		t.Fatal("expected explicit BLAKE3 backend error")
	}
	status := SignatureVerificationPlaceholder("registry")
	if status.Supported {
		t.Fatal("signature placeholder should be unsupported")
	}
}
