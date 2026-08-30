# Rivun Protocol, Engine & Crate Specifications

**Document Version**: 1.0.0  
**Schema Versions**: `RivunHeader` v1, `ZENV` v1, `ZAPD` v1, `PACT` v1, `AgentProtocol` v1, `DeviceProfile` v1, `ZMMR` v1, `DomainPack` v1, `FleetDoctor` v1  
**Target Repository**: `Hakille-ai/ZAP` (Rivun)

---

## Executive Summary & Architecture Overview

Rivun is a zero-trust, high-throughput, low-latency protocol stack and runtime designed for autonomous AI agents, industrial edge systems, and cryptographically verified multi-party automation. The architecture is layered cleanly into:

1. **Wire & Cryptographic Fabric**: 64-byte big-endian framing (`RivunHeader`), Ed25519 authentication trailers (`AuthTrailer`), Proof-of-Action consensus trailers (`PoaTrailer`), and ChaCha20-Poly1305 authenticated datagrams (`ZAPD`).
2. **Universal Messaging Envelope**: 74-byte zero-copy `ZENV` envelope carrying typed messages across 8 discrete message kinds (`Data`, `Event`, `Command`, `Query`, `Response`, `StreamChunk`, `Action`, `Control`).
3. **Consensus & Network Mesh**: Proof-of-Action 2-Phase BFT consensus engine ($T \le N$, $T = \lfloor 2N/3 \rfloor + 1$) with equivocation detection and automatic validator slashing, Epidemic Gossip mesh with vector clocks and anti-entropy reconciliation, $\Phi$-Accrual failure detection, and 2-hop relay routing.
4. **Ledger, Storage & Merkle Mountain Ranges**: High-speed append-only segmented journals (`.zjseg`), carry-over subtree merging Merkle Mountain Ranges (`.zmmr`), single-leaf and multi-leaf batch inclusion proofs (`MmrBatchInclusionProof`), non-membership exclusion proofs (`MmrExclusionProof`), and blind commitments.
5. **WASM Runtime & Zero-Copy Streaming**: Sandboxed deterministic execution via Wasmtime with fuel metering, wall-clock epoch timeouts, lock-free single-producer single-consumer circular ring-buffers (`SpscRingBuffer`), multi-driver IPC pipelines, and Modbus industrial protocol bridges.
6. **Agent Protocol & Multi-Party PACT**: Multi-party conditional contracts, escrow deposits, dispute resolution with threshold arbitration, and complete causal provenance chains ($H_{\text{intent}} \to H_{\text{negotiation}} \to H_{\text{policy}} \to H_{\text{consensus}} \to H_{\text{driver}} \to H_{\text{poa}} \to H_{\text{receipt}} \to H_{\text{root}}$).
7. **Domain Packs, RivunStore & Fleet Doctor**: 7 domain-specific policy packs, offline signed driver registries, and 7-Point Fleet Doctor cluster diagnostics.

---

## 1. Complete Inventory of the 26 Workspace Crates

The Rivun workspace is composed of 25 core Rust crates in `crates/*` and 1 Tauri operator desktop workstation in `apps/rivun-control/src-tauri`.

