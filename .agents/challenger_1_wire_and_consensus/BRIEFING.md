# BRIEFING — 2026-08-29T01:34:00Z

## Mission
Empirically challenge and stress-test the Rivun protocol wire implementation, binary codecs, and consensus engine across Rust crates, protocol.ts, and e2e harness.

## 🔒 My Identity
- Archetype: Challenger
- Roles: critic, specialist
- Working directory: c:\Users\Stagiaire\Documents\Amadou PGC\Prs\Rivun\.agents\challenger_1_wire_and_consensus
- Original parent: 0a28176c-5a67-4f34-9762-4b0f40e15367
- Milestone: Final Review
- Instance: 1 of 1

## 🔒 Key Constraints
- Review-only — do NOT modify implementation code
- Run verification code yourself; do NOT trust claims
- Write only to own folder (.agents/challenger_1_wire_and_consensus/)
- Ensure tests and stress harnesses run against actual implementations

## Current Parent
- Conversation ID: 0a28176c-5a67-4f34-9762-4b0f40e15367
- Updated: 2026-08-29T01:34:00Z

## Review Scope
- **Files to review**:
  - `crates/rivun-core/*`, `crates/rivun-crypto/*`, `crates/rivun-envelope/*`, `crates/rivun-net/*`, `crates/rivun-ledger/*`
  - `apps/marketing-site/lib/protocol.ts`
  - `sdks/typescript/src/protocol.ts`
  - `tests/e2e/harness/*`
- **Interface contracts**: PROJECT.md, crate_and_protocol_specs.md
- **Review criteria**: Wire protocol binary fidelity, edge case robustness, Byzantine fault tolerance, signature verification, malformed frame handling

## Attack Surface
- **Hypotheses tested**:
  - 64-byte `ZAP_` header big-endian layout, magic mutation, version fuzzing, invalid flag bits, >16MiB payload boundary, trailer invariant mismatch.
  - 74-byte `ZENV` universal envelope layout, 8 message kinds, missing subject rejection, boundary lengths, non-zero reserved field.
  - Ed25519 `ZSIG` trailer signing transcript prefix (56B), signature bit flips, fast-hint O(1) rejection, key/identity mismatch.
  - `ZPOA` consensus trailer layout, quorum threshold $T = \lfloor 2N/3 \rfloor + 1$, attestation bounds ($K \le 64$), 2-phase BFT engine, equivocation slashing.
  - Merkle Mountain Range (MMR) carry-over subtree merging, bagged root calculation, inclusion proofs, and monotonic range non-membership exclusion proofs.
  - Cross-codec binary interoperability between `apps/marketing-site/lib/protocol.ts`, `sdks/typescript`, and Rust `rivun-core`/`rivun-envelope`.
- **Vulnerabilities found**:
  1. `apps/marketing-site/lib/protocol.ts`: Payload length at offset 48..52 encoded as `u32` with zero at 52..56 instead of `u64` at 48..56, breaking cross-codec parsing in `rivun-core` with `PayloadTooLarge`.
  2. `apps/marketing-site/lib/protocol.ts`: ZENV 74B header internal layout offsets shifted by 2 bytes due to 1B `kind` + 1B `reserved` + 2B trailing padding, causing `unknown envelope kind 512` and corrupted UUID decoding.
  3. `tests/e2e/harness/zenvCodec.mjs`: Missing `reserved == 0` validation and non-empty `subject` enforcement for non-Data kinds.
- **Untested angles**: None.

## Loaded Skills
- None

## Key Decisions Made
- Executed `cargo test --workspace` (all passed).
- Executed `node test-runner.mjs` (280/280 E2E tests passed).
- Authored and executed standalone empirical stress harness `tests/e2e/challenger1_empirical_stress.mjs` (27 stress tests).
- Authored and executed `tests/e2e/test_marketing_codec_crosscheck.mjs` confirming binary layout divergence.
- Issued verdict: `REQUEST_CHANGES`.

## Artifact Index
- DISPATCH.md — Dispatch log
- BRIEFING.md — Persistent context
- progress.md — Liveness log
- handoff.md — Final handoff report
