# Handoff Report: Milestone 2 (R2: Signed Domain Pack Lifecycle & Marketplace)

## 1. Observation

### Existing Codebase Inspection
1. **`crates/rivun-cli` Domain Pack Commands (`crates/rivun-cli/src/main.rs`)**:
   - Lines 1065–1088: `PackCommand` enum currently only defines 3 subcommands: `Validate`, `Inspect`, and `List`.
   - Lines 7315–7630: Pack handling logic includes `DomainPackManifest`, `DomainPackCapability`, `DomainPackPathRef`, `validate_domain_pack`, `inspect_domain_pack`, `list_domain_packs`, and path/capability validation helpers.
   - Missing subcommands required for R2: `init`, `build`, `sign`, `verify`, `install`, and `audit`.

2. **`rivun-store` Domain Pack Data Structures & Registry (`crates/rivun-store/src/lib.rs`)**:
   - Lines 485–568: `DomainPackStatus`, `DomainPackRisk`, `DomainPackCompatibility`, `DomainPackArtifact`, `DomainPackRegistryEntry`, and `DomainPackRegistry`.
   - Lines 2066–2260: `DomainPackRegistry` supports entry addition, validation, Ed25519 signing over domain `rivun-DOMAIN-PACK-REGISTRY-v1`, and signature verification.
   - Missing domain pack features in `rivun-store`:
     - `.zpack` offline bundle reader/writer and payload checksum validation (`DomainPackBundle`, `DomainPackBundleManifest`).
     - Detached bundle signature structure (`DomainPackBundleSignature`) and signing/verification API over domain `rivun-DOMAIN-PACK-BUNDLE-v1`.
     - Multi-pack dependency resolution engine (`DomainPackDependencyResolver`) covering version range requirements and capability constraint resolution.
     - Offline store directory layout, atomic extraction, and registry index update.
     - Policy and route static validation engine (`DomainPackPolicyValidator`).

3. **Dependencies & Policy Crates**:
   - `rivun-policy` (`crates/rivun-policy/src/lib.rs`): Defines `PolicySet`, `PolicyRule`, `PolicyDecision`, and `ZapPolicyError`. Provides `PolicySet::from_toml_str()` and `validate()`.
   - `rivun-router` (`crates/rivun-router/src/lib.rs`): Defines `RouteTable`, `RouteRule`, and `ZapRouterError`. Provides `RouteTable::new()` and `validate()`.
   - `rivun-crypto` (`crates/rivun-crypto/src/lib.rs`): Provides Ed25519 keypairs, signing, verification, and SHA-256 helpers.

---

## 2. Logic Chain

1. **CLI Subcommand Lifecycle Gap**:
   - *Observation*: The CLI only exposes `validate`, `inspect`, and `list` for unpacked pack directories.
   - *Reasoning*: A domain pack must move through a secure packaging, distribution, and installation lifecycle: scaffolding (`init`) -> packaging (`build`) -> signing (`sign`) -> offline verification (`verify`) -> installation (`install`) -> security auditing (`audit`).
   - *Deduction*: Adding `init`, `build`, `sign`, `verify`, `install`, and `audit` to `PackCommand` in `crates/rivun-cli` completes the end-to-end user and operator workflow.

2. **Offline Bundle & Signature Verification**:
   - *Observation*: Without network access, node operators must be able to verify domain pack bundles before installation.
   - *Reasoning*: Security in air-gapped environments requires cryptographic signatures over single-file `.zpack` bundles and verification of internal artifact digests (`manifest.digest.json`) against signed checksums.
   - *Deduction*: `rivun-store` requires a dedicated `DomainPackBundle` container (ZIP/tar.gz format with `ZPACK001` magic bytes) and `DomainPackBundleSignature` struct with Ed25519 verification over domain `rivun-DOMAIN-PACK-BUNDLE-v1`.

3. **Dependency Resolution & Policy Validation Engine**:
   - *Observation*: Domain packs may depend on other domain packs or specific capability exports. Installing an incompatible or broken pack can corrupt node route tables or violate security policies.
   - *Reasoning*: Dependency resolution must check semver version requirements (e.g. `^1.0.0`), resolve required vs provided capabilities across installed domain packs, detect circular dependencies, and statically validate policy rules (`PolicySet`) and route tables (`RouteTable`).
   - *Deduction*: `rivun-store` requires `DomainPackDependencyResolver` and `DomainPackPolicyValidator` to ensure atomic, safe installation and policy compliance.

