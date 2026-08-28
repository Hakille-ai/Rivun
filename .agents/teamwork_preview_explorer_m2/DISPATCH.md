## 2026-08-14T02:03:09Z
You are teamwork_preview_explorer_m2 operating in working directory: c:\Users\Stagiaire\Documents\Amadou PGC\Prs\rivun\.agents\teamwork_preview_explorer_m2.
Read ORIGINAL_REQUEST.md at c:\Users\Stagiaire\Documents\Amadou PGC\Prs\rivun\.agents\ORIGINAL_REQUEST.md, PROJECT.md at c:\Users\Stagiaire\Documents\Amadou PGC\Prs\rivun\.agents\orchestrator\PROJECT.md, and survey report at c:\Users\Stagiaire\Documents\Amadou PGC\Prs\rivun\.agents\teamwork_preview_explorer_survey_1\handoff.md.

Objective: Formulate the detailed technical blueprint for Milestone 2 (R2: Signed Domain Pack Lifecycle & Marketplace).
Investigate and design:
1. Complete `rivun pack` CLI subcommands in `crates/rivun-cli` (and `crates/rivun-pack`): `init`, `build`, `sign`, `verify`, `install`, and `audit`.
   - `rivun pack init`: scaffold new domain pack template.
   - `rivun pack build`: compile pack into `.zpack` bundle.
   - `rivun pack sign`: sign bundle with private key into `.zpack.sig`.
   - `rivun pack verify`: check signature, manifest integrity, policy rules.
   - `rivun pack install`: validate offline bundle, verify signatures & dependencies, copy to pack store directory.
   - `rivun pack audit`: security audit of manifest capabilities, permissions, route policies.
2. `rivun-store` Registry Integration:
   - Offline bundle verification without network access.
   - Dependency graph resolution (resolving version constraints and required capabilities across domain packs).
   - Policy and route validation.

Write your concrete implementation blueprint (file paths, CLI struct args, data types, method signatures, test strategy) to `c:\Users\Stagiaire\Documents\Amadou PGC\Prs\rivun\.agents\teamwork_preview_explorer_m2\handoff.md`. Notify orchestrator via send_message when complete.

