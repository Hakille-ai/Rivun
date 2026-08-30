"use client";

import React, { useState } from "react";
import {
  KeyRound,
  Lock,
  Zap,
  Cpu,
  Layers,
  ShieldCheck,
  CheckCircle2,
  Code2,
  Terminal,
  ArrowRight,
  Sparkles,
  GitBranch,
} from "lucide-react";

interface InnovationItem {
  id: string;
  tabTitle: string;
  badge: string;
  title: string;
  icon: React.ElementType;
  description: string;
  mathEquation?: string;
  keyFeatures: string[];
  codeSample: {
    language: string;
    code: string;
  };
  deepDiveStats: Array<{ label: string; value: string }>;
}

const INNOVATIONS: InnovationItem[] = [
  {
    id: "ed25519-crypto",
    tabTitle: "Ed25519 & Blinded Commitments",
    badge: "CRYPTOGRAPHIC FABRIC",
    title: "Zero-Trust Identity, Deterministic UUIDv8 & Blinded Action Commitments",
    icon: KeyRound,
    description:
      "Every Rivun node derives its canonical identity directly from its Ed25519 public key using BLAKE3 domain separation. Frames include an 8-byte fast-rejection signature hint that discards forged or malformed frames in sub-microsecond CPU cycles without executing full curve math.",
    mathEquation: `\\text{node\\_id} = \\text{UUIDv8}(\\text{blake3}(\\text{"Rivun-NODE-ID-v1"} \\,|\\,\\, \\text{pubkey})[0..16])`,
    keyFeatures: [
      "Sub-microsecond fast rejection using 8-byte BLAKE3 signature hints (ZAP_SIGN)",
      "Zero ambient identity assumption: UUIDv8 mathematically derived from public key",
      "Salted blinded commitments for sensitive payloads (Rivun-BLINDED-COMMITMENT-v1)",
      "Universal canonical JSON sorting & domain-separated signing transcripts",
    ],
    codeSample: {
      language: "rust",
      code: `use rivun_crypto::{Keypair, sign_frame, derive_node_id};
use rivun_core::RivunHeader;

let keypair = Keypair::generate();
let node_id = derive_node_id(keypair.public_key());

// Fast-rejection 8-byte signature hint embedded directly into wire header
let (header, auth_trailer) = sign_frame(&keypair, &signing_prefix, &payload)?;
assert_eq!(header.magic, 0x5A41505F); // "ZAP_"
assert!(auth_trailer.verify(&signing_prefix, &payload, keypair.public_key()).is_ok());`,
    },
    deepDiveStats: [
      { label: "Signature Verification", value: "32 µs / op" },
      { label: "Fast-Hint Rejection", value: "0.12 µs / frame" },
      { label: "Identity Derivation", value: "RFC 4122 UUIDv8" },
      { label: "Domain Separation", value: "11 Isolated Transcripts" },
    ],
  },
  {
    id: "chacha20-transport",
    tabTitle: "ChaCha20-Poly1305 AEAD",
    badge: "LOW-LATENCY TRANSPORT",
    title: "High-Throughput Encrypted UDP Datagram Framing (ZAPD)",
    icon: Lock,
    description:
      "Encapsulates raw wire frames into 52-byte ZAPD datagram headers encrypted with authenticated ChaCha20-Poly1305 AEAD. Nonce replay protection combines a high-speed memory LRU sliding window with an append-only write-ahead log for instant crash recovery.",
    mathEquation: `\\text{ZAPD\\_Header}(52\\text{B}) = \\text{Magic}(\\text{"ZAPD"}) \\,|\\,\\, \\text{Version} \\,|\\,\\, \\text{Source\\_UUID} \\,|\\,\\, \\text{Target\\_UUID} \\,|\\,\\, \\text{Nonce}(12\\text{B})`,
    keyFeatures: [
      "Zero-handshake connectionless UDP transmission with instant AEAD authentication",
      "12-byte nonces combining 4-byte session seed with 8-byte monotonic sequence counters",
      "Dual-layer replay guard: Lock-free atomic LRU cache + Durable WAL on disk",
      "Microsecond jitter resilience with kernel bypass socket pooling (SO_REUSEPORT)",
    ],
    codeSample: {
      language: "rust",
      code: `use rivun_net::{RivunEndpoint, DatagramEnvelope, NonceReplayCache};

let endpoint = RivunEndpoint::bind("0.0.0.0:9100").await?;
let mut datagram = DatagramEnvelope::encrypt(&session_key, &source_id, &target_id, raw_frame)?;

// Transmit zero-copy UDP datagram
endpoint.send_to(&datagram.as_bytes(), peer_addr).await?;`,
    },
    deepDiveStats: [
      { label: "Header Overhead", value: "52 Bytes" },
      { label: "AEAD Encryption Time", value: "1.4 µs / 1KB" },
      { label: "Replay Window", value: "65,536 Nonces" },
      { label: "WAL Monotonic Skew", value: "< 30s Window" },
    ],
  },
  {
    id: "bft-consensus",
    tabTitle: "Proof-of-Action BFT Consensus",
    badge: "DETERMINISTIC CONSENSUS",
    title: "Deterministic 2-Phase BFT Quorum Mesh & Automatic Slashing",
    icon: Zap,
    description:
      "Guarantees deterministic action execution across distributed validator quorums. Requires threshold T <= N attestations before committing mutating commands to hardware or APIs. Automatic equivocation detectors slash malicious double-signing nodes in real-time.",
    mathEquation: `T = \\lfloor 2N / 3 \\rfloor + 1, \\quad \\text{Equivocation}(v_1, v_2) \\implies \\text{Slash}(Node_{\\text{UUID}})`,
    keyFeatures: [
      "2-Phase consensus pipeline: Propose -> Prevote -> Precommit -> Commit Certificate",
      "Threshold signature bitmask indexing matching exact participating validator UUIDs",
      "Zero-tolerance equivocation detector with automatic public slashing proofs",
      "Variable quorum configurations supporting 1-of-1 fast edge to 7-of-10 multi-region",
    ],
    codeSample: {
      language: "rust",
      code: `use rivun_net::{BftConsensusEngine, SwarmProposal, SwarmVote};

let mut engine = BftConsensusEngine::new(validator_set, local_keypair);
let proposal = engine.create_proposal(epoch, round, frame_digest)?;

// Validators broadcast Prevotes and Precommits
let polka_reached = engine.handle_vote(SwarmVote::Prevote(vote_sig))?;
if polka_reached {
    let commit_cert = engine.seal_commit_certificate()?;
    log::info!("PoA Quorum Sealed: {} signatures aggregated", commit_cert.count());
}`,
    },
    deepDiveStats: [
      { label: "Quorum Threshold", value: "T = floor(2N/3) + 1" },
      { label: "Round Latency", value: "0.45 ms (LAN) / 12ms (WAN)" },
      { label: "Slashing Evidence", value: "Tamper-Evident BLAKE3" },
      { label: "Certificate Bitmask", value: "ceil(N / 8) Bytes" },
    ],
  },
  {
    id: "wasm-runtime",
    tabTitle: "Wasmtime Sandboxing & Fuel",
    badge: "DETERMINISTIC RUNTIME",
    title: "Instruction-Metered WASM Guest Isolation & Zero-Copy SPSC Ring Buffers",
    icon: Cpu,
    description:
      "Untrusted agent guest drivers execute inside isolated WebAssembly sandboxes managed by Wasmtime. Hard fuel limits throttle CPU instruction counts, wall-clock epoch interruptions kill runaways, and lock-free SPSC ring buffers achieve zero-copy microsecond IPC.",
    mathEquation: `\\text{Host-Guest ABI}: \\text{rivun\\_execute}(ptr, len) \\to (\\text{res\\_ptr} \\ll 32) \\mid \\text{res\\_len}`,
    keyFeatures: [
      "Zero ambient system access: No network, filesystem, or clock without explicit host bridge",
      "Deterministic CPU instruction fuel metering with configurable budgets (e.g. 1M fuel)",
      "Single-Producer Single-Consumer (SPSC) atomic circular ring buffers for zero-copy IPC",
      "Multi-driver pipeline chaining: Input -> Driver1 -> Driver2 -> ActionReceipt",
    ],
    codeSample: {
      language: "rust",
      code: `use rivun_runtime::{WasmExecutor, ExecutionLimits, SpscRingBuffer};

let limits = ExecutionLimits { max_memory_bytes: 64 * 1024 * 1024, fuel_limit: 1_000_000 };
let executor = WasmExecutor::load_module(&wasm_bytes, limits)?;

// Execute guest driver with strictly bounded memory and instruction fuel
let receipt = executor.execute_action("plc.coil.write", payload_bytes)?;
assert!(receipt.fuel_consumed <= 1_000_000);`,
    },
    deepDiveStats: [
      { label: "Fuel Precision", value: "Exact 1-Instruction Tick" },
      { label: "Memory Isolation", value: "64 MiB Strict Cap" },
      { label: "Epoch Interruption", value: "50ms Hard Wall-Clock" },
      { label: "SPSC Ring Buffer Latency", value: "< 50 ns / message" },
    ],
  },
  {
    id: "mmr-accumulators",
    tabTitle: "Merkle Mountain Ranges (MMR)",
    badge: "IMMUTABLE STORAGE",
    title: "Incremental Peak-Bagged Merkle Mountain Ranges & Inclusion Proofs",
    icon: Layers,
    description:
      "All execution receipts are appended into continuous Merkle Mountain Ranges (.zmmr segments). Binary carry-over subtree merging provides logarithmic O(log N) inclusion proofs, monotonic exclusion proofs, and lightweight periodic root seals.",
    mathEquation: `\\text{parent\\_hash} = \\text{blake3}(\\text{left\\_child} \\,|\\,\\, \\text{right\\_child}), \\quad \\text{Proof\\_Size} = O(\\log_2 N)`,
    keyFeatures: [
      "Append-only logarithmic accumulator supporting millions of action receipts per second",
      "Compact single-leaf and multi-leaf batch inclusion proofs for zero-knowledge auditors",
      "Monotonic exclusion proofs for verifying non-membership of disputed transactions",
      "Air-gapped offline mathematical verification without running a full archive node",
    ],
    codeSample: {
      language: "rust",
      code: `use rivun_ledger::{IncrementalMmr, ActionReceipt, MmrInclusionProof};

let mut mmr = IncrementalMmr::open("ledger/receipts.zmmr")?;
let leaf_idx = mmr.append_receipt(&action_receipt)?;

// Generate O(log N) cryptographic proof of inclusion
let proof = mmr.generate_inclusion_proof(leaf_idx)?;
let root_hash = mmr.bagged_root_hash();
assert!(proof.verify(root_hash, &action_receipt.digest()));`,
    },
    deepDiveStats: [
      { label: "Proof Complexity", value: "O(log2 N)" },
      { label: "1 Million Receipts Proof", value: "640 Bytes" },
      { label: "Binary File Magic", value: "0x5A41504D4D523031" },
      { label: "Peak Bagging Direction", value: "MSB (Bit 63) -> LSB" },
    ],
  },
];

