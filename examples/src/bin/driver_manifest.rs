use zap_capability::DriverPermissions;
use zap_crypto::Keypair;
use zap_store::{DriverManifest, DriverRegistry};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== ZAP Signed Driver Manifests & Registry ===");

    // 1. Generate keys for the driver author and the system operator
    let author_keys = Keypair::generate();
    let operator_keys = Keypair::generate();

    println!("\nDeveloper/Author Node ID: {}", author_keys.node_id());
    println!("System Operator Node ID:  {}", operator_keys.node_id());

    // 2. Prepare dummy driver WebAssembly bytes (equivalent to echo.wat)
    let wasm_bytes = b"(module (memory (export \"memory\") 1) (func (export \"zap_execute\")))";

    // 3. Create and cryptographically sign a Driver Manifest
    // The developer signs the manifest which records the WASM's BLAKE3 hash,
    // allowed permissions, ABI version, name, and version.
    let permissions = DriverPermissions::none(); // Sandbox mode: no filesystem, no network, etc.
    let manifest = DriverManifest::new(
        "thermostat-driver",                                    // Driver Name
        "0.1.0",                                                // Version
        "thermostat.setpoint",                                  // Target ZAP action to handle
        wasm_bytes,                                             // WASM binary data
        permissions,                                            // Sandboxed permissions requested
        Some("Controls room temperature via GPIO".to_string()), // Description
        &author_keys,                                           // Author keypair for signing
    )?;

    println!("\n1. Driver Manifest Created and Signed:");
    println!("  Name: {}", manifest.name);
    println!("  Version: {}", manifest.version);
    println!("  Action Handled: {}", manifest.action);
    println!("  WASM BLAKE3 Hash: {}", manifest.wasm_hash);
    println!("  Signature (Base64): {}...", &manifest.signature[..30]);

    // Serialize manifest to TOML format (ready to be stored in the file system)
    let manifest_toml = manifest.to_toml_string()?;
    println!("\nManifest TOML representation:\n{}", manifest_toml);

    // 4. Verify the manifest against the driver file
    // Check that the file was not modified, matches the target action, and is correctly signed.
    println!("\n2. Verifying Driver Manifest against Wasm bytes:");
    match manifest.verify_for_driver("thermostat.setpoint", wasm_bytes) {
        Ok(()) => println!("  Verification Succeeded: Manifest matches driver perfectly!"),
        Err(e) => println!("  Verification Failed: {}", e),
    }

    // 5. Initialize a local Driver Registry (index index.toml)
    // The registry lists all permitted drivers and is signed by the network/system operator.
    println!("\n3. Managing the Driver Registry:");
    let mut registry = DriverRegistry::empty(Some("zap-store-cli-v1".to_string()));

    // Add our manifest to the registry index
    registry.add_manifest(
        &manifest,
        Some("drivers/thermostat.manifest.toml".to_string()),
    )?;
    println!(
        "  Manifest added to registry. Entries count: {}",
        registry.entries.len()
    );

    // Sign the registry index using the operator keys
    registry.sign(&operator_keys)?;
    println!("  Registry index signed by Operator.");
    println!(
        "  Registry Operator Key ID: {:?}",
        registry.operator_node_id
    );
    println!(
        "  Registry Signature: {:?}",
        registry.signature.as_ref().map(|s| &s[..30])
    );

    // Verify registry signature
    match registry.verify_signature() {
        Ok(()) => println!("  Registry signature verified successfully!"),
        Err(e) => println!("  Registry signature verification failed: {}", e),
    }

    // 6. Demonstrate Driver Revocation
    // If a driver is found to be buggy or vulnerable, the operator can revoke it in the index.
    println!("\n4. Revoking a Driver Version:");
    registry.revoke(
        "thermostat.setpoint",
        "0.1.0",
        "CVE-2026-XXXX: buffer overflow in driver parsing",
    )?;
    println!(
        "  Status of 'thermostat.setpoint' v0.1.0 in registry: {:?}",
        registry.entries[0].status
    );
    println!(
        "  Reason for revocation: {:?}",
        registry.entries[0].revoked_reason
    );

    // Verify manifest against the updated registry (should fail now since it's revoked)
    match registry.verify_manifest(&manifest) {
        Ok(()) => println!("  CRITICAL: Manifest still considered active!"),
        Err(e) => println!(
            "  Security check working: Manifest verification failed as expected: {}",
            e
        ),
    }

    Ok(())
}
