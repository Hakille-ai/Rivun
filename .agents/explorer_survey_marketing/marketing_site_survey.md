# Marketing Site Architecture & Implementation Survey: Rivun Marketing Showcase Platform

**Document Path**: `c:\Users\Stagiaire\Documents\Amadou PGC\Prs\Rivun\.agents\explorer_survey_marketing\marketing_site_survey.md`  
**Explorer**: Marketing Showcase Explorer (`explorer_survey_marketing`)  
**Target Path**: `c:\Users\Stagiaire\Documents\Amadou PGC\Prs\Rivun\apps\marketing-site`  
**Timestamp**: 2026-08-29T00:54:30Z  
**Status**: COMPLETE SURVEY & ARCHITECTURAL BLUEPRINT

---

## 1. Executive Summary

This survey defines the architecture, design system, component hierarchy, interactive visualizers, and build pipeline for the **Rivun Marketing Showcase Platform** (`apps/marketing-site`). 

Rivun (the implementation of the ZAP universal protocol) is a high-performance, low-latency, cryptographically signed-by-default execution and verification fabric. The marketing site must reflect an **Apple-grade dark aesthetic**, combining deep technical rigor with silky micro-interactions, responsive canvas animations, live protocol inspection, and interactive conversion funnels.

### Key Survey Findings:
1. **Current State**: The repository currently contains `apps/rivun-dashboard` (Next.js 15.5 / React 19 / Tailwind / Lucide) and `apps/rivun-control` (Tauri desktop operator app). The directory `apps/marketing-site` does not yet exist and will be constructed from the ground up to fulfill the specification in `ORIGINAL_REQUEST.md`.
2. **Framework Alignment**: The existing `apps/rivun-dashboard` demonstrates full compatibility with Next.js 15 App Router, React 19, Tailwind CSS v3, and Lucide React, achieving clean sub-10-second production builds. `apps/marketing-site` will adopt this proven stack with client-side cryptographic and visualizer components.
3. **Core Protocol Assets**: The protocol wire format (64-byte `ZAP_` header), universal envelope (74-byte `ZENV` header), `ZSIG` Ed25519 auth trailer, `ZPOA` Proof-of-Action consensus trailer, 7 Domain Packs, and multi-tenant Rivun Cloud SaaS architecture are fully specified across the 26 workspace crates and SDK fixtures.
4. **Interactive Showcase Requirements**: The marketing platform requires 8 high-impact interactive modules:
   - **Hero Section**: Live Real-Time Signed Frame Encoder/Decoder Visualizer with dual byte-tree and hex dump inspectors.
   - **P2P Swarm Visualizer**: Interactive HTML5 Canvas particle mesh with epidemic gossip waves ($k$-fanout), 2-phase BFT quorum rounds, and chaos partition testing.
   - **Protocol Innovations Matrix**: In-depth breakdowns of Ed25519 wire format, ChaCha20-Poly1305 transport, Proof-of-Action consensus, sandboxed WASM runtime, and incremental MMR rollups.
   - **Rivun Cloud & Zero-Trust Operator Station Showcase**: Visual architecture comparison and interactive 4-step staging & local offline signing simulator.
   - **7 Domain Packs Interactive Explorer**: Searchable, filterable catalog with capability risk tables, policy TOML previews, and `.zpack` bundle commands.
   - **Enterprise Security & SLA Matrix**: Defense-in-depth security model, SOC2/HIPAA/ISO/GDPR compliance mapping, <0.8ms p99 latency SLA, and mathematical offline verification proofs.
   - **Interactive Pricing & ROI Calculator**: 4-tier pricing model (Community $0, Pro $49, Enterprise $499, Sovereign Cloud) with dynamic node and throughput sliders.
   - **Live Protocol Sandbox / Playground**: Live frame creator, ephemeral key generator, policy rule tester, and multi-language code generators (Rust, TypeScript, Python, Go, cURL).

---

## 2. Workspace & Environment Analysis

