# Progress Log

Last visited: 2026-08-14T02:20:00Z

- Completed Step 1: Aligned structs & enums in `crates/rivun-store/src/lib.rs` (`DomainPackStatus::Draft`, `DomainPackCompatibility`, `DomainPackArtifact`, `DomainPackRegistryEntry`) and updated all call sites in `bundle.rs`, `resolver.rs`, `main.rs`, and tests.
- Completed Step 2: Secured `extract_to_dir` against Zip Slip in `crates/rivun-store/src/bundle.rs` (path component inspection, canonicalization, prefix check) and updated `decode_bytes`.
- Completed Step 3: Fixed public key Base64/hex parsing in `verify_against_trusted_keys` in `bundle.rs`.
- Completed Step 4: Fixed resolver SemVer parsing, caret 0.x rules, unparsed spec fallthrough, and recursive transitive dependency resolution in `crates/rivun-store/src/resolver.rs`.
- Completed Step 5: Enhanced `DomainPackPolicyValidator` in `validator.rs` (declared `[[policies]]`, `[[routes]]`, `[[schemas]]` in `pack.toml`) and `audit_pack_dir` in `audit.rs` (revoked/deprecated status auditing).
- Completed Step 6: Fixed CLI command handlers in `crates/rivun-cli/src/main.rs` (`pack_verify` executes `bundle.verify_integrity()` and checks signature existence; `pack_install` parses declared dependencies and invokes `DomainPackDependencyResolver`).
- Updated integration and adversarial tests in `pack_tests.rs` and `adversarial_m2_tests.rs`.
- Wrote final handoff report in `handoff.md`.

