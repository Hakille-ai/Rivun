# Rivun Thermostat WASM Driver Example

This folder contains a WebAssembly Text (WAT) driver showing how to return a static status payload from a sandboxed Rivun driver.

## What it does
The driver declares a linear memory and pre-loads a static response message `"thermostat:setpoint:accepted"` (28 bytes) into its memory at offset `2048` using a WebAssembly data segment.
When `rivun_execute` is called, the driver returns this pre-loaded response.

## Code walkthrough (`thermostat.wat`)
```wat
(module
  ;; 1. Export linear memory
  (memory (export "memory") 1)

  ;; 2. Place a static response string in memory starting at address 2048
  (data (i32.const 2048) "thermostat:setpoint:accepted")

  ;; 3. Heap pointer for allocations
  (global $heap (mut i32) (i32.const 4096))

  ;; 4. Allocation hook
  (func (export "rivun_alloc") (param $len i32) (result i32)
    global.get $heap
    global.get $heap
    local.get $len
    i32.add
    global.set $heap)

  ;; 5. Deallocation hook
  (func (export "rivun_dealloc") (param i32 i32))

  ;; 6. Execution hook
  ;; Ignores inputs and always returns the static memory address (2048) and length (28)
  (func (export "rivun_execute")
    (param $action_ptr i32) (param $action_len i32)
    (param $payload_ptr i32) (param $payload_len i32)
    (result i64)
    
    ;; Pack pointer 2048 into the high 32 bits of i64
    i64.const 2048
    i64.const 32
    i64.shl
    
    ;; Pack length 28 (size of "thermostat:setpoint:accepted") into low 32 bits
    i64.const 28
    i64.or)
)
```

## How to use it in Rivun

1. **Sign the manifest**:
   Create a manifest signed with the target node's key:
   ```bash
   cargo run -p rivun-cli -- driver-manifest create \
     --driver examples/wasm-drivers/thermostat/thermostat.wat \
     --action thermostat.setpoint \
     --author-key .rivun/node.key \
     --out examples/wasm-drivers/thermostat/thermostat.manifest.toml
   ```

2. **Add to node configuration**:
   ```toml
   [[drivers]]
   action = "thermostat.setpoint"
   path = "examples/wasm-drivers/thermostat/thermostat.wat"
   manifest = "examples/wasm-drivers/thermostat/thermostat.manifest.toml"
   ```

3. **Start the daemon and send an action**:
   ```bash
   cargo run -p rivun-cli -- run --config rivun.toml
   ```
   In a separate terminal, trigger the thermostat action:
   ```bash
   cargo run -p rivun-cli -- send --config client.toml --target <gateway-uuid> --action thermostat.setpoint --payload '{"temp": 22}'
   ```
   You will receive the response: `thermostat:setpoint:accepted`.

