# Swarm, Cluster Simulation, and Provenance

This document covers the high-level coordination surface of ZAP: multi-node
simulation, swarm benchmarking, chaos testing, and provenance verification.
The underlying mechanisms live in `zap-net` (consensus/gossip/mesh, see
[Network](network.md)) and `zap-agent` (swarm protocol and provenance chain).

## Cluster simulation

`zap cluster up` spawns an in-memory N-node cluster with mutual heartbeat mesh
and in-process derived keys — no daemon processes to manage:

```bash
# 3 nodes for 5 seconds
cargo run -p zap-cli -- cluster up --nodes 3 --duration-secs 5 --json

# Check simulated node status
cargo run -p zap-cli -- cluster status --nodes 3 --json
```

Options: `--nodes` (default 3), `--base-port` (default 9000),
`--duration-secs` (default 5). This is a fast way to validate topology,
heartbeat, and consensus bookkeeping before deploying real nodes.

## Swarm benchmarking

`zap swarm bench` runs a high-throughput P2P gossip consensus benchmark against
an in-memory swarm:

```bash
cargo run -p zap-cli -- swarm bench \
  --nodes 4 --rate 1000 --duration-secs 3 \
  --topic distributed_execution_lock --json
```

The benchmark measures end-to-end proposal/commit throughput for a swarm
topic. It exercises the same `BftConsensusEngine` and gossip dispatcher used
by real nodes, so results are representative of the protocol's cost model.

## Partition chaos test

`zap swarm partition-test` simulates Byzantine network partitions and
evaluates quorum safety:

```bash
cargo run -p zap-cli -- swarm partition-test \
  --nodes 5 --partition-fraction 0.4 --json
```

The test verifies that the 2/3 quorum math holds under partial partitions:
a minority subset cannot commit, while the majority can still reach consensus
(`Normal` vs `DegradedMinority`/`Isolated` partition statuses).

## Swarm protocol

`zap-agent::swarm` defines the intent lifecycle used by BFT consensus:

| State | Meaning |
| --- | --- |
| `Submitted` | Intent created by an agent |
| `Proposed` | Proposal broadcast to the swarm |
| `Prevoting` | Pre-votes being collected |
| `Precommitting` | Pre-commits being collected |
| `Committed` | A commit certificate (`{epoch, view, round, block_height}`) exists |
| `Executing` | The committed intent is being executed |
| `Finalized` | Execution complete and receipted |
| `Rejected` | Quorum rejected the intent |
| `TimedOut` | The round timed out without a certificate |

Restart-persistent anti-replay is layered at both the transport and daemon
levels: `zap-net::durable_replay` persists datagram nonces in a write-ahead log,
and `zap-node::durable_replay` (DurableReplayStore) persists frame fingerprints
across restarts.

## Provenance verification

`zap provenance verify` validates a cryptographic provenance chain digest
created by the gateway or node pipeline:

```bash
# Verify with the node key file
cargo run -p zap-cli -- provenance verify --chain chain.json --key .zap/node.key --json

# Or verify against a raw public key
cargo run -p zap-cli -- provenance verify \
  --chain chain.json --public-key <hex-or-base64-key> --json
```

The chain stages are `intent → negotiation → policy → driver → poa →
receipt`, root-signed by the node identity; any tampered link fails
verification (`MissingStep`, `StepVerificationFailed`).

Receipt journals extended with provenance digests can be verified in one pass:

```bash
cargo run -p zap-cli -- receipts verify --dir logs/receipts --provenance
```

See [Gateway](gateway.md) for the chain engine reference.

## Relationship to milestones

These commands and crates implement the M5/M6 features of the roadmap
(cluster simulator, swarm benchmarking, E2E integration): `PROJECT.md` maps
them to milestones; the `tests/e2e` suite (tiers 3–4) adds combined and
real-world scenarios on top.