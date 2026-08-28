## Summary

Describe the change and why it matters.

## Change Type

- [ ] Bug fix
- [ ] Feature
- [ ] Documentation
- [ ] Tests or fixtures
- [ ] Refactor with no behavior change
- [ ] Release, packaging, CI, or governance
- [ ] Security, protocol, crypto, ABI, config, or domain pack contract change

## Compatibility

- [ ] No @@@@rivun_HEADER@@WIRE@@ or `ZENV` binary layout change
- [ ] Wire/envelope change documented with tests and golden vectors
- [ ] CLI/config behavior change documented
- [ ] Security model unchanged or documented
- [ ] SDK compatibility reviewed
- [ ] Domain pack compatibility reviewed
- [ ] Migration or rollback notes included when behavior changes

## RFC / ZEP

- [ ] Not required
- [ ] Required and linked below
- [ ] Follow-up required before merge

RFC/ZEP:

## Validation

- [ ] `cargo fmt --all -- --check`
- [ ] `cargo test --workspace --all-targets`
- [ ] `cargo ci-smoke`
- [ ] `cargo ci-bench-smoke`
- [ ] `cargo clippy --workspace --all-targets -- -D warnings`
- [ ] Performance-sensitive changes reviewed against the benchmark summary
- [ ] Docker build checked when packaging changes
- [ ] Domain packs validated with `rivun pack validate`
- [ ] SDK tests run when SDK surfaces change
- [ ] Website/docs lint run when website changes

## Notes for Reviewers

Call out protocol, crypto, runtime, deployment, or migration risks.

## Release Notes

- User-facing change:
- Operator/security note:
- Breaking change:

