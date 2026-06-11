# Security Model

ZAP v1 treats identity, transport confidentiality, and execution isolation as separate layers.

## Identity

Each node has an Ed25519 keypair. The node UUID is deterministically derived from the public key with a domain-separated BLAKE3 hash and UUID version 8 bits.

`ZAP_SIGN` is only an 8-byte hint. It is useful for fast filtering, but the verifier must validate the full 64-byte Ed25519 signature in the auth trailer.

## Transport

`zap-net` encrypts UDP datagrams with ChaCha20-Poly1305. The clear envelope contains routing metadata and nonce; the entire envelope is authenticated as AEAD associated data. Static peer discovery is used in v1 because it is auditable and deterministic.

Transport nonces are 96 bits: a random 32-bit prefix generated when an endpoint binds, followed by a monotonic 64-bit counter. This prevents nonce reuse across normal process restarts even when a static transport key is reused. If the counter is ever exhausted, sending fails and the transport key must be rotated.

Receivers keep a bounded in-memory nonce cache per peer and reject repeated datagram nonces before returning inbound frames to higher layers. `zap-node` sizes this cache with `security.replay_cache_capacity`.

The crate also includes a Noise `NN_25519_ChaChaPoly_BLAKE2s` helper for future dynamic session bootstrap.

## Anti-Replay

`zap-node` validates timestamp freshness before dispatching an action. By default, frames outside a five-minute clock-skew window are rejected. The node also keeps an in-memory BLAKE3 fingerprint cache of recently accepted frames per process and rejects exact frame replays.

The policy is configured with:

```toml
[security]
max_clock_skew_micros = 300000000
replay_cache_capacity = 4096
```

Set `replay_cache_capacity = 0` only for specialized tests where frame replay and transport nonce replay detection must be disabled.

For production checks, run:

```bash
zap check-config --strict --config zap.toml
```

Strict mode exits non-zero when validation emits safety warnings, such as
unsigned frame acceptance, disabled replay detection, missing signed driver
manifests, missing PoA validators, reused peer transport keys, or overly broad
Unix key-file permissions.

## Proof-of-Action

Frames marked `REQUIRES_CONSENSUS` must carry a PoA certificate before `zap-node` dispatches them. The certificate contains a digest of the signed frame, a threshold, and Ed25519 attestations from configured validators.

Node config controls the verifier set:

```toml
[poa]
required_threshold = 1

[[poa.validators]]
node_id = "..."
public_key = "..."
```

`zap send --intent` refuses to emit critical intent steps unless a local
`--poa-validator-key` is supplied or `--poa-network` can collect a configured
validator quorum.

`zap send --poa-network` can request attestations from configured validator
peers. Responses are accepted only when the validator public key matches config,
the response signature verifies, and the response digest equals the requested
signed-frame digest. The sender waits up to `--poa-timeout-ms` for the required
threshold; the default is 2000 ms.

## Signed Receipts

When `[receipts].path` is configured, `zap-node` appends one Ed25519-signed JSONL receipt after each processed action. Receipts contain hashes, action metadata, and optional PoA summaries. They are audit records only, not financial records.

## Runtime Isolation

WASM drivers receive no host imports by default. Network, filesystem, clock, and environment capabilities are denied unless explicitly granted by future host APIs.

Signed ZapStore manifests bind a driver action to a BLAKE3 artifact hash, ABI version, declared permissions, and author Ed25519 identity. `zap-node` verifies the manifest signature and hash before compiling the driver. Local registry indexes can also carry an operator signature; set `[registry] require_signature = true` to reject unsigned or tampered indexes during config validation and daemon startup. ABI v1 still rejects every requested host permission because the host capability APIs do not exist yet.

Current enforced limits:

- max linear memory;
- one instance, one memory, one table;
- fuel budget;
- output byte limit;
- wall-clock epoch interruption.
