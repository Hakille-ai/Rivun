# Codebase Survey Handoff Report: R1 & R2

## 1. Observation

### Workspace Structure & Crate Layout
The workspace root (`Cargo.toml`) defines 20 workspace member crates under `crates/`, plus `examples` and `tools/xtask`:
- `crates/rivun-net`: Encrypted UDP transport, static peer management, Noise handshake.
- `crates/rivun-node`: Daemon, event loop, frame dispatch, policy evaluation, runtime execution, security configs.
- `crates/rivun-journal`: Low-level binary journal engine (`.zjseg`, `.zjidx`, `.zjmanifest.json`).
- `crates/rivun-ledger`: Signed action receipts (`SignedActionReceipt`), receipt journal store (`ReceiptJournalStore`), segment manifests (`SignedReceiptSegmentManifest`), and segment indexes (`ReceiptSegmentIndex`).
- `crates/rivun-cli`: CLI binary (`rivun`), subcommands for node administration, keygen, trust, pact, policy, pack, registry, receipts, memory, etc.
- `crates/rivun-store`: Registry index engine (`DriverRegistry`, `DomainPackRegistry`), driver manifests, publication, bundle manifests, install plans.
- Other member crates: `rivun-agent`, `rivun-capability`, `rivun-core`, `rivun-crypto`, `rivun-driver-sdk`, `rivun-envelope`, `rivun-machine`, `rivun-memory`, `rivun-ops`, `rivun-pact`, `rivun-policy`, `rivun-router`, `rivun-runtime`, `rivun-schema`.

### Requirements R1 Observations (Durable Core, Replay Protection, Receipt Journal Segment Rotation)
1. **Replay Protection in `rivun-net`**:
   - File: `crates/rivun-net/src/lib.rs` (lines 491–524)
   - `NonceReplayCache` uses an in-memory `HashSet<[u8; 12]>` and `VecDeque<[u8; 12]>`:
     ```rust
     struct NonceReplayCache {
         capacity: usize,
         seen: HashSet<[u8; NONCE_LEN]>,
         order: VecDeque<[u8; NONCE_LEN]>,
     }
     ```
   - Instantiated per peer in `PeerTables.inbound_nonces`.
2. **Replay Protection in `rivun-node`**:
   - File: `crates/rivun-node/src/lib.rs` (lines 4430–4469)
   - `ReplayGuard` tracks 16-byte frame fingerprints (`frame_fingerprint`) using in-memory `HashSet<[u8; 16]>` and `VecDeque<[u8; 16]>`:
     ```rust
     struct ReplayGuard {
         capacity: usize,
         seen: HashSet<[u8; 16]>,
         order: VecDeque<[u8; 16]>,
     }
     ```
   - Wrapped in `Mutex<ReplayGuard>` inside `ZapNode` (line 1198).
   - Frame timestamp skew validation (lines 3100–3118) checks if timestamp is within `max_clock_skew_micros`.
3. **Journal Framing & Segment Manifests in `rivun-journal`**:
   - File: `crates/rivun-journal/src/lib.rs`
   - `JournalStore` writes binary `.zjseg` segments (header magic `ZJSEG001`), `.zjidx` entry indexes, and `.zjmanifest.json` segment manifests.
   - `JournalSegmentManifest` struct (lines 208–221) contains unsigned segment metadata (`segment_id`, `segment_sequence`, `entries`, `segment_bytes`, `segment_hash`, `first_entry_hash`, `last_entry_hash`, `first_timestamp_micros`, `last_timestamp_micros`, `compression`).
   - Segment rotation occurs when segment size exceeds `max_segment_bytes` (lines 463–483).
4. **Receipt Ledger & Manifest Signing in `rivun-ledger`**:
   - File: `crates/rivun-ledger/src/lib.rs`
   - `ReceiptJournalStore` (lines 435–584) wraps `JournalStore` for `JournalProfile::Receipts`.
   - `ReceiptSegmentManifest` (lines 587–677) and `SignedReceiptSegmentManifest` (lines 679–760) define segment manifest creation and Ed25519 signing over domain `rivun-RECEIPT-SEGMENT-MANIFEST-v1`.
   - `ReceiptSegmentIndex` and `ReceiptSegmentIndexEntry` (lines 762–931) model segment index validation and chain verification (`previous_segment_hash`).

