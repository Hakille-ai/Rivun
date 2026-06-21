package zap

import (
	"bytes"
	"encoding/json"
	"net"
	"os"
	"path/filepath"
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

func TestEnvelopeRejectsInvalidUTF8(t *testing.T) {
	if _, err := NewEnvelope(KindControl, string([]byte{0xff}), "application/json", nil); err == nil {
		t.Fatal("expected invalid UTF-8 subject to be rejected")
	}

	env, err := NewEnvelope(KindControl, "zap.test", "application/json", []byte("{}"))
	if err != nil {
		t.Fatal(err)
	}
	encoded, err := env.Encode()
	if err != nil {
		t.Fatal(err)
	}
	encoded[EnvelopeHeaderLen] = 0xff

	if _, err := DecodeEnvelope(encoded); err == nil || !strings.Contains(err.Error(), "invalid UTF-8 in subject") {
		t.Fatalf("DecodeEnvelope error = %v", err)
	}
}

func TestRegistryBundleManifestRequestFixtureMatchesSDK(t *testing.T) {
	var fixture struct {
		Envelope struct {
			KindName    string                        `json:"kind_name"`
			KindValue   uint16                        `json:"kind_value"`
			Subject     string                        `json:"subject"`
			ContentType string                        `json:"content_type"`
			BodyJSON    RegistryBundleManifestRequest `json:"body_json"`
		} `json:"envelope"`
	}
	loadRootFixture(t, "zenv-control-registry-bundle-manifest-request.json", &fixture)

	frame, err := RegistryBundleManifestRequestFrame(
		fixture.Envelope.BodyJSON.RequirePublication,
		fixture.Envelope.BodyJSON.RequireDrivers,
	)
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

	if fixture.Envelope.KindName != "control" || fixture.Envelope.KindValue != uint16(KindControl) {
		t.Fatalf("fixture kind mismatch: %+v", fixture.Envelope)
	}
	if decoded.Subject != fixture.Envelope.Subject {
		t.Fatalf("subject = %q, fixture = %q", decoded.Subject, fixture.Envelope.Subject)
	}
	if decoded.ContentType != fixture.Envelope.ContentType {
		t.Fatalf("content_type = %q, fixture = %q", decoded.ContentType, fixture.Envelope.ContentType)
	}
	var body RegistryBundleManifestRequest
	if err := decoded.JSONBody(&body); err != nil {
		t.Fatal(err)
	}
	if body != fixture.Envelope.BodyJSON {
		t.Fatalf("body = %+v, fixture = %+v", body, fixture.Envelope.BodyJSON)
	}
}

func TestControlSubjectFixtureContainsSDKRegistrySubjects(t *testing.T) {
	var fixture struct {
		Envelope struct {
			KindValue uint16 `json:"kind_value"`
		} `json:"envelope"`
		Subjects []struct {
			Subject     string `json:"subject"`
			ContentType string `json:"content_type"`
		} `json:"subjects"`
	}
	loadRootFixture(t, "control-subjects-v1.json", &fixture)

	if fixture.Envelope.KindValue != uint16(KindControl) {
		t.Fatalf("control fixture kind = %d", fixture.Envelope.KindValue)
	}
	subjects := map[string]string{}
	for _, entry := range fixture.Subjects {
		subjects[entry.Subject] = entry.ContentType
	}
	expected := map[string]string{
		RegistryIndexRequestSubject:   RegistryIndexContentType,
		RegistryIndexResponseSubject:  RegistryIndexContentType,
		BundleManifestRequestSubject:  RegistryBundleContentType,
		BundleManifestResponseSubject: RegistryBundleContentType,
		PactBundleSubject:             PactContentType,
	}
	for subject, contentType := range expected {
		if subjects[subject] != contentType {
			t.Fatalf("fixture subject %s content type = %q, want %q", subject, subjects[subject], contentType)
		}
	}
}

func TestUnsignedControlFrameFixtureRoundTripsWithoutSecurityTrailers(t *testing.T) {
	var fixture struct {
		FixtureSchemaVersion uint8 `json:"fixture_schema_version"`
		Envelope             struct {
			Magic         string               `json:"magic"`
			Version       uint16               `json:"version"`
			KindName      string               `json:"kind_name"`
			KindValue     uint16               `json:"kind_value"`
			ID            UUID                 `json:"id"`
			CorrelationID UUID                 `json:"correlation_id"`
			CausationID   UUID                 `json:"causation_id"`
			Subject       string               `json:"subject"`
			ContentType   string               `json:"content_type"`
			Metadata      string               `json:"metadata_base64"`
			BodyJSON      RegistryIndexRequest `json:"body_json"`
		} `json:"envelope"`
		Security struct {
			Signed           bool    `json:"signed"`
			Encrypted        bool    `json:"encrypted"`
			SignatureHintHex string  `json:"signature_hint_hex"`
			AuthTrailer      *string `json:"auth_trailer"`
			POATrailer       *string `json:"poa_trailer"`
		} `json:"security"`
	}
	loadRootFixture(t, filepath.Join("protocol", "zenv-unsigned-control-frame-v1.json"), &fixture)

	frame, err := RegistryIndexRequestFrame(fixture.Envelope.BodyJSON.RequireSignature)
	if err != nil {
		t.Fatal(err)
	}
	frame.ID = fixture.Envelope.ID
	encoded, err := frame.Encode()
	if err != nil {
		t.Fatal(err)
	}
	decoded, err := DecodeEnvelope(encoded)
	if err != nil {
		t.Fatal(err)
	}

	if fixture.FixtureSchemaVersion != 1 {
		t.Fatalf("fixture schema version = %d", fixture.FixtureSchemaVersion)
	}
	if fixture.Envelope.Magic != EnvelopeMagic || fixture.Envelope.Version != EnvelopeVersion {
		t.Fatalf("fixture envelope version mismatch: %+v", fixture.Envelope)
	}
	if decoded.Kind != KindControl || fixture.Envelope.KindName != "control" || fixture.Envelope.KindValue != uint16(KindControl) {
		t.Fatalf("kind mismatch decoded=%d fixture=%+v", decoded.Kind, fixture.Envelope)
	}
	if decoded.ID != fixture.Envelope.ID || decoded.CorrelationID != nil || decoded.CausationID != nil {
		t.Fatalf("decoded ids mismatch: %+v", decoded)
	}
	if decoded.Subject != RegistryIndexRequestSubject || decoded.Subject != fixture.Envelope.Subject {
		t.Fatalf("subject = %q", decoded.Subject)
	}
	if decoded.ContentType != RegistryIndexContentType || decoded.ContentType != fixture.Envelope.ContentType {
		t.Fatalf("content type = %q", decoded.ContentType)
	}
	var body RegistryIndexRequest
	if err := json.Unmarshal(decoded.Body, &body); err != nil {
		t.Fatal(err)
	}
	if body != fixture.Envelope.BodyJSON {
		t.Fatalf("body = %+v, fixture = %+v", body, fixture.Envelope.BodyJSON)
	}
	if fixture.Security.Signed || fixture.Security.Encrypted || fixture.Security.AuthTrailer != nil || fixture.Security.POATrailer != nil {
		t.Fatalf("fixture should document absent security trailers: %+v", fixture.Security)
	}
	if fixture.Security.SignatureHintHex != "0000000000000000" {
		t.Fatalf("signature hint = %q", fixture.Security.SignatureHintHex)
	}
}

func TestReceiptSampleFixtureHasStableResponseShape(t *testing.T) {
	var fixture struct {
		FixtureSchemaVersion uint8  `json:"fixture_schema_version"`
		Subject              string `json:"subject"`
		ContentType          string `json:"content_type"`
		BodyJSON             struct {
			SchemaVersion uint8 `json:"schema_version"`
			RequestID     UUID  `json:"request_id"`
			Truncated     bool  `json:"truncated"`
			Receipts      []struct {
				SchemaVersion        uint8                  `json:"schema_version"`
				ReceiptID            UUID                   `json:"receipt_id"`
				NodeID               UUID                   `json:"node_id"`
				FrameID              UUID                   `json:"frame_id"`
				Subject              string                 `json:"subject"`
				ContentType          string                 `json:"content_type"`
				BodyHash             string                 `json:"body_hash"`
				PolicyDecision       string                 `json:"policy_decision"`
				Outcome              string                 `json:"outcome"`
				StartedAtUnixMicros  uint64                 `json:"started_at_unix_micros"`
				FinishedAtUnixMicros uint64                 `json:"finished_at_unix_micros"`
				Metadata             map[string]interface{} `json:"metadata"`
				SignerPublicKey      string                 `json:"signer_public_key"`
				Signature            string                 `json:"signature"`
			} `json:"receipts"`
		} `json:"body_json"`
	}
	loadRootFixture(t, filepath.Join("protocol", "receipt-sample-v1.json"), &fixture)

	if fixture.FixtureSchemaVersion != 1 || fixture.BodyJSON.SchemaVersion != 1 {
		t.Fatalf("fixture schema mismatch: %+v", fixture)
	}
	if fixture.Subject != "zap.receipts.response" || fixture.ContentType != "application/zap-receipts+json" {
		t.Fatalf("receipt fixture route mismatch: %s %s", fixture.Subject, fixture.ContentType)
	}
	if fixture.BodyJSON.Truncated || len(fixture.BodyJSON.Receipts) != 1 {
		t.Fatalf("receipt collection mismatch: %+v", fixture.BodyJSON)
	}
	receipt := fixture.BodyJSON.Receipts[0]
	if receipt.SchemaVersion != 1 || receipt.Subject != RegistryIndexRequestSubject || receipt.ContentType != RegistryIndexContentType {
		t.Fatalf("receipt protocol fields mismatch: %+v", receipt)
	}
	if receipt.PolicyDecision != "allow" || receipt.Outcome != "accepted" {
		t.Fatalf("receipt decision mismatch: %+v", receipt)
	}
	if !ValidateArtifactHash(receipt.BodyHash) {
		t.Fatalf("invalid body hash %q", receipt.BodyHash)
	}
	if receipt.FinishedAtUnixMicros < receipt.StartedAtUnixMicros {
		t.Fatalf("receipt timestamps inverted: %+v", receipt)
	}
}

func TestPactFixturesReproduceHashAndVerify(t *testing.T) {
	var record struct {
		Subject     string  `json:"subject"`
		ContentType string  `json:"content_type"`
		BodyJSON    ZapPact `json:"body_json"`
	}
	var bundle struct {
		Subject     string        `json:"subject"`
		ContentType string        `json:"content_type"`
		BodyJSON    ZapPactBundle `json:"body_json"`
	}
	loadRootFixture(t, "pact-record-v1.json", &record)
	loadRootFixture(t, "pact-bundle-v1.json", &bundle)

	if record.Subject != PactRecordSubject || record.ContentType != PactContentType {
		t.Fatalf("PACT record fixture route mismatch: %s %s", record.Subject, record.ContentType)
	}
	hash, err := PactHash(record.BodyJSON)
	if err != nil {
		t.Fatal(err)
	}
	if hash != record.BodyJSON.Hash {
		t.Fatalf("hash = %s, want %s", hash, record.BodyJSON.Hash)
	}
	now := uint64(1893457000000000)
	ok, err := VerifyPact(record.BodyJSON, &now)
	if err != nil {
		t.Fatal(err)
	}
	if !ok {
		t.Fatal("expected PACT record to verify")
	}
	if bundle.Subject != PactBundleSubject || bundle.ContentType != PactContentType {
		t.Fatalf("PACT bundle fixture route mismatch: %s %s", bundle.Subject, bundle.ContentType)
	}
	ok, err = VerifyPactBundle(bundle.BodyJSON, &now)
	if err != nil {
		t.Fatal(err)
	}
	if !ok {
		t.Fatal("expected PACT bundle to verify")
	}
}

func TestSecurityProtocolFixturesCoverSignedPoaCapabilityAndDatagramShapes(t *testing.T) {
	var signed struct {
		Security struct {
			Signed      bool `json:"signed"`
			AuthTrailer struct {
				Algorithm string `json:"algorithm"`
			} `json:"auth_trailer"`
		} `json:"security"`
	}
	var poa struct {
		Security struct {
			Signed     bool `json:"signed"`
			POATrailer struct {
				Threshold uint16 `json:"threshold"`
			} `json:"poa_trailer"`
		} `json:"security"`
	}
	var capability struct {
		Subject     string `json:"subject"`
		ContentType string `json:"content_type"`
		BodyJSON    struct {
			Capabilities []string `json:"capabilities"`
		} `json:"body_json"`
	}
	var datagram struct {
		Cipher   string `json:"cipher"`
		NonceHex string `json:"nonce_hex"`
	}

	loadRootFixture(t, filepath.Join("protocol", "signed-control-frame-v1.json"), &signed)
	loadRootFixture(t, filepath.Join("protocol", "poa-control-frame-v1.json"), &poa)
	loadRootFixture(t, filepath.Join("protocol", "capability-response-v1.json"), &capability)
	loadRootFixture(t, filepath.Join("protocol", "encrypted-datagram-v1.json"), &datagram)

	if !signed.Security.Signed || signed.Security.AuthTrailer.Algorithm != "ed25519" {
		t.Fatalf("signed fixture mismatch: %+v", signed.Security)
	}
	if !poa.Security.Signed || poa.Security.POATrailer.Threshold != 1 {
		t.Fatalf("poa fixture mismatch: %+v", poa.Security)
	}
	if capability.Subject != "zap.capability.response" || capability.ContentType != "application/zap-capability+json" {
		t.Fatalf("capability fixture route mismatch: %+v", capability)
	}
	if !containsString(capability.BodyJSON.Capabilities, "driver.execute:echo") {
		t.Fatalf("capability fixture missing driver capability: %+v", capability.BodyJSON.Capabilities)
	}
	if datagram.Cipher != "ChaCha20-Poly1305" || len(datagram.NonceHex) != 24 {
		t.Fatalf("datagram fixture mismatch: %+v", datagram)
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

	var buf bytes.Buffer
	enc := json.NewEncoder(&buf)
	enc.SetEscapeHTML(false)
	err := enc.Encode(struct {
		Request RegistryInstallPlanRequest `json:"request"`
		Entry   RegistryInstallPlanEntry   `json:"entry"`
	}{Request: request, Entry: entry})
	if err != nil {
		t.Fatal(err)
	}
	raw := buf.String()
	if !strings.Contains(raw, `"abi_requirement":">=2,<4"`) {
		t.Fatalf("abi requirement missing from json: %s", raw)
	}
	if !strings.Contains(raw, `"migration_driver_action":"echo.migrate"`) {
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

func loadRootFixture(t *testing.T, name string, out any) {
	t.Helper()
	path := filepath.Join("..", "..", "fixtures", name)
	raw, err := os.ReadFile(path)
	if err != nil {
		t.Fatalf("read fixture %s: %v", name, err)
	}
	if err := json.Unmarshal(raw, out); err != nil {
		t.Fatalf("parse fixture %s: %v", name, err)
	}
}

func containsString(values []string, needle string) bool {
	for _, value := range values {
		if value == needle {
			return true
		}
	}
	return false
}
