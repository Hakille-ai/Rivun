# Project: Rivun Web Platforms (Marketing Showcase & Developer Documentation Portal)

## Architecture
Rivun delivers two distinct, production-ready, Apple-grade web platforms designed to showcase and document the ZAP protocol ecosystem:
1. **Rivun Marketing Showcase Platform** (`apps/marketing-site`):
   - Apple-grade dark glassmorphism aesthetic with subtle micro-interactions, responsive navigation, and conversion funnels.
   - Interactive Hero with live browser-side binary signed frame encoder/decoder (`ZAP_` 64B header, `ZENV` 74B envelope, `ZSIG` 72B trailer, `ZPOA` trailer) and hex inspector.
   - 60 FPS HTML5 Canvas P2P Swarm & Gossip Particle Mesh visualizer ($k$-fanout, 2-phase BFT quorum, chaos partition toggle, telemetry HUD).
   - 5 Core Protocol Innovation deep-dives: Ed25519 & Blinded Commitments, ChaCha20-Poly1305 AEAD, Proof-of-Action BFT Consensus ($T \le N$), Wasmtime Sandboxing & Fuel Metering, Merkle Mountain Range (MMR) accumulators.
   - Rivun Cloud SaaS & Operator Workstation (`rivun-control` key vault) showcase with 4-step staging and local offline signing simulator.
   - 7 Domain Packs interactive showcase (`agentic-dev`, `cloud-ops`, `finance`, `healthcare`, `industrial`, `personal-ai`, `smart-building`) with capability risk matrix and `.zpack` install command generator.
   - Enterprise Security, Compliance (SOC2, HIPAA, ISO27001, GDPR) & <0.8ms p99 SLA guarantees with mathematical offline verification proofs.
   - Interactive 4-Tier Pricing & ROI Calculator with node/throughput volume sliders.
   - Live Developer Sandbox with multi-language code generation across Rust, TypeScript, Python, Go, and cURL.

2. **Rivun Developer Documentation Portal** (`apps/docs-portal`):
   - Dedicated Next.js 15 App Router documentation engine with instant client-side full-text search (<10ms latency, `Cmd+K` keyboard shortcut).
   - Multi-level collapsible sidebar navigation, dynamic breadcrumbs, and floating scroll-spy Table of Contents.
   - Copyable multi-language code tabs (Rust, TypeScript, Python, Go, CLI) with syntax highlighting.
   - Dark glassmorphism callouts (Note, Tip, Warning, Danger, Protocol Invariant) and client-side Mermaid diagram renderers.
   - Exhaustive documentation content tree covering A to Z:
     - Getting Started & Quickstart guides for all 4 SDKs (Rust, TypeScript, Python, Go).
     - Architecture & Core Protocol (`@@rivun_HEADER@@` wire format, ZENV envelopes, cryptographic signing, ChaCha20-Poly1305 transport).
     - Proof-of-Action consensus engine & BFT quorum mesh ($T \le N$).
     - Sandboxed WASM execution & zero-copy streaming runtime (`SpscRingBuffer`).
     - Multi-tenant Rivun Cloud SaaS & local operator workstation (`rivun-control` key vault, zero-trust staging & signing).
     - 26 Crate-by-crate API references with signatures, types, and examples.
     - 4 SDK developer manuals with copyable code snippets.
     - 7 Domain Packs guide & RivunStore bundle publishing.
     - 7-Point Fleet Doctor diagnostics, incident forensics, and MMR offline verifications.
     - Interactive API explorer & live protocol frame sandbox.

3. **E2E Testing Track & Verification Infrastructure** (`tests/e2e`):
   - Opaque-box requirement-driven multi-tier test suite (Tiers 1-4) published via `TEST_READY.md`.
   - Tier 1: Feature Coverage (≥5 tests per feature).
   - Tier 2: Boundary & Corner Cases (≥5 tests per boundary/error case).
   - Tier 3: Cross-Feature Combinations & Integration.
   - Tier 4: Real-World Application Workloads & E2E Verification.
   - Tier 5: Adversarial Coverage Hardening with white-box test generators.