| # | Crate Name | Path | Purpose & Responsibilities | Core Structs / Traits / Functions | Workspace Dependencies |
|---|---|---|---|---|---|
| 1 | `rivun-core` | `crates/rivun-core` | Root binary wire protocol, fixed 64-byte frame header, auth trailers, PoA trailers, and frame flags. | `RivunHeader`, `RivunFrame`, `AuthTrailer`, `PoaTrailer`, `PoaAttestation`, `RivunFlags`, `now_micros()` | None (Leaf) |
| 2 | `rivun-crypto` | `crates/rivun-crypto` | Ed25519 signing/verification, BLAKE3 domain separation, node ID derivation (UUID v8), blinded commitments, PoA validator sets. | `Keypair`, `PublicKey`, `node_id_from_public_key()`, `sign_frame()`, `verify_frame()`, `certify_frame()`, `PoaValidatorSet`, `SignedPoaValidatorSet`, `BlindedCommitment`, `BlindedReceiptCommitment`, `verify_batch_signatures()` | `rivun-core` |
| 3 | `rivun-envelope` | `crates/rivun-envelope` | Universal 74-byte zero-copy `ZENV` messaging envelope parser, builder, and validator. | `RivunEnvelope`, `RivunEnvelopeRef`, `RivunMessageKind`, `ZenvHeader` | `rivun-core` |
| 4 | `rivun-net` | `crates/rivun-net` | Encrypted UDP transport (`ZAPD`), ChaCha20-Poly1305 AEAD, BFT consensus engine, epidemic gossip mesh, $\Phi$-accrual failure detector, durable replay store. | `RivunEndpoint`, `DatagramEnvelope`, `NonceReplayCache`, `DurableNonceStore`, `BftConsensusEngine`, `SwarmProposal`, `SwarmVote`, `SwarmCommitCertificate`, `SwarmGossipEngine`, `VectorClock`, `PhiAccrualDetector`, `RivunRelayEnvelope` | `rivun-core`, `rivun-crypto`, `rivun-envelope` |
| 5 | `rivun-journal` | `crates/rivun-journal` | Segmented append-only disk storage, write-ahead logs (WAL), binary index files, segment rotation, and crash recovery. | `JournalStore`, `JournalSegment`, `JournalIndex`, `JournalProfile`, `JournalRecord`, `JournalRecordInput`, `SignedJournalManifest` | `rivun-core`, `rivun-crypto` |
| 6 | `rivun-ledger` | `crates/rivun-ledger` | Cryptographic action receipts, Merkle Mountain Range (MMR) accumulator, inclusion/exclusion proofs, receipt replication. | `ActionReceipt`, `SignedActionReceipt`, `ReceiptJournalStore`, `IncrementalMmr`, `MerkleMountainRange`, `MmrInclusionProof`, `MmrBatchInclusionProof`, `MmrExclusionProof`, `ReceiptSegmentManifest`, `ReceiptBatchSeal` | `rivun-core`, `rivun-crypto`, `rivun-envelope`, `rivun-journal` |
| 7 | `rivun-capability` | `crates/rivun-capability` | Hierarchical capability IDs, permission matrices, driver capability declarations, and cached grant trees. | `CapabilityId`, `CapabilitySet`, `DriverPermissions`, `CapabilityQuery`, `CapabilityResponse`, `JsonlCapabilityCache` | `rivun-core` |
| 8 | `rivun-driver-sdk` | `crates/rivun-driver-sdk` | Rust SDK for authoring WebAssembly guest action drivers with ABI v1 helpers. | `RivunDriver`, `AsyncRivunDriver`, `DriverInput`, `PackedResult`, `pack_result()`, `unpack_result()`, `PinnedBuffer`, `ZeroCopyBuffer`, `IpcChannel` | `rivun-core` |
| 9 | `rivun-runtime` | `crates/rivun-runtime` | Sandboxed WASM execution via Wasmtime, fuel metering, epoch timeouts, lock-free circular SPSC ring buffers, driver IPC pipelines. | `WasmExecutor`, `AsyncWasmExecutor`, `ExecutionLimits`, `DriverPipeline`, `SpscRingBuffer`, `StreamingBufferPool`, `AsyncModbusConnection` | `rivun-core`, `rivun-capability` |
| 10 | `rivun-agent` | `crates/rivun-agent` | Agent protocol contracts, intents, sessions, delegations, capability negotiations, causal provenance chain engine. | `AgentIntent`, `AgentSession`, `DelegationRequest`, `DelegationResponse`, `CapabilityNegotiationRequest`, `CapabilityNegotiationResponse`, `AgentResult`, `AgentStatusUpdate`, `ProvenanceChainBuilder`, `ProvenanceChainDigest`, `ProvenanceStage` | `rivun-core`, `rivun-crypto`, `rivun-capability` |
| 11 | `rivun-pact` | `crates/rivun-pact` | Multi-party conditional contracts, escrow deposits, threshold arbitration, slashing engine, PACT bundles. | `RivunPact`, `RivunPactBundle`, `RivunPactRevocation`, `DisputeEngine`, `EscrowPact`, `DisputeCase`, `DisputeEvidence`, `RulingOutcome`, `PactState` | `rivun-core`, `rivun-crypto` |
| 12 | `rivun-policy` | `crates/rivun-policy` | Deterministic message policy evaluation engine, rule matching, capability gates, and break-glass overrides. | `PolicySet`, `PolicyRule`, `PolicyDecision`, `PolicyInput`, `PolicyEvaluator` | `rivun-core`, `rivun-capability` |
| 13 | `rivun-pack` | `crates/rivun-pack` | Signed Domain Pack lifecycle, packaging, offline resolution, manifest parsing, and security audits. | `DomainPackBundle`, `DomainPackBundleSignature`, `DomainPackManifest`, `audit_bundle()`, `validate_pack()` | `rivun-core`, `rivun-crypto`, `rivun-store` |
| 14 | `rivun-store` | `crates/rivun-store` | Offline signed driver registry, semantic version resolution, migration trees, publication bundles. | `DriverRegistry`, `DriverManifest`, `RegistryPublication`, `RegistryInstallPlan`, `DomainPackRegistry`, `artifact_hash()` | `rivun-core`, `rivun-crypto`, `rivun-capability` |
| 15 | `rivun-router` | `crates/rivun-router` | Deterministic envelope route planning, pattern matching, peer forwarding, and routing explanations. | `RouteTable`, `RouteRule`, `RouteMatch`, `RouteTarget`, `RouteMessage`, `RouteDecision`, `RouteExplanation` | `rivun-core`, `rivun-capability` |
| 16 | `rivun-schema` | `crates/rivun-schema` | Typed JSON/binary message contracts for `ZENV` envelopes, schema validation, and field constraints. | `MessageContract`, `MessageContractSet`, `BodyContract`, `MetadataContract`, `MessageParts` | `rivun-core`, `rivun-envelope` |
| 17 | `rivun-machine` | `crates/rivun-machine` | Hardware-neutral machine connection primitives, device profiles, Modbus/Serial/TCP protocol adapters. | `DeviceProfile`, `MachineConnection`, `ProtocolAdapter`, `MachineState`, `MachineHealth`, `MachineCommand`, `CommandOutcome` | `rivun-core`, `rivun-capability` |
| 18 | `rivun-memory` | `crates/rivun-memory` | Auditable local binary memory journal, hash-chained entry trees, tombstones, and queries. | `MemoryStore`, `MemoryJournalStore`, `JsonlMemoryStore`, `MemoryRecord`, `MemoryTombstone`, `MemoryQuery`, `MemoryVerificationReport` | `rivun-core`, `rivun-journal` |
| 19 | `rivun-telemetry` | `crates/rivun-telemetry` | Prometheus metrics, OpenTelemetry tracing, fleet topology aggregation, 7-Point Fleet Doctor diagnostics, incident forensics. | `FleetDoctor`, `FleetDoctorReport`, `FleetDoctorCheck`, `FleetTopology`, `FleetNodeState`, `IncidentForensicsSnapshot` | `rivun-core`, `rivun-ledger`, `rivun-store` |
| 20 | `rivun-node` | `crates/rivun-node` | Main node daemon runtime, configuration validation, peer management, dispatch loop, discovery service. | `RivunNode`, `RivunNodeConfig`, `ConfigValidationReport`, `DiscoveryService`, `PeerTrustConfig`, `SignedDiscoveryAdvertisement` | `rivun-core`, `rivun-crypto`, `rivun-envelope`, `rivun-net`, `rivun-runtime`, `rivun-ledger`, `rivun-capability`, `rivun-policy`, `rivun-router`, `rivun-schema`, `rivun-memory`, `rivun-telemetry` |
| 21 | `rivun-gateway` | `crates/rivun-gateway` | AI Agent Gateway & Model Context Protocol (MCP) server over stdio, HTTP REST, SSE, and WebSockets. | `AgentGatewayServer`, `McpEngine`, `HttpAgentGateway`, `SseBroker`, `WebSocketHandler`, `GatewayConfig` | `rivun-core`, `rivun-crypto`, `rivun-envelope`, `rivun-agent`, `rivun-capability`, `rivun-net` |
| 22 | `rivun-ops` | `crates/rivun-ops` | Operational contracts, release manifests, governance approval policies, multi-signature quorums, audit trails. | `ObservabilityConfig`, `GovernanceConfig`, `ApprovalPolicy`, `AuditEntry`, `HealthReport`, `ServiceIdentity` | `rivun-core` |
| 23 | `rivun-cli` | `crates/rivun-cli` | Unified CLI binary for node operations, keygen, inspections, doctor, swarm testing, and pack management. | `Cli`, `Commands`, `run_doctor()`, `fleet_doctor()`, `incident_snapshot()` | All workspace crates |
| 24 | `rivun-cloud-api` | `crates/rivun-cloud-api` | Multi-tenant zero-trust SaaS REST/SSE API server for fleet observability, receipts ledger, and policy staging. | `CloudDatabase`, `EventBroker`, `AppState`, `build_app()` | `rivun-core`, `rivun-crypto`, `rivun-telemetry`, `rivun-ledger` |
| 25 | `rivun-cloud-bridge` | `crates/rivun-cloud-bridge` | Edge bridge daemon pushing telemetry/receipts to Rivun Cloud and pulling signed policy bundles. | `CloudBridgeDaemon`, `CloudBridgeClient`, `BridgeConfig`, `PolicyVerifier`, `PolicyBundle` | `rivun-core`, `rivun-crypto`, `rivun-telemetry`, `rivun-policy` |
| 26 | `rivun-control` | `apps/rivun-control/src-tauri` | Operator desktop station & local key vault (Tauri app) for staging and signing policies. | `OperatorVault`, `TauriCommands`, `PolicyStagingClient` | `rivun-core`, `rivun-crypto`, `rivun-policy`, `rivun-telemetry` |

