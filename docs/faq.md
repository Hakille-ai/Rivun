# rivun Protocol: Frequently Asked Questions

This document addresses common questions regarding the architecture, security model, performance characteristics, and intent of the rivun protocol.

---

## 1. rivun vs. Alternatives

### How does rivun compare to MQTT, gRPC, or NATS?
- **MQTT**: Designed for low-bandwidth client-broker message broker topologies. It relies on a central, single-point-of-failure broker, uses TCP (susceptible to HOL blocking), and lacks native payload signing or sandboxed processing. rivun is a decentralized, peer-to-peer UDP protocol with end-to-end cryptographic signatures and sandboxed edge execution.
- **gRPC**: A client-server RPC framework running over HTTP/2 (TCP). It has large protocol parsing overhead, lacks native multi-node consensus gates, and does not have built-in message policy gates or sandboxed driver execution.
- **NATS**: A high-performance pub-sub messaging system. While fast, it relies on NATS cluster servers and does not enforce end-to-end signature verification or action sandboxing at the protocol layer.

### Is rivun a replacement for TLS?
No. rivun uses **ChaCha20-Poly1305 authenticated encryption** at the application layer over UDP datagrams. It leverages elements from the Noise Protocol Framework for session setup. While TLS is designed for stream connection security, rivun is designed for secure, low-latency, packet-based messaging.

---

## 2. Architecture & Topology

### Does rivun require a central server?
No. rivun is fully **peer-to-peer (P2P)**. Nodes communicate directly with each other using configured peer tables. There is no broker, discovery server, or coordinator needed in the core architecture.

### What is the maximum payload size?
The @@@@rivun_HEADER@@WIRE@@ specification defines a maximum payload length of **16 MB**. However, because rivun transport uses UDP, large payloads will be fragmented. For low-latency edge applications, keeping payloads under the MTU (~1400 bytes) is highly recommended.

### Can I run rivun without WebAssembly (WASM)?
Yes. While rivun includes a sandboxed WASM driver runtime (`rivun-runtime`), the core protocol is a general message dispatch mechanism. You can use rivun solely as a secure peer-to-peer transport for raw data, events, or control messages without ever executing WASM.

---

## 3. Cryptography & Security

### Why Ed25519 instead of ECDSA or RSA?
Ed25519 signatures are extremely fast to generate and verify, have compact keys (32 bytes) and signatures (64 bytes), and are highly resistant to side-channel attacks.

### What is the purpose of the 8-byte `@@rivun_HEADER@@SIGN` signature hint?
To verify a full Ed25519 signature, rivun needs to run complex elliptic curve math, which is CPU-intensive. Under a Denial of Service (DoS) attack, a node could be overwhelmed by invalid signatures.
The `@@rivun_HEADER@@SIGN` field is a BLAKE3 hash of the signature. rivun checks this hint first. If the hint doesn't match the signature, the packet is rejected instantly without running the expensive Ed25519 verification.

### How does rivun prevent Replay Attacks?
rivun employs three mechanisms:
1. **Clock Skew checks**: Frames must have a timestamp within a configured window (default 5 minutes). Frames outside this window are rejected.
2. **Replay Cache**: rivun nodes maintain an in-memory cache of recently processed frame fingerprints (BLAKE3). If a duplicate fingerprint is received, it is immediately discarded.
3. **Durable windows**: deployments can persist transport nonces (`DurableNonceStore`) and frame fingerprints (`DurableReplayStore`) so replays cannot be replayed across process restarts. See [Security Model](security.md).

---

## 4. Proof-of-Action (PoA) Consensus

### What is Proof-of-Action?
Proof-of-Action (PoA) is a consensus gate for critical operations. When an action frame requires consensus, it cannot be executed unless it carries a `ZPOA` trailer containing cryptographic attestations (signatures) from a threshold number of designated validator nodes.

### Is this a blockchain?
No. rivun does not use a global distributed ledger, mining, or proof-of-stake. PoA is a lightweight, local, threshold-signature validator scheme designed to secure critical commands on a node-by-node basis.

### Is PACT a blockchain, payment system, or separate API?
No. PACT is a rivun profile for portable signed action records. A PACT record
captures identity, intent, consent, proof, terms, status, and revocation
evidence as `application/rivun-pact+json` inside normal `ZENV` envelopes. It uses
the existing BLAKE3 and Ed25519 trust stack and verifies offline through shared
fixtures and SDK helpers.

---

## 5. Extensibility & Runtime

### Why are host capabilities denied by default in WASM?
To enforce a zero-trust model. If a driver needs filesystem or network access, this must be explicitly requested in its manifest and approved by the operator in the node configuration. In ABI v1, all host imports are denied. The ABI v2 foundation exposes only scoped `rivun` imports (event emission, local memory interaction, device-call requests), and those remain inert until granted by the manifest, bounded by host-call byte limits, and enabled by node config gates such as `[memory] allow_driver_write = true`. Network, filesystem, clock, and environment access are still denied as broad ambient authority.

### Where does AI or natural-language planning run?
Outside rivun. Models, agents, or operator tools should produce strict typed
messages such as `kind=action`, `subject=safety.emergency_stop`, and a JSON
payload. rivun then enforces signatures, encryption, replay protection, routing,
message policy, PoA, and sandboxed execution deterministically.

