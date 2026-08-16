package zap

import (
	"crypto/ed25519"
	"encoding/base64"
	"encoding/json"
	"errors"
	"fmt"
	"regexp"
	"sort"
	"strings"

	"lukechampine.com/blake3"
)

const (
	RegistryIndexSyncSchemaVersion    = 1
	RegistryBundleSchemaVersion       = 1
	RegistryInstallPlanSchemaVersion  = 1
	DriverABIVersion                  = 1
	DriverHashPrefix                  = "blake3:"
	ReceiptSchemaVersion              = 1
	ReceiptReplicationSchemaVersion   = 1
	ReceiptReplicationContentType     = "application/zap-receipts+json"
	ReceiptReplicationRequestSubject  = "zap.receipts.request"
	ReceiptReplicationResponseSubject = "zap.receipts.response"
	ReceiptSignatureDomain            = "ZAP-ACTION-RECEIPT-v1"
	AgentContentType                  = "application/zap-agent+json"
	AgentIntentSubject                = "zap.agent.intent"
	AgentStatusSubject                = "zap.agent.status"
	AgentResultSubject                = "zap.agent.result"
	PactSchemaVersion                 = 1
	PactContentType                   = "application/zap-pact+json"
	PactRecordSubject                 = "zap.pact.record"
	PactVerifySubject                 = "zap.pact.verify"
	PactRevokeSubject                 = "zap.pact.revoke"
	PactBundleSubject                 = "zap.pact.bundle"
	PactSignatureDomain               = "ZAP-PACT-v1"
)

var artifactHashPattern = regexp.MustCompile(`^blake3:[0-9a-f]{64}$`)

type DriverRegistryStatus string

type ZapPactStatus string

const (
	ZapPactStatusDraft   ZapPactStatus = "draft"
	ZapPactStatusActive  ZapPactStatus = "active"
	ZapPactStatusExpired ZapPactStatus = "expired"
	ZapPactStatusRevoked ZapPactStatus = "revoked"
	ZapPactStatusInvalid ZapPactStatus = "invalid"
)

type ZapPact struct {
	SchemaVersion   uint8         `json:"schema_version"`
	PactID          UUID          `json:"pact_id"`
	Actor           string        `json:"actor"`
	Target          string        `json:"target"`
	Intent          string        `json:"intent"`
	Object          any           `json:"object"`
	Terms           any           `json:"terms"`
	Consent         any           `json:"consent"`
	Proof           any           `json:"proof"`
	CreatedAtMicros uint64        `json:"created_at_micros"`
	ExpiresAtMicros *uint64       `json:"expires_at_micros,omitempty"`
	ActorPublicKey  string        `json:"actor_public_key,omitempty"`
	Hash            string        `json:"hash,omitempty"`
	Signature       string        `json:"signature,omitempty"`
	Status          ZapPactStatus `json:"status,omitempty"`
}

type ZapPactBundle struct {
	SchemaVersion uint8          `json:"schema_version"`
	Pact          ZapPact        `json:"pact"`
	Verifications []any          `json:"verifications,omitempty"`
	Revocations   []any          `json:"revocations,omitempty"`
	Metadata      map[string]any `json:"metadata,omitempty"`
}

type zapPactSigningPayload struct {
	PactID          UUID    `json:"pact_id"`
	Actor           string  `json:"actor"`
	Target          string  `json:"target"`
	Intent          string  `json:"intent"`
	Object          any     `json:"object"`
	Terms           any     `json:"terms"`
	Consent         any     `json:"consent"`
	Proof           any     `json:"proof"`
	CreatedAtMicros uint64  `json:"created_at_micros"`
	ExpiresAtMicros *uint64 `json:"expires_at_micros"`
}

const (
	DriverRegistryStatusActive     DriverRegistryStatus = "active"
	DriverRegistryStatusDeprecated DriverRegistryStatus = "deprecated"
	DriverRegistryStatusRevoked    DriverRegistryStatus = "revoked"
)

type DriverRegistryMigration struct {
	FromVersionRequirement   string `json:"from_version_requirement"`
	FromABIRequirement       string `json:"from_abi_requirement,omitempty"`
	RequiresOperatorApproval bool   `json:"requires_operator_approval,omitempty"`
	MigrationDriverAction    string `json:"migration_driver_action,omitempty"`
	MigrationDriverVersion   string `json:"migration_driver_version,omitempty"`
	Notes                    string `json:"notes,omitempty"`
}