---

## 2. Rivun/ZAP Wire Protocol & Cryptographic Specifications

### 2.1 Fixed 64-Byte Rivun-Wire Header Layout

Every Rivun-Wire frame begins with a fixed 64-byte big-endian binary header.

```
 0                   1                   2                   3
 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                  Magic: 0x5A41505F ("ZAP_")                   |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|          Version (0x0001)     |          Flags                |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                                                               |
+                       Source Node UUID                        +
|                           (16 bytes)                          |
+                                                               +
|                                                               |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                                                               |
+                       Target Node UUID                        +
|                           (16 bytes)                          |
+                                                               +
|                                                               |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                    Timestamp Micros (High)                    |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                    Timestamp Micros (Low)                     |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                    Rivun Len (u32, Payload)                   |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                     Rivun Sign (8-byte Hint)                  |
|                                                               |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                 Payload Bytes (0 .. rivun_len)                 |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|            [Optional AuthTrailer (72 bytes: ZSIG)]            |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|        [Optional PoaTrailer (40 + 80*K bytes: ZPOA)]          |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
```

#### Field Specifications:
- `magic`: `0x5A41505F` (ASCII `ZAP_`). Fixed 4 bytes.
- `version`: `0x0001` (Version 1). Fixed 2 bytes.
- `flags`: 2-byte bitmask (`RivunFlags`):
  - `ENCRYPTED = 0x0001` (bit 0): Payload is encrypted.
  - `PRIORITY = 0x0002` (bit 1): Urgent low-latency routing.
  - `REQUIRES_CONSENSUS = 0x0004` (bit 2): Critical action requiring Proof-of-Action quorum.
  - `SIGNED = 0x0008` (bit 3): Auth trailer attached (`ZSIG`).
  - `BROADCAST = 0x0010` (bit 4): Frame addressed to all peers in the mesh.
- `source_node`: 16-byte UUID (RFC 4122 / UUID v8 derived from public key).
- `target_node`: 16-byte UUID (`00000000-0000-0000-0000-000000000000` for broadcast).
- `timestamp_micros`: 8-byte big-endian Unix timestamp in microseconds.
- `rivun_len`: 4-byte big-endian payload length (maximum 16 MiB = 16,777,216 bytes).
- `rivun_sign`: 8-byte signature hint derived as the first 8 bytes of `blake3("Rivun-SIGN-HINT-v1" || signature)`.

### 2.2 Trailers (Auth & Proof-of-Action)

#### 1. Authentication Trailer (`AuthTrailer` - 72 Bytes)
- `magic`: `0x5A534947` (ASCII `ZSIG`). Fixed 4 bytes.
- `algorithm`: `0x0001` (Ed25519). Fixed 2 bytes.
- `signature_len`: `0x0040` (64 bytes). Fixed 2 bytes.
- `signature`: 64-byte Ed25519 signature covering the 56-byte header prefix + payload.

#### 2. Proof-of-Action Trailer (`PoaTrailer` - Variable: $40 + 80 \times K$ Bytes)
- `magic`: `0x5A504F41` (ASCII `ZPOA`). Fixed 4 bytes.
- `version`: `0x0001` (Version 1). Fixed 2 bytes.
- `threshold`: `u16` required threshold ($T$).
- `attestation_count`: `u16` count ($K \ge T$) of validator attestations.
- `reserved`: `0x0000` (2 bytes zero).
- `frame_digest`: 32-byte BLAKE3 digest of `blake3("Rivun-POA-DIGEST-v1" || signing_prefix || payload)`.
- `attestations`: $K$ consecutive 80-byte records:
  - `validator_node`: 16-byte UUID of validator.
  - `signature`: 64-byte Ed25519 signature of `blake3("Rivun-POA-SIGNATURE-v1" || frame_digest)`.

### 2.3 Cryptographic Primitives & Domain Separation Strings

