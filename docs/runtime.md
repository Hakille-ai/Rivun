# WASM Runtime

ZAP drivers use ABI v1:

```wat
(memory (export "memory") 1)
(func (export "zap_alloc") (param i32) (result i32))
(func (export "zap_dealloc") (param i32 i32))
(func (export "zap_execute")
  (param $action_ptr i32)
  (param $action_len i32)
  (param $payload_ptr i32)
  (param $payload_len i32)
  (result i64))
```

`zap_execute` returns `(result_ptr << 32) | result_len`.

The node sends the action name and payload bytes into guest memory. The driver returns bytes only; any host side effect must eventually go through an explicit capability API.

`check-config` compiles every configured driver and validates ABI v1 before daemon startup. A module is rejected if a required export is missing, if `memory` is not a memory export, or if any function export has a different signature.

When a driver config includes `manifest = "..."`, `check-config` also verifies the signed ZapStore manifest. The manifest must match the configured action, the local driver hash, the supported ABI version, and the author signature.

The daemon repeats that compile-and-validate step during startup and keeps the compiled Wasmtime modules in memory. Driver files are not re-read for every action, so runtime behavior is stable after launch and per-message latency avoids repeated compilation.

Wasmtime fuel enforces deterministic instruction budgets. Epoch interruption enforces wall-clock deadlines for long-running code, with the store configured to trap when the deadline is reached.

ABI v1 provides no host imports. Signed manifests can declare future host permissions, but `zap-node` rejects drivers that currently request network, filesystem, clock, or environment access.

For v1 examples, see `examples/wasm-drivers/echo/echo.wat` and `examples/wasm-drivers/thermostat/thermostat.wat`.
