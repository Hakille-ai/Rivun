## 2026-08-14T00:16:57Z
You are Worker M2 Fix responsible for implementing Milestone 2 remediation.
Working Directory: c:\Users\Stagiaire\Documents\Amadou PGC\Prs\rivun\.agents\teamwork_preview_worker_m2_fix
Read ORIGINAL_REQUEST.md at c:\Users\Stagiaire\Documents\Amadou PGC\Prs\rivun\.agents\ORIGINAL_REQUEST.md
Read PROJECT.md at c:\Users\Stagiaire\Documents\Amadou PGC\Prs\rivun\.agents\orchestrator\PROJECT.md
Read Explorer Fix Roadmap at c:\Users\Stagiaire\Documents\Amadou PGC\Prs\rivun\.agents\explorer_m2_1\handoff.md

MANDATORY INTEGRITY WARNING:
DO NOT CHEAT. All implementations must be genuine. DO NOT hardcode test results, create dummy/facade implementations, or circumvent the intended task. A teamwork_preview_auditor will independently verify your work. Integrity violations WILL be detected and your work WILL be rejected.

Execute the 6-step remediation plan detailed in Explorer's handoff:
1. Align Structs & Enums in `crates/rivun-store/src/lib.rs` (`DomainPackStatus::Draft`, `DomainPackCompatibility`, `DomainPackArtifact`, `DomainPackRegistryEntry`). Update call sites in `bundle.rs`, `resolver.rs`, `main.rs`, and tests.
2. Secure `extract_to_dir` against Zip Slip in `crates/rivun-store/src/bundle.rs` (sanitize components, check prefix, canonicalize target/parent). Update `decode_bytes`.
3. Fix public key Base64/hex matching in `verify_against_trusted_keys`.
4. Fix resolver SemVer parsing, caret 0.x rules, unparsed spec fallthrough, and recursive transitive dependency resolution in `crates/rivun-store/src/resolver.rs`.
5. Enhance `DomainPackPolicyValidator` (parse `pack.toml`'s declared `[[policies]]`) and `audit_pack_dir` (check pack `status` for revoked/deprecated) in `crates/rivun-store`.
6. Fix CLI command handlers in `crates/rivun-cli/src/main.rs`: execute `bundle.verify_integrity()` in `pack_verify` and check signature file existence; invoke `DomainPackDependencyResolver` in `pack_install`.

Run verification commands:
- `cargo test -p rivun-store -p rivun-pack -p rivun-cli`
- `cargo clippy --workspace --all-targets -- -D warnings`

Write handoff.md in your working directory summarizing your changes, build/test results, and verification commands. Notify parent when finished.

