# BRIEFING — 2026-08-15T15:01:20Z

## Mission
Conduct an in-depth survey of the ZAP codebase specifically focusing on R1: P2P Swarm Gossip Consensus & Adaptive Quorum Mesh (`zap-net`, `zap-agent`, `zap-node`).

## 🔒 My Identity
- Archetype: explorer
- Roles: investigation, synthesis
- Working directory: c:\Users\Stagiaire\Documents\Amadou PGC\Prs\ZAP\.agents\explorer_survey_1
- Original parent: 5e9776fe-ebb1-46ac-b395-bfa1d62c469a
- Milestone: survey-phase

## 🔒 Key Constraints
- Read-only investigation — do NOT implement code changes in the main crates
- Write only to .agents/explorer_survey_1/
- Produce comprehensive analysis.md and handoff.md

## Current Parent
- Conversation ID: 5e9776fe-ebb1-46ac-b395-bfa1d62c469a
- Updated: 2026-08-15T15:01:20Z

## Investigation State
- **Explored paths**: `crates/zap-net` (UDP transport, Noise handshake, nonces, durable replay), `crates/zap-agent` (Agent message protocols, provenance chain engine), `crates/zap-node` (ZapNode daemon, handle_once, discovery, PoA verification, routing, durability), `crates/zap-core`, `crates/zap-crypto`, `crates/zap-router`, `crates/zap-ledger`, `crates/zap-cli`
- **Key findings**:
  - Current `zap-net` has encrypted UDP and nonce anti-replay, but static peer discovery, O(N) sequential broadcast, no epidemic gossip, no failure detector, and no failover routing.
  - Current `zap-agent` has complete 1-to-1 JSON contracts and 6-stage cryptographic provenance engine, but lacks swarm coordination state machines and collective quorum voting.
  - Current `zap-node` has a sequential `handle_once()` loop and static PoA attestation gathering ($44+80M$ bytes in `PoaTrailer`), but no BFT state machine replication or dynamic threshold signature aggregation.
  - Designed complete technical specification for R1: epidemic gossip protocol (fanout, dedup, PEX, anti-entropy), BFT swarm consensus state machine ($T = \lfloor 2N/3 \rfloor + 1$), bitmask-indexed threshold signatures (`ZSC1`), Phi Accrual Failure Detector ($\Phi$), jittered heartbeats, partition detection ($R = N_{\text{reachable}} / N$), and 2-hop failover relay routing.
- **Unexplored areas**: None for R1 survey scope.

## Key Decisions Made
- Fully documented the existing architecture and enumerated exact missing structs, enums, traits, wire formats, algorithms, and integration points in `analysis.md` and `handoff.md`.

## Artifact Index
- analysis.md — In-depth architectural & technical survey for R1
- handoff.md — 5-component handoff report
- progress.md — Liveness & task execution log
- DISPATCH.md — Initial dispatch message
