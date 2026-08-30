# Orchestration Plan: Rivun Web Platforms

## Objective
Build two production-ready, Apple-grade web platforms:
1. **Marketing Showcase Platform** (`apps/marketing-site`)
2. **Developer Documentation Portal** (`apps/docs-portal`)
3. **E2E Testing Track** (opaque-box multi-tier test suite + verification)

## Execution Phases
### Phase 0: Survey & Codebase Inventory (Active)
- Spawn 3 parallel Explorers:
  - `spec_miner_crates`: Extract protocol wire format, cryptographic primitives, 26 crates, 4 SDKs, 7 domain packs, Rivun Cloud & CLI specs.
  - `explorer_marketing`: Assess `apps/marketing-site` directory, dependencies, framework setup (Next.js/Vite/Tailwind), existing components, visual design system, particle visualizers, pricing calculators, interactive sandboxes.
  - `explorer_docs`: Assess `apps/docs-portal` directory, documentation engine, search integration, syntax highlighting, multi-language tabs, crate reference structure, playground.
- Synthesize findings into `PROJECT.md` with full Architecture, Feature Inventory, Milestones, and Interface Contracts.

### Phase 1: E2E Test Suite & Infrastructure Setup
- Spawn E2E Testing Orchestrator / Test Writers for opaque-box requirement tests (Tiers 1-4).
- Write `TEST_INFRA.md` and publish `TEST_READY.md`.

### Phase 2: Implementation Track
- Sub-Orchestrator M1: Marketing Showcase Core & Apple-Grade UI / Canvas Visualizers / Hero / Features / Cloud SaaS / Pricing / Sandboxes.
- Sub-Orchestrator M2: Docs Portal Engine / Sidebar / Search / 26 Crate References / 4 SDK Manuals / 7 Domain Packs / Diagnostics / Interactive Frame Explorer.
- Sub-Orchestrator M3: Integration, Build & Polish (0 errors, 0 warnings, verified routing, responsive layouts, assets).

### Phase 3: Final E2E Test Pass & Adversarial Hardening
- Tier 1-4 verification across both applications.
- Tier 5 Adversarial Coverage Hardening (Challengers + Reviewers + Auditors).
- Final Gate & Verification Sign-off.