| Domain Separator String | Purpose / Signing Transcript Target | Key Algorithm |
|---|---|---|
| `Rivun-NODE-ID-v1` | Node UUID derivation: `UUID::from_bytes(blake3(pubkey)[..16])` (formatted as UUID v8) | BLAKE3 |
| `Rivun-SIGN-HINT-v1` | Fast-rejection 8-byte signature hint: `blake3(hint_prefix \|\| sig)[..8]` | BLAKE3 |
| `Rivun-POA-DIGEST-v1` | Digest of frame prefix and payload for validator signing | BLAKE3 |
| `Rivun-POA-SIGNATURE-v1` | Attestation signature transcript: `domain \|\| frame_digest` | Ed25519 |
| `Rivun-POA-VALIDATOR-SET-v1` | Canonical signing transcript for validator set reconfiguration | Ed25519 |
| `Rivun-BLINDED-COMMITMENT-v1` | Action commitment blinding: `blake3(domain \|\| salt \|\| payload)` | BLAKE3 |
| `Rivun-BLINDED-RECEIPT-v1` | Receipt hash commitment blinding: `blake3(domain \|\| salt \|\| receipt_hash)` | BLAKE3 |
| `Rivun-BATCH-SEAL-v1` | MMR peak seal transcript: `blake3(domain \|\| root_hash \|\| range)` | BLAKE3 / Ed25519 |
| `Rivun-PROVENANCE-CHAIN-v1` | Full causal provenance execution chain root signature | Ed25519 |
| `ZAP-PACT-v1` | PACT record canonical JSON signing transcript | Ed25519 |
| `ZAP-PACT-REVOCATION-v1` | PACT revocation evidence signing transcript | Ed25519 |

### 2.4 Universal 74-Byte ZENV Envelope Layout

Every message across the network mesh is encapsulated in a zero-copy `ZENV` envelope:

```
 0                   1                   2                   3
 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                  Magic: 0x5A454E56 ("ZENV")                   |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|          Version (1)          |  Kind (1..8)  |  Reserved (0) |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                                                               |
+                     Envelope ID (UUID v4)                     +
|                           (16 bytes)                          |
+                                                               +
|                                                               |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                                                               |
+                 Correlation ID (Optional UUID)                +
|                           (16 bytes)                          |
+                                                               +
|                                                               |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                                                               |
+                  Causation ID (Optional UUID)                 +
|                           (16 bytes)                          |
+                                                               +
|                                                               |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|        Subject Len (u16)      |      Content-Type Len (u16)   |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                       Metadata Len (u32)                      |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                       Body Len (u64 High)                     |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                       Body Len (u64 Low)                      |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                 Subject Bytes (UTF-8, <= 512 B)               |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|              Content-Type Bytes (ASCII, <= 128 B)             |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                  Metadata Bytes (<= 64 KiB)                   |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                   Body Bytes (<= 16 MiB)                      |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
```

#### Message Kinds:
1. `Data` (0x01): Raw data stream or sensor reading.
2. `Event` (0x02): Publish-subscribe event announcement.
3. `Command` (0x03): Direct instruction to a machine or subsystem.
4. `Query` (0x04): Idempotent read-only state query.
5. `Response` (0x05): Synchronous or asynchronous response to a command/query.
6. `StreamChunk` (0x06): Flow-controlled chunk in a multi-part stream.
7. `Action` (0x07): Intent-based action targeting a sandboxed WASM driver.
8. `Control` (0x08): Mesh control (discovery, gossip, consensus, replication).

### 2.5 Encrypted UDP Datagram Framing (`ZAPD`)

When transmitted over UDP, frames are encapsulated into `ZAPD` datagrams encrypted with ChaCha20-Poly1305 AEAD.

- **Header Length**: 52 bytes.
- **Magic**: `0x5A415044` (ASCII `ZAPD`).
- **Version**: `0x01`.
- **Reserved**: `0x000000` (3 zero bytes).
- **Source Node**: 16 bytes UUID.
- **Target Node**: 16 bytes UUID.
- **Nonce**: 12 bytes total:
  - 4 bytes random prefix (generated per session/handshake).
  - 8 bytes monotonic counter (big-endian).
- **Ciphertext**: Variable length payload encrypted with ChaCha20-Poly1305.
- **Authentication Tag**: 16-byte Poly1305 MAC appended to ciphertext.
- **Replay Protection**: Managed via memory LRU window (`NonceReplayCache`) and write-ahead log (`DurableNonceStore`).

---

## 3. Consensus Engine & Merkle Mountain Range (MMR)

### 3.1 Proof-of-Action 2-Phase BFT Quorum Mesh

The BFT Consensus Engine (`BftConsensusEngine` in `rivun-net`) guarantees deterministic transaction ordering and non-repudiable execution authorization across a quorum of $N$ validators.

```
       [Start]
          │
          ▼
   ┌──────────────┐
   │   PROPOSE    │ ─── (Leader broadcasts SwarmProposal with proposal_hash)
   └──────────────┘
          │
          ▼
   ┌──────────────┐
   │   PREVOTE    │ ─── (Validators verify frame, sign SwarmVote::Prevote)
   └──────────────┘
          │
          ▼  (Quorum: >= T = floor(2N/3) + 1 Prevotes -> Polka)
   ┌──────────────┐
   │  PRECOMMIT   │ ─── (Validators commit state lock, sign SwarmVote::Precommit)
   └──────────────┘
          │
          ▼  (Quorum: >= T Precommits)
   ┌───────────────────────┐
   │ COMMIT CERTIFICATE    │ ─── (Bitmask + Aggregated Ed25519 Signatures)
   └───────────────────────┘
          │
          ▼
    [Block Height + 1, Round 0]
```