### Requirements R2 Observations (Signed Domain Pack Lifecycle & Marketplace)
1. **Domain Pack CLI in `rivun-cli`**:
   - File: `crates/rivun-cli/src/main.rs` (lines 1066–1088, 7341–7588)
   - `PackCommand` enum defines only 3 subcommands:
     ```rust
     enum PackCommand {
         Validate { pack: PathBuf, json: bool },
         Inspect { pack: PathBuf, json: bool },
         List { root: PathBuf, json: bool },
     }
     ```
   - `pack_validate`, `pack_inspect`, `pack_list` inspect `pack.toml`, count capabilities/policies/schemas, and run static validation.
2. **Domain Pack Registry in `rivun-store`**:
   - File: `crates/rivun-store/src/lib.rs` (lines 485–568, 2066–2255)
   - Data structures: `DomainPackRegistry`, `DomainPackRegistryEntry`, `DomainPackArtifact`, `DomainPackCompatibility`, `DomainPackStatus`, `DomainPackRisk`.
   - `DomainPackRegistry::sign` and `verify_signature` implement operator signing/verification of the domain pack registry index over domain `rivun-DOMAIN-PACK-REGISTRY-v1`.

---

## 2. Logic Chain

1. **Replay Protection Durability Gap**:
   - *Observation*: Both `NonceReplayCache` (`rivun-net`) and `ReplayGuard` (`rivun-node`) store seen datagram nonces and frame fingerprints in standard memory datastructures (`HashSet`, `VecDeque`).
   - *Reasoning*: When a node process stops, crashes, or restarts, these in-memory data structures are destroyed. Upon restart, a newly initialized node has empty replay caches.
   - *Deduction*: Any validly signed frame or encrypted datagram whose timestamp falls within the allowed clock skew window (`now - max_clock_skew_micros` to `now + max_clock_skew_micros`) that was processed shortly before node restart can be retransmitted by an attacker after restart and will be accepted.
   - *Conclusion*: Zero replay vulnerability across node restarts requires a persistent durable replay window (e.g. disk-backed bloom filter, persistent WAL log, or disk-backed timestamp-based replay window state).

2. **Receipt Journal Rotation & Sealing Gap**:
   - *Observation*: `JournalStore.append()` in `rivun-journal` automatically rotates to a new `.zjseg` file when `max_segment_bytes` is reached, writing an unsigned JSON `.zjmanifest.json` file. `rivun-ledger` contains `SignedReceiptSegmentManifest::sign` and `ReceiptSegmentIndex`, but `ReceiptJournalStore` never calls `SignedReceiptSegmentManifest::sign` or seals closed segments.
   - *Reasoning*: Segment rotation currently operates purely at the file-system level without a cryptographic boundary transition. The node's Ed25519 identity key is not used during segment rotation to seal the completed segment or sign its `SegmentManifest`.
   - *Deduction*: Rotated segment manifests remain unsigned, leaving completed receipt segments vulnerable to undetected file system modification or deletion of trailing records without proof of node signature over the segment state. Furthermore, indexed queries rely on line-based `.zjidx` files rather than fast signed segment manifest index lookup tables (`ReceiptSegmentIndex`).
   - *Conclusion*: Automatic cryptographic segment sealing, manifest signing (`SignedReceiptSegmentManifest`), and fast index querying must be integrated into the `ReceiptJournalStore` rotation pipeline.

3. **Domain Pack Tooling & Lifecycle Gap**:
   - *Observation*: `rivun pack` in `rivun-cli` currently supports `validate`, `inspect`, and `list`.
   - *Reasoning*: Requirement R2 requires the complete CLI lifecycle: `init`, `build`, `sign`, `verify`, `install`, and `audit`.
   - *Deduction*: Developers and operators cannot initialize new pack templates, build single-file signed pack archives/bundles, sign packs with Ed25519 node/author keys, verify pack bundles offline, install packs into node stores, or perform security audits on domain pack capability grants and risk levels.
   - *Conclusion*: 6 CLI subcommands (`rivun pack init`, `build`, `sign`, `verify`, `install`, `audit`) are missing and must be implemented.