### 2.1 Monorepo Structure & Node Environment
- **Node.js**: `v24.14.1`
- **npm**: `11.11.0`
- **Root Directory**: `c:\Users\Stagiaire\Documents\Amadou PGC\Prs\Rivun`
- **Apps Directory**: `c:\Users\Stagiaire\Documents\Amadou PGC\Prs\Rivun\apps`
- **Workspace Crates**: 26 Rust crates in `crates/` (e.g., `rivun-core`, `rivun-envelope`, `rivun-crypto`, `rivun-net`, `rivun-ledger`, `rivun-runtime`, `rivun-pack`, `rivun-cloud-api`, `rivun-cloud-bridge`).
- **Official SDKs**: `sdks/rust`, `sdks/typescript`, `sdks/python`, `sdks/go`.
- **Test Vectors & Protocol Fixtures**: `fixtures/protocol/*.json` containing exact test vectors for `signed-control-frame-v1.json`, `poa-control-frame-v1.json`, etc.

### 2.2 Reusable Patterns from `apps/rivun-dashboard`
- **Tailwind Theme Configuration**: Dark palette with semantic tokens (`bg-base: #0A0B0D`, `bg-surface: #111318`, `bg-surface-raised: #181B22`, `accent-primary: #5B8CFF`, `status-verified: #3DD68C`, `status-warning: #E8B339`, `status-critical: #F0554D`).
- **Typography**: Inter / SF Pro font stack with JetBrains Mono for cryptographic hex dumps and hash outputs.
- **Glassmorphism Styling**: `.glass-panel` and `.glass-modal` with `backdrop-filter: blur(16px)` and border highlights (`#22262F`, `#2E3440`).
- **TypeScript Declarations**: Dedicated `declarations.d.ts` guaranteeing 0 TypeScript compilation errors under Next.js 15 and React 19.

---

## 3. Visual Design System & Apple-Grade Aesthetic

### 3.1 Color Palette & Tokens
| Token | Hex Value | Semantic Usage |
|---|---|---|
| `bg-base` | `#0A0B0D` | Deep void background for infinite contrast |
| `bg-surface` | `#111318` | Primary card and section container background |
| `bg-surface-raised` | `#181B22` | Modals, interactive dropdowns, elevated panels |
| `bg-subtle` | `#14171F` | Subtle badge backgrounds and alternating tables |
| `border-subtle` | `#22262F` | Primary structural borders |
| `border-strong` | `#2E3440` | Hover states, active tabs, highlighted cards |
| `border-highlight` | `#3A4150` | Focused inputs, primary card glow borders |
| `accent-primary` | `#5B8CFF` | Electric blue brand accent, primary CTA buttons |
| `accent-hover` | `#4378F0` | Button hover and active states |
| `accent-glow` | `rgba(91, 140, 255, 0.15)` | Glowing halos, radial background highlights |
| `status-verified` | `#3DD68C` | Cryptographic signature pass, active nodes, valid proofs |
| `status-warning` | `#E8B339` | Staged policies, threshold warnings, partition alerts |
| `status-critical` | `#F0554D` | Tamper alerts, signature mismatch, fail-closed states |
| `accent-purple` | `#A855F7` | WASM sandboxing, multi-party escrow indicators |
| `text-primary` | `#F4F5F7` | High-contrast headlines and values |
| `text-secondary` | `#9AA1AE` | Subtitles, descriptions, protocol metadata |
| `text-muted` | `#6B7280` | Labels, byte offsets, auxiliary notes |

### 3.2 Glassmorphism & Micro-Interactions
- **Glass Cards**: `bg-[#111318]/80 backdrop-blur-xl border border-white/5 shadow-2xl hover:border-[#5B8CFF]/40 transition-all duration-300`
- **Glow Accents**: Radial gradient mesh backdrops (`radial-gradient(circle at 50% 0%, rgba(91, 140, 255, 0.12) 0%, transparent 70%)`) creating depth without visual clutter.
- **Interactive States**: Subtle scale transitions (`transform hover:-translate-y-0.5`), glowing borders on hover, smooth copy-to-clipboard badges, and animated tabs with sliding indicator pills.

---

## 4. Comprehensive Feature Inventory: What Exists vs What Needs to Be Built