---

## 3. Caveats

- **Scope Boundary**: This blueprint provides complete technical specifications, data structures, method signatures, CLI interfaces, and test strategies for Milestone 2. Code implementation must be executed by the implementer agent.
- **Archive Format**: `.zpack` bundles are defined as compressed archives (tar.gz format) with a top-level `manifest.digest.json` file.
- **Crypto Compatibility**: Detached Ed25519 signatures use `rivun-crypto`'s Ed25519 implementation and `rivun-DOMAIN-PACK-BUNDLE-v1` domain separation prefix.

---

## 4. Conclusion: Technical Blueprint for Milestone 2

### 4.1 Architecture & File Structure

```
crates/
├── rivun-cli/
│   └── src/
│       ├── main.rs            # Update PackCommand enum & dispatcher
│       └── pack.rs            # Dedicated CLI pack module for init/build/sign/verify/install/audit
└── rivun-store/
    └── src/
        ├── lib.rs             # Export new module and error variants
        ├── bundle.rs          # DomainPackBundle, DomainPackBundleManifest, DomainPackBundleSignature
        ├── resolver.rs        # DomainPackDependencyResolver, VersionReq matching, capability solver
        └── validator.rs       # DomainPackPolicyValidator, route/policy/schema static checker
```

---

### 4.2 CLI Command Definitions (`crates/rivun-cli`)

#### Updated `PackCommand` Enum (`crates/rivun-cli/src/main.rs`)

```rust
#[derive(Debug, Subcommand)]
enum PackCommand {
    /// Scaffold a new domain pack template directory.
    Init {
        #[arg(long, help = "Directory path for the new domain pack")]
        dir: PathBuf,
        #[arg(long, help = "Domain pack identifier (e.g., com.example.finance)")]
        id: Option<String>,
        #[arg(long, help = "Human-readable name")]
        name: Option<String>,
        #[arg(long, help = "Initial version (default: 0.1.0)")]
        version: Option<String>,
        #[arg(long, help = "Scaffold template variant: default, minimal, full")]
        template: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// Compile a domain pack directory into a single .zpack archive bundle.
    Build {
        #[arg(long, help = "Path to domain pack directory containing pack.toml")]
        pack: PathBuf,
        #[arg(long, help = "Output path for .zpack bundle archive")]
        out: Option<PathBuf>,
        #[arg(long)]
        json: bool,
    },
    /// Sign a .zpack archive bundle with an Ed25519 private key.
    Sign {
        #[arg(long, help = "Path to .zpack archive bundle")]
        bundle: PathBuf,
        #[arg(long, help = "Path to Ed25519 keypair or seed file")]
        key: PathBuf,
        #[arg(long, help = "Output signature file path (defaults to <bundle>.sig)")]
        out: Option<PathBuf>,
        #[arg(long)]
        json: bool,
    },
    /// Verify a .zpack bundle signature, manifest integrity, and policy rules.
    Verify {
        #[arg(long, help = "Path to .zpack archive bundle")]
        bundle: PathBuf,
        #[arg(long, help = "Path to detached .zpack.sig signature file")]
        signature: Option<PathBuf>,
        #[arg(long, help = "Expected publisher public key (hex or base64)")]
        public_key: Option<String>,
        #[arg(long, help = "Skip route/policy static validation")]
        no_policy_check: bool,
        #[arg(long)]
        json: bool,
    },
    /// Validate offline bundle, verify signatures & dependencies, copy to store directory.
    Install {
        #[arg(long, help = "Path to .zpack archive bundle file")]
        bundle: PathBuf,
        #[arg(long, help = "Path to detached signature file (optional if alongside bundle)")]
        signature: Option<PathBuf>,
        #[arg(long, help = "Target pack store installation directory")]
        store_dir: PathBuf,
        #[arg(long, help = "Trusted publisher public key(s) for offline signature check")]
        trusted_key: Vec<String>,
        #[arg(long, help = "Force overwrite if version is already installed")]
        force: bool,
        #[arg(long)]
        json: bool,
    },
    /// Perform security audit of capability grants, permissions, and route policies.
    Audit {
        #[arg(long, help = "Path to domain pack directory or .zpack bundle")]
        pack: PathBuf,
        #[arg(long, help = "Maximum acceptable risk level (low, medium, high, critical)")]
        max_risk: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// Validate a domain pack manifest and referenced policy/schema files.
    Validate {
        #[arg(long)]
        pack: PathBuf,
        #[arg(long)]
        json: bool,
    },
    /// Summarize a domain pack manifest.
    Inspect {
        #[arg(long)]
        pack: PathBuf,
        #[arg(long)]
        json: bool,
    },
    /// List and validate domain packs under a root directory.
    List {
        #[arg(long)]
        root: PathBuf,
        #[arg(long)]
        json: bool,
    },
}
```

