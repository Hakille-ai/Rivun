## Summary

Describe the change and why it matters.

## Compatibility

- [ ] No ZAP-Wire or `ZENV` binary layout change
- [ ] Wire/envelope change documented with tests and golden vectors
- [ ] CLI/config behavior change documented
- [ ] Security model unchanged or documented

## Validation

- [ ] `cargo fmt --all -- --check`
- [ ] `cargo test --workspace --all-targets`
- [ ] `cargo clippy --workspace --all-targets -- -D warnings`
- [ ] Docker build checked when packaging changes

## Notes for Reviewers

Call out protocol, crypto, runtime, deployment, or migration risks.
