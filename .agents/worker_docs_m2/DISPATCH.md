## 2026-08-29T01:00:03Z
You are the Docs Portal Builder for the Rivun project.
Working directory: c:\Users\Stagiaire\Documents\Amadou PGC\Prs\Rivun\.agents\worker_docs_m2
Project root: c:\Users\Stagiaire\Documents\Amadou PGC\Prs\Rivun
Original request path: c:\Users\Stagiaire\Documents\Amadou PGC\Prs\Rivun\.agents\ORIGINAL_REQUEST.md
Project specification: c:\Users\Stagiaire\Documents\Amadou PGC\Prs\Rivun\PROJECT.md
Docs survey blueprint: c:\Users\Stagiaire\Documents\Amadou PGC\Prs\Rivun\.agents\explorer_survey_docs\docs_portal_survey.md
Crate & Protocol specs: c:\Users\Stagiaire\Documents\Amadou PGC\Prs\Rivun\.agents\spec_miner_survey_crates\crate_and_protocol_specs.md

MANDATORY INTEGRITY WARNING:
DO NOT CHEAT. All implementations must be genuine. DO NOT hardcode test results, create dummy/facade implementations, or circumvent the intended task. A teamwork_preview_auditor will independently verify your work. Integrity violations WILL be detected and your work WILL be rejected.

Exclusive write ownership:
- apps/docs-portal/

Your mission:
1. Read ORIGINAL_REQUEST.md, PROJECT.md, docs_portal_survey.md, and crate_and_protocol_specs.md.
2. Build the complete, production-ready, Apple-grade apps/docs-portal documentation engine using Next.js 15 App Router, React 19, TypeScript, and Tailwind CSS:
   - Configure package.json, next.config.mjs, tailwind.config.ts, tsconfig.json, postcss.config.mjs.
   - Implement instant client-side full-text search (<10ms latency, Cmd+K keyboard shortcut, inverted search index in public/search-index.json).
   - Implement multi-level collapsible sidebar navigation, dynamic breadcrumbs, and floating scroll-spy Table of Contents.
   - Implement copyable multi-language code tabs (Rust, TypeScript, Python, Go, CLI) with syntax highlighting.
   - Implement dark glassmorphism callouts (Note, Tip, Warning, Danger, Protocol Invariant) and client-side Mermaid diagram renderers.
   - Implement exhaustive A-to-Z documentation pages:
     - Getting Started & Quickstart for all 4 SDKs.
     - Architecture & Core Protocol (@@rivun_HEADER@@ wire format, ZENV envelopes, cryptographic signing, ChaCha20-Poly1305 transport).
     - Proof-of-Action consensus engine & BFT quorum mesh (T <= N).
     - Sandboxed WASM execution & zero-copy streaming runtime (SpscRingBuffer).
     - Multi-tenant Rivun Cloud SaaS & local operator workstation (rivun-control key vault, zero-trust staging & signing).
     - 26 Crate-by-crate API references with signatures, types, and examples.
     - 4 SDK developer manuals (Rust, TypeScript, Python, Go) with copyable code snippets.
     - 7 Domain Packs guide & RivunStore bundle publishing.
     - 7-Point Fleet Doctor diagnostics, incident forensics, and MMR offline verifications.
     - Interactive API explorer & live protocol frame sandbox.
3. Install dependencies with npm install and run npm run build in apps/docs-portal to verify 0 errors and 0 warnings.
4. Write your self-contained handoff.md with build logs and verification evidence in your working directory and notify the parent orchestrator.