#### CLI Command Behavior Specs & Report Structs

1. **`rivun pack init`**:
   - Creates directory `<dir>` and populates:
     - `pack.toml`:
       ```toml
       schema_version = 1
       id = "com.example.my-pack"
       name = "My Pack"
       version = "0.1.0"
       status = "active"

       [[capabilities]]
       id = "cap.example.read"
       risk = "low"

       [[policies]]
       path = "policies/default.policy"

       [[schemas]]
       path = "schemas/default.json"

       [dependencies]
       ```
     - `policies/default.policy` (sample policyTOML)
     - `schemas/default.json` (sample schema)
     - `README.md`
   - Data Struct:
     ```rust
     #[derive(Debug, Serialize, Deserialize)]
     pub struct PackInitReport {
         pub dir: String,
         pub id: String,
         pub version: String,
         pub created_files: Vec<String>,
     }
     ```

2. **`rivun pack build`**:
   - Parses `<pack_dir>/pack.toml`. Validates referenced files exist.
   - Computes SHA-256 for all artifacts (`pack.toml`, `policies/*`, `schemas/*`, `drivers/*`).
   - Writes `manifest.digest.json` containing list of `DomainPackArtifactDigest`.
   - Packages into tar.gz archive `.zpack`.
   - Data Struct:
     ```rust
     #[derive(Debug, Serialize, Deserialize)]
     pub struct PackBuildReport {
         pub pack_id: String,
         pub version: String,
         pub bundle_path: String,
         pub bundle_sha256: String,
         pub size_bytes: u64,
         pub artifact_count: usize,
     }
     ```

3. **`rivun pack sign`**:
   - Reads Ed25519 private key from file or environment.
   - Computes SHA-256 digest of `.zpack` file.
   - Domain prefix: `rivun-DOMAIN-PACK-BUNDLE-v1`.
   - Constructs `DomainPackBundleSignature` and writes to `<bundle>.sig` or `--out`.
   - Data Struct:
     ```rust
     #[derive(Debug, Serialize, Deserialize)]
     pub struct PackSignReport {
         pub bundle_path: String,
         pub signature_path: String,
         pub signer_node_id: Uuid,
         pub signer_public_key: String,
         pub bundle_sha256: String,
     }
     ```

4. **`rivun pack verify`**:
   - Opens `.zpack` archive, checks file hashes against `manifest.digest.json`.
   - Verifies Ed25519 signature over domain `rivun-DOMAIN-PACK-BUNDLE-v1`.
   - Performs static validation on policies (`PolicySet`) and routes (`RouteTable`) unless `--no-policy-check`.
   - Data Struct:
     ```rust
     #[derive(Debug, Serialize, Deserialize)]
     pub struct PackVerifyReport {
         pub bundle_path: String,
         pub pack_id: String,
         pub version: String,
         pub integrity_ok: bool,
         pub signature_ok: bool,
         pub policy_ok: bool,
         pub errors: Vec<String>,
     }
     ```

5. **`rivun pack install`**:
   - Performs offline bundle verification.
   - Validates Ed25519 signature against trusted public key whitelist (`--trusted-key`).
   - Runs `DomainPackDependencyResolver` against existing installed packs in `store_dir`.
   - Performs static route/policy validation.
   - Extracts bundle atomically to `<store_dir>/packs/<pack_id>/<version>/`.
   - Updates `<store_dir>/registry.json` (`DomainPackRegistry`).
   - Data Struct:
     ```rust
     #[derive(Debug, Serialize, Deserialize)]
     pub struct PackInstallReport {
         pub pack_id: String,
         pub version: String,
         pub store_path: String,
         pub installed_dependencies: Vec<String>,
         pub status: String,
     }
     ```