| Module / Component | Current Repo State | Implementation Specification for `apps/marketing-site` |
|---|---|---|
| **Root & Config** | Exists for dashboard | `package.json`, `tsconfig.json`, `next.config.ts`, `tailwind.config.ts`, `postcss.config.mjs`, `declarations.d.ts` |
| **Header & Nav** | Only internal dashboard nav | Sticky glassmorphic navbar with logo pulse, links to Features, Architecture, Domain Packs, Cloud, Security, Pricing, Sandbox, Docs Portal button, and mobile hamburger drawer |
| **Hero Section** | None | High-impact Apple-grade hero with glowing badge, animated headline, primary/secondary CTAs, live stats ribbon, and the embedded **Signed Frame Encoder/Decoder Visualizer** |
| **Real-Time Frame Encoder / Decoder** | None | Live interactive widget allowing users to change Message Kind, Subject, Content-Type, Payload, Flags, and Node IDs; computes in-memory 64B wire header, 74B `ZENV` envelope, `ZSIG` Ed25519 signature, and `ZPOA` consensus trailer; features dual Annotated Byte-Tree and Color-Coded Hex Dump views |
| **P2P Swarm Canvas Visualizer** | None | High-performance HTML5 Canvas rendering active swarm nodes, validator nodes, and edge nodes; interactive buttons for "Broadcast Action" (gossip wave animation), "Simulate 2-Phase BFT Quorum", and "Simulate Network Partition" with live HUD |
| **5 Core Innovations Matrix** | Docs exist | Tabbed and card-based deep dive into Ed25519 Wire Format, ChaCha20-Poly1305 Encrypted UDP, Proof-of-Action BFT Consensus, Sandboxed WASM Runtime, and Incremental MMR Rollups |
| **Rivun Cloud & Operator Station** | Crate/Docs exist | Visual dual-panel layout comparing Cloud SaaS and Local Operator Workstation (`~/.rivun/operator_keys/`), emphasizing zero-trust key isolation, accompanied by an interactive 4-step staging and local signing workflow simulator |
| **7 Domain Packs Showcase** | 7 dirs in `examples/` | Searchable & filterable pack catalog covering `agentic-dev`, `cloud-ops`, `finance`, `healthcare`, `industrial`, `personal-ai`, and `smart-building`; slide-over / modal pack inspector with capability risk tables (`low`/`med`/`high`/`critical`), TOML preview, JSON schemas, and CLI install commands |
| **Security, Compliance & SLA** | Docs exist | Defense-in-depth architecture matrix, SOC2 / HIPAA / ISO / GDPR compliance cards, <0.8ms p99 latency SLA guarantee, and offline air-gapped mathematical proof verification ($H_{\text{intent}} \to \dots \to H_{\text{root}}$) |
| **Interactive Pricing Calculator** | None | 4-tier cards (Community $0, Pro $49/mo, Enterprise $499/mo, Sovereign Cloud custom), Annual/Monthly toggle (20% discount), and dynamic ROI sliders for Edge Nodes (1..1000+) and Monthly Receipts (100k..500M) |
| **Interactive Protocol Playground** | None | Comprehensive developer sandbox allowing live frame authoring, ephemeral Ed25519 key generation, policy rule simulation (Allow/Deny/PoA/Grant), and instant code generation in Rust, TypeScript, Python, Go, and cURL |
| **Footer & Conversion Funnel** | None | Multi-column footer with 26-crate directory, SDK quick links, protocol whitepaper links, security disclosure, RFC process, community discord/github, and newsletter subscription |

---

## 5. Technical Blueprint for Interactive Components

### 5.1 Real-Time Signed Frame Visualizer (`components/HeroFrameVisualizer.tsx`)
1. **State Management**:
   - `kind`: `data` (1), `event` (2), `command` (3), `query` (4), `response` (5), `streamChunk` (6), `action` (7), `control` (8).
   - `subject`: String (default: `sensor.temperature.read`).
   - `contentType`: String (default: `application/json`).
   - `payload`: JSON string (default: `{"celsius": 21.5, "device_id": "temp-sensor-04"}`).
   - `flags`: Bitmask (`SIGNED`, `REQUIRES_CONSENSUS`, `ENCRYPTED`, `PRIORITY`, `BROADCAST`).
   - `sourceNode`: Deterministic UUID derived from sender public key.
   - `targetNode`: Target UUID or nil UUID.