---

## Code Layout
- `apps/marketing-site/` (owned by Marketing Site Builder):
  - `package.json`, `next.config.mjs`, `tailwind.config.ts`, `tsconfig.json`, `postcss.config.mjs`
  - `src/app/` (layout.tsx, page.tsx, globals.css)
  - `src/components/` (Navbar, HeroFrameVisualizer, P2PSwarmCanvas, ProtocolInnovations, CloudWorkstationShowcase, DomainPacksShowcase, EnterpriseSecurity, PricingCalculator, DeveloperPlayground, Footer, UI primitives)
  - `src/lib/` (wireCodec.ts, p2pSimulator.ts, pricingCalculator.ts, domainPacksData.ts, protocolsData.ts)
- `apps/docs-portal/` (owned by Docs Portal Builder):
  - `package.json`, `next.config.mjs`, `tailwind.config.ts`, `tsconfig.json`, `postcss.config.mjs`
  - `src/app/` (layout.tsx, globals.css, docs/[...slug]/page.tsx, docs/layout.tsx, api-explorer/page.tsx, sandbox/page.tsx)
  - `src/components/` (DocsSidebar, SearchModal, CodeTabs, MermaidViewer, Callout, TableOfContents, FrameSandbox, QuorumSimulator, CrateReferenceCard, DoctorCheckViewer, ApiExplorer)
  - `src/lib/` (searchEngine.ts, docsNavigation.ts, crateMetadata.ts, sdkManuals.ts, domainPackDocs.ts, doctorChecks.ts, wireCodec.ts)
  - `public/search-index.json` (pre-compiled inverted search index with 77 documents)
- `tests/e2e/` (owned by E2E Testing Track):
  - `test-runner.mjs`, `tier1-features.test.mjs`, `tier2-boundaries.test.mjs`, `tier3-integration.test.mjs`, `tier4-scenarios.test.mjs`, `challenger1_empirical_stress.mjs`, `test_marketing_codec_crosscheck.mjs`
  - `TEST_INFRA.md`, `TEST_READY.md`

---

