# Handoff Report: Milestone 2 Remediation & Actionable Fix Strategy

## 1. Observation

A detailed investigation of the Milestone 2 codebase (`crates/zap-store`, `crates/zap-pack`, `crates/zap-cli`) and synthesis of the four gate reviewer/challenger reports (`reviewer_m2_1`, `reviewer_m2_2`, `challenger_m2_1`, `challenger_m2_2`) revealed 8 distinct, verified defects across compilation, security, CLI execution, dependency resolution, policy validation, and auditing.

### 1. Workspace Compilation Failures (Struct & Enum Field Mismatches)
- **`DomainPackStatus` Enum**:
  - File: `crates/zap-store/src/lib.rs:513-518` defines variants `Active`, `Deprecated`, `Revoked`.
  - Usage: `crates/zap-store/src/bundle.rs:257` attempts to match `"draft" => DomainPackStatus::Draft`. Variant `Draft` is missing.
- **`DomainPackCompatibility` Struct**:
  - File: `crates/zap-store/src/lib.rs:530-540` defines `min_zap_version`, `max_zap_version`, `runtimes`, `abi_versions`.
  - Usage: `crates/zap-store/src/resolver.rs:99,104` queries `capabilities_required` and `capabilities_provided`.
  - Usage: `crates/zap-cli/src/main.rs:7758-7763` and `crates/zap-store/tests/pack_tests.rs:132-137` attempt to instantiate `zap_version_req`, `abi_version_req`, `capabilities_required`, `capabilities_provided`.
- **`DomainPackRegistryEntry` Struct**:
  - File: `crates/zap-store/src/lib.rs:553-576` defines `id`, `name`, `version`, `status`, `risk`, `description`, `deprecated_reason`, `revoked_reason`, `compatibility`, `manifest`, `archive`, `policies`, `schemas`, `labels`.
  - Usage: `crates/zap-cli/src/main.rs:7751-7775`, `crates/zap-store/tests/pack_tests.rs:125-149`, and `crates/zap-store/tests/adversarial_m2_tests.rs:103-122` attempt to construct `author_node_id`, `drivers`, `metadata`.
- **`DomainPackArtifact` Struct**:
  - File: `crates/zap-store/src/lib.rs:542-550` defines `path`, `hash`, `content_type`, `size_bytes`.
  - Usage: `crates/zap-cli/src/main.rs:7764-7769` and `crates/zap-store/tests/pack_tests.rs:138-143` instantiate with `relative_path` and `sha256_hex`.

### 2. Critical Security Vulnerability: Zip Slip / Path Traversal in Pack Extraction
- File: `crates/zap-store/src/bundle.rs:495-510` (`extract_to_dir`)
  ```rust
  for (rel_path, content) in &self.files {
      let out_path = target_dir.join(rel_path);
      if let Some(parent) = out_path.parent() {
          fs::create_dir_all(parent)...;
      }
      fs::write(&out_path, content)...;
  }
  ```
  `rel_path` from `.zpack` is joined directly to `target_dir` via `target_dir.join(rel_path)` without checking for relative directory traversal components (`..`) or checking `out_path.starts_with(target_dir)`.

### 3. Hardcoded Facade Check & Missing Signature Error in `zap pack verify`
- File: `crates/zap-cli/src/main.rs:7617-7688` (`pack_verify`)
  - Line 7666 hardcodes `integrity_ok: true` without invoking `bundle.verify_integrity()`.
  - If `--public-key` or `--signature` is specified but `sig_path.exists()` is false, `errors` is left empty and the command returns `Ok(())` (SUCCESS), silently bypassing signature verification.

### 4. Unenforced Dependency Resolution in `zap pack install`
- File: `crates/zap-cli/src/main.rs:7690-7785` (`pack_install`)
  - Extracts bundle and updates `registry.json` without parsing declared dependencies in `pack.toml` or invoking `DomainPackDependencyResolver`.

### 5. Dependency Resolver Bugs (Transitive Traversal, SemVer Caret & Unparsed Fallthrough)
- File: `crates/zap-store/src/resolver.rs:40-67` (`matches_version_req`):
  - Line 66: Unrecognized requirement strings fall through and return `true`.
  - Lines 50-53: Caret requirement `^0.1.0` matching `0.2.0` returns `true`, violating SemVer breaking change rules for `0.x` releases.
