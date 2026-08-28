# rivun Network

`rivun-net` provides the encrypted UDP transport plus the distributed-network
subsystems built on top of it: BFT consensus, epidemic gossip, an adaptive
mesh with failure detection, and restart-persistent anti-replay.

The transport is the only hard dependency of a basic rivun node. Consensus,
gossip, and mesh are optional subsystems that deployments enable when they
need swarm coordination, discovery, or failover.

## Encrypted UDP transport

```text
magic: "ZAPD"
version: u8 = 1
reserved: 3 bytes
source_node: 16 bytes
target_node: 16 bytes
nonce: 12 bytes
ciphertext: ChaCha20-Poly1305(frame bytes)
```

- The datagram header is AEAD associated data; reserved bytes must be zero.
- The nonce is a random 32-bit endpoint prefix plus a monotonic 64-bit counter,
  which prevents nonce reuse across process restarts even with a static
  transport key.
- Peer trust is configured statically; a Noise
  `NN_25519_ChaChaPoly_BLAKE2s` handshake (`NoiseHandshake`) can derive
  transport keys dynamically for future session bootstrap.

## Durable anti-replay

The in-memory `NonceReplayCache` (LRU, per peer) can be backed by a
`DurableNonceStore`: a write-ahead log (`ZAPNONC1` records, 36 bytes each) that
survives restarts. `compact` rewrites the log atomically. This closes the
restart replay window for deployments that rotate process state frequently.

```bash
cargo run -p rivun-cli -- run --config rivun.toml   # durable replay via config flag
```

## BFT consensus

`BftConsensusEngine` (`rivun-net::consensus`) is a two-phase BFT consensus engine
over the swarm protocol:

```text
proposal → pre-vote → polka (2/3) → pre-commit → commit certificate ("ZSC1")
```

- `ValidatorSet`: `2n/3 + 1` quorum, leader rotation by
  `(view + round) % n`.
- Proposals and votes are Ed25519-signed with BLAKE3 domain separation.
- Equivocation (a validator voting twice in the same round) is detected and
  produces an `EquivocationProof`; offenders are slashed from the set.
- Commit certificates use a compact binary format with a signer bitmask and
  bundled signatures; `verify_threshold_signatures` performs batched Ed25519
  verification.
- Epoch reconfiguration (`reconfigure_epoch`) supports validator set changes
  without stopping the engine.

Swarm intents move through states published by `rivun-agent::swarm`:
`Submitted → Proposed → Prevoting → Precommitting → Committed → Executing →
Finalized` (with `Rejected`/`TimedOut` terminal states). Commit certificates
reference `{epoch, view, round, block_height}`.

## Gossip

`SwarmGossipDispatcher` (`rivun-net::gossip`) is an epidemic broadcast layer:

- Signed envelopes (`ZGSP`) with BLAKE3 message ids and hop damping
  (default `MAX_HOPS = 16`);
- fan-out broadcast with LRU+TTL de-duplication;
- publish/subscribe topics over mpsc channels;
- PEX (peer exchange) by XOR-distance for peer discovery;
- anti-entropy sync with `StateDigest`/`MissingRange` so peers converge under
  message loss;
- `VectorClock` causality tracking (`increment`/`merge`/`compare`).

The legacy `GossipMesh` tracks peer health (`Alive`/`Suspect`/`Dead`), detects
partitions beyond 1/3 of the set, and can propose quorum-based actions.

Example benchmark with the CLI:

```bash
cargo run -p rivun-cli -- swarm bench --nodes 4 --rate 1000 --duration-secs 3
```

## Adaptive mesh

`SwarmMeshTopology` (`rivun-net::mesh`) supervises peer health and failover:

- `PhiAccrualDetector`: Gaussian phi-accrual failure detection (complementary
  error function with archived coefficients), thresholds 8/14;
- `HeartbeatScheduler`: exponential backoff (factor 1.5) with jitter;
- `ZapRelayEnvelope` (`ZRLY`): encrypted relay messages, max 2 hops, for
  partitioned subsets;
- `PartitionStatus`: `Normal` / `DegradedMinority` / `Isolated`;
- `select_relay_route` picks relay paths deterministically.

## Control subjects used by the network layer

| Subject | Content type | Purpose |
| --- | --- | --- |
| `rivun.discovery.announce` | `application/rivun-discovery+json` | Signed service/peer advertisement |
| `rivun.discovery.query` / `.response` | `application/rivun-discovery+json` | Signed discovery request/response |
| `rivun.swarm.intent.propose` / `.commit` | `application/rivun-swarm+json` | Swarm consensus proposals and commits |

See [Protocol](protocol.md) for the full control-subject catalogue.

## Testing

`crates/rivun-net/tests/`:

- `gossip_test.rs` — fan-out convergence, broadcast storms, TTL/hops, PEX,
  anti-entropy under loss, signature tampering;
- `durable_replay_stress.rs` — flood after crash/restart, clock jumps,
  partial writes, concurrency;
- `consensus_test.rs` — 4-phase commits, single-byzantine tolerance,
  equivocation proofs, leader rotation, bitmask batches, epoch transitions.
