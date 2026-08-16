# Milestone 5 & FINAL Investigation and Blueprint Report

## 1. Observation

### 1.1 Multi-Language SDK Conformance (`sdks/rust`, `sdks/typescript`, `sdks/python`, `sdks/go`)
- **Go SDK (`sdks/go/zapstore.go`)**:
  - `ReceiptReplicationResponseBody` and `ReceiptSample` types are **missing entirely** from `sdks/go/zapstore.go`.
  - In `sdks/go/protocol_test.go:222-248`, `TestReceiptSampleFixtureHasStableResponseShape` defines an ad-hoc inline anonymous struct instead of using SDK types.
  - Missing receipt validation and signing helper functions in `sdks/go/zapstore.go`:
    - `ValidateReceiptShape(receipt ReceiptSample) error`
    - `ValidateReceiptResponseShape(response ReceiptReplicationResponseBody) error`
    - `ReceiptSigningMessage(receipt ReceiptSample) ([]byte, error)`
    - `ReceiptBodyHash(input []byte) (string, error)`
  - Missing protocol constants in `sdks/go/zapstore.go`:
    - `ReceiptSchemaVersion = 1`
    - `ReceiptReplicationSchemaVersion = 1`
    - `ReceiptReplicationContentType = "application/zap-receipts+json"`
    - `ReceiptReplicationRequestSubject = "zap.receipts.request"`
    - `ReceiptReplicationResponseSubject = "zap.receipts.response"`
    - `ReceiptSignatureDomain = "ZAP-ACTION-RECEIPT-v1"`
    - `AgentContentType = "application/zap-agent+json"`
    - `AgentIntentSubject = "zap.agent.intent"`
    - `AgentStatusSubject = "zap.agent.status"`
    - `AgentResultSubject = "zap.agent.result"`
  - Ed25519 signature verification in `sdks/go/zapstore.go:461-477` is implemented via `VerifyEd25519Signature(message []byte, signatureBase64 string, publicKeyBase64 string) (bool, error)`.
- **Rust SDK (`sdks/rust/src/lib.rs`)**:
  - `sdks/rust/src/lib.rs` currently does **not** provide `ZapUdpClient`.
  - Comment in `sdks/rust/src/lib.rs:3-6` states: *"This crate keeps a small, application-friendly surface around the canonical ZAP crates. It is intentionally network-free: callers can build and parse ZENV control payloads, then hand the bytes to their chosen transport."*
  - Re-exports in `sdks/rust/src/lib.rs` include `zap_core`, `zap_envelope`, `zap_pact`, and `zap_store`. Re-exports of `zap_ledger` (`SignedActionReceipt`, `ReceiptJournalStore`) and a high-level `ZapUdpClient` helper (wrapping `std::net::UdpSocket` / `tokio::net::UdpSocket`) will achieve full parity with TS, Python, and Go SDKs.
- **TypeScript SDK (`sdks/typescript/src/zapstore.ts`)**:
  - Contains full `ZapUdpClient` (`sdks/typescript/src/protocol.ts:217-284`), `ReceiptSample`, `ReceiptReplicationResponseBody`, `receiptSigningMessage`, `verifyPact`, `verifyPactBundle`, `verifyEd25519Signature` (`@noble/ed25519`).
- **Python SDK (`sdks/python/src/zap_sdk/zapstore.py`)**:
  - Contains full `ZapUdpClient` (`sdks/python/src/zap_sdk/protocol.py:168-208`), `ReceiptSample`, `ReceiptReplicationResponseBody`, `receipt_signing_message`, `verify_pact`, `verify_pact_bundle`, `verify_ed25519_signature` (`PyNaCl`).

### 1.2 Protocol Golden Fixtures
- The repository contains 17 JSON fixtures in `fixtures/` and `fixtures/protocol/`:
  - `agent-capability-negotiation-request-message-v1.json`
  - `agent-capability-negotiation-response-message-v1.json`
  - `agent-delegation-request-message-v1.json`
  - `agent-delegation-response-message-v1.json`
  - `agent-intent-message-v1.json`
  - `agent-session-message-v1.json`
  - `control-subjects-v1.json`
  - `pact-bundle-v1.json`
  - `pact-record-v1.json`
  - `zenv-control-registry-bundle-manifest-request.json`
  - `protocol/capability-response-v1.json`
  - `protocol/encrypted-datagram-v1.json`
  - `protocol/poa-control-frame-v1.json`
  - `protocol/receipt-sample-v1.json`
  - `protocol/signed-control-frame-v1.json`
  - `protocol/signed-pact-record-frame-v1.json`
  - `protocol/zenv-unsigned-control-frame-v1.json`