type RegistryIndexRequest struct {
	SchemaVersion    uint8 `json:"schema_version"`
	RequireSignature bool  `json:"require_signature"`
}

type RegistryIndexResponse struct {
	SchemaVersion     uint8           `json:"schema_version"`
	NodeID            UUID            `json:"node_id"`
	Registry          *DriverRegistry `json:"registry,omitempty"`
	UnavailableReason string          `json:"unavailable_reason,omitempty"`
}

type RegistryBundleManifestRequest struct {
	SchemaVersion      uint8 `json:"schema_version"`
	RequirePublication bool  `json:"require_publication"`
	RequireDrivers     bool  `json:"require_drivers"`
}

type RegistryBundleManifestResponse struct {
	SchemaVersion     uint8                   `json:"schema_version"`
	NodeID            UUID                    `json:"node_id"`
	Manifest          *RegistryBundleManifest `json:"manifest,omitempty"`
	UnavailableReason string                  `json:"unavailable_reason,omitempty"`
}

type DriverRegistryEntry struct {
	Name             string                    `json:"name"`
	Version          string                    `json:"version"`
	Action           string                    `json:"action"`
	ABIVersion       uint16                    `json:"abi_version"`
	WASMHash         string                    `json:"wasm_hash"`
	ManifestPath     string                    `json:"manifest_path,omitempty"`
	AuthorNodeID     UUID                      `json:"author_node_id"`
	Status           DriverRegistryStatus      `json:"status,omitempty"`
	RevokedReason    string                    `json:"revoked_reason,omitempty"`
	DeprecatedReason string                    `json:"deprecated_reason,omitempty"`
	Migrations       []DriverRegistryMigration `json:"migrations,omitempty"`
}

type DriverRegistry struct {
	SchemaVersion     uint8                 `json:"schema_version"`
	GeneratedBy       string                `json:"generated_by,omitempty"`
	OperatorNodeID    *UUID                 `json:"operator_node_id,omitempty"`
	OperatorPublicKey string                `json:"operator_public_key,omitempty"`
	Signature         string                `json:"signature,omitempty"`
	Entries           []DriverRegistryEntry `json:"entries"`
}

type RegistryBundleManifest struct {
	SchemaVersion   uint8                 `json:"schema_version"`
	GeneratedBy     string                `json:"generated_by,omitempty"`
	RegistryPath    string                `json:"registry_path"`
	RegistryHash    string                `json:"registry_hash"`
	PublicationPath string                `json:"publication_path,omitempty"`
	PublicationHash string                `json:"publication_hash,omitempty"`
	Entries         []RegistryBundleEntry `json:"entries"`
}

type RegistryBundleEntry struct {
	Action       string               `json:"action"`
	Version      string               `json:"version"`
	Name         string               `json:"name"`
	ABIVersion   uint16               `json:"abi_version"`
	WASMHash     string               `json:"wasm_hash"`
	AuthorNodeID UUID                 `json:"author_node_id"`
	Status       DriverRegistryStatus `json:"status"`
	ManifestPath string               `json:"manifest_path,omitempty"`
	ManifestHash string               `json:"manifest_hash,omitempty"`
	DriverPath   string               `json:"driver_path,omitempty"`
	DriverHash   string               `json:"driver_hash,omitempty"`
}

type RegistryInstallPlanRequest struct {
	Action         string  `json:"action"`
	Requirement    string  `json:"requirement"`
	ABIVersion     *uint16 `json:"abi_version,omitempty"`
	ABIRequirement string  `json:"abi_requirement,omitempty"`
}

type RegistryInstallPlanEntry struct {
	Action                  string                    `json:"action"`
	Requirement             string                    `json:"requirement"`
	RequestedABIVersion     *uint16                   `json:"requested_abi_version,omitempty"`
	RequestedABIRequirement string                    `json:"requested_abi_requirement,omitempty"`
	SelectedVersion         string                    `json:"selected_version"`
	Name                    string                    `json:"name"`
	ABIVersion              uint16                    `json:"abi_version"`
	WASMHash                string                    `json:"wasm_hash"`
	ManifestPath            string                    `json:"manifest_path,omitempty"`
	AuthorNodeID            UUID                      `json:"author_node_id"`
	Migrations              []DriverRegistryMigration `json:"migrations,omitempty"`
}