- File: `crates/zap-store/src/resolver.rs:120-166` (`resolve_dep`):
  - Post-resolution code does not recurse on sub-dependencies of resolved entries, making transitive resolution (A -> B -> C) fail.

### 6. Policy Validator Filename Filter Evasion
- File: `crates/zap-store/src/validator.rs:27,40,57,98,111,128`:
  - Validates policy files only if `lower_path.contains("policy")`. Policy files declared under `[[policies]]` in `pack.toml` with custom names like `custom_rules.toml` are completely skipped.

### 7. Security Audit Status Discrepancy
- File: `crates/zap-store/src/audit.rs:25-95` (`audit_pack_dir`):
  - `audit_bundle` evaluates `status = "revoked"` / `"deprecated"` and sets `highest_risk = Critical` for revoked status, but `audit_pack_dir` completely ignores the `status` field in `pack.toml`.

### 8. Base64 / Hex Public Key Format Mismatch in `verify_against_trusted_keys`
- File: `crates/zap-store/src/bundle.rs:156-177`:
  - `signer_key_hex = self.signer_public_key.to_lowercase()` assumes `signer_public_key` is hex encoded. If `signer_public_key` is Base64 encoded, key matching fails even when public key bytes are identical.

---

## 2. Logic Chain

1. **Self-Certifying Work & Build Failure**:
   - Mismatches between struct definitions in `crates/zap-store/src/lib.rs` and call sites in `resolver.rs`, `main.rs`, and test files prevent `cargo check` and `cargo test` from succeeding.
   - *Deduction*: Struct definitions in `lib.rs` must be expanded with missing fields and serde annotations to restore full workspace compilation.

2. **Zip Slip Exploitation**:
   - An attacker crafting a `.zpack` archive with paths like `../../../../tmp/malicious.sh` causes `target_dir.join(rel_path)` to escape `target_dir` during extraction (`zap pack install`).
   - *Deduction*: Strict component inspection rejecting `..` / root paths, combined with canonicalized parent path prefix checks (`canonical_parent.starts_with(canonical_target)`), is mandatory.

3. **Incomplete Command Implementation & Facade Check**:
   - `pack_verify` reporting `integrity_ok: true` without invoking `bundle.verify_integrity()` allows corrupt bundles to pass.
   - `pack_verify` ignoring missing `.sig` files allows unverified bundles to pass.
   - *Deduction*: `pack_verify` must execute `bundle.verify_integrity()` and record errors when signature files are expected but missing.

4. **Transitive Dependency Graph Resolution**:
   - If pack A depends on pack B, and B depends on C, installing A without checking C leads to broken runtime states.
   - *Deduction*: `resolve_dep` must recursively process `entry.dependencies` using depth-first post-order traversal to build complete installation plans.

---

## 3. Caveats

- Investigation was performed via read-only static analysis and cross-referencing all 4 failure reports, workspace source code, and test harnesses (`adversarial_m2_tests.rs`, `pack_tests.rs`).
- No source code files were modified during this investigation.

---

## 4. Conclusion & Actionable Fix Roadmap for Worker

The worker must implement the following step-by-step fix strategy:

### STEP 1: Align Structs & Enums in `crates/zap-store/src/lib.rs`
1. Update `DomainPackStatus`:
   ```rust
   #[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
   #[serde(rename_all = "snake_case")]
   pub enum DomainPackStatus {
       #[default]
       Active,
       Deprecated,
       Revoked,
       Draft,
   }
   ```
