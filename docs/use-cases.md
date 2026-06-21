# ZAP: Real-World Use Cases

ZAP is a secure, low-latency, signed, and encrypted message dispatch protocol. It is designed to run efficiently on everything from embedded microcontrollers to cloud instances, providing cryptographic auditability and sandboxed driver execution.

Here are the primary real-world scenarios where ZAP adds significant value compared to generic alternatives (like MQTT, HTTP, or gRPC).

---

## 1. Multi-Agent AI Coordination & Auditable Workflows

In modern AI agent networks, multiple autonomous agents collaborate to solve complex tasks. These agents need to send commands, query state, and share context with each other.

### The Problem
Traditional RPC protocols (gRPC, HTTP) do not sign or attribute requests to their source cryptographically at the application level. There is no proof of who requested an action, what the exact model prompt/input was, or whether a response was modified in transit.

### How ZAP Solves It
- **Cryptographic Provenance**: Every message is wrapped in a `ZAP_` frame and signed by the sender's Ed25519 identity key.
- **Envelope Metadata**: The `ZENV` (Universal Envelope) matches action parameters, correlation IDs, and causation IDs, linking agent decision chains together.
- **Signed Receipts**: When a node executes an action, it logs a signed receipt to its binary journal, providing an immutable audit trail of agent behaviors.

```
┌───────────────┐                  ┌───────────────┐                  ┌────────────────┐
│ Planning Agent│ ──(Signed ZENV)─►│  Coder Agent  │ ──(Signed ZENV)─►│Executor Runtime│
│ Ed25519 Key A │                  │ Ed25519 Key B │                  │ Ed25519 Key C  │
└───────────────┘                  └───────────────┘                  └────────────────┘
```

---

## 2. Secure IoT, Robotics, & Industrial Control

Industrial settings require high reliability and low latency for sensor readings, command execution, and emergency stops.

### The Problem
MQTT and HTTP have significant protocol overhead, run over TCP (susceptible to HOL blocking), and rely solely on TLS. If the TLS connection drops or a broker is compromised, unauthorized commands can be injected. Furthermore, critical commands (like "shut down reactor") should never rely on a single compromise-prone client.

### How ZAP Solves It
- **Low-Latency Transport**: ZAP runs over encrypted UDP using ChaCha20-Poly1305, avoiding TCP handshake and head-of-line blocking overhead.
- **Proof-of-Action (PoA) Consensus**: High-risk operations (e.g., moving a robotic arm, opening a valve) require a `REQUIRES_CONSENSUS` flag. The receiver will reject the command unless it includes validator signatures meeting a configured threshold (e.g., 2 out of 3 validators must attest).
- **Anti-Replay**: Bounded nonce caches and clock-skew checks protect against signal interception and replay attacks.

---

## 3. Sandboxed Extensibility (Smart Home & Edge Devices)

Modern smart home hubs and edge gateways must support custom third-party integrations (drivers) to interface with new devices (Zigbee, Z-Wave, smart plugs).

### The Problem
Running third-party Python, JavaScript, or C plugins directly on a smart home hub exposes the entire system to memory leaks, crashes, and malicious credential theft.

### How ZAP Solves It
- **WASM Driver Sandboxing**: ZAP executes custom device drivers inside a restricted WebAssembly virtual machine (using `wasmtime`).
- **Resource Limits**: The host node enforces strict execution budgets (fuel/instructions), memory usage limits (e.g., 16 MB), and wall-clock execution timeouts.
- **Deny-by-Default Imports**: Guest WASM drivers cannot access the network, filesystem, environment, or system clock unless explicitly granted by the operator config and driver manifest.
- **Signed Manifests**: Drivers must have a signed manifest detailing the developer's identity, the compiled WASM hash, and requested capabilities, preventing supply chain tampering.

---

## 4. Auditable Microservices & Zero-Trust Cloud Architecture

In enterprise architectures, services need to communicate securely while maintaining a complete, verifiable log of all transactions.

### The Problem
Traditional logging databases can be tampered with (either by database administrators or intruders), making post-incident forensic investigations unreliable.

### How ZAP Solves It
- **Hash-Chained Memory Store**: ZAP's local memory crate (`zap-memory`) records key-value updates in an append-only binary journal where each entry references the cryptographic hash of the previous entry.
- **Tamper-Evident Logs**: Any modification, insertion, or deletion of past events breaks the hash chain and is immediately flagged by the node's verification routine.
- **Offline Export**: Journals can be verified offline and exported to JSONL archives for human review or long-term interchange.
