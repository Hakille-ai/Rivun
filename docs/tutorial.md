# ZAP End-to-End Tutorial: Smart Factory Telemetry & Control

This tutorial guides you through a complete, production-style deployment of ZAP. 

We will build a smart factory telemetry system with two nodes:
- **Node A** (Receiver / Gateway): Responsible for executing a sandboxed WASM driver to control a thermostat, logging signed execution receipts, and enforcing safety consensus.
- **Node B** (Sender / Operator terminal): Responsible for emitting typed actions and forwarding encrypted UDP frames to Node A.

```
                  Encrypted UDP
  ┌────────────┐  (ChaCha20-Poly1305)  ┌────────────┐
  │   Node B   │ ────────────────────► │   Node A   │ ──► Sandboxed WASM Driver
  │  (Sender)  │                       │ (Receiver) │     (thermostat.wat)
  └────────────┘                       └────────────┘
                                             │
                                             ▼
                                     Signed Binary Journal
                                     (receipts/*.zjseg)
```

---

## 1. Directory Structure Setup

Make sure your workspace matches the layout below:

```
ZAP/
├── Cargo.toml
├── .zap/
│   ├── node-a.key
│   └── node-b.key
├── examples/
│   ├── configs/
│   │   ├── node-a.toml
│   │   └── node-b.toml
│   └── wasm-drivers/
│       └── thermostat/
│           ├── thermostat.wat
│           └── thermostat.manifest.toml
```

---

## 2. Generating Identities

Generate the Ed25519 node keys for both systems:

```bash
cargo run -p zap-cli -- keygen --out .zap/node-a.key
cargo run -p zap-cli -- keygen --out .zap/node-b.key
```

Look inside the files and note down:
1. The `node_id` (UUID format) of both nodes.
2. The `public_key` (Base64 format) of both nodes.

---

## 3. Creating the sandboxed WASM driver

We will write a minimal WebAssembly Text (WAT) driver representing a thermostat controller.
Create a file at `examples/wasm-drivers/thermostat/thermostat.wat`:

```wat
(module
  ;; Export the linear memory that the host can read and write to
  (memory (export "memory") 1)
  
  ;; Simple heap pointer for allocations
  (global $heap (mut i32) (i32.const 1024))
  
  ;; Host allocation helper
  (func (export "zap_alloc") (param $len i32) (result i32)
    global.get $heap
    global.get $heap
    local.get $len
    i32.add
    global.set $heap)
    
  (func (export "zap_dealloc") (param $ptr i32) (param $len i32))
  
  ;; The execution hook: gets action and payload pointers
  ;; Returns a single i64 containing (result_pointer << 32) | result_length
  (func (export "zap_execute")
    (param $action_ptr i32) (param $action_len i32)
    (param $payload_ptr i32) (param $payload_len i32)
    (result i64)
    
    ;; Echo back the payload as driver output for demonstration
    local.get $payload_ptr
    i64.extend_i32_u
    i64.const 32
    i64.shl
    local.get $payload_len
    i64.extend_i32_u
    i64.or))
```

### Sign the Driver Manifest
The driver author must sign the driver package to guarantee integrity before the gateway executes it.

```bash
cargo run -p zap-cli -- driver-manifest create \
  --driver examples/wasm-drivers/thermostat/thermostat.wat \
  --action thermostat.setpoint \
  --author-key .zap/node-a.key \
  --out examples/wasm-drivers/thermostat/thermostat.manifest.toml
```

Verify that the manifest matches the driver wasm file:

```bash
cargo run -p zap-cli -- driver-manifest verify \
  --driver examples/wasm-drivers/thermostat/thermostat.wat \
  --manifest examples/wasm-drivers/thermostat/thermostat.manifest.toml
```

---

## 4. Writing Configuration Files

Create the configuration TOMLs for both Node A and Node B. Replace the placeholder public keys with the actual ones generated in Step 2.