#### Quorum Invariants & Slashing:
- **Threshold Rule**: $T \le N$, where $T = \lfloor 2N/3 \rfloor + 1$.
- **Equivocation Detection**: If any validator emits two distinct proposals or votes for the same `(epoch, round, step)`, the engine detects the conflicting cryptographic signatures, broadcasts a slashing proof, and immediately adds the validator UUID to the permanent `slashed_nodes` set.
- **Threshold Signature Bitmask**: Bitmask of length $\lceil N/8 \rceil$ bytes deterministically indexing which validators attested, followed by consecutive 64-byte Ed25519 signatures.

### 3.2 Merkle Mountain Range (MMR) Accumulator Specification

The MMR engine in `rivun-ledger` provides an $O(\log N)$ peak-bagged Merkle accumulator for immutable receipt indexing.

#### 1. Binary Peak Merging & Carry-Over
- Leaves are 0-indexed positions ($pos = 0, 1, 2, \dots$).
- For each append, binary carry-over merges adjacent trees of equal height:
  $$\text{parent\_hash} = \text{blake3}(\text{left\_child\_hash} \,\|\, \text{right\_child\_hash})$$
- Peak-bagging folds the active mountain peaks from highest bit (bit 63) down to bit 0:
  $$\text{bagged\_root} = \text{fold}(\text{peaks}, \text{blake3}(\text{accum} \,\|\, \text{peak}))$$

#### 2. Inclusion & Exclusion Proof Formats
- **Single-Leaf Inclusion Proof (`MmrInclusionProof`)**:
  - `leaf_index`: Position of leaf.
  - `leaf_hash`: BLAKE3 hash of receipt.
  - `sister_hashes`: $O(\log N)$ sibling hashes up to the mountain peak.
  - `peak_hashes`: Hashes of all other mountain peaks.
- **Batch Multi-Leaf Proof (`MmrBatchInclusionProof`)**:
  - Deduplicates shared parent nodes across multiple receipt indices into a minimal DAG sister list.
- **Exclusion Proof (`MmrExclusionProof`)**:
  - Proves non-membership by bounding the target between adjacent monotonic receipt sequence numbers and proving their inclusion.

#### 3. Persistent Binary Layout (`.zmmr` / `ZAPMMR01`)
- Magic: `0x5A41504D4D523031` (`ZAPMMR01`).
- Fixed 64-byte header: Leaf count (`u64`), Peak count (`u16`), Root hash (`[u8; 32]`).
- Segment Data: Continuous flat array of 32-byte node hashes.

---

## 4. WASM Sandboxing & Zero-Copy Streaming Runtime

### 4.1 WASM Execution Engine (`rivun-runtime`)

The sandboxed execution environment runs untrusted driver code via Wasmtime.

#### Host-Guest ABI Specification (ABI v1):
- **Exports Required from Guest**:
  - `memory`: Exported linear memory.
  - `rivun_alloc(len: i32) -> i32`: Allocates `len` bytes in guest memory and returns offset pointer.
  - `rivun_dealloc(ptr: i32, len: i32)`: Releases allocated memory buffer.
  - `rivun_execute(action_ptr: i32, action_len: i32, payload_ptr: i32, payload_len: i32) -> i64`: Executes action. Returns packed 64-bit integer:
    $$\text{packed} = (\text{result\_ptr} \ll 32) \mid \text{result\_len}$$
- **Execution Limits & Safety**:
  - Fuel metering: Hard limit on CPU instructions (e.g., $10^6$ fuel units).
  - Epoch interruption: Background timer triggering wall-clock execution timeouts (e.g., 50ms default).
  - Strict linear memory bounding (e.g., 64 MiB maximum).
  - Complete denial of ambient system resources (no network, filesystem, environment, or system clock access unless explicitly bridged via imported host capabilities).

### 4.2 Lock-Free SPSC Circular Ring Buffer (`SpscRingBuffer`)

To achieve zero-copy microsecond latency, inter-thread and inter-driver communications utilize a lock-free Single-Producer Single-Consumer (SPSC) circular byte ring buffer.

- **Atomic State**: `head: AtomicUsize`, `tail: AtomicUsize`.
- **Capacity**: Power-of-two size with bitmask wrapping (`index = pos & (capacity - 1)`).
- **Backpressure Policies**:
  1. `DropOldest`: Overwrites unread data at the head pointer.
  2. `DropNewest`: Discards incoming write data if buffer is full.
  3. `BlockWithTimeout`: Yields/spins producer until consumer frees space or timeout expires.
  4. `Error`: Returns `BufferError::Full` immediately.

### 4.3 Multi-Driver IPC Pipeline (`DriverPipeline`)

Chains multiple WASM drivers in sequence:
$$\text{Input} \to \text{Driver}_1 \xrightarrow{\text{IPC}} \text{Driver}_2 \xrightarrow{\text{IPC}} \dots \xrightarrow{\text{IPC}} \text{Driver}_K \to \text{Receipt}$$
- Inter-driver buffers are passed directly via zero-copy pinned memory slices (`PinnedBuffer`).
- Causal intermediate step hashes are recorded at each stage for inclusion in the final receipt.

---

## 5. Agent Protocol, PACT Contracts & Causal Provenance

### 5.1 Agent Protocol Contracts (`rivun-agent`)

The agent protocol defines JSON contracts travelling in `ZENV` envelopes under content-type `application/rivun-agent+json`.

- `rivun.agent.intent`: `AgentIntent` declaring `objective`, `required_capabilities`, `constraints`, `context`, `deadline`, and `priority`.
- `rivun.agent.session`: `AgentSession` tracking multi-step agent conversational lifecycle.
- `rivun.agent.delegation.request` / `.response`: `DelegationRequest` / `DelegationResponse` transferring tasks between specialized subagents.
- `rivun.agent.capability_negotiation.request` / `.response`: Dynamic discovery and handshake of supported features.
- `rivun.agent.status`: Progress heartbeat (`progress_per_mille: 0..1000`).
- `rivun.agent.result`: Terminal result payload with `outputs`, `artifacts`, and optional `AgentErrorInfo`.

### 5.2 Multi-Party Conditional Escrow PACT (`rivun-pact`)