- `zap fixtures verify --fixtures fixtures` is implemented in `crates/zap-cli/src/main.rs:8788-9477` and enforces fixture contracts and SDK coverage:
  - TypeScript: verifies `test/fixtures.test.ts` and `package.json`
  - Python: verifies `tests/test_protocol.py` and `pyproject.toml`
  - Go: verifies `protocol_test.go` and `go.mod`
  - Rust: verifies `src/lib.rs` and `Cargo.toml`

### 1.3 CLI Test Suite Diagnostics (`crates/zap-cli`)
- `crates/zap-cli/tests/cli.rs`: All 76 tests execute and pass in ~5.50s (`test result: ok. 76 passed; 0 failed`).
- `crates/zap-cli/tests/gateway_cli_tests.rs`:
  - 4 of 5 tests pass (`test_cli_provenance_verify_tampered_fails`, `test_cli_provenance_verify_with_keyfile`, `test_cli_provenance_verify_with_public_key_hex`, `test_cli_receipts_verify_with_provenance_flag`).
  - 1 test failure: `test_cli_gateway_status_query` at `crates/zap-cli/tests/gateway_cli_tests.rs:264:5`.
  - Cause: In `test_cli_gateway_status_query`, the test starts `server.run_on_listener(listener)` in `tokio::spawn` and immediately spawns the `zap gateway status` child process without yielding to allow the server listener to begin accepting connections. Adding a small startup delay (`tokio::time::sleep(Duration::from_millis(50)).await;`) resolves this connection race.

### 1.4 E2E Test Suite Compilation Errors (`tests/e2e/tests/e2e_suite.rs`)
Compiling `zap-e2e` produced 61 errors caused by API signature mismatches from M1-M4 evolutions:
1. `tests/e2e/tests/e2e_suite.rs:28`: `use zap_journal::ReceiptJournalStore;` -> Unresolved import (should be `use zap_ledger::ReceiptJournalStore;`).
2. `tests/e2e/tests/e2e_suite.rs:1820`: `sha2::Sha256::digest(wasm_code);` -> `sha2` crate not in `tests/e2e/Cargo.toml` dependencies (use `zap_store::driver_hash` or add `sha2` to `Cargo.toml`).
3. `tests/e2e/tests/e2e_suite.rs:79, 94, 99, 205, 217, 247`: `MemoryJournalStore::open(...)` returns `MemoryJournalStore`, not `Result<MemoryJournalStore, _>`. The `?` operator fails.
4. `tests/e2e/tests/e2e_suite.rs:194, 235, 1694`: `ReceiptReplicationRequest` does not have `from_sequence` field. The correct fields are `schema_version`, `after_processed_at_micros`, `until_processed_at_micros`, `kind`, `subject`, `limit`.
5. `tests/e2e/tests/e2e_suite.rs:259, 274, 290, 306, 320, 350, 361, 1705`: `DriverManifest::new` expects 7 arguments (`name, version, action, wasm: &[u8], permissions: DriverPermissions, description: Option<String>, author: &Keypair`) and returns `Result<DriverManifest, ZapStoreError>`. Manifest signing occurs inside `DriverManifest::new`.
6. `tests/e2e/tests/e2e_suite.rs:349, 360, 380`: `DriverRegistry::open(...)` does not exist. The constructors are `DriverRegistry::empty(Option<String>)` or `DriverRegistry::from_toml_str(&str)`.
7. `tests/e2e/tests/e2e_suite.rs:426, 576, 594, 612, 631, 1888`: `ZapNodeConfig` does not implement `Default` and `ZapNode::new(config)` does not exist; node initialization uses `ZapNode::from_config(config).await?`.
8. `tests/e2e/tests/e2e_suite.rs:1127, 1139, 1849, 1850`: `Keypair` does not have `.sign()` directly on struct; use `keypair.sign_domain_message(domain, data)` or `ed25519_dalek::SigningKey::from_bytes(&key.secret_bytes()).sign(data)`.
9. `tests/e2e/tests/e2e_suite.rs:1192, 2105`: `ZapPact` has `signature: Option<String>` (not `signatures: vec![]`).
10. `tests/e2e/tests/e2e_suite.rs:1201`: `schema.contains("AgentMessage")` -> `schema` is `serde_json::Value`, use `schema.to_string().contains("AgentMessage")`.
11. `tests/e2e/tests/e2e_suite.rs:1234`: `ZapFlags::NONE` -> `ZapFlags::empty()`.
12. `tests/e2e/tests/e2e_suite.rs:2005-2007`: `registry.add_manifest(&manifest, None)?` and finding entries via `registry.entries.iter().find(...)`.
13. `tests/e2e/tests/e2e_suite.rs:2040-2054`: `DelegationRequest` struct fields: `delegation_id`, `session_id`, `parent_intent_id`, `from_agent`, `to_agent`, `objective`, `required_capabilities: BTreeSet<CapabilityId>`, `constraints: Vec<IntentConstraint>`, `context: Vec<ContextReference>`, `deadline_unix_micros: Option<u64>`, `metadata: BTreeMap<String, Value>`. Must also import trait `use zap_agent::Validate;`.
14. `tests/e2e/tests/e2e_suite.rs:2109-2115`: `ZapPactRevocation` has `revoked_by: String` (not `revoker`) and `signature: Option<String>`.