type RegistryInstallPlan struct {
	SchemaVersion          uint8                      `json:"schema_version"`
	RegistryHash           string                     `json:"registry_hash"`
	RegistryEntries        int                        `json:"registry_entries"`
	RegistryOperatorNodeID *UUID                      `json:"registry_operator_node_id,omitempty"`
	PublicationHash        string                     `json:"publication_hash,omitempty"`
	RequestedAtMicros      uint64                     `json:"requested_at_micros"`
	Target                 string                     `json:"target,omitempty"`
	Labels                 []string                   `json:"labels"`
	Entries                []RegistryInstallPlanEntry `json:"entries"`
	PlannerNodeID          UUID                       `json:"planner_node_id"`
	PlannerPublicKey       string                     `json:"planner_public_key"`
	Signature              string                     `json:"signature"`
}

type ReceiptSample struct {
	SchemaVersion        uint8          `json:"schema_version"`
	ReceiptID            UUID           `json:"receipt_id"`
	NodeID               UUID           `json:"node_id"`
	FrameID              UUID           `json:"frame_id"`
	Subject              string         `json:"subject"`
	ContentType          string         `json:"content_type"`
	BodyHash             string         `json:"body_hash"`
	PolicyDecision       string         `json:"policy_decision"`
	Outcome              string         `json:"outcome"`
	StartedAtUnixMicros  uint64         `json:"started_at_unix_micros"`
	FinishedAtUnixMicros uint64         `json:"finished_at_unix_micros"`
	Metadata             map[string]any `json:"metadata,omitempty"`
	SignerPublicKey      string         `json:"signer_public_key"`
	Signature            string         `json:"signature"`
}

type ReceiptReplicationResponseBody struct {
	SchemaVersion uint8           `json:"schema_version"`
	RequestID     UUID            `json:"request_id"`
	Truncated     bool            `json:"truncated"`
	Receipts      []ReceiptSample `json:"receipts"`
}

type receiptSigningPayload struct {
	Receipt         any    `json:"receipt"`
	SignerNodeID    string `json:"signer_node_id"`
	SignerPublicKey string `json:"signer_public_key"`
}

type SignatureVerificationStatus struct {
	Supported bool   `json:"supported"`
	Reason    string `json:"reason"`
}

func ReceiptBodyHash(body []byte) (string, error) {
	return ArtifactHash(body)
}

func ReceiptSigningMessage(receipt any) ([]byte, error) {
	raw, err := json.Marshal(receipt)
	if err != nil {
		return nil, err
	}
	var data map[string]any
	if err := json.Unmarshal(raw, &data); err != nil {
		return nil, err
	}
	signerPublicKey, _ := data["signer_public_key"].(string)
	if signerPublicKey == "" {
		return nil, errors.New("receipt signer_public_key is required")
	}
	signerNodeID, _ := data["signer_node_id"].(string)
	if signerNodeID == "" {
		signerNodeID, _ = data["node_id"].(string)
	}
	if signerNodeID == "" {
		return nil, errors.New("receipt signer_node_id or node_id is required")
	}
	unsignedReceipt := map[string]any{}
	for k, v := range data {
		if k != "signature" && k != "signer_public_key" {
			unsignedReceipt[k] = v
		}
	}
	payload := receiptSigningPayload{
		Receipt:         unsignedReceipt,
		SignerNodeID:    signerNodeID,
		SignerPublicKey: signerPublicKey,
	}
	encoded, err := json.Marshal(payload)
	if err != nil {
		return nil, err
	}
	output := make([]byte, 0, len(ReceiptSignatureDomain)+len(encoded))
	output = append(output, []byte(ReceiptSignatureDomain)...)
	output = append(output, encoded...)
	return output, nil
}