2. **Binary Frame Encoding**:
   - **Wire Header (64 bytes)**:
     - `0..4`: Magic Number `0x5A41505F` (`ZAP_`)
     - `4..6`: Version `0x0001`
     - `6..8`: Flags bitmask (e.g. `0x0008` for `SIGNED`, `0x000C` for `SIGNED | REQUIRES_CONSENSUS`)
     - `8..24`: Source Node UUID (16 bytes)
     - `24..40`: Target Node UUID (16 bytes)
     - `40..48`: Timestamp in microseconds ($8$ bytes big-endian)
     - `48..56`: Payload Length ($8$ bytes big-endian)
     - `56..64`: `ZAP_SIGN` fast signature hint ($8$ bytes)
   - **Envelope Body (ZENV Header 74 bytes + Subject + ContentType + Body)**:
     - `0..4`: Magic `ZENV`
     - `4..6`: Version `1`
     - `6..8`: Kind (`u16`)
     - `8..10`: Reserved (`0x0000`)
     - `10..26`: Envelope UUID
     - `26..42`: Correlation UUID
     - `42..58`: Causation UUID
     - `58..60`: Subject length
     - `60..62`: Content-type length
     - `62..66`: Metadata length ($4$ bytes)
     - `66..74`: Body length ($8$ bytes)
     - Followed by subject UTF-8 bytes, content-type UTF-8 bytes, metadata bytes, and body payload bytes.
   - **Auth Trailer (`ZSIG`, 72 bytes)**:
     - Magic `ZSIG` (4 bytes), Algorithm `1` (Ed25519, 2 bytes), Signature Length `64` (2 bytes), Ed25519 signature (64 bytes).
   - **Proof-of-Action Trailer (`ZPOA`, $44 + 80 \times K$ bytes)**:
     - Magic `ZPOA` (4 bytes), Version `1` (2 bytes), Threshold $T$ (2 bytes), Attestation count $K$ (2 bytes), Reserved (2 bytes), Frame digest BLAKE3 (32 bytes), followed by $K \times (16\text{B node} + 64\text{B signature})$.
3. **Inspector Modes**:
   - **Annotated Structure View**: Visual breakdown displaying exact offsets, field names, hex values, decoded ASCII/JSON representations, and technical explanations.
   - **Live Hex Dump View**: 16-byte-per-line formatted hex dump with byte offset markers (`0000:`, `0010:`, etc.) and ASCII sidebar, with synchronized hover highlighting that illuminates the corresponding protocol field.

### 5.2 P2P Swarm & Consensus Canvas Visualizer (`components/SwarmVisualizer.tsx`)
1. **Rendering Engine**:
   - HTML5 2D Canvas with `requestAnimationFrame` loop targeting 60 FPS.
   - High-DPI support (`window.devicePixelRatio` scaling).
2. **Simulation Model**:
   - **Nodes**: $N$ nodes arranged in a dynamic radial/orbital topology with subtle physics drift and spring links.
   - **Node Roles**: Edge Clients (Cyan), Swarm Relay Nodes (Blue), BFT Validators (Gold), Byzantine / Faulty Nodes (Red).
   - **Epidemic Gossip Propagation**: When triggered, message particles radiate from the source node to its $k=3$ nearest peers, cascading exponentially across the entire swarm with glowing trail vectors.
   - **BFT 2-Phase Quorum**: Step-by-step visual animation for `Propose` (gold pulse) $\to$ `Prevote` (cyan rays) $\to$ `Precommit` (green arcs) $\to$ `Commit Certificate` (emerald ring flash) with live threshold counter ($T/N \ge 2/3$).
   - **Network Partition & Healing**: Simulates split-brain defense with animated partition barrier wall, showing nodes failing closed and falling back to 2-hop relay routing.
3. **Interactive Control Bar**:
   - Node count slider ($12$ to $48$ nodes).
   - "Broadcast Frame" trigger button.
   - "Execute BFT PoA Round" trigger button.
   - "Toggle Network Partition" chaos mode.
   - Real-time telemetry HUD displaying: Active Nodes, Gossip Latency ($0.38\text{ms}$), Active Mesh Links, Swarm Health ($100\%$), Consensus Ops/sec ($12,450$).