4. **RivunStore Registry & Bundle Resolution Gap**:
   - *Observation*: `rivun-store` defines `DomainPackRegistry` structs and operator signing, while `DriverRegistry` handles WASM driver manifest publishing, install plans (`RegistryInstallPlan`), and driver bundle manifests (`RegistryBundleManifest`).
   - *Reasoning*: `DomainPackRegistry` has structure definitions, but lacks offline bundle verification for full domain pack archives (which include policies, schemas, capabilities, and drivers) and lacks dependency resolution.
   - *Deduction*: There is no mechanism to verify offline domain pack bundles against registry entries or resolve multi-pack capability/policy dependencies during installation or offline deployment.
   - *Conclusion*: RivunStore integration for domain packs requires offline bundle verification (`DomainPackBundle`) and a dependency resolution engine.

---

## 3. Caveats

- **Unexplored Areas**: Requirements R3 (Fleet Topology, Health & Incident Telemetry), R4 (AI Agent Gateway & MCP), and R5 (Multi-Language SDK Conformance) were outside the scope of this survey.
- **Assumptions**: The presence of `SignedReceiptSegmentManifest` in `rivun-ledger` and `DomainPackRegistry` in `rivun-store` indicates these types were designed as foundation structures for R1 and R2 requirements.
- **Alternative Interpretations**: Durable replay protection could be implemented either via a lightweight WAL/disk journal of active nonces or a disk-persisted sliding window state file.

---

## 4. Conclusion

### Summary Assessment
The codebase has clean, modular foundational structures for cryptographic operations (`rivun-crypto`), binary journal storage (`rivun-journal`), action receipt signing (`rivun-ledger`), and driver registry management (`rivun-store`). However, critical functional gaps exist for Requirements R1 and R2:

### Detailed Feature Gaps

#### Requirement R1 Gaps:
1. **Durable Replay Protection**:
   - Persistent disk backing for `NonceReplayCache` (`rivun-net`) and `ReplayGuard` (`rivun-node`).
   - Crash recovery & restart re-hydration of active replay protection windows.
2. **Cryptographic Segment Sealing & Signed Manifests**:
   - Automatic invocation of `SignedReceiptSegmentManifest::sign()` upon segment rotation in `ReceiptJournalStore`.
   - Creation and cryptographic verification of sealed segment manifests (`.zjmanifest.json` signed with node Ed25519 key).
   - Fast indexed queries utilizing `ReceiptSegmentIndex` across rotated sealed segments.

#### Requirement R2 Gaps:
1. **Domain Pack CLI Tooling (`rivun pack`)**:
   - `rivun pack init`: Initialize domain pack directory structure with template `pack.toml`, policy, schema, and driver files.
   - `rivun pack build`: Package domain pack directory into an archive bundle (`.zpk` / `.rivun-pack.tar.gz`).
   - `rivun pack sign`: Sign domain pack manifest and bundle with Ed25519 key.
   - `rivun pack verify`: Verify domain pack signature, artifact hashes, and offline bundle integrity.
   - `rivun pack install`: Install domain pack into rivun node configuration/store with policy and route validation.
   - `rivun pack audit`: Perform security audit of capability requirements, risk levels, and policy permissions.
2. **RivunStore Domain Pack Registry & Offline Verification**:
   - Domain pack registry publication and registry bundle verification (`DomainPackBundle`).
   - Offline bundle verification for domain packs containing policies, schemas, and drivers.
   - Dependency resolution engine for domain packs (capability requirements, version constraint matching).

---

## 5. Verification Method

To verify current state and baseline test suite:
1. Run full workspace tests:
   ```powershell
   cargo test --workspace --all-targets
   ```
2. Run workspace clippy lints:
   ```powershell
   cargo clippy --workspace --all-targets -- -D warnings
   ```
3. Test existing CLI domain pack commands:
   ```powershell
   cargo run --bin rivun -- pack --help
   ```
   (Confirms currently only `validate`, `inspect`, `list` are present).

### Invalidation Conditions
- Findings regarding `ReplayGuard` in-memory state are invalidated if a disk persistence mechanism for nonces/fingerprints is discovered.
- Findings regarding segment sealing are invalidated if `JournalStore` auto-signing hooks exist elsewhere in `rivun-ops` or `rivun-node`.

