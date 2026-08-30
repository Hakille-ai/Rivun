# Progress: Rivun Docs Portal (M2)

- **Status**: COMPLETED
- **Last visited**: 2026-08-29T01:18:45Z

## Checklist
- [x] Initialized DISPATCH.md and BRIEFING.md
- [x] Read survey blueprint, specs, and project docs
- [x] Configured `apps/docs-portal/package.json`, `tsconfig.json`, `next.config.mjs`, `tailwind.config.ts`, `postcss.config.mjs`
- [x] Built core UI components: Sidebar, Header, Breadcrumbs, Table of Contents, Search Modal (Cmd+K), CodeBlock/Tabs, Callouts, MermaidRenderer, ProtocolFrameSandbox, ApiExplorer, PoaQuorumSimulator, PactVisualizer
- [x] Built complete docs routing and page hierarchy covering:
  - Overview / Getting Started / Quickstart (Rust, TS, Python, Go)
  - Architecture & Core Protocol (Wire format `0x5A41505F`, ZENV envelopes, ChaCha20-Poly1305, Ed25519)
  - Proof-of-Action consensus & BFT quorum mesh
  - Sandboxed WASM execution & zero-copy streaming (`SpscRingBuffer`)
  - Cloud SaaS & Operator workstation (`rivun-control`)
  - 26 Crate API references (all 26 crates from specs)
  - 4 SDK manuals (Rust, TS, Python, Go, Conformance Matrix)
  - 7 Domain Packs & RivunStore publishing
  - 7-Point Fleet Doctor diagnostics & MMR forensics
  - Interactive API Explorer & Live Protocol Frame Sandboxes
- [x] Generated comprehensive `public/search-index.json`
- [x] Ran `npm install`, `npm run typecheck`, and `npm run build` in `apps/docs-portal` (87/87 static routes, 0 errors, 0 warnings)
- [x] Final verification and `handoff.md` written
