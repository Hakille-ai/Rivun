# Handoff Report: Milestone 2 Adversarial Challenge

## Verdict
**REQUEST_CHANGES**

---

## 1. Observation

- **Path Traversal / Zip-Slip in `DomainPackBundle::extract_to_dir`**:
  - File: `crates/rivun-store/src/bundle.rs`, lines 495–510:
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
  - `rel_path` from `.zpack` archive is joined directly with `target_dir` (`target_dir.join(rel_path)`). There is no check verifying that `out_path.starts_with(target_dir)` or rejecting relative paths containing `..` or `..\\`.

- **Missing Dependency Verification in `rivun pack install`**:
  - File: `crates/rivun-cli/src/main.rs`, lines 7690–7785:
  - `pack_install` receives `--bundle` and `--store-dir`, extracts the bundle to `store_dir/packs/<pack_id>/<version>/`, and updates `registry.json`.
  - It NEVER inspects the bundle's `pack.toml` dependencies and NEVER invokes `DomainPackDependencyResolver` to verify that required dependencies are present in `registry.json`.

- **Incomplete Transitive Dependency Resolution**:
  - File: `crates/rivun-store/src/resolver.rs`, lines 158–165:
    ```rust
    visited_branch.insert(dep.pack_id.clone());

    // Recursive sub-dependencies can be declared in entry metadata if needed
    visited_branch.remove(&dep.pack_id);
    resolved_ids.insert(dep.pack_id.clone());
    install_order.push(entry.clone());
    ```
  - `resolve_dep` processes only the direct dependencies in `dependencies: &[DomainPackDependencySpec]`. It has zero code to recurse on sub-dependencies of `entry`, making transitive resolution (A -> B -> C) impossible.

- **Policy Validator Bypass via File Naming**:
  - File: `crates/rivun-store/src/validator.rs`, lines 27, 40, 57:
    ```rust
    if lower_path.contains("policy") && (lower_path.ends_with(".toml") || lower_path.ends_with(".policy"))
    ```
  - Files are filtered solely by checking if `lower_path.contains("policy")`, `"route"`, or `"schema"`. Policy or route files declared in `pack.toml` under `[[policies]]` with names like `access.toml` or `rules.toml` are completely ignored and skipped by the validator.

- **Version Requirement Fallthrough & SemVer Caret Bug**:
  - File: `crates/rivun-store/src/resolver.rs`, lines 40–67:
    - Line 66: If `req_str` is invalid (e.g. `"INVALID_SPEC"` or `"<1.0"`), the function falls through and returns `true`.
    - Line 52: For caret `^` requirements (e.g. `^0.1.0`), `v.major == target.major && v >= target` returns `true` for version `0.2.0`, violating SemVer breaking change rules for 0.x releases.

- **Public Key Format Inconsistency in `verify_against_trusted_keys`**:
  - File: `crates/rivun-store/src/bundle.rs`, line 167:
    ```rust
    if hex::encode(bytes) == signer_key_hex
    ```
  - Compares `hex::encode(bytes)` with `signer_key_hex` (`self.signer_public_key.to_lowercase()`). If `self.signer_public_key` is base64 encoded, this comparison fails even when the public key bytes match.

---

## 2. Logic Chain

1. **Path Traversal Vulnerability**:
   - *Observation*: `DomainPackBundle::extract_to_dir` constructs `let out_path = target_dir.join(rel_path);` and writes content without path canonicalization or safety boundary checks.
   - *Reasoning*: A path string containing `../` causes `Path::join` to escape the root directory on all major operating systems.
   - *Deduction*: An attacker can build a `.zpack` bundle with `rel_path` equal to `../../../../tmp/evil` or `../system_file`, leading to arbitrary file write outside `target_dir` during `rivun pack install`.

2. **Unenforced Dependency Resolution during Installation**:
   - *Observation*: Requirement R2 mandates offline bundle verification and dependency resolution for domain pack installation. `pack_install` extracts files and adds an entry to `registry.json` without evaluating dependency specs.
   - *Reasoning*: Installing a domain pack whose dependencies are missing or incompatible will break downstream runtime execution.
   - *Deduction*: `pack_install` must validate that all non-optional dependencies declared in the pack manifest are satisfied in `store_dir/registry.json` using `DomainPackDependencyResolver` prior to extraction.

3. **Incomplete Dependency Resolver**:
   - *Observation*: `DomainPackDependencyResolver::resolve_dep` places `entry` into `install_order` without extracting sub-dependencies of `entry`.
   - *Reasoning*: A dependency graph of A -> B -> C requires resolving B's dependencies when A is installed.
   - *Deduction*: Without recursive traversal, resolution plans are incomplete and omit indirect required packs and capabilities.

4. **Policy Validator Evasion**:
   - *Observation*: `DomainPackPolicyValidator` uses string pattern matching (`lower_path.contains("policy")`) instead of parsing `pack.toml`'s declared `[[policies]]`, `[[schemas]]`, `[[routes]]` tables.
   - *Reasoning*: Custom policy filenames like `rules.toml` or `security.policy.toml` will not match `lower_path.contains("policy")` if named `rules.toml`.
   - *Deduction*: Invalid or malformed policies with custom filenames bypass validation completely, causing `res.valid` to return `true`.

---

## 3. Caveats

- CLI commands could not be executed via `run_command` in this non-interactive runner context due to terminal permission timeout.
- Verification was conducted empirically via static analysis and by creating a dedicated Rust test harness file (`crates/rivun-store/tests/adversarial_m2_tests.rs`).

---

## 4. Conclusion

Milestone 2 implementation requires changes prior to approval due to:
1. **Critical Security Vulnerability**: Zip-Slip path traversal in `DomainPackBundle::extract_to_dir`.
2. **Missing Requirement**: Dependency resolution is omitted during `rivun pack install`.
3. **Architectural Gaps**: Absence of transitive dependency resolution, policy validator file filtering bypass, version requirement fallthrough bug, and public key format mismatch.

**Verdict: REQUEST_CHANGES**

---

## 5. Verification Method

To verify these findings independently, run the adversarial test harness:

```powershell
cargo test --test adversarial_m2_tests
```

Or inspect the test file created at:
`crates/rivun-store/tests/adversarial_m2_tests.rs`

