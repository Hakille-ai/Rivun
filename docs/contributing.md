# Contributing

The canonical contributor guide is [../CONTRIBUTING.md](../CONTRIBUTING.md).

Protocol-specific reminders:

- the 64-byte ZAP-Wire header is fixed for v1;
- `zap-core` owns frames and trailers, not application semantics;
- `zap-envelope` owns universal message semantics;
- `ZAP_SIGN` is a hint, never a complete signature;
- WASM host permissions must remain explicit and denied by default;
- `zap check-config` must stay aligned with daemon startup validation;
- wire-format changes need golden vectors and migration notes.

Before opening a pull request:

```bash
cargo ci-fmt
cargo ci-test
cargo ci-clippy
```
