# Rivun Echo WASM Driver Example

This folder contains a simple WebAssembly Text (WAT) driver representing the minimal implementation of the Rivun Driver ABI v1.

## What it does
The driver exports the required memory and allocation helper functions (`rivun_alloc` and `rivun_dealloc`), and the execution entry point `rivun_execute`. 
Upon invocation, it takes the input payload and returns it directly back to the host, acting as a basic echo service.

## Code walkthrough (`echo.wat`)
```wat
(module
  ;; 1. Export the module's linear memory so the Rivun host can read/write data
  (memory (export "memory") 1)

  ;; 2. A simple global pointer representing the current end of our heap
  (global $heap (mut i32) (i32.const 1024))

  ;; 3. Allocation hook: called by the host node to reserve space for payload & action parameters
  (func (export "rivun_alloc") (param $len i32) (result i32)
    global.get $heap
    global.get $heap
    local.get $len
    i32.add
    global.set $heap)

  ;; 4. Deallocation hook (no-op in this basic heap design)
  (func (export "rivun_dealloc") (param i32 i32))

  ;; 5. Execution hook: receives pointers and lengths of the target action and payload
  ;; Returns a single packed i64: (result_pointer << 32) | result_length
  (func (export "rivun_execute")
    (param $action_ptr i32) (param $action_len i32)
    (param $payload_ptr i32) (param $payload_len i32)
    (result i64)
    
    ;; We simply take the payload pointer and pack it with its length to return it.
    local.get $payload_ptr
    i64.extend_i32_u
    i64.const 32
    i64.shl
    local.get $payload_len
    i64.extend_i32_u
    i64.or))
```

## How to use it in Rivun

1. **Sign the manifest**:
   Before a node will load a driver, it must have a signed manifest. Run this to sign the driver:
   ```bash
   cargo run -p rivun-cli -- driver-manifest create \
     --driver examples/wasm-drivers/echo/echo.wat \
     --action echo \
     --author-key .rivun/node.key \
     --out examples/wasm-drivers/echo/echo.manifest.toml
   ```

2. **Run it**:
   Add the driver to your node's configuration:
   ```toml
   [[drivers]]
   action = "echo"
   path = "examples/wasm-drivers/echo/echo.wat"
   manifest = "examples/wasm-drivers/echo/echo.manifest.toml"
   ```
   Start the node daemon:
   ```bash
   cargo run -p rivun-cli -- run --config rivun.toml
   ```

3. **Send an action**:
   Send a message to invoke the driver:
   ```bash
   cargo run -p rivun-cli -- send --config client.toml --target <gateway-uuid> --action echo --payload "hello"
   ```
