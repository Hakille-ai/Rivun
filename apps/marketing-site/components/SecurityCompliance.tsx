"use client";

import React, { useState } from "react";
import {
  ShieldCheck,
  Lock,
  FileCheck2,
  CheckCircle2,
  Cpu,
  Clock,
  ArrowRight,
  Sparkles,
  Search,
  Check,
  Key,
  Database,
  Layers,
} from "lucide-react";

interface ProvenanceStage {
  step: number;
  name: string;
  hashSymbol: string;
  source: string;
  mockHash: string;
  description: string;
}

const PROVENANCE_STAGES: ProvenanceStage[] = [
  {
    step: 1,
    name: "Agent Intent Signed",
    hashSymbol: "H_intent",
    source: "Agent Client (UUID v8)",
    mockHash: "blake3:8f2a41d9c0e2b14470984...",
    description: "The agent cryptographically signs its high-level objective and required capabilities in a ZENV envelope.",
  },
  {
    step: 2,
    name: "Capability Negotiation",
    hashSymbol: "H_negotiation",
    source: "Mesh Gateway Router",
    mockHash: "blake3:1c99f4302ba4098fe2331...",
    description: "Verifies whether the target subsystem supports the requested capability set and dynamic protocol version.",
  },
  {
    step: 3,
    name: "Policy Engine Evaluation",
    hashSymbol: "H_policy",
    source: "Deterministic Evaluator",
    mockHash: "blake3:67e1a09d9402ffbb88421...",
    description: "Evaluates the signed policy bundle AST. Checks rate limits, parameters, and determines if BFT PoA is required.",
  },
  {
    step: 4,
    name: "2-Phase BFT PoA Quorum",
    hashSymbol: "H_consensus",
    source: "Validator Mesh (T=2)",
    mockHash: "blake3:bb39c122049dffa084128...",
    description: "Consensus validators execute Prevote and Precommit phases, producing an aggregated multi-signature certificate.",
  },
  {
    step: 5,
    name: "Sandboxed WASM Execution",
    hashSymbol: "H_driver",
    source: "Wasmtime Guest Engine",
    mockHash: "blake3:34ef911082aa990145be7...",
    description: "The isolated driver executes under 1,000,000 fuel limit and outputs packed results via zero-copy SPSC ring buffer.",
  },
  {
    step: 6,
    name: "Action Receipt Generation",
    hashSymbol: "H_receipt",
    source: "Incremental MMR Ledger",
    mockHash: "blake3:fa923004bb18429188e40...",
    description: "Constructs immutable ActionReceipt with causal chain linkage and appends to .zjseg journal segment.",
  },
  {
    step: 7,
    name: "Merkle Mountain Range Root Seal",
    hashSymbol: "H_root",
    source: "Global MMR Accumulator",
    mockHash: "blake3:90214aefbc901844991aa...",
    description: "Logarithmic peak bagging folds active mountain peaks into the authoritative cryptographic root seal.",
  },
];

export function SecurityCompliance() {
  const [activeStage, setActiveStage] = useState(1);
  const [verifiedAll, setVerifiedAll] = useState(false);

  const handleVerifyChain = () => {
    setVerifiedAll(true);
    setTimeout(() => {
      setActiveStage(7);
    }, 400);
  };

  return (
    <section id="security" className="py-24 relative overflow-hidden">
      <div className="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8">
        {/* Section Header */}
        <div className="text-center max-w-3xl mx-auto mb-16 space-y-4">
          <div className="inline-flex items-center gap-2 px-3 py-1 rounded-full bg-[#3DD68C]/10 border border-[#3DD68C]/20 text-[#3DD68C] text-xs font-semibold">
            <ShieldCheck className="w-3.5 h-3.5" />
            <span>ENTERPRISE SECURITY & PROOFS</span>
          </div>
          <h2 className="text-3xl sm:text-4xl font-extrabold tracking-tight text-white">
            Mathematical Non-Repudiation & &lt;0.8ms SLA Guarantees
          </h2>
          <p className="text-sm sm:text-base text-[#9AA1AE]">
            Built for mission-critical production. From SOC2 Type II and HIPAA compliance to 
            air-gapped offline Merkle proofs, Rivun delivers mathematical auditability at microsecond speeds.
          </p>
        </div>

        {/* 4 Compliance Standards Grid */}
        <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-4 mb-16">
          <div className="p-5 rounded-2xl bg-[#111318] border border-[#22262F] relative group hover:border-[#5B8CFF]/40 transition-all">
            <div className="flex items-center justify-between mb-3">
              <span className="font-mono text-xs font-bold text-[#5B8CFF] bg-[#5B8CFF]/15 px-2 py-0.5 rounded border border-[#5B8CFF]/30">
                SOC2 TYPE II
              </span>
              <CheckCircle2 className="w-4 h-4 text-[#3DD68C]" />
            </div>
            <h4 className="text-base font-bold text-white mb-1">Continuous Auditability</h4>
            <p className="text-xs text-[#9AA1AE] leading-relaxed">
              Automated Merkle inclusion proofs replace manual log gathering with mathematical certainty.
            </p>
          </div>

          <div className="p-5 rounded-2xl bg-[#111318] border border-[#22262F] relative group hover:border-[#3DD68C]/40 transition-all">
            <div className="flex items-center justify-between mb-3">
              <span className="font-mono text-xs font-bold text-[#3DD68C] bg-[#3DD68C]/15 px-2 py-0.5 rounded border border-[#3DD68C]/30">
                HIPAA & PHI
              </span>
              <CheckCircle2 className="w-4 h-4 text-[#3DD68C]" />
            </div>
            <h4 className="text-base font-bold text-white mb-1">Air-Gapped Privacy</h4>
            <p className="text-xs text-[#9AA1AE] leading-relaxed">
              Client-side secret blinding ensures zero PHI leakage while retaining verifiable audit receipts.
            </p>
          </div>

          <div className="p-5 rounded-2xl bg-[#111318] border border-[#22262F] relative group hover:border-purple-500/40 transition-all">
            <div className="flex items-center justify-between mb-3">
              <span className="font-mono text-xs font-bold text-purple-400 bg-purple-500/15 px-2 py-0.5 rounded border border-purple-500/30">
                ISO 27001
              </span>
              <CheckCircle2 className="w-4 h-4 text-[#3DD68C]" />
            </div>
            <h4 className="text-base font-bold text-white mb-1">ISMS Cryptographic Core</h4>
            <p className="text-xs text-[#9AA1AE] leading-relaxed">
              Fail-closed policy ASTs and zero-trust sovereign operator keys guarantee non-repudiation.
            </p>
          </div>

          <div className="p-5 rounded-2xl bg-[#111318] border border-[#22262F] relative group hover:border-amber-500/40 transition-all">
            <div className="flex items-center justify-between mb-3">
              <span className="font-mono text-xs font-bold text-amber-400 bg-amber-500/15 px-2 py-0.5 rounded border border-amber-500/30">
                GDPR PRIVACY
              </span>
              <CheckCircle2 className="w-4 h-4 text-[#3DD68C]" />
            </div>
            <h4 className="text-base font-bold text-white mb-1">Cryptographic Erasure</h4>
            <p className="text-xs text-[#9AA1AE] leading-relaxed">
              Support for hash-tombstoned memory records and right-to-be-forgotten without breaking tree integrity.
            </p>
          </div>
        </div>

        {/* Interactive Mathematical Offline Verification Proof Simulator */}
        <div className="bg-[#111318] border border-[#22262F] rounded-2xl p-6 lg:p-8 shadow-2xl">
          <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-4 pb-6 border-b border-[#22262F]">
            <div>
              <div className="flex items-center gap-2">
                <h3 className="text-lg font-bold text-white">
                  Cryptographic Causal Provenance Engine
                </h3>
                <span className="px-2 py-0.5 text-[10px] font-mono font-bold bg-[#3DD68C]/15 text-[#3DD68C] border border-[#3DD68C]/30 rounded">
                  7-STAGE HASH CHAIN
                </span>
              </div>
              <p className="text-xs text-[#9AA1AE] font-mono mt-0.5">
                H_intent → H_negotiation → H_policy → H_consensus → H_driver → H_receipt → H_root
              </p>
            </div>

            <button
              onClick={handleVerifyChain}
              className="px-4 py-2 text-xs font-semibold text-white bg-[#3DD68C] hover:bg-[#34BE7B] text-black rounded-xl shadow-glow-emerald transition-all flex items-center gap-2 font-sans"
            >
              <CheckCircle2 className="w-3.5 h-3.5 text-black" />
              <span>Verify Offline Mathematical Proof</span>
            </button>
          </div>

          {/* Stepper Chain Progress */}
          <div className="grid grid-cols-2 sm:grid-cols-4 lg:grid-cols-7 gap-2 my-6">
            {PROVENANCE_STAGES.map((st) => (
              <button
                key={st.step}
                onClick={() => setActiveStage(st.step)}
                className={`p-3 rounded-xl border text-left transition-all ${
                  activeStage === st.step
                    ? "bg-[#181B22] border-[#5B8CFF] shadow-glow"
                    : "bg-[#14171F] border-[#22262F] hover:border-[#3A4150]"
                }`}
              >
                <div className="flex items-center justify-between mb-1">
                  <span className="font-mono text-[10px] text-[#5B8CFF] font-bold">{st.hashSymbol}</span>
                  {(verifiedAll || activeStage >= st.step) && (
                    <Check className="w-3 h-3 text-[#3DD68C]" />
                  )}
                </div>
                <span className="text-[11px] font-bold text-white truncate block">{st.name}</span>
                <span className="text-[10px] text-[#6B7280] font-mono block">Stage 0{st.step}</span>
              </button>
            ))}
          </div>

          {/* Active Stage Inspection Box */}
          <div className="p-6 rounded-xl bg-[#0A0B0D] border border-[#22262F] grid grid-cols-1 lg:grid-cols-12 gap-6 items-center">
            <div className="lg:col-span-8 space-y-3">
              <div className="flex items-center gap-2">
                <span className="px-2 py-0.5 rounded bg-[#5B8CFF]/15 text-[#5B8CFF] border border-[#5B8CFF]/30 font-mono text-[10px] font-bold">
                  STAGE 0{activeStage}: {PROVENANCE_STAGES[activeStage - 1].hashSymbol}
                </span>
                <span className="text-xs text-[#9AA1AE] font-mono">
                  Source: {PROVENANCE_STAGES[activeStage - 1].source}
                </span>
              </div>

              <h4 className="text-base font-bold text-white">
                {PROVENANCE_STAGES[activeStage - 1].name}
              </h4>

              <p className="text-xs text-[#9AA1AE] leading-relaxed">
                {PROVENANCE_STAGES[activeStage - 1].description}
              </p>

              <div className="p-3 bg-[#111318] rounded-lg border border-[#22262F] font-mono text-xs text-[#3DD68C] flex items-center justify-between">
                <span>{PROVENANCE_STAGES[activeStage - 1].mockHash}</span>
                <span className="text-[10px] text-[#6B7280]">BLAKE3-256</span>
              </div>
            </div>

            <div className="lg:col-span-4 p-4 rounded-xl bg-[#14171F] border border-[#22262F] space-y-2 text-xs font-mono">
              <div className="text-white font-bold mb-1">Causal Verification Invariant</div>
              <div className="text-[11px] text-[#9AA1AE] leading-relaxed">
                {`Hash_i = BLAKE3(Hash_{i-1} || StageData_i)`}
              </div>
              <div className="pt-2 text-[10px] text-[#3DD68C] flex items-center gap-1.5">
                <CheckCircle2 className="w-3.5 h-3.5" />
                <span>Non-repudiable Proof Verified Offline</span>
              </div>
            </div>
          </div>
        </div>
      </div>
    </section>
  );
}