func ValidateReceiptShape(receipt ReceiptSample) error {
	if receipt.SchemaVersion != ReceiptSchemaVersion {
		return fmt.Errorf("unsupported receipt schema version %d", receipt.SchemaVersion)
	}
	if receipt.ReceiptID == (UUID{}) || receipt.NodeID == (UUID{}) || receipt.FrameID == (UUID{}) {
		return errors.New("receipt ids must not be nil")
	}
	if !ValidateArtifactHash(receipt.BodyHash) {
		return fmt.Errorf("invalid receipt body hash %q", receipt.BodyHash)
	}
	if receipt.FinishedAtUnixMicros < receipt.StartedAtUnixMicros {
		return errors.New("receipt finished_at_unix_micros is before started_at_unix_micros")
	}
	return nil
}

func ValidateReceiptResponseShape(response ReceiptReplicationResponseBody) error {
	if response.SchemaVersion != ReceiptReplicationSchemaVersion {
		return fmt.Errorf("unsupported receipt replication schema version %d", response.SchemaVersion)
	}
	if response.RequestID == (UUID{}) {
		return errors.New("request_id must not be nil")
	}
	for _, receipt := range response.Receipts {
		if err := ValidateReceiptShape(receipt); err != nil {
			return err
		}
	}
	return nil
}

type ZapStoreClient struct{}

func (ZapStoreClient) RegistryIndexRequest(requireSignature bool) (ControlFrame, error) {
	return RegistryIndexRequestFrame(requireSignature)
}

func (ZapStoreClient) RegistryBundleManifestRequest(requirePublication bool, requireDrivers bool) (ControlFrame, error) {
	return RegistryBundleManifestRequestFrame(requirePublication, requireDrivers)
}

func RegistryIndexRequestFrame(requireSignature bool) (ControlFrame, error) {
	return NewJSONControlFrame(
		RegistryIndexRequestSubject,
		RegistryIndexContentType,
		RegistryIndexRequest{
			SchemaVersion:    RegistryIndexSyncSchemaVersion,
			RequireSignature: requireSignature,
		},
	)
}

func RegistryBundleManifestRequestFrame(requirePublication bool, requireDrivers bool) (ControlFrame, error) {
	return NewJSONControlFrame(
		BundleManifestRequestSubject,
		RegistryBundleContentType,
		RegistryBundleManifestRequest{
			SchemaVersion:      RegistryBundleSchemaVersion,
			RequirePublication: requirePublication,
			RequireDrivers:     requireDrivers,
		},
	)
}

func (response RegistryBundleManifestResponse) VerifyShape(request RegistryBundleManifestRequest) error {
	if response.SchemaVersion != RegistryBundleSchemaVersion {
		return fmt.Errorf("unsupported registry bundle schema version %d", response.SchemaVersion)
	}
	if request.SchemaVersion != RegistryBundleSchemaVersion {
		return fmt.Errorf("unsupported registry bundle request schema version %d", request.SchemaVersion)
	}
	if response.Manifest == nil {
		return nil
	}
	if err := response.Manifest.ValidateShape(); err != nil {
		return err
	}
	if request.RequirePublication && (response.Manifest.PublicationPath == "" || response.Manifest.PublicationHash == "") {
		return errors.New("registry bundle publication path/hash metadata is incomplete")
	}
	if request.RequireDrivers {
		for _, entry := range response.Manifest.Entries {
			if entry.DriverPath == "" || entry.DriverHash == "" {
				return fmt.Errorf("registry bundle entry %s@%s lacks driver metadata", entry.Action, entry.Version)
			}
		}
	}
	return nil
}

func (manifest RegistryBundleManifest) ValidateShape() error {
	if manifest.SchemaVersion != RegistryBundleSchemaVersion {
		return fmt.Errorf("unsupported registry bundle schema version %d", manifest.SchemaVersion)
	}
	if err := validateRelativePath(manifest.RegistryPath); err != nil {
		return err
	}
	if !ValidateArtifactHash(manifest.RegistryHash) {
		return fmt.Errorf("invalid registry hash %q", manifest.RegistryHash)
	}
	if (manifest.PublicationPath == "") != (manifest.PublicationHash == "") {
		return errors.New("registry bundle publication path/hash metadata is incomplete")
	}
	if manifest.PublicationPath != "" {
		if err := validateRelativePath(manifest.PublicationPath); err != nil {
			return err
		}
	}
	if manifest.PublicationHash != "" && !ValidateArtifactHash(manifest.PublicationHash) {
		return fmt.Errorf("invalid publication hash %q", manifest.PublicationHash)
	}
	seen := map[string]struct{}{}
	for _, entry := range manifest.Entries {
		key := entry.Action + "@" + entry.Version
		if _, ok := seen[key]; ok {
			return fmt.Errorf("duplicate registry bundle entry %s", key)
		}
		seen[key] = struct{}{}
		if err := entry.ValidateShape(); err != nil {
			return err
		}
	}
	return nil
}

