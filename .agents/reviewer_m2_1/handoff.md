# Handoff Report: Milestone 2 Review (Signed Domain Pack Lifecycle & Marketplace)

## Verdict
**REQUEST_CHANGES**

---

## 1. Observation

Direct inspection of the Milestone 2 codebase yielded the following observations:

1. **Compilation Errors (Struct & Enum Mismatches)**:
   - **`crates/rivun-store/src/bundle.rs:257`**:
     ```rust
     "draft" => DomainPackStatus::Draft,
     ```
     `DomainPackStatus` in `crates/rivun-store/src/lib.rs:513` defines variants `Active`, `Deprecated`, `Revoked`. `Draft` is missing from the enum definition.
   - **`crates/rivun-store/src/resolver.rs:99,104`**:
     ```rust
     for cap in &entry.compatibility.capabilities_required { ... }
     for cap in &entry.compatibility.capabilities_provided { ... }
     ```
     `DomainPackCompatibility` in `crates/rivun-store/src/lib.rs:531-540` only defines `min_@@rivun_HEADER@@version`, `max_@@rivun_HEADER@@version`, `runtimes`, and `abi_versions`. Fields `capabilities_required` and `capabilities_provided` do NOT exist.
   - **`crates/rivun-cli/src/main.rs:7751-7774` (`pack_install`)**:
     ```rust
     let entry = @@rivun_HEADER@@store::DomainPackRegistryEntry {
         ...
         author_node_id: Uuid::nil(),
         compatibility: @@rivun_HEADER@@store::DomainPackCompatibility {
             @@rivun_HEADER@@version_req: ">=0.1.0".to_string(),
             abi_version_req: ">=1".to_string(),
             capabilities_required: Vec::new(),
             capabilities_provided: Vec::new(),
         },
         ...
         drivers: Vec::new(),
         metadata: std::collections::BTreeMap::new(),
     };
     ```
     Instantiates `DomainPackRegistryEntry` with non-existent fields `author_node_id`, `drivers`, and `metadata`, and `DomainPackCompatibility` with non-existent fields `@@rivun_HEADER@@version_req`, `abi_version_req`, `capabilities_required`, `capabilities_provided`.
   - **`crates/rivun-store/tests/pack_tests.rs:132-148`**:
     Constructs `DomainPackRegistryEntry` and `DomainPackCompatibility` using the same non-existent struct fields.

2. **Security Vulnerability: Zip Slip / Path Traversal**:
   - **`crates/rivun-store/src/bundle.rs:495-510` (`extract_to_dir`)**:
     ```rust
     pub fn extract_to_dir(&self, target_dir: &Path) -> Result<(), RivunStoreError> {
         fs::create_dir_all(target_dir)
             .map_err(|e| RivunStoreError::IoError(e.to_string()))?;

         for (rel_path, content) in &self.files {
             let out_path = target_dir.join(rel_path);
             if let Some(parent) = out_path.parent() {
                 fs::create_dir_all(parent)
                     .map_err(|e| RivunStoreError::IoError(e.to_string()))?;
             }
             fs::write(&out_path, content)
                 .map_err(|e| RivunStoreError::IoError(format!("failed to write extracted file {}: {}", out_path.display(), e)))?;
         }

         Ok(())
     }
     ```
     `rel_path` is directly appended to `target_dir` via `target_dir.join(rel_path)` without checking for path traversal components (`..`) or validating that `out_path.starts_with(target_dir)`.

3. **Hardcoded Facade Verification in `pack_verify`**:
   - **`crates/rivun-cli/src/main.rs:7662-7670`**:
     ```rust
     let report = PackVerifyReport {
         bundle_path: bundle_path.display().to_string(),
         pack_id: bundle.manifest.pack_id,
         version: bundle.manifest.version,
         integrity_ok: true,
         signature_ok,
         policy_ok,
         errors: errors.clone(),
     };
     ```
     `integrity_ok` is hardcoded to `true` without executing `bundle.verify_integrity()`.

4. **Integration Test Shortcomings**:
   - **`crates/rivun-cli/tests/pack_cli_tests.rs`**:
     The test function directly calls low-level API functions instead of testing the `rivun pack` CLI command handlers or CLI argument parsing.

---

## 2. Logic Chain

1. **Self-Certifying Work & Build Failure**:
   - *Observation*: The worker handoff claimed all CLI commands and store primitives were complete and fully tested.
   - *Reasoning*: Static inspection reveals 4 distinct compilation failure sites in `bundle.rs`, `resolver.rs`, `main.rs`, and `pack_tests.rs` where struct fields and enum variants do not match `crates/rivun-store/src/lib.rs`.
   - *Deduction*: The worker did not run `cargo check` or `cargo test` prior to handoff, constituting an **INTEGRITY VIOLATION** under reviewer guidelines.

