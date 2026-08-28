# rivun SDKs

The first external SDK distribution lives under `sdks/` and focuses on
protocol-compatible helpers with lightweight local transports:

- `sdks/python`: Python dataclasses for `ZENV` control envelopes, RivunStore
  request/response payloads, PACT helpers, and a stdlib UDP client.
- `sdks/typescript`: TypeScript helpers that run in Node, including UDP,
  BLAKE3, Ed25519 verification, typecheck, and declaration build scripts.
- `sdks/go`: Go package for control envelope bytes, UDP transport, canonical
  BLAKE3 hashes, Ed25519 verification, and RivunStore JSON types.
- `sdks/rust`: Rust SDK crate that wraps the canonical local rivun crates through
  path dependencies.

## Common Surface

Each SDK can build and parse rivun control envelopes for current RivunStore control
subjects:

- `rivun.registry.index.request`
- `rivun.registry.index.response`
- `rivun.registry.bundle.manifest.request`
- `rivun.registry.bundle.manifest.response`

Each SDK also includes base RivunStore types for registry index entries, bundle
manifests, bundle entries, install plan requests, install plans, install plan
entries, and PACT record/bundle conformance helpers.

Shared protocol subjects are catalogued in `fixtures/control-subjects-v1.json`
and now cover agent protocol messages (`rivun.agent.*`), receipt replication
(`rivun.receipts.*`), PACT (`rivun.pact.*`), registry (`rivun.registry.*`),
discovery (`rivun.discovery.*`), and PoA validator sets
(`rivun.poa.validator_set.*`). SDK helpers build and parse envelopes for the
subjects the SDK's test surface supports; see the per-SDK test files for the
exact coverage.

Shared protocol fixtures live in `fixtures/`. They provide readable JSON
examples for ZENV control envelopes, agent protocol payloads, and the current
control subject catalogue so SDK tests can converge on the same field names,
media types, receipt shapes, and signing-message helpers.
PACT fixtures are shared across Rust, TypeScript, Python, and Go to prove that
all SDKs reproduce the same canonical BLAKE3 hash and offline verification
result.

## Shared Fixtures

The fixture directory is the SDK conformance source of truth for stable protocol
examples:

- `ZENV-control-registry-bundle-manifest-request.json`: a v1 `ZENV` control
  envelope carrying `rivun.registry.bundle.manifest.request`.
- `control-subjects-v1.json`: the current v1 control subject catalogue and
  media types.
- `agent-intent-message-v1.json`: a v1 `application/rivun-agent+json` intent
  payload that can be carried by SDK control envelope helpers.
- `pact-record-v1.json`: a signed v1 `application/rivun-pact+json` action record
  used for canonical hash and signature verification.
- `pact-bundle-v1.json`: a portable PACT bundle containing the signed record.
- `protocol/ZENV-unsigned-control-frame-v1.json`: a deterministic unsigned
  registry index request frame for header and body round-trip checks.
- `protocol/signed-control-frame-v1.json`: a deterministic signed control frame
  shape with an Ed25519 auth trailer.
- `protocol/poa-control-frame-v1.json`: a deterministic signed control frame
  shape with a Proof-of-Action trailer summary.
- `protocol/capability-response-v1.json`: a capability response body for
  control-message and discovery helper tests.
- `protocol/encrypted-datagram-v1.json`: a v1 encrypted UDP datagram shape with
  nonce, AAD, and ciphertext fields.
- `protocol/receipt-sample-v1.json`: a deterministic receipt replication
  response used by SDK receipt shape and signing-message helper tests.

When adding a fixture, keep it small, readable, and deterministic. Include a
`fixture_schema_version`, a short `description`, the protocol subject and media
type, and a JSON body that can be asserted by at least one SDK test. Prefer
adding or updating tests in every SDK that can parse the fixture without pulling
in extra runtime services.

## Conformance Matrix

The CLI can also check fixture coverage for each SDK layout:

```bash
cargo run --locked -p rivun-cli -- fixtures verify --fixtures fixtures --sdk sdks/typescript --json
cargo run --locked -p rivun-cli -- fixtures verify --fixtures fixtures --sdk sdks/python --json
cargo run --locked -p rivun-cli -- fixtures verify --fixtures fixtures --sdk sdks/go --json
cargo run --locked -p rivun-cli -- fixtures verify --fixtures fixtures --sdk sdks/rust --json
```

| SDK | ZENV encode/decode | RivunStore payloads | Shared fixture tests | Integrity helpers | Local test command |
| --- | --- | --- | --- | --- | --- |
| Python | Yes | Yes | `ZENV`, `agent-intent`, unsigned frame, receipt, PACT | Shape validation always; receipt and PACT signing-message helpers; BLAKE3 and Ed25519 with `crypto` extra | `PYTHONPATH=sdks/python/src python -m unittest discover -s sdks/python/tests` |
| TypeScript | Yes | Yes | `ZENV`, `control-subjects`, `agent-intent`, unsigned frame, receipt, PACT | BLAKE3, receipt/PACT signing-message helpers, and Ed25519 through Noble packages | `npm --prefix sdks/typescript test` |
| Go | Yes | Yes | `ZENV`, `control-subjects`, unsigned frame, receipt, PACT | BLAKE3 and standard Ed25519 | `go test ./sdks/go/...` |
| Rust | Yes, via canonical crates | Yes, via canonical crates | Canonical crate tests plus PACT fixture verification | Canonical rivun crate helpers | `cargo test --manifest-path sdks/rust/Cargo.toml` |

Known limitations:

- Go checks require a local Go toolchain. CI installs Go before running the SDK
  workflow, but a workstation without `go` cannot run `go test ./sdks/go/...`.
- Python cryptographic hashing and signature verification require
  `python -m pip install -e "sdks/python[crypto]"`.
- Rust intentionally depends on local path crates, so it is the reference SDK
  for canonical behavior rather than a dependency-free client package.

## Integrity Helpers

RivunStore artifact hashes are canonical `blake3:<64 hex chars>` values.

Python and TypeScript expose constants for the receipt replication subjects
(`rivun.receipts.request`, `rivun.receipts.response`), the receipt media type
(`application/rivun-receipts+json`), and the agent message media type/subjects.
They also expose receipt response shape validation helpers and
`receipt_signing_message` / `receiptSigningMessage`, which builds the exact
domain-prefixed message bytes that Ed25519 verification expects for current
receipt payloads. The helper does not invent missing signature material; callers
must still provide the canonical receipt JSON, signer public key, and signature.

The Rust SDK reuses `rivun-store` and can compute canonical BLAKE3 hashes and run
existing signature verification methods. Python can compute/verify when its
`crypto` extra is installed. TypeScript uses `@noble/hashes` and
`@noble/ed25519`. Go uses `lukechampine.com/blake3` and the standard Ed25519
package.

PACT helpers use the same cross-SDK canonicalization rule: top-level signing
fields are emitted in protocol order, while nested JSON object keys are sorted
before BLAKE3 hashing and Ed25519 verification.

## Local Tests

```bash
python -m unittest discover -s sdks/python/tests
npm --prefix sdks/typescript run typecheck
npm --prefix sdks/typescript run build:types
npm --prefix sdks/typescript test
cargo test --manifest-path sdks/rust/Cargo.toml
```

Go tests are included and can be run with:

```bash
go test ./sdks/go/...
```

The SDK workflow installs Python, Node, Go, and Rust toolchains and runs these
checks in CI.

