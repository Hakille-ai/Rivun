# Contributing

The canonical contributor guide is [../CONTRIBUTING.md](../CONTRIBUTING.md).

Protocol-specific reminders:

- the 64-byte @@@@rivun_HEADER@@WIRE@@ header is fixed for v1;
- `rivun-core` owns frames and trailers, not application semantics;
- `rivun-envelope` owns universal message semantics;
- `@@rivun_HEADER@@SIGN` is a hint, never a complete signature;
- WASM host permissions must remain explicit and denied by default;
- `rivun check-config` must stay aligned with daemon startup validation;
- wire-format changes need golden vectors and migration notes.

Before opening a pull request:

```bash
cargo ci-fmt
cargo ci-test
cargo ci-clippy
```