export function ProtocolInnovations() {
  const [activeTab, setActiveTab] = useState<string>("ed25519-crypto");

  const currentItem = INNOVATIONS.find((item) => item.id === activeTab) || INNOVATIONS[0];
  const Icon = currentItem.icon;

  return (
    <section id="innovations" className="py-24 relative overflow-hidden">
      <div className="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8">
        {/* Section Header */}
        <div className="text-center max-w-3xl mx-auto mb-12 space-y-4">
          <div className="inline-flex items-center gap-2 px-3 py-1 rounded-full bg-[#5B8CFF]/10 border border-[#5B8CFF]/20 text-[#5B8CFF] text-xs font-semibold">
            <Sparkles className="w-3.5 h-3.5" />
            <span>5 CORE PROTOCOL PILLARS</span>
          </div>
          <h2 className="text-3xl sm:text-4xl font-extrabold tracking-tight text-white">
            Cryptographic Innovations Built for Absolute Zero-Trust
          </h2>
          <p className="text-sm sm:text-base text-[#9AA1AE]">
            Engineered from the ground up in Rust without legacy baggage. Explore the mathematical
            foundations powering Rivun&apos;s sub-millisecond execution and audit fabric.
          </p>
        </div>

        {/* Horizontal Tab Navigation */}
        <div className="flex items-center justify-start lg:justify-center gap-2 overflow-x-auto pb-4 mb-8 no-scrollbar">
          {INNOVATIONS.map((item) => (
            <button
              key={item.id}
              onClick={() => setActiveTab(item.id)}
              className={`px-4 py-2.5 rounded-xl text-xs font-semibold whitespace-nowrap transition-all border flex items-center gap-2 ${
                activeTab === item.id
                  ? "bg-[#181B22] text-[#5B8CFF] border-[#5B8CFF]/40 shadow-glow"
                  : "bg-[#111318]/60 text-[#9AA1AE] border-[#22262F] hover:text-white hover:border-[#3A4150]"
              }`}
            >
              <item.icon className="w-4 h-4" />
              <span>{item.tabTitle}</span>
            </button>
          ))}
        </div>

        {/* Active Feature Deep Dive Showcase */}
        <div className="bg-[#111318] border border-[#22262F] rounded-2xl p-6 lg:p-10 shadow-2xl relative overflow-hidden">
          <div className="grid grid-cols-1 lg:grid-cols-12 gap-8 items-start">
            {/* Left Column: Narrative & Technical Breakdown (7 cols) */}
            <div className="lg:col-span-7 space-y-6">
              <div>
                <span className="px-2.5 py-1 rounded-md text-[10px] font-mono font-bold bg-[#5B8CFF]/15 text-[#5B8CFF] border border-[#5B8CFF]/30 tracking-wider">
                  {currentItem.badge}
                </span>
                <h3 className="text-xl sm:text-2xl font-bold text-white tracking-tight mt-3">
                  {currentItem.title}
                </h3>
                <p className="text-sm text-[#9AA1AE] leading-relaxed mt-3">
                  {currentItem.description}
                </p>
              </div>

              {/* Mathematical Equation Callout */}
              {currentItem.mathEquation && (
                <div className="p-3.5 rounded-xl bg-[#0A0B0D] border border-[#22262F] font-mono text-xs text-[#3DD68C] flex items-center gap-3">
                  <span className="text-[#6B7280] font-bold">MATH:</span>
                  <code className="text-xs break-all">{currentItem.mathEquation}</code>
                </div>
              )}

              {/* Key Architectural Invariants */}
              <div className="space-y-2.5">
                <h4 className="text-xs font-semibold text-white uppercase tracking-wider">
                  Architectural Guarantees
                </h4>
                <div className="grid grid-cols-1 sm:grid-cols-2 gap-2.5">
                  {currentItem.keyFeatures.map((feat, fIdx) => (
                    <div
                      key={fIdx}
                      className="p-3 rounded-lg bg-[#181B22]/70 border border-[#22262F] flex items-start gap-2.5 text-xs text-[#9AA1AE]"
                    >
                      <CheckCircle2 className="w-4 h-4 text-[#3DD68C] shrink-0 mt-0.5" />
                      <span>{feat}</span>
                    </div>
                  ))}
                </div>
              </div>

              {/* Metric Highlights */}
              <div className="grid grid-cols-2 sm:grid-cols-4 gap-3 pt-2">
                {currentItem.deepDiveStats.map((stat, sIdx) => (
                  <div key={sIdx} className="p-3 rounded-lg bg-[#14171F] border border-[#22262F]">
                    <span className="text-[10px] text-[#6B7280] block uppercase font-mono">{stat.label}</span>
                    <span className="text-xs font-bold text-white font-mono mt-0.5 block">{stat.value}</span>
                  </div>
                ))}
              </div>
            </div>

            {/* Right Column: Code Snippet Card (5 cols) */}
            <div className="lg:col-span-5 bg-[#0A0B0D] border border-[#22262F] rounded-xl overflow-hidden shadow-inner flex flex-col">
              <div className="flex items-center justify-between px-4 py-2.5 bg-[#14171F] border-b border-[#22262F]">
                <div className="flex items-center gap-2">
                  <div className="flex gap-1.5">
                    <span className="w-2.5 h-2.5 rounded-full bg-rose-500/80" />
                    <span className="w-2.5 h-2.5 rounded-full bg-amber-500/80" />
                    <span className="w-2.5 h-2.5 rounded-full bg-emerald-500/80" />
                  </div>
                  <span className="text-[11px] font-mono text-[#9AA1AE] ml-2">
                    {currentItem.codeSample.language}.rs
                  </span>
                </div>
                <span className="text-[10px] font-mono text-[#6B7280]">Rivun Engine API</span>
              </div>

              <div className="p-4 font-mono text-xs overflow-x-auto text-[#9AA1AE] leading-relaxed">
                <pre className="text-gray-300">
                  <code>{currentItem.codeSample.code}</code>
                </pre>
              </div>
            </div>
          </div>
        </div>
      </div>
    </section>
  );
}
