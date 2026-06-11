# Release Process

This document defines the maintainer checklist for cutting ZAP releases.

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
3. Run:

   ```bash
   cargo fmt --all -- --check
   cargo test --workspace --all-targets
   cargo clippy --workspace --all-targets -- -D warnings
   docker build -t zap:release-candidate .
   ```

4. Run `zap check-config` against any updated example configs.
5. Verify docs mention any behavior, config, or security posture changes.
6. Tag the release as `vMAJOR.MINOR.PATCH`.
7. Publish release notes with:
   - compatibility notes;
   - security notes;
   - Docker image digest, if an image is published;
   - known limitations.

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