## Feature Inventory
| # | Feature | Description | Milestone | Source | Status |
|---|---------|-------------|-----------|--------|--------|
| 1 | Marketing Hero & Signed Frame Visualizer | Interactive 64B wire header, 74B ZENV, ZSIG Ed25519 & ZPOA consensus trailer encoder/decoder with live hex inspector | M1 | ORIGINAL_REQUEST §1 | DONE |
| 2 | P2P Swarm & Gossip Particle Mesh | 60 FPS HTML5 Canvas visualizer with $k$-fanout gossip waves, BFT quorum rings, partition chaos toggle, and HUD | M1 | ORIGINAL_REQUEST §1 | DONE |
| 3 | 5 Core Protocol Innovations Showcase | Deep-dive tabs for Ed25519, ChaCha20-Poly1305, Proof-of-Action BFT, Wasmtime Sandboxing, and MMR accumulators | M1 | ORIGINAL_REQUEST §1 | DONE |
| 4 | Rivun Cloud SaaS & Operator Workstation | Interactive 4-step staging and local offline signing workflow simulation (`rivun-control` key vault) | M1 | ORIGINAL_REQUEST §1 | DONE |
| 5 | 7 Domain Packs Showcase | Filterable cards with capability risk classification matrices, policy TOML viewers, and CLI install generators | M1 | ORIGINAL_REQUEST §1 | DONE |
| 6 | Enterprise Security & Compliance | Matrix for SOC2, HIPAA, ISO27001, GDPR, <0.8ms p99 SLA guarantees, and cryptographic offline verification proofs | M1 | ORIGINAL_REQUEST §1 | DONE |
| 7 | Interactive Pricing & ROI Calculator | 4-tier pricing model (Community, Pro, Enterprise, Sovereign) with live node count/throughput volume sliders | M1 | ORIGINAL_REQUEST §1 | DONE |
| 8 | Live Developer Sandbox & Code Gen | Interactive frame builder generating copyable snippets across Rust, TypeScript, Python, Go, and cURL | M1 | ORIGINAL_REQUEST §1 | DONE |
| 9 | Apple-Grade Aesthetics & Navigation | Dark glassmorphism, responsive navigation bar, mobile drawer, footer with ecosystem links, conversion funnels | M1 | ORIGINAL_REQUEST §1 | DONE |
| 10 | Instant Client-Side Full-Text Search | Inverted search index with <10ms response (0.69ms p99), `Cmd+K` keyboard shortcut, fuzzy term highlighting | M2 | ORIGINAL_REQUEST §2 | DONE |
| 11 | Multi-Level Sidebar & Scroll-Spy TOC | Collapsible categorized hierarchy, active route indicators, dynamic breadcrumbs, and floating scroll-spy TOC | M2 | ORIGINAL_REQUEST §2 | DONE |
| 12 | Multi-Language Code Tabs & Callouts | Copyable multi-language syntax-highlighted code blocks (Rust/TS/Py/Go/CLI) and glassmorphic styled callouts | M2 | ORIGINAL_REQUEST §2 | DONE |
| 13 | Mermaid & KaTeX Diagram Renderers | Interactive client-side Mermaid state/sequence diagram rendering and mathematical KaTeX formula rendering | M2 | ORIGINAL_REQUEST §2 | DONE |
| 14 | Architecture & Core Protocol Docs | Comprehensive specification chapters for `@@rivun_HEADER@@` wire format, ZENV envelopes, ChaCha20, Ed25519 | M2 | ORIGINAL_REQUEST §2 | DONE |
| 15 | Consensus Engine & BFT Quorum Docs | Proof-of-Action 2-Phase BFT state machine ($T \le N$), validator sets, bitmask threshold signatures, equivocation slashing | M2 | ORIGINAL_REQUEST §2 | DONE |
| 16 | WASM Sandbox & Zero-Copy Streaming Docs | Wasmtime fuel limits, epoch interrupts, ABI v1 exports, lock-free SPSC circular ring-buffers | M2 | ORIGINAL_REQUEST §2 | DONE |
| 17 | Rivun Cloud SaaS & Key Vault Docs | Multi-tenant SaaS architecture, local operator key vault (`~/.rivun/operator_keys/`), zero-trust staging & signing | M2 | ORIGINAL_REQUEST §2 | DONE |
| 18 | 26 Workspace Crates API Reference | Exhaustive reference for all 26 crates with purposes, struct definitions, method signatures, and usage examples | M2 | ORIGINAL_REQUEST §2 | DONE |
| 19 | 4 SDK Developer Manuals | Full developer manuals and quickstart guides for Rust, TypeScript, Python, and Go SDKs | M2 | ORIGINAL_REQUEST §2 | DONE |
| 20 | 7 Domain Packs Guide & RivunStore Docs | Complete packaging, capability manifests, signing, and bundle publishing documentation for all 7 preview packs | M2 | ORIGINAL_REQUEST §2 | DONE |
| 21 | 7-Point Fleet Doctor & MMR Forensics | Diagnostic guides for all 7 health checks, incident forensic dumps, and offline Merkle Mountain Range verifications | M2 | ORIGINAL_REQUEST §2 | DONE |
| 22 | Interactive API Explorer & Live Sandbox | In-browser protocol frame sandbox and REST/SSE API testing console for Rivun Cloud endpoints | M2 | ORIGINAL_REQUEST §2 | DONE |
| 23 | Cross-Platform Build & Integration | 0 TypeScript/build errors across both `apps/marketing-site` and `apps/docs-portal`, zero broken links, responsive layout | M3 | ORIGINAL_REQUEST §3 | DONE |
| 24 | E2E Testing Suite (Tiers 1-4) | Opaque-box automated test harness covering all features, boundaries, cross-feature interactions, and scenarios (280/280 passed) | E2E | ORIGINAL_REQUEST §3 | DONE |
| 25 | Adversarial Coverage Hardening (Tier 5) | White-box adversarial testing, edge-case stress verification, and forensic integrity audit sign-off (CLEAN, 27 stress tests, 1079 assertions passed) | E2E | ORIGINAL_REQUEST §3 | DONE |

