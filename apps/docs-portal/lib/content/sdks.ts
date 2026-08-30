import { DocPage } from '../types';

export const SDK_DOCS: DocPage[] = [
  {
    slug: ['sdks', 'rust'],
    path: '/docs/sdks/rust',
    title: 'Rust SDK Developer Manual',
    description: 'Comprehensive manual for building zero-overhead, memory-safe Rivun applications in Rust.',
    section: '7. 4 SDK Developer Manuals',
    subSection: 'SDK Manuals',
    headings: [
      { id: 'cargo-config', text: 'Cargo Configuration', level: 2 },
      { id: 'frame-builder', text: 'Frame & Envelope Builders', level: 2 },
      { id: 'async-transport', text: 'Async Tokio Transport', level: 2 },
      { id: 'driver-authoring', text: 'Authoring WASM Guest Drivers', level: 2 },
      { id: 'receipt-verification', text: 'MMR Proof Verification', level: 2 },
    ],
    multiLangSnippets: [
      {
        id: 'rust-manual-full',
        snippets: {
          rust: {
            title: 'client.rs',
            code: `use rivun_core::{RivunFrame, RivunFlags};\nuse rivun_crypto::Keypair;\nuse rivun_net::UdpTransport;\n\n#[tokio::main]\nasync fn main() -> Result<(), Box<dyn std::error::Error>> {\n    let keypair = Keypair::generate();\n    let transport = UdpTransport::bind("0.0.0.0:0").await?;\n    \n    let frame = RivunFrame::builder()\n        .flags(RivunFlags::SIGNED | RivunFlags::PRIORITY)\n        .source(keypair.node_id())\n        .target([0u8; 16])\n        .payload(br#"{"action":"PING"}"#)\n        .sign(&keypair)?;\n        \n    transport.send_to(&frame.to_bytes(), "127.0.0.1:9001").await?;\n    println!("Dispatched frame successfully!");\n    Ok(())\n}`,
          },
        },
      },
    ],
    rawContent: `
The **Rivun Rust SDK** provides low-level wire primitives, zero-copy memory parsing, and seamless Tokio async integration.
    `,
  },
  {
    slug: ['sdks', 'typescript'],
    path: '/docs/sdks/typescript',
    title: 'TypeScript SDK Developer Manual',
    description: 'Manual for Node.js, Bun, and browser applications using @rivun/sdk.',
    section: '7. 4 SDK Developer Manuals',
    subSection: 'SDK Manuals',
    headings: [
      { id: 'install-and-setup', text: 'Installation & Setup', level: 2 },
      { id: 'noble-crypto', text: 'WebCrypto & Noble Ed25519 Signing', level: 2 },
      { id: 'zenv-envelopes', text: 'Building & Parsing ZENV Envelopes', level: 2 },
      { id: 'pact-verification', text: 'PACT Record Verification in the Browser', level: 2 },
    ],
    multiLangSnippets: [
      {
        id: 'ts-manual-full',
        snippets: {
          typescript: {
            title: 'client.ts',
            code: `import { RivunKeypair, RivunEnvelope, RivunWireFrame, RivunFlags } from '@rivun/sdk';\n\nconst keypair = RivunKeypair.generate();\nconst envelope = RivunEnvelope.create({\n  kind: 7, // Action\n  subject: 'iot.actuator.set',\n  contentType: 'application/json',\n  body: JSON.stringify({ state: 'ACTIVE' }),\n});\n\nconst frame = RivunWireFrame.encode({\n  flags: RivunFlags.SIGNED | RivunFlags.REQUIRES_CONSENSUS,\n  sourceNode: keypair.nodeId,\n  payload: envelope.toBytes(),\n  keypair,\n});\n\nconsole.log('Signed frame ready for transmission:', frame.byteLength);`,
          },
        },
      },
    ],
    rawContent: `
The **TypeScript SDK** supports ESM and CommonJS, zero native C/C++ build requirements, and universal cryptographic execution across Node.js, Deno, Bun, and modern browsers.
    `,
  },
  {
    slug: ['sdks', 'python'],
    path: '/docs/sdks/python',
    title: 'Python SDK Developer Manual',
    description: 'Manual for Python 3.10+ with dataclasses, typing, and standard socket transport.',
    section: '7. 4 SDK Developer Manuals',
    subSection: 'SDK Manuals',
    headings: [
      { id: 'setup', text: 'Package Setup', level: 2 },
      { id: 'dataclasses', text: 'Dataclass Framing & Serialization', level: 2 },
      { id: 'agent-integration', text: 'AI Agent Framework Integration', level: 2 },
    ],
    multiLangSnippets: [
      {
        id: 'py-manual-full',
        snippets: {
          python: {
            title: 'agent_driver.py',
            code: `from rivun_sdk import Keypair, RivunFrame, RivunEnvelope, RivunFlags\n\nkp = Keypair.generate()\nenv = RivunEnvelope.create_action(subject="llm.agent.action", body={"decision": "APPROVE"})\nframe = RivunFrame.create(source=kp.node_id, payload=env.encode(), flags=RivunFlags.SIGNED, keypair=kp)\nprint(f"Wire Frame Hash: {frame.blake3_digest().hex()}")`,
          },
        },
      },
    ],
    rawContent: `
The **Python SDK** bridges Python AI systems (LangChain, AutoGen, CrewAI) with the verified Rivun consensus mesh.
    `,
  },
  {
    slug: ['sdks', 'go'],
    path: '/docs/sdks/go',
    title: 'Go SDK Developer Manual',
    description: 'Idiomatic Go package with BLAKE3 hashing, Ed25519 verification, and high-concurrency dispatchers.',
    section: '7. 4 SDK Developer Manuals',
    subSection: 'SDK Manuals',
    headings: [
      { id: 'go-install', text: 'Installation', level: 2 },
      { id: 'binary-codecs', text: 'Binary Codecs & Transports', level: 2 },
      { id: 'microservices', text: 'Microservices & Worker Pools', level: 2 },
    ],
    multiLangSnippets: [
      {
        id: 'go-manual-full',
        snippets: {
          go: {
            title: 'main.go',
            code: `package main\n\nimport (\n\t"fmt"\n\t"github.com/rivun/rivun/sdks/go/pkg/rivun"\n)\n\nfunc main() {\n\tkp, _ := rivun.GenerateKeypair()\n\tenv := rivun.NewEnvelope(rivun.KindAction, "system.backup", "application/json", []byte("{}"))\n\tframe, _ := rivun.NewFrameBuilder().SetFlags(rivun.FlagSigned).SetSource(kp.NodeIDBytes()).SetPayload(env.Encode()).Sign(kp)\n\tfmt.Printf("Go Frame Length: %d\\n", len(frame.Bytes()))\n}`,
          },
        },
      },
    ],
    rawContent: `
The **Go SDK** delivers high-performance networking for cloud infrastructure, Kubernetes sidecars, and edge gateways.
    `,
  },
  {
    slug: ['sdks', 'conformance-matrix'],
    path: '/docs/sdks/conformance-matrix',
    title: 'Cross-SDK Conformance & Test Fixtures Matrix',
    description: '11 shared JSON test vectors proving bit-for-bit wire and cryptographic equivalence across all 4 SDKs.',
    section: '7. 4 SDK Developer Manuals',
    subSection: 'Conformance',
    headings: [
      { id: 'test-fixtures', text: 'The 11 Shared Test Fixtures', level: 2 },
      { id: 'conformance-table', text: 'Language Conformance Matrix', level: 2 },
      { id: 'running-conformance', text: 'Running Conformance Tests', level: 2 },
    ],
    callouts: [
      {
        type: 'invariant',
        title: 'Bit-for-Bit Determinism Invariant',
        content: 'All 4 SDKs produce byte-identical binary outputs for identical frame inputs, confirmed by SHA-256 and BLAKE3 fixture checksums.',
      },
    ],
    rawContent: `
### The 11 Shared JSON Test Fixtures (\`fixtures/\`)
1. \`01_wire_header_minimal.json\`: 64-byte header encoding and endianness.
2. \`02_wire_header_flags.json\`: Bitflag combinations and parsing.
3. \`03_zenv_8_kinds.json\`: Universal envelope encoding across all 8 message kinds.
4. \`04_ed25519_zsig.json\`: Detached signature transcript and 72-byte trailer.
5. \`05_zpoa_multisig.json\`: 40 + 80*K byte PoA trailer and attestation verification.
6. \`06_zapd_chacha_aead.json\`: AEAD ChaCha20-Poly1305 encryption and 12-byte nonces.
7. \`07_blake3_domains.json\`: Domain-separated hash transcripts.
8. \`08_pact_canonical_json.json\`: Deterministic JSON key sorting and PACT signatures.
9. \`09_mmr_peak_bagging.json\`: Incremental MMR leaf insertion and peak-bagging roots.
10. \`10_secret_redactor_regex.json\`: Redaction patterns for tokens, keys, and credentials.
11. \`11_driver_abi_v1.json\`: Linear memory packed result 64-bit return layout.

### Conformance Status Table
| Fixture | Rust SDK | TypeScript SDK | Python SDK | Go SDK |
|---|---|---|---|---|
| 01 - Header Minimal | 100% PASS | 100% PASS | 100% PASS | 100% PASS |
| 02 - Header Flags | 100% PASS | 100% PASS | 100% PASS | 100% PASS |
| 03 - ZENV 8 Kinds | 100% PASS | 100% PASS | 100% PASS | 100% PASS |
| 04 - Ed25519 ZSIG | 100% PASS | 100% PASS | 100% PASS | 100% PASS |
| 05 - ZPOA Multi-Sig | 100% PASS | 100% PASS | 100% PASS | 100% PASS |
| 06 - ZAPD AEAD | 100% PASS | 100% PASS | 100% PASS | 100% PASS |
| 07 - BLAKE3 Domains | 100% PASS | 100% PASS | 100% PASS | 100% PASS |
| 08 - PACT Canonical | 100% PASS | 100% PASS | 100% PASS | 100% PASS |
| 09 - MMR Peak Bagging | 100% PASS | 100% PASS | 100% PASS | 100% PASS |
| 10 - Secret Redactor | 100% PASS | 100% PASS | 100% PASS | 100% PASS |
| 11 - Driver ABI v1 | 100% PASS | 100% PASS | 100% PASS | 100% PASS |
    `,
  },
];