A PACT is an auditable signed contract between autonomous nodes (`application/rivun-pact+json`).

- **Deterministic Field Ordering**: Top-level keys (`pact_id`, `actor`, `target`, `intent`, `object`, `terms`, `consent`, `proof`, `created_at_micros`, `expires_at_micros`) are emitted in strict protocol order. Nested JSON structures are sorted recursively by key.
- **Hashing**: `blake3:<64 hex chars>` over canonical signing bytes.
- **Dispute Engine (`DisputeEngine`)**:
  - States: `Locked` $\to$ `Settled` | `Disputed` $\to$ `Slashed`.
  - Ruling Outcomes: `ReleaseToRecipient`, `SlashRefundToSender`, `SplitEqual`.
  - Quorum Arbitration: Multi-signature threshold voting ($K \ge \text{threshold}$) by designated arbitrator node UUIDs.
  - Durable Snapshot Store: Atomic write via fsync tempfile + BLAKE3 envelope checksum.

### 5.3 Cryptographic Causal Provenance Engine

Enforces end-to-end mathematical non-repudiation across the entire lifecycle:
$$H_{\text{intent}} \to H_{\text{negotiation}} \to H_{\text{policy}} \to H_{\text{consensus}} \to H_{\text{driver}} \to H_{\text{poa}} \to H_{\text{receipt}} \to H_{\text{root}}$$
- Each stage hashes the previous step's hash concatenated with its current input data:
  $$H_i = \text{SHA-256}(H_{i-1} \,\|\, ":" \,\|\, \text{SHA-256}(\text{data}_i))$$
- Merkle root hash is computed over all stages and signed by the node's Ed25519 identity key using domain `Rivun-PROVENANCE-CHAIN-v1`.

---

## 6. Official Domain Packs & RivunStore Package Format

### 6.1 The 7 Official Domain Packs

Domain packs provide domain-specific capability vocabulary, message schemas, fail-closed policy templates, and routing rules.

| # | Pack Identifier | Target Domain | Core Capabilities & Risk Classification | Default Safety Gate |
|---|---|---|---|---|
| 1 | `rivun-pack-agentic-dev` | Autonomous Coding & DevOps Agents | `repo.read` (low), `repo.patch` (medium), `test.run` (medium), `ci.inspect` (low), `pr.create` (medium) | Patch dry-run & automated test receipt verification |
| 2 | `rivun-pack-cloud-ops` | Cloud Infrastructure Automation | `infra.read` (low), `infra.provision` (high), `deploy.rollout` (high), `incident.escalate` (medium) | Human approval & canary rollback simulation |
| 3 | `rivun-pack-finance` | Automated Trading & Financial Settlement | `quote.read` (low), `risk.evaluate` (low), `order.submit` (high), `settlement.reconcile` (critical) | Double-entry balance check & multi-sig PoA |
| 4 | `rivun-pack-healthcare` | Clinical Care Coordination | `records.read` (medium), `consent.verify` (low), `care.dispatch` (high), `audit.seal` (critical) | Strict PHI redaction, HIPAA audit seal, consent gate |
| 5 | `rivun-pack-industrial` | SCADA & Industrial Control | `sensor.read` (low), `plc.write` (high), `safety.override` (critical), `emergency.halt` (critical) | Hardware interlock checks & PoA validator quorum |
| 6 | `rivun-pack-personal-ai` | Personal Assistant Actions | `calendar.read` (low), `email.draft` (low), `purchase.authorize` (high), `device.control` (medium) | User explicit consent & spending limit gates |
| 7 | `rivun-pack-smart-building` | Building IoT & Energy Management | `telemetry.read` (low), `hvac.setpoint` (medium), `badge.access` (high), `lighting.control` (low) | Thermal safety envelope & physical access logs |

### 6.2 RivunStore Bundle & Registry Architecture

- **Bundle Format (`.zpack` / `RivunStore-bundle`)**: Directory containing `pack.toml`, `README.md`, `schemas/`, `policies/`, `routes/`, `drivers/`, and signed manifest `RivunStore.bundle.json`.
- **Registry Index (`registry.index.toml`)**: Lists all signed driver versions, hashes, ABI requirements (`>=1,<=2`), deprecations, and migrations.
- **Publication Statement (`registry.publication.json`)**: Cryptographic statement binding registry BLAKE3 hash to release channel (`stable`, `beta`) signed by publisher key.
- **Install Plan (`registry.install-plan.json`)**: Signed deployment intent resolving dependencies offline before disk extraction.

---

## 7. 7-Point Fleet Doctor Diagnostics & Incident Forensics

The Fleet Doctor (`rivun_telemetry::FleetDoctor` / `rivun doctor` / `rivun fleet doctor`) continuously checks 7 core criteria to guarantee cluster integrity and production readiness:

```
┌────────────────────────────────────────────────────────────────────────┐
│                     7-POINT FLEET DOCTOR DIAGNOSTIC                    │
├────┬───────────────────────┬──────────────────────────────────────────┤
│ 1  │ Network Reachability  │ UDP socket bind, active peer count       │
├────┼───────────────────────┼──────────────────────────────────────────┤
│ 2  │ Storage Mounts        │ Receipt and memory directory existence   │
├────┼───────────────────────┼──────────────────────────────────────────┤
│ 3  │ Replay Guard WAL      │ Durable WAL framing (`ZAPFRM01`), skew<30s│
├────┼───────────────────────┼──────────────────────────────────────────┤
│ 4  │ Journal Integrity     │ Segment magic (`ZJSEG001`), signed manifests│
├────┼───────────────────────┼──────────────────────────────────────────┤
│ 5  │ Pack Registry         │ RivunStore signatures (`sig_valid = 1`)  │
├────┼───────────────────────┼──────────────────────────────────────────┤
│ 6  │ Quorum & Certificates │ Ed25519 key match, Quorum T <= N met     │
├────┼───────────────────────┼──────────────────────────────────────────┤
│ 7  │ Peer Trust Status     │ Zero untrusted / revoked / banned peers  │
└────┴───────────────────────┴──────────────────────────────────────────┘
```

