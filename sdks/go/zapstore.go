package zap

import (
	"encoding/json"
	"errors"
	"fmt"
	"regexp"
	"strings"
)

const (
	RegistryIndexSyncSchemaVersion = 1
	RegistryBundleSchemaVersion    = 1
	RegistryInstallPlanSchemaVersion = 1
	DriverABIVersion               = 1
	DriverHashPrefix               = "blake3:"
)

var artifactHashPattern = regexp.MustCompile(`^blake3:[0-9a-f]{64}$`)

type DriverRegistryStatus string

const (
	DriverRegistryStatusActive     DriverRegistryStatus = "active"
	DriverRegistryStatusDeprecated DriverRegistryStatus = "deprecated"
	DriverRegistryStatusRevoked    DriverRegistryStatus = "revoked"
)

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
	Name             string               `json:"name"`
	Version          string               `json:"version"`
	Action           string               `json:"action"`
	ABIVersion       uint16               `json:"abi_version"`
	WASMHash         string               `json:"wasm_hash"`
	ManifestPath     string               `json:"manifest_path,omitempty"`
	AuthorNodeID     UUID                 `json:"author_node_id"`
	Status           DriverRegistryStatus `json:"status,omitempty"`
	RevokedReason    string               `json:"revoked_reason,omitempty"`
	DeprecatedReason string               `json:"deprecated_reason,omitempty"`
}

type DriverRegistry struct {
	SchemaVersion    uint8                 `json:"schema_version"`
	GeneratedBy      string                `json:"generated_by,omitempty"`
	OperatorNodeID   *UUID                 `json:"operator_node_id,omitempty"`
	OperatorPublicKey string               `json:"operator_public_key,omitempty"`
	Signature        string                `json:"signature,omitempty"`
	Entries          []DriverRegistryEntry `json:"entries"`
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
	Action      string  `json:"action"`
	Requirement string  `json:"requirement"`
	ABIVersion  *uint16 `json:"abi_version,omitempty"`
}

type RegistryInstallPlanEntry struct {
	Action              string  `json:"action"`
	Requirement         string  `json:"requirement"`
	RequestedABIVersion *uint16 `json:"requested_abi_version,omitempty"`
	SelectedVersion     string  `json:"selected_version"`
	Name                string  `json:"name"`
	ABIVersion          uint16  `json:"abi_version"`
	WASMHash            string  `json:"wasm_hash"`
	ManifestPath        string  `json:"manifest_path,omitempty"`
	AuthorNodeID        UUID    `json:"author_node_id"`
}

type RegistryInstallPlan struct {
	SchemaVersion         uint8                      `json:"schema_version"`
	RegistryHash          string                     `json:"registry_hash"`
	RegistryEntries       int                        `json:"registry_entries"`
	RegistryOperatorNodeID *UUID                     `json:"registry_operator_node_id,omitempty"`
	PublicationHash       string                     `json:"publication_hash,omitempty"`
	RequestedAtMicros     uint64                     `json:"requested_at_micros"`
	Target                string                     `json:"target,omitempty"`
	Labels                []string                   `json:"labels"`
	Entries               []RegistryInstallPlanEntry `json:"entries"`
	PlannerNodeID         UUID                       `json:"planner_node_id"`
	PlannerPublicKey      string                     `json:"planner_public_key"`
	Signature             string                     `json:"signature"`
}

type SignatureVerificationStatus struct {
	Supported bool   `json:"supported"`
	Reason    string `json:"reason"`
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

func ArtifactHash(_ []byte) (string, error) {
	return "", errors.New("canonical ZAP artifact hashes use BLAKE3; Go standard library does not expose BLAKE3. Use zap-cli, the Rust SDK, or a vetted BLAKE3 backend")
}

func RegistryHash(registry DriverRegistry) (string, error) {
	encoded, err := json.Marshal(registry)
	if err != nil {
		return "", err
	}
	return ArtifactHash(encoded)
}

func SignatureVerificationPlaceholder(kind string) SignatureVerificationStatus {
	return SignatureVerificationStatus{
		Supported: false,
		Reason:    kind + " signatures are Ed25519 signatures over ZAP domain-separated payloads. This dependency-free Go SDK does not verify them yet; use zap-cli or the Rust SDK.",
	}
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