6. **`rivun pack audit`**:
   - Audits capabilities, requested permissions, wildcard route rules, and deprecation status.
   - Calculates overall risk (`low`, `medium`, `high`, `critical`).
   - Data Struct:
     ```rust
     #[derive(Debug, Serialize, Deserialize)]
     pub struct PackAuditReport {
         pub pack_id: String,
         pub version: String,
         pub overall_risk: DomainPackRisk,
         pub max_risk_allowed: DomainPackRisk,
         pub passed: bool,
         pub issues: Vec<AuditIssue>,
     }

     #[derive(Debug, Serialize, Deserialize)]
     pub struct AuditIssue {
         pub severity: DomainPackRisk,
         pub category: String,
         pub message: String,
     }
     ```

---

### 4.3 `rivun-store` Engine Integration (`crates/rivun-store`)

#### Data Types (`crates/rivun-store/src/bundle.rs`)

```rust
pub const DOMAIN_PACK_BUNDLE_SIGNATURE_DOMAIN: &[u8] = b"rivun-DOMAIN-PACK-BUNDLE-v1";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DomainPackBundleManifest {
    pub schema_version: u8,
    pub pack_id: String,
    pub name: String,
    pub version: String,
    pub status: DomainPackStatus,
    pub created_at_micros: u64,
    pub artifacts: Vec<DomainPackArtifactDigest>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DomainPackArtifactDigest {
    pub relative_path: String,
    pub sha256_hex: String,
    pub size_bytes: u64,
    pub content_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DomainPackBundleSignature {
    pub schema_version: u8,
    pub pack_id: String,
    pub pack_version: String,
    pub bundle_sha256: String,
    pub signer_node_id: Uuid,
    pub signer_public_key: String, // Hex or Base64
    pub signature: String,         // Base64
    pub signed_at_micros: u64,
}

impl DomainPackBundleSignature {
    pub fn sign(
        pack_id: &str,
        pack_version: &str,
        bundle_sha256: &str,
        keypair: &Keypair,
    ) -> Result<Self, RivunStoreError>;

    pub fn verify(&self, expected_bundle_sha256: &str) -> Result<(), RivunStoreError>;

    pub fn verify_against_trusted_keys(
        &self,
        expected_bundle_sha256: &str,
        trusted_public_keys: &[String],
    ) -> Result<(), RivunStoreError>;
}

pub struct DomainPackBundle {
    pub manifest: DomainPackBundleManifest,
    pub raw_tarball_bytes: Vec<u8>,
    pub bundle_sha256: String,
}

impl DomainPackBundle {
    pub fn build_from_dir(pack_dir: &Path) -> Result<Self, RivunStoreError>;
    pub fn open_from_file(bundle_path: &Path) -> Result<Self, RivunStoreError>;
    pub fn write_to_file(&self, output_path: &Path) -> Result<(), RivunStoreError>;
    pub fn verify_integrity(&self) -> Result<(), RivunStoreError>;
    pub fn extract_to_dir(&self, target_dir: &Path) -> Result<(), RivunStoreError>;
}
```

#### Dependency Resolution Engine (`crates/rivun-store/src/resolver.rs`)

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DomainPackDependencySpec {
    pub pack_id: String,
    pub version_req: String, // semver range, e.g. "^1.0.0"
    pub optional: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DomainPackResolutionPlan {
    pub target_pack: String,
    pub target_version: String,
    pub install_order: Vec<DomainPackRegistryEntry>,
    pub required_capabilities: Vec<String>,
}

pub struct DomainPackDependencyResolver<'a> {
    pub store_registry: &'a DomainPackRegistry,
}

