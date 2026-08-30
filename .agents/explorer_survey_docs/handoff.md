# Handoff Report: Rivun Developer Documentation Portal Survey

**Agent**: Docs Portal Explorer  
**Working Directory**: `c:\Users\Stagiaire\Documents\Amadou PGC\Prs\Rivun\.agents\explorer_survey_docs`  
**Report Artifact**: `c:\Users\Stagiaire\Documents\Amadou PGC\Prs\Rivun\.agents\explorer_survey_docs\docs_portal_survey.md`  
**Date**: 2026-08-29  

---

## 1. Observation

1. **Monorepo Structure & Existing Web Apps**:
   - `apps/` currently contains `rivun-dashboard/` (Next.js 15.1.7, React 19, Tailwind CSS 3.4.17, Lucide React 0.475.0, TypeScript 5.7.3) and `rivun-control/` (Tauri desktop app & secure Ed25519 key vault).
   - `apps/docs-portal` is not yet created on disk, requiring scaffolding and implementation as mandated by `ORIGINAL_REQUEST.md`.

2. **26 Workspace Crates**:
   - `Cargo.toml` lines 1–32 lists 25 workspace crates in `crates/` (`rivun-agent`, `rivun-capability`, `rivun-cli`, `rivun-core`, `rivun-crypto`, `rivun-driver-sdk`, `rivun-envelope`, `rivun-gateway`, `rivun-journal`, `rivun-ledger`, `rivun-machine`, `rivun-memory`, `rivun-net`, `rivun-node`, `rivun-ops`, `rivun-pact`, `rivun-pack`, `rivun-policy`, `rivun-router`, `rivun-runtime`, `rivun-schema`, `rivun-store`, `rivun-telemetry`, `rivun-cloud-bridge`, `rivun-cloud-api`) plus `apps/rivun-control/src-tauri` (26th crate/binary).

3. **Core Protocol & Binary Wire Constants**:
   - `crates/rivun-core/src/lib.rs` (lines 13–37): `MAGIC_NUMBER = 0x5A41_505F` (`ZAP_`), `VERSION = 0x0001`, `HEADER_LEN = 64`, `AUTH_TRAILER_MAGIC = *b"ZSIG"`, `AUTH_TRAILER_LEN = 72`, `POA_TRAILER_MAGIC = *b"ZPOA"`, `POA_TRAILER_VERSION = 1`, `MAX_POA_ATTESTATIONS = 64`.
   - `crates/rivun-envelope/src/lib.rs` (lines 12–44): `MAGIC_BYTES = *b"ZENV"`, `HEADER_LEN = 74`, 8 envelope kinds (`Data=1`, `Event=2`, `Command=3`, `Query=4`, `Response=5`, `StreamChunk=6`, `Action=7`, `Control=8`).
   - `crates/rivun-net/src/lib.rs` (lines 122–125): `DATAGRAM_MAGIC = *b"ZAPD"`, version `1`, ChaCha20-Poly1305 encrypted UDP payload.

4. **Consensus, WASM & Cloud Architecture**:
   - `crates/rivun-net/src/consensus.rs`: BFT Swarm consensus with dynamic threshold signatures ($T \le N$), validator sets, equivocation proofs.
   - `crates/rivun-runtime/src/lib.rs`: Wasmtime sandbox, Driver ABI v1 (`rivun_alloc`, `rivun_dealloc`, `rivun_execute`), async pipelines, lock-free SPSC ring-buffers.
   - `crates/rivun-cloud-api/src/lib.rs` & `docs/cloud.md`: Zero-trust invariant (private keys strictly isolated to local `~/.rivun/operator_keys/`), Axum 0.8 REST + SSE server, staged policy diff review and signing with domain `Rivun-POLICY-BUNDLE-v1`.

5. **4 SDKs & 7 Domain Packs**:
   - `sdks/` contains 4 official SDKs: `rust`, `typescript`, `python`, `go`, tested against 11 shared JSON fixtures in `fixtures/`.
   - `examples/domain-packs/` contains 7 domain packs: `agentic-dev`, `cloud-ops`, `finance`, `healthcare`, `industrial`, `personal-ai`, `smart-building`.

6. **7-Point Fleet Doctor Diagnostics**:
   - `crates/rivun-telemetry/src/doctor.rs` (lines 112–288): 7 core diagnostic checks: (1) `network` / `cluster_network_reachability`, (2) `storage` / `storage_mounts_and_permissions`, (3) `replay_guard` / `durable_replay_store_wal`, (4) `journal` / `segment_rotation_and_manifest_signatures`, (5) `pack_registry` / `rivun_store_index_and_signatures`, (6) `certificate_validity` / `node_identity_key_and_poa_quorum`, (7) `peer_trust` / `peer_trust_status`.

---

## 2. Logic Chain

1. From **Observation 1 & 2**, `apps/docs-portal` must be scaffolded with Next.js 15 App Router, TypeScript, React 19, and Tailwind CSS to guarantee design and build consistency with `apps/rivun-dashboard`.
2. From **Observation 3 & 4**, documentation must cover the exact binary layouts, wire headers, universal envelopes, BFT consensus mechanisms, and zero-trust invariants with mathematical rigor and copyable multi-language examples.
3. From **Observation 5 & 6**, all 26 workspace crates, 4 SDKs, 7 domain packs, and 7-Point Fleet Doctor diagnostic checks require dedicated, structured reference chapters.
4. From the search and interactive requirements, a pre-indexed client-side inverted index (`public/search-index.json`) and interactive React sandbox components (Wire Frame Sandbox, PoA Quorum Simulator, PACT Hasher, API Explorer) must be integrated directly into the portal.
5. Therefore, a complete architectural blueprint, component design, route mapping, search engine design, and content inventory was produced and saved to `docs_portal_survey.md`.

---

## 3. Caveats

- `apps/docs-portal` has not yet been implemented or built in this exploratory step (investigation was strictly read-only).
- The package dependencies for `apps/docs-portal` should mirror `apps/rivun-dashboard`'s proven versions (`next@^15.1.7`, `react@^19.0.0`, `tailwindcss@^3.4.17`, `lucide-react@^0.475.0`).
- No other caveats.

---

## 4. Conclusion

The architectural survey for `apps/docs-portal` is **100% complete and fully specified**. The resulting survey report `c:\Users\Stagiaire\Documents\Amadou PGC\Prs\Rivun\.agents\explorer_survey_docs\docs_portal_survey.md` provides an exhaustive blueprint for:
- Next.js 15 App Router structure with SSG.
- Instant client-side full-text search engine (<10ms latency, `Cmd+K` modal).
- Multi-level collapsible sidebar navigation, dynamic breadcrumbs, and floating scroll-spy Table of Contents.
- Copyable multi-language code tabs (Rust, TypeScript, Python, Go, CLI).
- Dark glassmorphism callouts, client-side Mermaid diagrams, and KaTeX mathematical formulas.
- 10-section documentation content tree covering A-to-Z from Getting Started to all 26 Crates, 4 SDKs, 7 Domain Packs, Fleet Doctor, and Interactive Sandboxes.
- Zero-warning build and typecheck setup.

---

## 5. Verification Method

To verify this survey:
1. Inspect the survey report:
   ```powershell
   Get-Content "c:\Users\Stagiaire\Documents\Amadou PGC\Prs\Rivun\.agents\explorer_survey_docs\docs_portal_survey.md"
   ```
2. Verify all references to crates, SDKs, and fixtures:
   ```powershell
   cargo check --workspace
   npm --prefix sdks/typescript test
   python -m unittest discover -s sdks/python/tests
   ```