### Node A Config (`examples/configs/node-a.toml`)
```toml
node_id = "<node-a-uuid>"
bind = "127.0.0.1:7000"
key_file = "../../.zap/node-a.key"

[security]
enforce_signatures = true
enforce_replay_protection = true

[receipts]
dir = "../../.zap/node-a-receipts"

[[drivers]]
action = "thermostat.setpoint"
path = "../wasm-drivers/thermostat/thermostat.wat"
manifest = "../wasm-drivers/thermostat/thermostat.manifest.toml"

[[peers]]
node_id = "<node-b-uuid>"
addr = "127.0.0.1:7001"
public_key = "<node-b-public-key-base64>"
transport_key = "0102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f20"
```

### Node B Config (`examples/configs/node-b.toml`)
```toml
node_id = "<node-b-uuid>"
bind = "127.0.0.1:7001"
key_file = "../../.zap/node-b.key"

[security]
enforce_signatures = true
enforce_replay_protection = true

[[peers]]
node_id = "<node-a-uuid>"
addr = "127.0.0.1:7000"
public_key = "<node-a-public-key-base64>"
transport_key = "0102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f20"
```

---

## 5. Starting the Cluster & Sending Telemetry

### Start Node A (Receiver Daemon)
```bash
cargo run -p zap-cli -- run --config examples/configs/node-a.toml
```

### Send Temperature Event from Node B
We send a JSON event envelope containing the temperature sensor reading:

```bash
cargo run -p zap-cli -- send --config examples/configs/node-b.toml \
  --target <node-a-uuid> \
  --kind event \
  --subject sensor.temperature \
  --payload '{"c":21.5}' \
  --content-type application/json
```

You should see Node A log:
```
INFO zap_net: decrypted inbound datagram of size 147 from 127.0.0.1:7001
INFO zap_node: received event envelope 'sensor.temperature' (ID: ...)
```

---

## 6. Driving Actions & Verifying Receipts

Send a thermostat setpoint action command:

```bash
cargo run -p zap-cli -- send --config examples/configs/node-b.toml \
  --target <node-a-uuid> \
  --action thermostat.setpoint \
  --payload '{"temperature_c":22.0}'
```

On Node A, ZAP will:
1. Verify the incoming envelope's signature.
2. Compile and execute `thermostat.wat` inside Wasmtime with a resource limit.
3. Write a signed execution receipt to `.zap/node-a-receipts`.

### Verify the Audit Trail
Check that Node A's receipts ledger has not been tampered with:

```bash
cargo run -p zap-cli -- receipts verify --dir .zap/node-a-receipts
```
Output:
```
Verification successful: verified 1 action receipt signatures.
```

---

## 7. Consensus-Gated Safety Stops (Proof-of-Action)

For critical factory operations, like emergency shutdowns, we do not want a compromised sender to trigger commands unilaterally. We enforce a Proof-of-Action (PoA) validation requiring validator attestations.

### Update Node A configuration
Add validator settings and a message policy to `examples/configs/node-a.toml`
to require 1 validator approval for safety actions:

```toml
[poa]
required_threshold = 1

[[poa.validators]]
node_id = "<node-b-uuid>"
public_key = "<node-b-public-key-base64>"

[message_policy]

[[message_policy.rules]]
kind = "action"
subject = "safety.*"
decision = "require_poa"
reason = "safety actions require validator quorum"
```

Restart the Node A daemon.

### Send a critical action
Try sending an emergency stop action from Node B:

```bash
cargo run -p zap-cli -- send --config examples/configs/node-b.toml \
  --target <node-a-uuid> \
  --kind action --subject safety.emergency_stop \
  --payload '{"reason":"operator_request"}' --content-type application/json \
  --requires-consensus --poa-network
```

ZAP will broadcast a `poa.attestation_request` to validator Node B, assemble the
response signatures into a `ZPOA` trailer, attach it to the `ZAP_` frame, and
dispatch it to Node A. Node A verifies the threshold signatures, enforces
`message_policy`, and executes the safety procedure safely.