---

## 2. Logic Chain

1. **SDK Conformance Parity**:
   - TypeScript and Python SDKs already expose receipt replication response bodies, verification routines, and `ZapUdpClient`.
   - Go SDK lacks `ReceiptReplicationResponseBody`, `ReceiptSample`, receipt validation, and signature verification routines. Adding these to `sdks/go/zapstore.go` brings Go SDK into 100% schema conformance.
   - Rust SDK (`sdks/sdk`) can expose an ergonomic `ZapUdpClient` and re-export `ReceiptJournalStore` / `SignedActionReceipt` to complete parity across all 4 SDKs.

2. **Protocol Golden Fixtures Parity**:
   - The fixture set in `fixtures/` and `fixtures/protocol/` is complete and covers all protocol shapes (signed envelopes, PoA attestation, capabilities, ChaCha20-Poly1305 encrypted datagrams, PACT records and bundles, receipt logs).
   - `zap fixtures verify --fixtures fixtures --sdk <path>` verifies that each SDK test file imports and asserts the expected fixture set. Updating Go test to use the new `ReceiptReplicationResponseBody` and `ReceiptSample` types ensures full conformance.

3. **CLI Test Suite Parity**:
   - 76 integration tests in `crates/zap-cli/tests/cli.rs` pass cleanly.
   - The single failure in `gateway_cli_tests.rs::test_cli_gateway_status_query` is a trivial listener startup race in the test harness. Adding a 50ms startup delay prior to executing `Command::new(env!("CARGO_BIN_EXE_zap"))` fixes it.

4. **E2E Suite Conformance**:
   - The 61 compilation errors in `tests/e2e/tests/e2e_suite.rs` stem exclusively from early draft mock signatures that diverged as M1-M4 implemented production versions of `DriverManifest`, `ZapNode`, `ReceiptJournalStore`, `DelegationRequest`, and `ZapPact`.
   - Aligning all 61 call sites with the canonical crate APIs will enable `cargo test --workspace --all-targets` and `cargo clippy --workspace --all-targets -- -D warnings` to pass with 0 errors and 0 warnings.

---

## 3. Caveats

- **Network Mode**: Running background subprocesses in some environments requires bounding CLI test execution timeouts to avoid hanging CI runners.
- **WASM Drivers**: Domain pack tests use standard WAT or WASM bytecode fixtures; runtime driver execution requires valid module export signatures.

---

## 4. Conclusion & Actionable Blueprint

### Action 1: Update Go SDK (`sdks/go/zapstore.go`)
Add the following types, constants, and helper functions to `sdks/go/zapstore.go`:
```go
const (
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
)

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
```

Update `sdks/go/protocol_test.go` to use `ReceiptReplicationResponseBody` and `ReceiptSample` directly in `TestReceiptSampleFixtureHasStableResponseShape`.