### 5.3 7 Domain Packs Showcase (`components/DomainPacksShowcase.tsx`)
1. **Data Model**:
   - **Agentic Dev** (`rivun-pack-agentic-dev`): Capabilities: `repo.read`, `repo.patch`, `test.run`, `ci.inspect`, `pr.create`. Policies: fail-closed workspace boundaries, diff inspection before patch application.
   - **Cloud Ops** (`rivun-pack-cloud-ops`): Capabilities: `k8s.pod.restart`, `terraform.plan`, `aws.iam.inspect`, `rollback.trigger`. Policies: mandatory Proof-of-Action ($T=2$) on production namespaces.
   - **Finance** (`rivun-pack-finance`): Capabilities: `order.propose`, `risk.check`, `escrow.lock`, `settlement.execute`. Policies: multi-party conditional pacts, deterministic dispute evaluation.
   - **Healthcare** (`rivun-pack-healthcare`): Capabilities: `patient.telemetry.ingest`, `records.access`, `device.alert.dispatch`. Policies: HIPAA compliance, client-side secret redaction, air-gapped cryptographic proofs.
   - **Industrial** (`rivun-pack-industrial`): Capabilities: `plc.register.read`, `modbus.coil.write`, `safety.e_stop`, `opcua.stream`. Policies: strict WASM sandboxing, Modbus streaming ring-buffers, safety interlocks.
   - **Personal AI** (`rivun-pack-personal-ai`): Capabilities: `calendar.schedule`, `email.draft`, `credential.access`, `local_model.invoke`. Policies: zero-trust local vault isolation, human confirmation gates for sensitive actions.
   - **Smart Building** (`rivun-pack-smart-building`): Capabilities: `hvac.zone.set_temp`, `access.door.unlock`, `sensor.occupancy.stream`. Policies: multi-tenant zoning, time-of-day access schedules.
2. **Interactive UI**:
   - Category filter pills (All, AI & Dev, Cloud, Enterprise, Physical Systems).
   - Interactive pack cards featuring version badges, risk summary pills, and capability counts.
   - Full Slide-over Drawer / Modal Inspector showing:
     - Manifest metadata (`pack.toml`)
     - Complete Capabilities Table with risk classification (`low` = green, `medium` = blue, `high` = amber, `critical` = red)
     - Fail-Closed Policy TOML Viewer with syntax styling
     - JSON Schema definitions
     - One-click copyable CLI installation command: `rivun pack install --bundle <pack-id>.zpack --trusted-key <key>`

### 5.4 Rivun Cloud & Zero-Trust Operator Station Showcase (`components/CloudShowcase.tsx`)
1. **Visual Architecture Diagram**:
   - High-contrast visual schematic showing:
     - **Rivun Cloud SaaS Control Plane** (Axum 0.8 REST & SSE, multi-tenant DB, real-time telemetry ingestion, visual policy drafting).
     - **Operator Workstation (`rivun-control`)** (Desktop application with secure local key vault in `~/.rivun/operator_keys/`).
     - **Edge Fleet & `rivun-cloud-bridge`** (Daemon sidecar running on industrial / cloud / edge nodes).
2. **Zero-Trust Sovereign Invariant Highlight**:
   - Callout badge emphasizing: *Private Ed25519 keys NEVER leave the operator workstation. The SaaS control plane only receives signed policy bundles.*
3. **Interactive 4-Step Staging Simulator**:
   - **Step 1: Visual Policy Drafting**: Draft policy rule in cloud UI.
   - **Step 2: Policy Staged**: Policy is stored in cloud with status `staged` (inactive).
   - **Step 3: Local Offline Signing**: Operator pulls diff in `rivun-control`, reviews rule AST, and cryptographically signs with private key (`Rivun-POLICY-BUNDLE-v1`).
   - **Step 4: Atomic Edge Deployment**: Edge bridge daemon pulls signed bundle, verifies signature against local whitelist, and performs atomic filesystem swap (`tempfile::persist`).

### 5.5 Interactive Protocol Playground / Sandbox (`components/ProtocolSandbox.tsx`)
1. **Interactive Controls**:
   - Keypair Management: Generates ephemeral Ed25519 keypair in browser; derives deterministic UUIDv8 Node ID.
   - Frame Builder: Allows selecting Message Kind, Subject, Content Type, Custom Payload, and Security Flags.
   - Policy Engine Simulator: Allows inputting custom TOML rules (e.g. `[[rules]] subject = "repo.patch" decision = "deny"`) and evaluating whether the constructed frame is `ALLOWED`, `DENIED`, or `REQUIRES_POA`.