2. Update `DomainPackCompatibility`:
   ```rust
   #[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
   pub struct DomainPackCompatibility {
       #[serde(default, skip_serializing_if = "Option::is_none")]
       pub min_zap_version: Option<String>,
       #[serde(default, skip_serializing_if = "Option::is_none")]
       pub max_zap_version: Option<String>,
       #[serde(default, skip_serializing_if = "Vec::is_empty")]
       pub runtimes: Vec<String>,
       #[serde(default, skip_serializing_if = "Vec::is_empty")]
       pub abi_versions: Vec<u16>,
       #[serde(default, skip_serializing_if = "String::is_empty")]
       pub zap_version_req: String,
       #[serde(default, skip_serializing_if = "String::is_empty")]
       pub abi_version_req: String,
       #[serde(default, skip_serializing_if = "Vec::is_empty")]
       pub capabilities_required: Vec<String>,
       #[serde(default, skip_serializing_if = "Vec::is_empty")]
       pub capabilities_provided: Vec<String>,
   }
   ```
3. Update `DomainPackArtifact`:
   ```rust
   #[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
   pub struct DomainPackArtifact {
       #[serde(default, alias = "relative_path")]
       pub path: String,
       #[serde(default, alias = "sha256_hex")]
       pub hash: String,
       #[serde(default, skip_serializing_if = "Option::is_none")]
       pub content_type: Option<String>,
       #[serde(default, skip_serializing_if = "Option::is_none")]
       pub size_bytes: Option<u64>,
       #[serde(default, skip_serializing_if = "Option::is_none")]
       pub relative_path: Option<String>,
       #[serde(default, skip_serializing_if = "Option::is_none")]
       pub sha256_hex: Option<String>,
   }
   ```
4. Update `DomainPackRegistryEntry`:
   ```rust
   #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
   pub struct DomainPackRegistryEntry {
       pub id: String,
       pub name: String,
       pub version: String,
       pub status: DomainPackStatus,
       pub risk: DomainPackRisk,
       #[serde(default, skip_serializing_if = "Option::is_none")]
       pub description: Option<String>,
       #[serde(default, skip_serializing_if = "Option::is_none")]
       pub deprecated_reason: Option<String>,
       #[serde(default, skip_serializing_if = "Option::is_none")]
       pub revoked_reason: Option<String>,
       #[serde(default)]
       pub author_node_id: Uuid,
       #[serde(default)]
       pub compatibility: DomainPackCompatibility,
       pub manifest: DomainPackArtifact,
       #[serde(default, skip_serializing_if = "Option::is_none")]
       pub archive: Option<DomainPackArtifact>,
       #[serde(default, skip_serializing_if = "Vec::is_empty")]
       pub policies: Vec<DomainPackArtifact>,
       #[serde(default, skip_serializing_if = "Vec::is_empty")]
       pub schemas: Vec<DomainPackArtifact>,
       #[serde(default, skip_serializing_if = "Vec::is_empty")]
       pub drivers: Vec<String>,
       #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
       pub metadata: BTreeMap<String, String>,
       #[serde(default, skip_serializing_if = "Vec::is_empty")]
       pub dependencies: Vec<DomainPackDependencySpec>,
       #[serde(default, skip_serializing_if = "Vec::is_empty")]
       pub labels: Vec<String>,
   }
   ```

### STEP 2: Secure `extract_to_dir` Against Zip Slip in `crates/zap-store/src/bundle.rs`
1. Sanitize `rel_path` components to reject `ParentDir`, `RootDir`, and `Prefix`.
2. Canonicalize target directory and verify `canonical_parent.starts_with(&canonical_target)` before writing content:
   ```rust
   pub fn extract_to_dir(&self, target_dir: &Path) -> Result<(), ZapStoreError> {
       let canonical_target = target_dir
           .canonicalize()
           .or_else(|_| {
               fs::create_dir_all(target_dir)?;
               target_dir.canonicalize()
           })
           .map_err(|e| ZapStoreError::IoError(e.to_string()))?;

       for (rel_path, content) in &self.files {
           let rel_path_buf = PathBuf::from(rel_path);
           for component in rel_path_buf.components() {
               match component {
                   std::path::Component::ParentDir => {
                       return Err(ZapStoreError::InvalidDomainPackArtifactPath(format!(
                           "path traversal detected in artifact path: {}",
                           rel_path
                       )));
                   }
                   std::path::Component::RootDir | std::path::Component::Prefix(_) => {
                       return Err(ZapStoreError::InvalidDomainPackArtifactPath(format!(
                           "absolute path detected in artifact path: {}",
                           rel_path
                       )));
                   }
                   _ => {}
               }
           }

           let out_path = target_dir.join(&rel_path_buf);
           if let Some(parent) = out_path.parent() {
               fs::create_dir_all(parent)
                   .map_err(|e| ZapStoreError::IoError(e.to_string()))?;
           }

           let canonical_parent = out_path
               .parent()
               .ok_or_else(|| ZapStoreError::InvalidDomainPackArtifactPath(rel_path.clone()))?
               .canonicalize()
               .map_err(|e| ZapStoreError::IoError(e.to_string()))?;

           if !canonical_parent.starts_with(&canonical_target) {
               return Err(ZapStoreError::InvalidDomainPackArtifactPath(format!(
                   "path traversal outside target directory: {}",
                   rel_path
               )));
           }

           fs::write(&out_path, content)
               .map_err(|e| ZapStoreError::IoError(format!("failed to write extracted file {}: {}", out_path.display(), e)))?;
       }

       Ok(())
   }
   ```
