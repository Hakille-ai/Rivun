import Image from 'next/image';

export default function DocsPage() {
  return (
    <>
      <div className="relative rounded-2xl overflow-hidden mb-10 border border-zinc-800 bg-zinc-950/20 p-8 shadow-lg min-h-[220px] flex items-center">
        <Image 
          src="/images/zap_docs_banner.png" 
          alt="Developer Console" 
          fill
          style={{ objectFit: 'cover' }}
          className="opacity-25 absolute inset-0 z-0"
          priority
        />
        <div className="absolute inset-0 bg-gradient-to-r from-black via-black/70 to-transparent z-5"></div>
        <div className="relative z-10 max-w-xl">
          <span className="text-xs font-semibold uppercase tracking-wider text-blue-400 mb-2 block">Technical Reference</span>
          <h1 className="text-3xl font-bold tracking-tight text-white m-0">ZAP Protocol Documentation</h1>
          <p className="text-zinc-400 text-sm mt-2">
            Deep technical specifications, runtime isolation models, and deployment guides for the Zero-trust Autonomous Protocol.
          </p>
        </div>
      </div>

      <p className="lead">ZAP is a universal low-latency protocol independent of AI models/LLM providers. It provides a secure, deterministic, and extremely fast transport layer for distributed systems.</p>
      
      <h2>What is ZAP?</h2>
      <p>ZAP stands for the <strong>Zero-trust Autonomous Protocol</strong> (or conceptually, a fast action-oriented transport). It is designed to replace legacy broker-based protocols (like MQTT) and heavy connection-oriented protocols (like gRPC) in environments where latency, cryptographic provenance, and execution safety are critical.</p>

      <h2>Key Concepts</h2>
      <ul>
        <li><strong>Universal Envelope (ZENV):</strong> A typed 74-byte header supporting data, events, commands, queries, responses, streams, actions, and control messages.</li>
        <li><strong>Wire Frame:</strong> A 64-byte big-endian header that prefixes every message.</li>
        <li><strong>Cryptographic Provenance:</strong> Every frame is signed with an Ed25519 keypair. The identity is derived from the public key using a domain-separated BLAKE3 hash.</li>
        <li><strong>Proof-of-Action (PoA):</strong> A consensus mechanism requiring multiple validators to cryptographically attest to an action before a node executes it.</li>
        <li><strong>WASM Runtime:</strong> Actions are executed by sandboxed WebAssembly drivers. Host imports are deny-by-default, preventing unwanted network or filesystem access.</li>
        <li><strong>Decentralized Transport:</strong> ChaCha20-Poly1305 encrypted UDP datagrams with 96-bit nonces to prevent replay attacks.</li>
      </ul>
    </>
  );
}