func (entry RegistryBundleEntry) ValidateShape() error {
	if strings.TrimSpace(entry.Action) == "" {
		return errors.New("driver action must not be empty")
	}
	if !ValidateArtifactHash(entry.WASMHash) {
		return fmt.Errorf("invalid wasm hash %q", entry.WASMHash)
	}
	if (entry.ManifestPath == "") != (entry.ManifestHash == "") {
		return fmt.Errorf("registry bundle entry %s@%s has incomplete manifest metadata", entry.Action, entry.Version)
	}
	if entry.ManifestPath != "" {
		if err := validateRelativePath(entry.ManifestPath); err != nil {
			return err
		}
	}
	if entry.ManifestHash != "" && !ValidateArtifactHash(entry.ManifestHash) {
		return fmt.Errorf("invalid manifest hash %q", entry.ManifestHash)
	}
	if (entry.DriverPath == "") != (entry.DriverHash == "") {
		return fmt.Errorf("registry bundle entry %s@%s has incomplete driver metadata", entry.Action, entry.Version)
	}
	if entry.DriverPath != "" {
		if err := validateRelativePath(entry.DriverPath); err != nil {
			return err
		}
	}
	if entry.DriverHash != "" {
		if !ValidateArtifactHash(entry.DriverHash) {
			return fmt.Errorf("invalid driver hash %q", entry.DriverHash)
		}
		if entry.DriverHash != entry.WASMHash {
			return fmt.Errorf("driver hash does not match wasm hash for %s@%s", entry.Action, entry.Version)
		}
	}
	return nil
}

func ValidateArtifactHash(value string) bool {
	return artifactHashPattern.MatchString(value)
}

func ArtifactHash(input []byte) (string, error) {
	sum := blake3.Sum256(input)
	return DriverHashPrefix + fmt.Sprintf("%x", sum[:]), nil
}

func RegistryHash(registry DriverRegistry) (string, error) {
	encoded, err := json.Marshal(registry)
	if err != nil {
		return "", err
	}
	return ArtifactHash(encoded)
}

func PactCanonicalSigningBytes(pact ZapPact) ([]byte, error) {
	if err := ValidatePactShape(pact); err != nil {
		return nil, err
	}
	payload := zapPactSigningPayload{
		PactID:          pact.PactID,
		Actor:           pact.Actor,
		Target:          pact.Target,
		Intent:          pact.Intent,
		Object:          normalizeJSONValue(pact.Object),
		Terms:           normalizeJSONValue(pact.Terms),
		Consent:         normalizeJSONValue(pact.Consent),
		Proof:           normalizeJSONValue(pact.Proof),
		CreatedAtMicros: pact.CreatedAtMicros,
		ExpiresAtMicros: pact.ExpiresAtMicros,
	}
	return json.Marshal(payload)
}

func PactHash(pact ZapPact) (string, error) {
	encoded, err := PactCanonicalSigningBytes(pact)
	if err != nil {
		return "", err
	}
	return ArtifactHash(encoded)
}

func ValidatePactShape(pact ZapPact) error {
	if pact.SchemaVersion != PactSchemaVersion {
		return fmt.Errorf("unsupported PACT schema version %d", pact.SchemaVersion)
	}
	if pact.PactID == (UUID{}) {
		return errors.New("PACT pact_id must not be nil")
	}
	if strings.TrimSpace(pact.Actor) == "" {
		return errors.New("PACT actor must not be empty")
	}
	if strings.TrimSpace(pact.Target) == "" {
		return errors.New("PACT target must not be empty")
	}
	if strings.TrimSpace(pact.Intent) == "" {
		return errors.New("PACT intent must not be empty")
	}
	if pact.ExpiresAtMicros != nil && *pact.ExpiresAtMicros <= pact.CreatedAtMicros {
		return errors.New("PACT expires_at_micros must be greater than created_at_micros")
	}
	if pact.Hash != "" && !ValidateArtifactHash(pact.Hash) {
		return fmt.Errorf("invalid PACT hash %q", pact.Hash)
	}
	return nil
}