3. Update `DomainPackBundle::decode_bytes` to reject paths containing `..` or absolute prefixes.

### STEP 3: Fix `verify_against_trusted_keys` Public Key Parsing
In `crates/zap-store/src/bundle.rs`:
```rust
let signer_key_bytes = parse_public_key_str(&self.signer_public_key).ok();
for trusted in trusted_public_keys {
    let cleaned = trusted.trim().to_lowercase();
    if cleaned == self.signer_public_key.to_lowercase() {
        matched = true;
        break;
    }
    if let (Some(sig_bytes), Ok(trust_bytes)) = (signer_key_bytes, parse_public_key_str(&cleaned)) {
        if sig_bytes == trust_bytes {
            matched = true;
            break;
        }
    }
}
```

### STEP 4: Fix Resolver SemVer, Fallthrough & Transitive Traversal in `crates/zap-store/src/resolver.rs`
1. Update `matches_version_req`:
   - Caret requirements: handle `0.0.x`, `0.x.y`, `x.y.z`.
   - Bare version requirement: exact match `v == target`.
   - Return `false` on unparseable requirement strings.
2. In `resolve_dep`:
   ```rust
   visited_branch.insert(dep.pack_id.clone());

   for sub_dep in &entry.dependencies {
       self.resolve_dep(sub_dep, visited_branch, resolved_ids, install_order)?;
   }

   visited_branch.remove(&dep.pack_id);
   resolved_ids.insert(dep.pack_id.clone());
   install_order.push(entry.clone());
   ```

### STEP 5: Enhance Policy Validator & Audit in `crates/zap-store/`
1. In `validator.rs`: Parse `pack.toml`'s declared `[[policies]]`, `[[routes]]`, `[[schemas]]` tables alongside path substring checks.
2. In `audit.rs` (`audit_pack_dir`): Check `pack_toml.get("status")`. If `"deprecated"`, add medium issue and elevate risk; if `"revoked"`, add critical issue and set `highest_risk = DomainPackRisk::Critical`.

### STEP 6: Fix CLI Handlers in `crates/zap-cli/src/main.rs`
1. `pack_verify`:
   - Invoke `bundle.verify_integrity()` and set `report.integrity_ok` accordingly.
   - Record signature errors when signature files are expected but missing.
2. `pack_install`:
   - Extract declared dependencies from bundle `pack.toml`.
   - Resolve dependencies via `DomainPackDependencyResolver::new(&registry).resolve(...)` before writing files or updating registry.
   - Set `entry.dependencies` when creating `DomainPackRegistryEntry`.

---

## 5. Verification Method

To independently verify these fixes:
1. **Compilation Check**:
   ```powershell
   cargo check --workspace --all-targets
   ```
2. **Unit & Integration Test Suite**:
   ```powershell
   cargo test -p zap-store -p zap-pack -p zap-cli
   ```
3. **Adversarial Security Verification**:
   ```powershell
   cargo test --test adversarial_m2_tests
   ```
4. **Clippy Quality Check**:
   ```powershell
   cargo clippy --workspace --all-targets -- -D warnings
   ```
