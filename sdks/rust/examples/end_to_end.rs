use zap_sdk::{
    ControlFrame, RegistryBundleManifestRequest, registry_bundle_manifest_request_frame,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let frame = registry_bundle_manifest_request_frame(true, true)?;
    let encoded = frame.encode();
    let parsed = ControlFrame::decode(&encoded)?;
    let body: RegistryBundleManifestRequest = parsed.json_body()?;

    println!(
        "built {} ({} bytes, require_drivers={})",
        parsed.subject(),
        encoded.len(),
        body.require_drivers
    );
    Ok(())
}
