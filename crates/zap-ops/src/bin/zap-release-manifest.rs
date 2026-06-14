use sha2::{Digest as _, Sha256};
use std::{
    env, fs,
    path::{Path, PathBuf},
    process,
    time::{SystemTime, UNIX_EPOCH},
};
use zap_ops::{
    ArtifactKind, ArtifactSignature, RELEASE_SCHEMA_VERSION, ReleaseArtifact, ReleaseChannel,
    ReleaseManifest,
};

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        process::exit(2);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = env::args().skip(1).peekable();
    if matches!(args.peek().map(String::as_str), Some("-h" | "--help")) {
        println!("{}", usage());
        return Ok(());
    }

    if matches!(args.peek().map(String::as_str), Some("--check")) {
        args.next();
        let path = next_path(&mut args, "--check")?;
        let manifest: ReleaseManifest = serde_json::from_str(&fs::read_to_string(&path)?)?;
        manifest.validate()?;
        println!("validated {}", path.display());
        return Ok(());
    }

    let mut version = None;
    let mut channel = None;
    let mut git_sha = None;
    let mut out = None;
    let mut artifacts = Vec::new();

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--version" => version = Some(next_value(&mut args, "--version")?),
            "--channel" => channel = Some(parse_channel(&next_value(&mut args, "--channel")?)?),
            "--git-sha" => git_sha = Some(next_value(&mut args, "--git-sha")?),
            "--out" => out = Some(next_path(&mut args, "--out")?),
            other if other.starts_with('-') => {
                return Err(format!("unknown option `{other}`").into());
            }
            path => artifacts.push(PathBuf::from(path)),
        }
    }

    let version = version.ok_or("missing --version")?;
    let channel = channel.ok_or("missing --channel")?;
    let git_sha = git_sha.ok_or("missing --git-sha")?;
    let out = out.ok_or("missing --out")?;
    if artifacts.is_empty() {
        return Err("at least one artifact path is required".into());
    }
    artifacts.sort();

    let mut release_artifacts = Vec::with_capacity(artifacts.len());
    let mut signatures = Vec::new();
    for path in &artifacts {
        let artifact = archive_artifact(&version, path)?;
        if let Some(signature) = sigstore_signature(&artifact, path) {
            signatures.push(signature);
        }
        release_artifacts.push(artifact);
    }

    let manifest = ReleaseManifest {
        schema_version: RELEASE_SCHEMA_VERSION,
        version,
        channel,
        git_sha,
        created_at_micros: now_micros()?,
        artifacts: release_artifacts,
        signatures,
        sbom_path: None,
    };
    manifest.validate()?;

    if let Some(parent) = out.parent().filter(|parent| !parent.as_os_str().is_empty()) {
        fs::create_dir_all(parent)?;
    }
    fs::write(&out, serde_json::to_string_pretty(&manifest)? + "\n")?;
    println!("wrote {}", out.display());
    Ok(())
}

fn usage() -> &'static str {
    "usage: zap-release-manifest --version <semver> --channel <nightly|preview|stable|security> --git-sha <sha> --out <path> <artifact>...\n\
     usage: zap-release-manifest --check <manifest.json>"
}

fn next_value(
    args: &mut std::iter::Peekable<impl Iterator<Item = String>>,
    option: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    args.next()
        .ok_or_else(|| format!("missing value for {option}").into())
}

fn next_path(
    args: &mut std::iter::Peekable<impl Iterator<Item = String>>,
    option: &str,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    Ok(PathBuf::from(next_value(args, option)?))
}

fn parse_channel(input: &str) -> Result<ReleaseChannel, Box<dyn std::error::Error>> {
    match input {
        "nightly" => Ok(ReleaseChannel::Nightly),
        "preview" => Ok(ReleaseChannel::Preview),
        "stable" => Ok(ReleaseChannel::Stable),
        "security" => Ok(ReleaseChannel::Security),
        other => Err(format!("unknown release channel `{other}`").into()),
    }
}

fn archive_artifact(
    version: &str,
    path: &Path,
) -> Result<ReleaseArtifact, Box<dyn std::error::Error>> {
    let data = fs::read(path)?;
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| format!("artifact path `{}` has no file name", path.display()))?
        .to_string();
    let target = target_from_archive_name(version, &name);
    let sha256 = {
        let mut hasher = Sha256::new();
        hasher.update(&data);
        hex::encode(hasher.finalize())
    };
    let blake3 = hex::encode(blake3::hash(&data).as_bytes());
    Ok(ReleaseArtifact {
        name,
        kind: ArtifactKind::CliBinary,
        target,
        path: path.display().to_string(),
        size_bytes: data.len() as u64,
        sha256,
        blake3,
    })
}

fn sigstore_signature(
    artifact: &ReleaseArtifact,
    artifact_path: &Path,
) -> Option<ArtifactSignature> {
    let bundle_path = artifact_path.with_file_name(format!("{}.sigstore.json", artifact.name));
    bundle_path.exists().then(|| ArtifactSignature {
        artifact_name: artifact.name.clone(),
        signer: "github-actions-oidc".to_string(),
        public_key: "sigstore-keyless".to_string(),
        signature: bundle_path.display().to_string(),
    })
}

fn target_from_archive_name(version: &str, file_name: &str) -> String {
    let prefix = format!("zap-{version}-");
    file_name
        .strip_prefix(&prefix)
        .unwrap_or(file_name)
        .strip_suffix(".tar.gz")
        .or_else(|| file_name.strip_suffix(".zip"))
        .unwrap_or(file_name)
        .to_string()
}

fn now_micros() -> Result<u64, std::time::SystemTimeError> {
    Ok(SystemTime::now().duration_since(UNIX_EPOCH)?.as_micros() as u64)
}
