# Release Process

This document defines the maintainer checklist for cutting ZAP releases.

Public release packaging is automated by `.github/workflows/release.yml`. The
workflow validates tag versioning, builds `zap` for Linux, macOS, and Windows,
publishes SHA-256 checksums, creates keyless Sigstore bundles with GitHub OIDC,
and uploads the release artifacts.

## Release Types

- **Patch**: bug fixes, docs fixes, security fixes, compatibility-preserving
  hardening.
- **Minor**: compatible features, new CLI commands, new optional config fields,
  new envelope helpers.
- **Major**: breaking public API, documented CLI behavior, or protocol behavior.

ZAP is pre-1.0. Even so, releases should be predictable and migration notes
should be clear.

## Checklist

1. Confirm the changelog has an entry for the release.
2. Confirm version numbers in workspace `Cargo.toml` are correct.
3. Confirm governance approval for the release policy in
   [governance.md](governance.md).
4. Run:

   ```bash
   cargo fmt --all -- --check
   cargo test --workspace --all-targets
   cargo clippy --workspace --all-targets -- -D warnings
   docker build -t zap:release-candidate .
   ```

5. Run `zap check-config` against any updated example configs.
6. Verify docs mention any behavior, config, or security posture changes.
7. Tag the release as `vMAJOR.MINOR.PATCH`.
8. Publish release notes with:
   - compatibility notes;
   - security notes;
   - Docker image digest, if an image is published;
   - known limitations.

## Version Gate

The release workflow accepts either a tag named `vMAJOR.MINOR.PATCH` or a manual
dispatch version. In both cases the requested version must match the workspace
package version reported by `cargo metadata`. This prevents publishing binaries
whose filename and crate metadata disagree.

Pre-release tags such as `v0.2.0-rc.1` are accepted by the packaging scripts
when they match the workspace version.

## Release Artifacts

Each platform job emits:

- `zap-VERSION-x86_64-unknown-linux-gnu.tar.gz`;
- `zap-VERSION-x86_64-apple-darwin.tar.gz`;
- `zap-VERSION-x86_64-pc-windows-msvc.zip`;
- a per-archive `.sha256` file.

The publish job creates:

- `SHA256SUMS`;
- `SHA256SUMS.sigstore.json`;
- one `.sigstore.json` bundle per archive;
- `zap-release-manifest.json` using the `zap-ops` release schema, with
  per-artifact SHA-256 and BLAKE3 digests.

Consumers should verify the checksum first, then verify the Sigstore bundle
against the GitHub workflow identity that produced the release.

## Registry and Bundle Releases

Driver registry changes are release inputs, not incidental files. Before a
stable release:

```bash
cargo run -p zap-cli -- registry verify-signature --registry registry.index.toml
cargo run -p zap-cli -- registry publication verify --registry registry.index.toml --publication registry.publication.json --json
cargo run -p zap-cli -- registry bundle verify --bundle zapstore-bundle --require-drivers --json
```

Archive the signed registry, publication JSON, install plans, and bundle
manifest with the release evidence. If a registry is mirrored during release
preparation, re-sign it after conflict review and before package publication.

## Rollback

Rollback is another governed release action. Use the previous release's
checksums, Sigstore bundles, registry publication, and install plan. Do not
reuse unsigned local registry mirrors as rollback input.

## Protocol Changes

Protocol changes require extra care:

- update [protocol.md](protocol.md);
- add or update golden vectors;
- add parser rejection tests for malformed old/new data;
- update [versioning.md](versioning.md) when compatibility rules change.

## Security Releases

Security releases may skip feature freeze rules. Keep details private until a
fixed version is available, then publish an advisory and concise upgrade
guidance.