#### Incident Forensics Snapshots (`IncidentForensicsSnapshot`):
Generates deterministic sanitized diagnostic archives without capturing private keys or unredacted confidential payload data:
- Configuration summary & validation warnings.
- Node UUID & derived public key.
- Active peer list with trust statuses and latency metrics.
- Last 100 receipt digests and MMR root hash.
- Memory journal tail and tombstone count.

---

## 8. Multi-Language SDKs & CLI Reference

### 8.1 4 Language SDK Distributions

1. **Rust SDK (`sdks/rust`)**: Direct path wrapper over canonical workspace crates. Provides full native support for encoding/decoding, BFT consensus, WASM runtime, and cryptographic proofs.
2. **TypeScript SDK (`sdks/typescript`)**: Node.js/Browser compatible client utilizing `@noble/ed25519` and `@noble/hashes` for zero-dependency BLAKE3 hashing, `ZENV` envelope parsing, and PACT verification.
3. **Python SDK (`sdks/python`)**: Pure Python stdlib dataclasses for `ZENV` frames, receipt signing helpers, and optional C-accelerated `crypto` extra for BLAKE3/Ed25519 verification.
4. **Go SDK (`sdks/go`)**: High-performance Go package implementing `ZENV` binary decoding, UDP datagram transport, BLAKE3 hashing (`lukechampine.com/blake3`), and standard Ed25519 signature checks.

### 8.2 CLI Command Reference (`rivun-cli`)

- `rivun keygen --out <path>`: Generates Ed25519 identity key.
- `rivun run --config <path> [--strict]`: Runs node daemon.
- `rivun check-config --config <path> [--json]`: Static configuration validation.
- `rivun doctor --config <path> [--strict] [--json]`: Local node 7-point readiness check.
- `rivun send --config <path> --target <uuid> [--requires-consensus] [--poa-network]`: Sends frame.
- `rivun capability list / query / cache`: Capability discovery and verification.
- `rivun agent session / intent / status / result / delegate / negotiate`: Agent protocol tooling.
- `rivun pact create / sign / verify / revoke / bundle`: PACT lifecycle management.
- `rivun pack init / build / sign / verify / install / audit`: Domain pack workflows.
- `rivun driver-manifest create / verify`: RivunStore driver manifest signing.
- `rivun registry init / add / sign / verify / resolve / export / import`: Driver registry management.
- `rivun receipts verify / replicate`: Receipt log verification.
- `rivun incident snapshot --out <path>`: Captures forensics snapshot.
- `rivun fleet doctor --config <path>`: Multi-node cluster health aggregation.
- `rivun cluster up --nodes <N> / status`: Local cluster simulation.
- `rivun gateway run --config <path>`: Launches AI Agent MCP / REST / WebSocket gateway.

---

## 9. Features Discovered

| # | Category | Feature | Description | Inputs | Outputs | Error Behavior | Discovered Via |
|---|---|---|---|---|---|---|---|
| 1 | Wire Protocol | Fixed 64-byte Header | Standard big-endian binary frame header layout | `RivunHeader` fields | 64-byte binary slice | `InvalidMagic`, `UnsupportedVersion`, `PayloadTooLarge` | `crates/rivun-core/src/lib.rs` |
| 2 | Wire Protocol | Auth Trailer Signing | Attaches 72-byte `ZSIG` Ed25519 signature over 56B prefix + payload | `Keypair`, `RivunFrame` | Signed `RivunFrame` | `InvalidSignature`, `KeyMismatch` | `crates/rivun-crypto/src/lib.rs` |
| 3 | Wire Protocol | Proof-of-Action Trailer | Attaches $K \ge T$ validator signatures over frame digest | `PoaAttestation` list, threshold $T$ | Certified `RivunFrame` | `ThresholdNotMet`, `DuplicateValidator` | `crates/rivun-crypto/src/lib.rs` |
| 4 | Universal Envelope | 74-byte ZENV Framing | Universally structured envelope with 8 message kinds | Binary slice or struct | `RivunEnvelope` / `RivunEnvelopeRef` | `InvalidZenvMagic`, `SubjectTooLong`, `MetadataTooLong` | `crates/rivun-envelope/src/lib.rs` |
| 5 | Network Mesh | Encrypted UDP Datagram | ChaCha20-Poly1305 AEAD encapsulation (`ZAPD`) | Frame payload, key, nonce | 52B header + ciphertext + 16B tag | `DecryptionFailed`, `ReplayDetected` | `crates/rivun-net/src/lib.rs` |
| 6 | Network Mesh | BFT Consensus Engine | 2-Phase Propose-Prevote-Precommit state machine | `SwarmProposal`, `SwarmVote` | `SwarmCommitCertificate` | Equivocation slashes validator immediately | `crates/rivun-net/src/consensus/` |
| 7 | Network Mesh | Epidemic Gossip Mesh | Anti-entropy digest exchange with vector clocks | Gossip messages, peer table | Propagated events across cluster | Dropped if duplicate in Bloom filter / LRU | `crates/rivun-net/src/gossip/` |
| 8 | Network Mesh | $\Phi$-Accrual Failure Detector | Adaptive network heartbeat failure detector ($8 \le \phi \le 14$) | Heartbeat arrival timestamps | Calculated $\Phi$ value | Marks peer suspected/dead if $\phi > \phi_{\text{threshold}}$ | `crates/rivun-net/src/mesh/` |
| 9 | Ledger / Storage | Merkle Mountain Range | Carry-over subtree peak accumulator | Action receipts | `IncrementalMmr` peaks and `.zmmr` file | Fails closed on corrupted node hashes | `crates/rivun-ledger/src/mmr.rs` |
| 10 | Ledger / Storage | Multi-Leaf Batch Proof | Deduplicated DAG sister inclusion proof for $M$ leaves | Leaf indices | `MmrBatchInclusionProof` | Fails if sister hash path does not reach peak | `crates/rivun-ledger/src/mmr.rs` |
| 11 | Ledger / Storage | Non-Membership Proof | Range bounding proof of leaf non-existence | Target timestamp/seq | `MmrExclusionProof` | Fails if adjacent bounding receipts missing | `crates/rivun-ledger/src/mmr.rs` |
| 12 | Runtime | WASM Sandbox Engine | Wasmtime execution with fuel and epoch interrupts | WASM bytecode, `DriverInput` | Output byte vector | `OutOfFuel`, `Timeout`, `MemoryOutOfBounds` | `crates/rivun-runtime/src/lib.rs` |
| 13 | Runtime | Lock-Free SPSC Ring Buffer | Zero-copy circular byte buffer with backpressure policies | Byte slices | Read/Written byte slices | `BufferError::Full`, `BufferError::Timeout` | `crates/rivun-runtime/src/streaming.rs` |
| 14 | Runtime | Driver IPC Pipeline | Multi-driver execution chaining with intermediate step hashing | Driver sequence | Pipeline output + step digests | Fails if any step execution fails | `crates/rivun-runtime/src/pipeline.rs` |
| 15 | Agent Protocol | Causal Provenance Chain | Complete cryptographic execution trace ($H_{\text{intent}} \dots H_{\text{root}}$) | Execution stage digests | Signed `ProvenanceChainDigest` | Fails verification on causal link mismatch | `crates/rivun-agent/src/provenance.rs` |
| 16 | PACT | Multi-Party Escrow & Slashing | Conditional escrow locking with threshold arbitration | PACT terms, deposits | `EscrowPact` state changes | `DisputeError::ArbitrationThresholdNotMet` | `crates/rivun-pact/src/dispute.rs` |
| 17 | Domain Packs | Domain Pack Packaging | Signed `.zpack` bundle compiler, verifier, and installer | Pack directory | Verified `.zpack` archive | Rejects unsigned / untrusted author keys | `crates/rivun-pack/src/lib.rs` |
| 18 | Store / Registry | Offline Driver Registry | Semantic version resolver with migration paths | `registry.index.toml` | Resolved driver version & ABI | Fails on hash mismatch or revocation | `crates/rivun-store/src/lib.rs` |
| 19 | Diagnostics | 7-Point Fleet Doctor | Cluster health evaluation across 7 core criteria | Node config & topology | `FleetDoctorReport` | Fails strict gate on warning or failure | `crates/rivun-telemetry/src/doctor.rs` |
| 20 | Gateway | MCP / REST / WebSocket Bridge | AI Model Context Protocol stdio/HTTP server | JSON-RPC 2.0 / HTTP / WS | Envelope frames / agent messages | Rejects unauthenticated / unauthorized requests | `crates/rivun-gateway/src/lib.rs` |

