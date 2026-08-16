# Handoff Report: Milestone 1 Explorer 1 (`zap-net` Blueprint)

## 1. Observation
- **Existing `zap-net` Codebase (`crates/zap-net/src/lib.rs:1-1140`)**:
  - `ZapEndpoint` provides authenticated encrypted UDP transport using ChaCha20-Poly1305 with a 52-byte header (`ZAPD`), 12-byte nonce, and sliding window replay protection.
  - Broadcast is currently implemented as sequential unicast loop across `peers.by_id.keys()` (`lib.rs:382-396`), requiring upgrade to epidemic gossip fanout.
- **Existing Replay Storage (`crates/zap-net/src/durable_replay.rs:1-215` & `tests/durable_replay_stress.rs:1-264`)**:
  - `DurableNonceStore` writes 36-byte binary records to a WAL journal (`ZAPNONC1`).
  - Executed `cargo test -p zap-net`: all 22 unit tests and 5 stress tests passed (`test result: ok. 22 passed; 0 failed`, stress tests passed in 37.30s).
- **Existing Prototype in `src/gossip.rs:1-397`**:
  - Contains basic `VectorClock` (lines 35-101), `PeerHealth` (lines 104-109), `SwarmPeer` (lines 112-122), and `GossipMesh` (lines 137-307) with timeout-based partition checks.
- **Downstream Usages**:
  - `zap-cli`, `zap-node`, `zap-agent`, and `tests/e2e` import `Peer`, `TransportKey`, `ZapEndpoint`, `ZapEndpointConfig`, `InboundZap`, `MAX_DATAGRAM_SIZE`.

## 2. Logic Chain
1. **Preserving Backward Compatibility**: Because `ZapEndpoint` is imported across `zap-cli`, `zap-node`, and `tests/e2e`, all root public exports in `crates/zap-net/src/lib.rs` must maintain identical signatures and behavior while delegating or exposing new capabilities.
2. **Modular Architecture**: To achieve clean separation of concerns as mandated by Milestone 1 scope:
   - `src/gossip/` handles epidemic dissemination (`GossipEnvelope`, $k$-fanout, LRU deduplication cache, PEX peer exchange, and anti-entropy synchronization).
   - `src/consensus/` handles BFT 2-phase state machine replication (`SwarmProposal`, `SwarmVote`, `VoteKind::Prevote` / `VoteKind::Precommit`, `SwarmCommitCertificate`, bitmask packing, batch Ed25519 threshold verification, dynamic validator sets, and equivocation slashing).
   - `src/mesh/` handles adaptive quorum health tracking (`PhiAccrualDetector` with continuous erf calculation, randomized jitter heartbeats, `PartitionStatus` detection, and `ZapRelayEnvelope` 2-hop dynamic routing).
3. **High-Throughput Cryptographic Threshold Verification**: To satisfy the acceptance criterion of $< 1\text{ ms}$ threshold verification for up to $N=64$ nodes, `SwarmCommitCertificate` utilizes bitmask signer indexing ($\lceil N/8 \rceil$ bytes) and verifies all $T$ Ed25519 signatures concurrently via `ed25519_dalek::verify_batch`.

## 3. Caveats
- `zap-net/Cargo.toml` must add `ed25519-dalek = { workspace = true, features = ["batch", "rand_core"] }` and `zap-crypto = { path = "../zap-crypto" }` to enable batch signature verification.
- In leaderless or proposer-rotating BFT consensus, round timeouts must account for network jitter to prevent unnecessary view changes.
- 2-hop relay routing requires intermediate nodes to have forwarding trust enabled in `PeerTrustConfig`.

## 4. Conclusion
The comprehensive blueprint for `crates/zap-net` has been designed and documented in detail in `.agents/m1_explorer_1/analysis.md`. The design provides exact Rust data structures, traits, wire formats (`ZGSP`, `ZSC1`, `ZRLY`), error taxonomies, and a file-by-file implementation plan for Milestone 1 workers while guaranteeing 100% backward compatibility with all existing tests.

## 5. Verification Method
1. **Inspect Blueprint**: View `c:\Users\Stagiaire\Documents\Amadou PGC\Prs\ZAP\.agents\m1_explorer_1\analysis.md`.
2. **Execute Current Tests**:
   ```powershell
   cargo test -p zap-net
   ```
3. **Invalidation Conditions**:
   - Any signature changes to `ZapEndpoint::bind`, `send`, `send_frame`, `broadcast`, `recv`, or `add_peer`.
   - Failure of `cargo test -p zap-net` or `cargo clippy -p zap-net -- -D warnings`.
