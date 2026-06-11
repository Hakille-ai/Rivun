# Contributing

Thank you for helping build ZAP. The project is intentionally conservative:
protocol correctness, security, and reproducibility matter more than cleverness.

## Development Setup

Install Rust using the toolchain declared in [rust-toolchain.toml](rust-toolchain.toml).

```bash
cargo ci-fmt
cargo ci-test
cargo ci-smoke
cargo ci-bench-smoke
cargo ci-clippy
```

The aliases are defined in [.cargo/config.toml](.cargo/config.toml) and match
the CI workflow.

Performance-sensitive changes must also review the benchmark summary produced by
GitHub Actions. The strict benchmark gate compares pull-request code against the
base commit on the same runner and fails when configured thresholds in
[tools/bench-thresholds.toml](tools/bench-thresholds.toml) are exceeded. Full
local benchmark runs use `cargo ci-bench-full`; they require enough free disk
for release-profile Criterion builds.

Docker users can build the CLI image with:

```bash
docker build -t zap:local .
```

## Contribution Rules

- Do not change the 64-byte ZAP-Wire header layout without a new protocol
  version and golden vectors.
- Keep `ZAP_SIGN` as a fast hint only; never treat it as a complete signature.
- Keep `zap-core` focused on wire frames. Put universal message semantics in
  `zap-envelope`.
- Keep host capabilities denied by default in the WASM runtime.
- Keep `zap check-config` aligned with daemon startup validation.
- Prefer typed errors and deterministic behavior over ad hoc parsing.
- Add or update tests for every user-visible behavior change.
- Do not add financial, billing, reward, or settlement behavior to the protocol
  core.

## Pull Request Checklist

- [ ] `cargo ci-fmt`
- [ ] `cargo ci-test`
- [ ] `cargo ci-smoke`
- [ ] `cargo ci-bench-smoke`
- [ ] `cargo ci-clippy`
- [ ] performance-sensitive changes reviewed against benchmark output
- [ ] docs updated when behavior, configuration, CLI, or security posture
      changes
- [ ] golden vectors updated for wire-format changes
- [ ] changelog entry added for externally visible changes

## Commit Style

Use small, reviewable commits. Suggested prefixes:

- `core:` wire/header/frame behavior
- `envelope:` universal message format
- `crypto:` signing, identity, PoA
- `net:` transport and peer behavior
- `runtime:` WASM execution and permissions
- `node:` daemon behavior
- `cli:` operator commands
- `docs:` documentation-only changes

## Compatibility Expectations

ZAP is pre-1.0, but wire compatibility is still treated seriously. Any change
that affects parsing, encoding, signatures, trailers, or default security policy
needs tests and an explicit note in [CHANGELOG.md](CHANGELOG.md).