---

## Milestones
| # | Name | Scope | Dependencies | Status |
|---|------|-------|-------------|--------|
| E2E | E2E Testing Track | Requirement-driven test harness, Tiers 1-4 test suite (280 tests), published `TEST_READY.md` | none | DONE |
| M1 | Marketing Showcase Platform | Complete `apps/marketing-site` with Apple-grade dark UI, Canvas particle swarm, hero frame encoder, protocol deep-dives, Cloud showcase, 7 domain packs, pricing calculator, developer playground | none | DONE |
| M2 | Developer Documentation Portal | Complete `apps/docs-portal` with Next.js 15 App Router, instant full-text search, sidebar/TOC, code tabs, Mermaid rendering, 26 crate references, 4 SDK manuals, 7 domain packs, Fleet Doctor, live API sandbox | none | DONE |
| M3 | Cross-Platform Integration & Build Gate | Full integration, build verification (`npm run build` with 0 errors/warnings on both apps), asset alignment, route checks | M1, M2 | DONE |
| M4 | Final E2E Test Suite Pass & Adversarial Hardening | Execute 100% E2E test suite (Tiers 1-4), execute Tier 5 Adversarial Coverage Hardening, Forensic Audit verification | M3, E2E | DONE |

---

## Interface Contracts
### Protocol Wire Framing Contract
- Magic Number: `0x5A41_505F` (`ZAP_`), Big-Endian u32
- Version: `0x0001`, Big-Endian u16
- Flags: u16 bitmask (`ENCRYPTED=0x0001, PRIORITY=0x0002, REQUIRES_CONSENSUS=0x0004, SIGNED=0x0008, BROADCAST=0x0010`)
- Channel ID: u32
- Sequence Number: u64
- Timestamp: u64 (Unix microseconds)
- Payload Length: u64 (total header length = 64 bytes)
- Fast-Rejection Signature Hint: 8 bytes
- Auth Trailer (`ZSIG`): 72 bytes (4B magic `0x5A534947`, 4B key_id, 64B Ed25519 signature)
- PoA Trailer (`ZPOA`): 44 bytes header + $K \times 68$ bytes attestations ($K \le 64$)

### Universal Envelope Contract (`ZENV`)
- Magic: `0x5A454E56` (`ZENV`), 4 bytes
- Version: u16 (`1`)
- Kind: u16 (1=Data, 2=Event, 3=Command, 4=Query, 5=Response, 6=StreamChunk, 7=Action, 8=Control)
- Reserved: u16 (`0`)
- Envelope ID: 16 bytes (UUID v8 / BLAKE3 truncated)
- Correlation ID: 16 bytes
- Causation ID: 16 bytes
- Subject Length: u16
- Content-Type Length: u16
- Metadata Length: u32
- Body Length: u64 (Total Header length = 74 bytes)

### 7 Domain Packs Invariant
- Packs: `agentic-dev`, `cloud-ops`, `finance`, `healthcare`, `industrial`, `personal-ai`, `smart-building`
- Manifest: TOML with schema version 1, permissions, memory limits, and driver bindings.
- Risk ratings: `low`, `medium`, `high`, `critical`.

### 7-Point Fleet Doctor Invariant
- Checks: `network`, `storage`, `replay_guard`, `journal`, `pack_registry`, `certificate_validity`, `peer_trust`.