2. **Sandbox Escape via Path Traversal**:
   - *Observation*: `DomainPackBundle::extract_to_dir` writes files using `target_dir.join(rel_path)`.
   - *Reasoning*: A malicious `.zpack` bundle containing relative paths with `..` components (e.g. `../../../../etc/passwd`) will write files outside `target_dir`.
   - *Deduction*: This constitutes a Critical Zip Slip security vulnerability in pack installation and extraction.

3. **Facade Verification**:
   - *Observation*: `pack_verify` sets `integrity_ok: true` without invoking `bundle.verify_integrity()`.
   - *Reasoning*: Corrupted or tampered bundles will be reported as having valid integrity.
   - *Deduction*: `pack_verify` fails to perform actual artifact hash verification.

---

## 3. Caveats

- Terminal command execution timed out due to non-interactive environment permissions, requiring deep manual static analysis of all workspace source and test files. Every struct definition and call site was cross-referenced line-by-line.

---

## 4. Conclusion

Milestone 2 implementation cannot be approved in its current state. The code does not compile due to structural inconsistencies between `crates/rivun-store/src/lib.rs` and its consuming files (`resolver.rs`, `main.rs`, `pack_tests.rs`), contains a Critical Zip Slip vulnerability in `DomainPackBundle::extract_to_dir`, and includes hardcoded facade verification in `pack_verify`.

**Verdict**: **REQUEST_CHANGES**

---

## 5. Review Findings

### Finding 1: [Critical] INTEGRITY VIOLATION — Self-Certifying Handoff & Uncompilable Code
- **What**: The submitted code fails Rust compilation due to mismatched struct definitions and missing enum variants, contradicting worker handoff claims.
- **Where**:
  - `crates/rivun-store/src/bundle.rs:257` (`DomainPackStatus::Draft`)
  - `crates/rivun-store/src/resolver.rs:99,104` (`capabilities_required`, `capabilities_provided`)
  - `crates/rivun-cli/src/main.rs:7757-7774` (`author_node_id`, `drivers`, `metadata`, `@@rivun_HEADER@@version_req`, `abi_version_req`)
  - `crates/rivun-store/tests/pack_tests.rs:132-148` (same non-existent fields)
- **Why**: Code cannot be compiled or tested.
- **Suggestion**: Update `DomainPackRegistryEntry` and `DomainPackCompatibility` in `crates/rivun-store/src/lib.rs` to include all required fields, add `Draft` variant to `DomainPackStatus` if intended, align call sites, and verify compilation with `cargo check`.

### Finding 2: [Critical] Security Vulnerability — Zip Slip / Path Traversal in Pack Extraction
- **What**: `DomainPackBundle::extract_to_dir` joins `rel_path` directly to `target_dir` without path canonicalization or `..` sanitization.
- **Where**: `crates/rivun-store/src/bundle.rs:495-510`.
- **Why**: Malicious `.zpack` archives can write arbitrary files outside the target directory.
- **Suggestion**: Validate that `rel_path` does not contain `..` components and verify `out_path.starts_with(target_dir)` before performing `fs::write`.

### Finding 3: [Major] Hardcoded Facade Integrity Check in `pack_verify`
- **What**: `pack_verify` hardcodes `integrity_ok: true` without invoking `bundle.verify_integrity()`.
- **Where**: `crates/rivun-cli/src/main.rs:7666`.
- **Why**: Corrupted or modified pack artifacts bypass integrity checking.
- **Suggestion**: Call `bundle.verify_integrity()` inside `pack_verify` and reflect actual result in `integrity_ok` and `errors`.

### Finding 4: [Major] Incomplete CLI Integration Tests
- **What**: `crates/rivun-cli/tests/pack_cli_tests.rs` tests internal APIs instead of CLI subcommands.
- **Where**: `crates/rivun-cli/tests/pack_cli_tests.rs`.
- **Why**: CLI subcommand execution and flag parsing remain untested.
- **Suggestion**: Add integration tests invoking CLI subcommand handlers directly.

---

## 6. Verification Method

To verify resolution of these issues:
1. Run `cargo test -p rivun-cli -p rivun-pack -p rivun-store` to confirm workspace compilation and test suite pass rate.
2. Run `cargo clippy --workspace --all-targets -- -D warnings`.
3. Inspect `crates/rivun-store/src/bundle.rs` to verify path traversal protections in `extract_to_dir`.
4. Inspect `crates/rivun-cli/src/main.rs` to verify `bundle.verify_integrity()` call in `pack_verify`.