---

## 10. Edge Cases & Observed System Behaviors

| # | Feature | Input / Condition | Observed & Enforced Behavior |
|---|---|---|---|
| 1 | Wire Frame Parsing | Frame payload declared as 16,777,217 bytes (> 16 MiB max) | Rejected immediately with `RivunError::PayloadTooLarge`. Parsing aborts without buffer allocation. |
| 2 | Signature Verification | Corrupted 8-byte `rivun_sign` hint vs valid 64-byte signature | `verify_frame` computes hint first; rejects immediately in $O(1)$ time prior to running costly Ed25519 point multiplication. |
| 3 | Proof-of-Action Quorum | Validator threshold $T = 3$, but only 2 valid signatures supplied | `certify_frame` rejects with `RivunCryptoError::ThresholdNotMet { required: 3, actual: 2 }`. |
| 4 | BFT Consensus | Validator emits two different proposals for same `(epoch, round, step)` | Equivocation detected; validator is immediately slashed and added to permanent `slashed_nodes` set. |
| 5 | Datagram Replay | Duplicate 12-byte nonce received within 30-second window | `NonceReplayCache` drops frame silently and logs warning without decrypting ciphertext. |
| 6 | MMR Peak Merging | Leaf count is odd ($N = 2k + 1$) during peak bagging | Lowest mountain peak is isolated at height 0; folded directly into bagged root accumulator from left to right. |
| 7 | WASM Fuel Depletion | Driver enters infinite loop `loop {}` during `rivun_execute` | Wasmtime trap triggered when fuel drops to 0; returns `DriverError::Execution("out of fuel")`. |
| 8 | WASM Wall Timeout | Driver blocks on async Modbus socket for longer than epoch timeout | Epoch timer increments; Wasmtime engine interrupts guest and unblocks host thread safely. |
| 9 | Ring Buffer Overflow | `SpscRingBuffer` full with policy `BackpressurePolicy::DropOldest` | Producer advances atomic `head` pointer over unread byte, overwriting oldest entry without blocking. |
| 10 | PACT Hash Verification | JSON object keys in `terms` reordered in payload | `normalize_json_value` sorts all object keys recursively; canonical hash matches consistently across Rust/TS/Py/Go. |
| 11 | PACT Dispute Timeout | `execute_timeout_slash` called before `now_micros > timeout_micros` | Returns `DisputeError::PactNotExpired`; escrow units remain locked in pact state. |
| 12 | Provenance Break | Step 3 (`Policy`) `previous_hash` points to invalid hash string | `verify_chain` returns `valid: false` with `failure_reason: "Causal break at stage Policy"`. |
| 13 | Fleet Doctor Quorum | Topology configured with 3 nodes, but 0 active peers connected | Criterion 6 (`node_identity_key_and_poa_quorum`) emits `warning: "Active nodes (1) below quorum threshold (3/3)"`. |
| 14 | Fleet Doctor Untrusted Peer | Peer in topology has `trust_status = "quarantined"` | Criterion 7 (`peer_trust_status`) marks check as `Failed` and fails strict readiness check. |
| 15 | Registry Verification | Registry entry contains valid signature but is marked `revoked = true` | `registry resolve` excludes entry from candidate pool; returns next highest active compatible version. |