2. **Multi-Language Code Generator**:
   - Generates production-ready, copyable code snippets for the currently configured frame in:
     - **Rust** (`rivun-core` + `rivun-envelope`)
     - **TypeScript** (`@rivun-protocol/sdk`)
     - **Python** (`rivun-sdk`)
     - **Go** (`rivun-sdk-go`)
     - **CLI / cURL** (`rivun send ...`)

### 5.6 Interactive Pricing & ROI Calculator (`components/PricingCalculator.tsx`)
1. **4 Tier Cards**:
   - **Community**: Free / Open Source ($0). Unlimited nodes, local CLI, 26 crates, core SDKs, Apache-2.0 / MIT.
   - **Pro**: $49/mo ($39/mo billed annually). 25 edge nodes, 1M receipts/mo, Rivun Cloud SaaS dashboard, 30-day retention, 7 Domain Packs.
   - **Enterprise**: $499/mo ($399/mo billed annually). 250 edge nodes, 50M receipts/mo, multi-region BFT consensus, SSO/SAML, 24/7 dedicated SLA, custom domain packs.
   - **Sovereign Cloud**: Custom Pricing. Self-hosted multi-tenant control plane, HSM integration, offline verifier, dedicated support engineering.
2. **Interactive ROI & Cost Calculator**:
   - Slider for **Active Edge Nodes** ($1$ to $1,000+$).
   - Slider for **Monthly Receipts Volume** ($100\text{k}$ to $500\text{M}$ receipts).
   - Dynamic monthly cost calculation and estimated compute/bandwidth cost savings vs legacy JSON-RPC / broker architectures.

---

## 6. Directory Layout for `apps/marketing-site`

```
apps/marketing-site/
├── package.json
├── tsconfig.json
├── next.config.ts
├── tailwind.config.ts
├── postcss.config.mjs
├── declarations.d.ts
├── public/
│   ├── favicon.ico
│   └── og-image.png
├── app/
│   ├── layout.tsx
│   ├── page.tsx
│   ├── globals.css
│   └── sandbox/
│       └── page.tsx
├── components/
│   ├── Navbar.tsx
│   ├── Footer.tsx
│   ├── HeroSection.tsx
│   ├── HeroFrameVisualizer.tsx
│   ├── SwarmVisualizer.tsx
│   ├── ProtocolInnovations.tsx
│   ├── CloudShowcase.tsx
│   ├── DomainPacksShowcase.tsx
│   ├── SecurityCompliance.tsx
│   ├── PricingCalculator.tsx
│   └── ProtocolSandbox.tsx
└── lib/
    ├── crypto.ts
    ├── protocol.ts
    ├── domain-packs-data.ts
    └── types.ts
```

---

## 7. Build System & Verification Criteria

1. **Package Scripts**:
   - `npm run dev`: `next dev --port 3001`
   - `npm run build`: `next build`
   - `npm run start`: `next start --port 3001`
   - `npm run lint`: `next lint`
2. **TypeScript & Build Integrity**:
   - 0 TypeScript errors (`tsc --noEmit` clean).
   - 0 Next.js build errors or warnings during static site generation.
   - Completely responsive across mobile ($320\text{px}$), tablet ($768\text{px}$), and desktop ($1440\text{px}+$ viewports).
   - Accessible ARIA labels on all interactive buttons, tabs, sliders, and modals.
   - Fully optimized metadata, OpenGraph tags, and Twitter cards for SEO.

---

## 8. Conclusion & Implementation Recommendations

The architecture surveyed herein provides complete coverage of every requirement in `ORIGINAL_REQUEST.md`. The design system is fully cohesive with the existing `apps/rivun-dashboard` dark aesthetic while elevating the presentation to an Apple/Linear standard with bespoke interactive visualizers.

All required data models, protocol specifications, cryptographic algorithms, domain packs, and CLI workflows are verified against the 26 workspace crates and documented with exact byte layouts and code references.

The next step is for the implementation team to scaffold `apps/marketing-site`, write the Next.js components, install dependencies, and verify a clean `npm run build` with 0 errors.
