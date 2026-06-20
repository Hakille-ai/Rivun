# ZAP Protocol Fixtures

This directory contains small, stable JSON fixtures for protocol and SDK
interoperability work. They are intentionally human-readable descriptions of
ZENV envelopes, frame security trailers, datagram shapes, and JSON bodies
rather than opaque binary blobs.

Use these fixtures when adding SDK tests, docs examples, or conformance checks
that should agree on subjects, content types, schema versions, UUID handling,
and body field names.

## Files

- `zenv-control-registry-bundle-manifest-request.json` describes a v1 `ZENV`
  control envelope for `zap.registry.bundle.manifest.request`.
- `agent-intent-message-v1.json` contains a v1 `application/zap-agent+json`
  intent payload for the `zap.agent.intent` subject.
- `agent-session-message-v1.json` contains a v1 agent session payload for
  `zap.agent.session`.
- `agent-delegation-request-message-v1.json` and
  `agent-delegation-response-message-v1.json` contain v1 delegation contracts.
- `agent-capability-negotiation-request-message-v1.json` and
  `agent-capability-negotiation-response-message-v1.json` contain v1 capability
  negotiation contracts.
- `pact-record-v1.json` contains a signed v1 `application/zap-pact+json`
  action record for the `zap.pact.record` subject.
- `pact-bundle-v1.json` contains a portable v1 PACT bundle with the same signed
  record for offline verification tests.
- `control-subjects-v1.json` lists the current v1 control subjects and media
  types documented in `docs/protocol.md`.
- `protocol/zenv-unsigned-control-frame-v1.json` describes a deterministic
  unsigned v1 control envelope with no auth or PoA trailer.
- `protocol/signed-control-frame-v1.json` describes a signed control frame with
  an Ed25519 auth trailer.
- `protocol/poa-control-frame-v1.json` describes a signed control frame with a
  Proof-of-Action trailer summary.
- `protocol/capability-response-v1.json` contains a deterministic capability
  response body for SDK and control-message tests.
- `protocol/encrypted-datagram-v1.json` documents the v1 encrypted UDP datagram
  header, nonce, AAD, and ciphertext shape.
- `protocol/receipt-sample-v1.json` contains a deterministic receipts response
  body that SDKs can load without requiring live signing keys.
- `protocol/signed-pact-record-frame-v1.json` describes a signed `ZENV` action
  frame carrying `application/zap-pact+json`.

## Conventions

- `kind_value` follows the v1 envelope kind table where `8` is `control`.
- Nil correlation and causation IDs are represented as the nil UUID string.
- `body_json` is the decoded JSON body that should be encoded as UTF-8 bytes
  when constructing a ZENV envelope.
- Fixtures prefer deterministic UUIDs and compact bodies so every SDK can load
  them without external services or signing keys.
- Nested `protocol/` fixtures are golden interop samples. They are intentionally
  shaped for SDK and crate tests, while the top-level fixtures remain the stable
  set consumed by the current CLI fixture verifier.
- `zap fixtures verify --fixtures fixtures` validates both top-level fixtures
  and nested protocol fixtures. Add `--sdk <path>` to check that a local SDK has
  fixture conformance coverage for its language-specific test layout.
