package zap

import (
	"encoding/json"
	"net"
	"strings"
	"testing"
	"time"
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
	hash, err := ArtifactHash([]byte("driver"))
	if err != nil {
		t.Fatal(err)
	}
	if !ValidateArtifactHash(hash) {
		t.Fatalf("invalid artifact hash %q", hash)
	}
	status := SignatureVerificationPlaceholder("registry")
	if status.Supported {
		t.Fatal("signature placeholder should be unsupported")
	}
}

func TestInstallPlanTypesCarryABIRequirementsAndMigrations(t *testing.T) {
	migration := DriverRegistryMigration{
		FromVersionRequirement:   "<2.0.0",
		FromABIRequirement:       ">=1,<=2",
		RequiresOperatorApproval: true,
		MigrationDriverAction:    "echo.migrate",
		MigrationDriverVersion:   "1.0.0",
	}
	request := RegistryInstallPlanRequest{
		Action:         "echo",
		Requirement:    "^2.0.0",
		ABIRequirement: ">=2,<4",
	}
	entry := RegistryInstallPlanEntry{
		Action:                  "echo",
		Requirement:             "^2.0.0",
		RequestedABIRequirement: ">=2,<4",
		SelectedVersion:         "2.1.0",
		Name:                    "echo-driver",
		ABIVersion:              2,
		WASMHash:                "blake3:" + strings.Repeat("0", 64),
		AuthorNodeID:            UUID{2},
		Migrations:              []DriverRegistryMigration{migration},
	}

	raw, err := json.Marshal(struct {
		Request RegistryInstallPlanRequest `json:"request"`
		Entry   RegistryInstallPlanEntry   `json:"entry"`
	}{Request: request, Entry: entry})
	if err != nil {
		t.Fatal(err)
	}
	if !strings.Contains(string(raw), `"abi_requirement":">=2,<4"`) {
		t.Fatalf("abi requirement missing from json: %s", raw)
	}
	if !strings.Contains(string(raw), `"migration_driver_action":"echo.migrate"`) {
		t.Fatalf("migration missing from json: %s", raw)
	}
}

func TestUDPClientSendsControlEnvelope(t *testing.T) {
	server, err := net.ListenPacket("udp", "127.0.0.1:0")
	if err != nil {
		t.Fatal(err)
	}
	defer server.Close()
	done := make(chan error, 1)
	go func() {
		buf := make([]byte, 65535)
		n, addr, err := server.ReadFrom(buf)
		if err != nil {
			done <- err
			return
		}
		_, err = server.WriteTo(buf[:n], addr)
		done <- err
	}()

	client, err := NewUDPClient("127.0.0.1:0")
	if err != nil {
		t.Fatal(err)
	}
	defer client.Close()
	frame, err := (ZapStoreClient{}).RegistryBundleManifestRequest(false, true)
	if err != nil {
		t.Fatal(err)
	}
	response, err := client.RequestControl(frame, server.LocalAddr().String(), 2*time.Second)
	if err != nil {
		t.Fatal(err)
	}
	if err := <-done; err != nil {
		t.Fatal(err)
	}
	if response.Subject != BundleManifestRequestSubject {
		t.Fatalf("subject = %q", response.Subject)
	}
}