impl<'a> DomainPackDependencyResolver<'a> {
    pub fn new(store_registry: &'a DomainPackRegistry) -> Self;

    pub fn resolve(
        &self,
        manifest: &DomainPackManifest,
        dependencies: &[DomainPackDependencySpec],
    ) -> Result<DomainPackResolutionPlan, RivunStoreError>;
}
```

#### Policy & Route Static Validator (`crates/rivun-store/src/validator.rs`)

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DomainPackValidationResult {
    pub valid: bool,
    pub syntax_errors: Vec<String>,
    pub policy_rule_count: usize,
    pub route_rule_count: usize,
    pub capability_warnings: Vec<String>,
}

pub struct DomainPackPolicyValidator;

impl DomainPackPolicyValidator {
    pub fn validate_bundle_policies(bundle: &DomainPackBundle) -> DomainPackValidationResult;
    pub fn validate_dir_policies(pack_dir: &Path) -> DomainPackValidationResult;
}
```

#### RivunStoreError Variant Extensions (`crates/rivun-store/src/lib.rs`)

```rust
    // Domain pack bundle & offline verification errors
    #[error("invalid domain pack bundle format: {0}")]
    InvalidDomainPackBundleFormat(String),
    #[error("domain pack bundle digest mismatch for `{path}`: expected {expected}, actual {actual}")]
    DomainPackBundleDigestMismatch { path: String, expected: String, actual: String },
    #[error("domain pack signature missing or invalid")]
    InvalidDomainPackBundleSignature,
    #[error("domain pack signature signer `{signer}` is not in trusted public keys whitelist")]
    UntrustedDomainPackSigner { signer: String },
    #[error("unsatisfied domain pack dependency `{pack_id}` version requirement `{requirement}`")]
    UnsatisfiedDomainPackDependency { pack_id: String, requirement: String },
    #[error("circular dependency detected in domain pack graph: {0}")]
    CircularDomainPackDependency(String),
    #[error("domain pack policy validation failed: {0}")]
    DomainPackPolicyValidationFailed(String),
```

---

### 4.4 Store Installation Directory Layout

```
<store_dir>/
├── registry.json             # DomainPackRegistry index
└── packs/
    └── <pack_id>/
        └── <version>/
            ├── pack.toml
            ├── manifest.digest.json
            ├── pack.sig
            ├── policies/
            │   └── default.policy
            ├── schemas/
            │   └── default.json
            └── drivers/
```

---

## 5. Verification Method

### 1. Unit & Integration Tests

Run full test suite:
```powershell
cargo test --workspace --all-targets
```

Run specific `rivun-cli` and `rivun-store` pack tests:
```powershell
cargo test -p rivun-store --test pack_tests
cargo test -p rivun-cli --test pack_cli_tests
```

### 2. Manual / CLI End-to-End Test Sequence

1. **Scaffold pack template**:
   ```powershell
   cargo run --bin rivun -- pack init --dir ./tmp/test-pack --id com.example.finance --name "Finance Pack" --version 1.0.0
   ```
   *Verification*: Confirms `./tmp/test-pack/pack.toml`, `policies/default.policy`, and `schemas/default.json` are created.

2. **Build bundle**:
   ```powershell
   cargo run --bin rivun -- pack build --pack ./tmp/test-pack --out ./tmp/finance-1.0.0.zpack
   ```
   *Verification*: Confirms `./tmp/finance-1.0.0.zpack` file is generated containing `manifest.digest.json`.

3. **Sign bundle**:
   ```powershell
   cargo run --bin rivun -- keygen --out ./tmp/author.key
   cargo run --bin rivun -- pack sign --bundle ./tmp/finance-1.0.0.zpack --key ./tmp/author.key --out ./tmp/finance-1.0.0.zpack.sig
   ```
   *Verification*: Confirms detached signature file `./tmp/finance-1.0.0.zpack.sig` is created.

4. **Verify bundle**:
   ```powershell
   cargo run --bin rivun -- pack verify --bundle ./tmp/finance-1.0.0.zpack --signature ./tmp/finance-1.0.0.zpack.sig
   ```
   *Verification*: Returns `integrity_ok: true`, `signature_ok: true`, `policy_ok: true`.

5. **Install bundle offline**:
   ```powershell
   cargo run --bin rivun -- pack install --bundle ./tmp/finance-1.0.0.zpack --signature ./tmp/finance-1.0.0.zpack.sig --store-dir ./tmp/store
   ```
   *Verification*: Confirms extraction into `./tmp/store/packs/com.example.finance/1.0.0/` and entry in `./tmp/store/registry.json`.

6. **Audit pack**:
   ```powershell
   cargo run --bin rivun -- pack audit --pack ./tmp/test-pack --max-risk medium
   ```
   *Verification*: Evaluates pack risk level and route policy safety, outputs structured JSON report.