### Action 2: Update Rust SDK (`sdks/rust/src/lib.rs`)
Add `ZapUdpClient` to `sdks/rust/src/lib.rs`:
```rust
pub struct ZapUdpClient {
    socket: std::net::UdpSocket,
}

impl ZapUdpClient {
    pub fn bind(addr: impl std::net::ToSocketAddrs) -> Result<Self> {
        let socket = std::net::UdpSocket::bind(addr).map_err(|e| {
            SdkError::Envelope(zap_envelope::ZapEnvelopeError::InvalidHeader(e.to_string()))
        })?;
        Ok(Self { socket })
    }

    pub fn send_envelope(
        &self,
        envelope: &ZapEnvelope,
        target: impl std::net::ToSocketAddrs,
    ) -> Result<usize> {
        let bytes = envelope.encode();
        self.socket.send_to(&bytes, target).map_err(|e| {
            SdkError::Envelope(zap_envelope::ZapEnvelopeError::InvalidHeader(e.to_string()))
        })
    }

    pub fn send_control(
        &self,
        frame: &ControlFrame,
        target: impl std::net::ToSocketAddrs,
    ) -> Result<usize> {
        let bytes = frame.encode();
        self.socket.send_to(&bytes, target).map_err(|e| {
            SdkError::Envelope(zap_envelope::ZapEnvelopeError::InvalidHeader(e.to_string()))
        })
    }

    pub fn recv_envelope(
        &self,
        timeout: Option<std::time::Duration>,
    ) -> Result<(ZapEnvelope, std::net::SocketAddr)> {
        self.socket.set_read_timeout(timeout).ok();
        let mut buf = [0u8; 65535];
        let (n, addr) = self.socket.recv_from(&mut buf).map_err(|e| {
            SdkError::Envelope(zap_envelope::ZapEnvelopeError::InvalidHeader(e.to_string()))
        })?;
        let env_ref = ZapEnvelopeRef::parse(&buf[..n])?;
        let owned = ZapEnvelope::new(
            env_ref.kind(),
            env_ref.subject(),
            env_ref.content_type(),
            Bytes::copy_from_slice(env_ref.body()),
        )?
        .with_id(env_ref.id())
        .with_metadata(Bytes::copy_from_slice(env_ref.metadata()))?;
        Ok((owned, addr))
    }

    pub fn request_control(
        &self,
        frame: &ControlFrame,
        target: impl std::net::ToSocketAddrs,
        timeout: std::time::Duration,
    ) -> Result<ControlFrame> {
        self.send_control(frame, target)?;
        let (env, _) = self.recv_envelope(Some(timeout))?;
        if env.kind() != ZapMessageKind::Control {
            return Err(SdkError::ExpectedControl { actual: env.kind() });
        }
        let encoded = env.encode();
        ControlFrame::decode(&encoded)
    }
}
```

### Action 3: Fix CLI Gateway Status Test Race (`crates/zap-cli/tests/gateway_cli_tests.rs`)
In `test_cli_gateway_status_query` (`crates/zap-cli/tests/gateway_cli_tests.rs:251`):
Add `tokio::time::sleep(std::time::Duration::from_millis(50)).await;` before `Command::new(env!("CARGO_BIN_EXE_zap"))`.

### Action 4: Update `tests/e2e/Cargo.toml` and `tests/e2e/tests/e2e_suite.rs`
1. In `tests/e2e/Cargo.toml`, add `sha2 = "0.10"` to `[dependencies]`.
2. In `tests/e2e/tests/e2e_suite.rs`:
   - Replace `use zap_journal::ReceiptJournalStore;` with `use zap_ledger::ReceiptJournalStore;`.
   - Remove `?` from all `MemoryJournalStore::open(...)` calls.
   - Update `ReceiptReplicationRequest` initializations to use proper fields (`schema_version: 1, after_processed_at_micros: 0, until_processed_at_micros: u64::MAX, kind: None, subject: None, limit: Some(10)`).
   - Update `DriverManifest::new` calls to pass all 7 arguments and unwrap `Result`:
     `DriverManifest::new(name, version, action, wasm_bytes, zap_capability::DriverPermissions::default(), Some(description.to_string()), &author)?`.
   - Update `DriverRegistry` usage: `DriverRegistry::empty(None)` and `registry.add_manifest(&manifest, None)?`.
   - Update `ZapNodeConfig` and `ZapNode` instantiation: construct config explicitly without `..Default::default()` and use `ZapNode::from_config(config).await?`.
   - Update `Keypair` signing calls: `key.signing_key().sign(data)` or `key.sign_domain_message(domain, data)`.
   - Update `ZapPact` struct initialization: `signature: None` (or `Some(...)`), not `signatures`.
   - Update `schema.contains("AgentMessage")` to `schema.to_string().contains("AgentMessage")`.
   - Update `ZapFlags::NONE` to `ZapFlags::empty()`.
   - Update `DelegationRequest` struct fields and bring `use zap_agent::Validate;` into scope.
   - Update `ZapPactRevocation` struct fields: `revoked_by: "agent_a".into(), signature: None`.

---

## 5. Verification Method

Once changes are applied by the implementer:
1. **Workspace Test Suite**:
   ```powershell
   cargo test --workspace --all-targets
   ```
   *Expected*: 0 failures across all 19 workspace crates, SDKs, and E2E suites.
2. **Clippy Static Analysis**:
   ```powershell
   cargo clippy --workspace --all-targets -- -D warnings
   ```
   *Expected*: Clean execution with 0 warnings.
3. **Fixture Verification & SDK Conformance**:
   ```powershell
   cargo run -p zap-cli -- fixtures verify --fixtures fixtures --sdk sdks/typescript
   cargo run -p zap-cli -- fixtures verify --fixtures fixtures --sdk sdks/python
   cargo run -p zap-cli -- fixtures verify --fixtures fixtures --sdk sdks/go
   cargo run -p zap-cli -- fixtures verify --fixtures fixtures --sdk sdks/rust
   ```
   *Expected*: `valid: true` and 100% check pass for all 4 SDKs.