func VerifyPact(pact ZapPact, nowMicros *uint64) (bool, error) {
	if err := ValidatePactShape(pact); err != nil {
		return false, err
	}
	if pact.Status == ZapPactStatusRevoked {
		return false, nil
	}
	if nowMicros != nil && pact.ExpiresAtMicros != nil && *nowMicros > *pact.ExpiresAtMicros {
		return false, nil
	}
	hash, err := PactHash(pact)
	if err != nil {
		return false, err
	}
	if pact.Hash == "" || pact.Hash != hash || pact.Signature == "" || pact.ActorPublicKey == "" {
		return false, nil
	}
	message, err := PactCanonicalSigningBytes(pact)
	if err != nil {
		return false, err
	}
	return VerifyEd25519Signature(ZapDomainMessage([]byte(PactSignatureDomain), message), pact.Signature, pact.ActorPublicKey)
}

func VerifyPactBundle(bundle ZapPactBundle, nowMicros *uint64) (bool, error) {
	if bundle.SchemaVersion != PactSchemaVersion {
		return false, fmt.Errorf("unsupported PACT bundle schema version %d", bundle.SchemaVersion)
	}
	if len(bundle.Revocations) > 0 {
		return false, nil
	}
	return VerifyPact(bundle.Pact, nowMicros)
}

func ZapDomainMessage(domain []byte, message []byte) []byte {
	output := make([]byte, 0, len(domain)+1+len(message))
	output = append(output, domain...)
	output = append(output, 0)
	output = append(output, message...)
	return output
}

func SignatureVerificationPlaceholder(kind string) SignatureVerificationStatus {
	return SignatureVerificationStatus{
		Supported: false,
		Reason:    kind + " signatures are Ed25519 signatures over ZAP domain-separated payloads. Build the exact canonical message and call VerifyEd25519Signature, or use zap-cli/Rust for canonical registry verification.",
	}
}

func VerifyEd25519Signature(message []byte, signatureBase64 string, publicKeyBase64 string) (bool, error) {
	signature, err := decodeBase64NoPad(signatureBase64)
	if err != nil {
		return false, err
	}
	publicKey, err := decodeBase64NoPad(publicKeyBase64)
	if err != nil {
		return false, err
	}
	if len(publicKey) != ed25519.PublicKeySize {
		return false, fmt.Errorf("invalid public key length: got %d", len(publicKey))
	}
	if len(signature) != ed25519.SignatureSize {
		return false, fmt.Errorf("invalid signature length: got %d", len(signature))
	}
	return ed25519.Verify(ed25519.PublicKey(publicKey), message, signature), nil
}

func decodeBase64NoPad(value string) ([]byte, error) {
	if decoded, err := base64.StdEncoding.WithPadding(base64.NoPadding).DecodeString(value); err == nil {
		return decoded, nil
	}
	return base64.StdEncoding.DecodeString(value)
}

func validateRelativePath(path string) error {
	if path == "" || strings.HasPrefix(path, "/") || strings.HasPrefix(path, "\\") {
		return fmt.Errorf("bundle path %q is not a safe relative path", path)
	}
	parts := strings.Split(strings.ReplaceAll(path, "\\", "/"), "/")
	for _, part := range parts {
		if part == "" || part == "." || part == ".." {
			return fmt.Errorf("bundle path %q is not a safe relative path", path)
		}
	}
	return nil
}

func normalizeJSONValue(value any) any {
	switch typed := value.(type) {
	case map[string]any:
		keys := make([]string, 0, len(typed))
		for key := range typed {
			keys = append(keys, key)
		}
		sort.Strings(keys)
		normalized := make(map[string]any, len(typed))
		for _, key := range keys {
			normalized[key] = normalizeJSONValue(typed[key])
		}
		return normalized
	case []any:
		normalized := make([]any, len(typed))
		for index, item := range typed {
			normalized[index] = normalizeJSONValue(item)
		}
		return normalized
	default:
		return value
	}
}
