"use client";

import React, { useState } from "react";
import {
  Package,
  Shield,
  Layers,
  Code,
  FileText,
  Copy,
  Check,
  X,
  ExternalLink,
  ChevronRight,
  Sparkles,
  AlertTriangle,
  CheckCircle2,
  Terminal,
} from "lucide-react";
import { DOMAIN_PACKS } from "../lib/domain-packs-data";
import { DomainPackInfo, RiskLevel } from "../lib/types";

const CATEGORIES = [
  { id: "all", label: "All Domain Packs" },
  { id: "ai", label: "AI & Coding Agents" },
  { id: "cloud", label: "Cloud & Kubernetes" },
  { id: "enterprise", label: "Enterprise & Regulated" },
  { id: "iot", label: "Physical Systems & SCADA" },
];

export function DomainPacksShowcase() {
  const [activeCategory, setActiveCategory] = useState("all");
  const [selectedPack, setSelectedPack] = useState<DomainPackInfo | null>(null);
  const [activeInspectorTab, setActiveInspectorTab] = useState<"capabilities" | "policy" | "manifest" | "schema">("capabilities");
  const [copiedCli, setCopiedCli] = useState(false);

  const filteredPacks = DOMAIN_PACKS.filter(
    (pack) => activeCategory === "all" || pack.category === activeCategory
  );

  const getRiskBadge = (risk: RiskLevel) => {
    switch (risk) {
      case "low":
        return <span className="px-2 py-0.5 text-[10px] font-mono font-bold bg-[#3DD68C]/15 text-[#3DD68C] border border-[#3DD68C]/30 rounded">LOW RISK</span>;
      case "medium":
        return <span className="px-2 py-0.5 text-[10px] font-mono font-bold bg-blue-500/15 text-blue-400 border border-blue-500/30 rounded">MEDIUM</span>;
      case "high":
        return <span className="px-2 py-0.5 text-[10px] font-mono font-bold bg-amber-500/15 text-amber-400 border border-amber-500/30 rounded">HIGH RISK</span>;
      case "critical":
        return <span className="px-2 py-0.5 text-[10px] font-mono font-bold bg-rose-500/15 text-rose-400 border border-rose-500/30 rounded">CRITICAL GATE</span>;
    }
  };

  const copyCliCommand = (packId: string) => {
    const cmd = `rivun pack install --bundle ${packId}.zpack --trusted-key key_sec_ops_01`;
    navigator.clipboard.writeText(cmd);
    setCopiedCli(true);
    setTimeout(() => setCopiedCli(false), 2000);
  };

  return (
    <section id="domain-packs" className="py-24 relative overflow-hidden">
      <div className="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8">
        {/* Section Header */}
        <div className="text-center max-w-3xl mx-auto mb-12 space-y-4">
          <div className="inline-flex items-center gap-2 px-3 py-1 rounded-full bg-[#5B8CFF]/10 border border-[#5B8CFF]/20 text-[#5B8CFF] text-xs font-semibold">
            <Package className="w-3.5 h-3.5" />
            <span>7 OFFICIAL DOMAIN PACKS</span>
          </div>
          <h2 className="text-3xl sm:text-4xl font-extrabold tracking-tight text-white">
            Pre-Built Zero-Trust Policies for Every Agent Frontier
          </h2>
          <p className="text-sm sm:text-base text-[#9AA1AE]">
            Turnkey, signed capability matrices, WASM action drivers, and fail-closed TOML policies.
            Deploy instant mathematical boundaries across coding, cloud, finance, healthcare, and industrial automation.
          </p>
        </div>

        {/* Category Filters */}
        <div className="flex items-center justify-start sm:justify-center gap-2 overflow-x-auto pb-4 mb-8 no-scrollbar">
          {CATEGORIES.map((cat) => (
            <button
              key={cat.id}
              onClick={() => setActiveCategory(cat.id)}
              className={`px-4 py-2 rounded-xl text-xs font-semibold whitespace-nowrap transition-all border ${
                activeCategory === cat.id
                  ? "bg-[#181B22] text-[#5B8CFF] border-[#5B8CFF]/40 shadow-glow"
                  : "bg-[#111318]/60 text-[#9AA1AE] border-[#22262F] hover:text-white hover:border-[#3A4150]"
              }`}
            >
              {cat.label}
            </button>
          ))}
        </div>

        {/* Grid of Domain Packs */}
        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-6">
          {filteredPacks.map((pack) => (
            <div
              key={pack.id}
              onClick={() => setSelectedPack(pack)}
              className="p-6 rounded-2xl bg-[#111318] border border-[#22262F] hover:border-[#5B8CFF]/40 hover:shadow-glow transition-all duration-300 cursor-pointer flex flex-col justify-between group"
            >
              <div>
                <div className="flex items-center justify-between gap-2 mb-3">
                  <span className="text-[10px] font-mono font-semibold px-2 py-0.5 rounded bg-[#181B22] text-[#9AA1AE] border border-[#22262F]">
                    v{pack.version}
                  </span>
                  <span className="text-xs text-[#5B8CFF] font-medium flex items-center gap-1 group-hover:translate-x-0.5 transition-transform">
                    Inspect Pack
                    <ChevronRight className="w-3.5 h-3.5" />
                  </span>
                </div>

                <h3 className="text-lg font-bold text-white group-hover:text-[#5B8CFF] transition-colors">
                  {pack.name}
                </h3>
                <p className="text-xs text-[#5B8CFF] font-mono mt-0.5 mb-2">
                  {pack.tagline}
                </p>
                <p className="text-xs text-[#9AA1AE] leading-relaxed line-clamp-2 mb-4">
                  {pack.description}
                </p>

                {/* Capability Badges preview */}
                <div className="flex flex-wrap gap-1.5 mb-4">
                  {pack.capabilities.slice(0, 3).map((cap, cIdx) => (
                    <span
                      key={cIdx}
                      className="px-2 py-0.5 rounded bg-[#181B22] text-[10px] font-mono text-gray-300 border border-[#22262F]"
                    >
                      {cap.name}
                    </span>
                  ))}
                  {pack.capabilities.length > 3 && (
                    <span className="px-2 py-0.5 rounded bg-[#181B22] text-[10px] font-mono text-[#9AA1AE]">
                      +{pack.capabilities.length - 3} more
                    </span>
                  )}
                </div>
              </div>

              <div className="pt-4 border-t border-[#22262F] flex items-center justify-between text-xs text-[#6B7280]">
                <span>Gate: {pack.defaultSafetyGate.split(" ")[0]}...</span>
                <span className="font-mono text-white font-medium">{pack.capabilitiesCount} Capabilities</span>
              </div>
            </div>
          ))}
        </div>
      </div>

      {/* Slide-Over / Modal Pack Inspector */}
      {selectedPack && (
        <div className="fixed inset-0 z-50 flex items-center justify-center p-4 sm:p-6 bg-black/80 backdrop-blur-md animate-fade-in">
          <div className="relative w-full max-w-4xl max-h-[90vh] bg-[#111318] border border-[#22262F] rounded-2xl shadow-modal overflow-hidden flex flex-col">
            {/* Modal Header */}
            <div className="p-6 bg-[#181B22] border-b border-[#22262F] flex items-center justify-between">
              <div>
                <div className="flex items-center gap-2 mb-1">
                  <span className="px-2 py-0.5 rounded text-[10px] font-mono font-bold bg-[#5B8CFF]/15 text-[#5B8CFF] border border-[#5B8CFF]/30">
                    OFFICIAL DOMAIN PACK
                  </span>
                  <span className="text-xs font-mono text-[#9AA1AE]">v{selectedPack.version}</span>
                </div>
                <h3 className="text-xl font-bold text-white">{selectedPack.name}</h3>
                <p className="text-xs text-[#9AA1AE]">{selectedPack.tagline}</p>
              </div>

              <button
                onClick={() => setSelectedPack(null)}
                className="p-2 rounded-xl bg-[#111318] hover:bg-[#22262F] text-[#9AA1AE] hover:text-white transition-colors"
              >
                <X className="w-5 h-5" />
              </button>
            </div>

            {/* CLI Command Strip */}
            <div className="px-6 py-3 bg-[#0A0B0D] border-b border-[#22262F] flex items-center justify-between gap-4 font-mono text-xs">
              <div className="flex items-center gap-2 text-[#9AA1AE] truncate">
                <Terminal className="w-4 h-4 text-[#5B8CFF] shrink-0" />
                <span className="text-[#3DD68C]">$</span>
                <span className="text-white truncate">
                  rivun pack install --bundle {selectedPack.id}.zpack --trusted-key key_sec_ops_01
                </span>
              </div>
              <button
                onClick={() => copyCliCommand(selectedPack.id)}
                className="px-2.5 py-1 rounded bg-[#181B22] hover:bg-[#22262F] text-[#9AA1AE] hover:text-white border border-[#22262F] transition-all shrink-0 flex items-center gap-1 text-[11px]"
              >
                {copiedCli ? <Check className="w-3.5 h-3.5 text-[#3DD68C]" /> : <Copy className="w-3.5 h-3.5" />}
                <span>{copiedCli ? "Copied" : "Copy CLI"}</span>
              </button>
            </div>

            {/* Inspector Tabs */}
            <div className="px-6 pt-4 bg-[#14171F] border-b border-[#22262F] flex items-center gap-2 overflow-x-auto">
              <button
                onClick={() => setActiveInspectorTab("capabilities")}
                className={`px-3.5 py-2 text-xs font-semibold rounded-t-lg border-b-2 transition-all flex items-center gap-1.5 ${
                  activeInspectorTab === "capabilities"
                    ? "text-[#5B8CFF] border-[#5B8CFF] bg-[#181B22]"
                    : "text-[#9AA1AE] border-transparent hover:text-white"
                }`}
              >
                <Shield className="w-3.5 h-3.5" />
                <span>Capabilities ({selectedPack.capabilities.length})</span>
              </button>
              <button
                onClick={() => setActiveInspectorTab("policy")}
                className={`px-3.5 py-2 text-xs font-semibold rounded-t-lg border-b-2 transition-all flex items-center gap-1.5 ${
                  activeInspectorTab === "policy"
                    ? "text-[#5B8CFF] border-[#5B8CFF] bg-[#181B22]"
                    : "text-[#9AA1AE] border-transparent hover:text-white"
                }`}
              >
                <FileText className="w-3.5 h-3.5" />
                <span>policy.toml</span>
              </button>
              <button
                onClick={() => setActiveInspectorTab("manifest")}
                className={`px-3.5 py-2 text-xs font-semibold rounded-t-lg border-b-2 transition-all flex items-center gap-1.5 ${
                  activeInspectorTab === "manifest"
                    ? "text-[#5B8CFF] border-[#5B8CFF] bg-[#181B22]"
                    : "text-[#9AA1AE] border-transparent hover:text-white"
                }`}
              >
                <Package className="w-3.5 h-3.5" />
                <span>pack.toml</span>
              </button>
              <button
                onClick={() => setActiveInspectorTab("schema")}
                className={`px-3.5 py-2 text-xs font-semibold rounded-t-lg border-b-2 transition-all flex items-center gap-1.5 ${
                  activeInspectorTab === "schema"
                    ? "text-[#5B8CFF] border-[#5B8CFF] bg-[#181B22]"
                    : "text-[#9AA1AE] border-transparent hover:text-white"
                }`}
              >
                <Code className="w-3.5 h-3.5" />
                <span>schema.json</span>
              </button>
            </div>

            {/* Modal Body Content */}
            <div className="p-6 overflow-y-auto flex-1 font-mono text-xs">
              {activeInspectorTab === "capabilities" && (
                <div className="space-y-3">
                  <div className="text-[11px] text-[#9AA1AE] font-sans mb-3">
                    Safety Gate: <strong className="text-white">{selectedPack.defaultSafetyGate}</strong>
                  </div>

                  <div className="border border-[#22262F] rounded-xl overflow-hidden">
                    <table className="w-full text-left">
                      <thead className="bg-[#181B22] text-[#9AA1AE] text-[11px] border-b border-[#22262F]">
                        <tr>
                          <th className="p-3">Capability ID</th>
                          <th className="p-3">Risk Classification</th>
                          <th className="p-3">Description</th>
                          <th className="p-3">Required Proof</th>
                        </tr>
                      </thead>
                      <tbody className="divide-y divide-[#22262F]">
                        {selectedPack.capabilities.map((cap, cIdx) => (
                          <tr key={cIdx} className="hover:bg-[#14171F]/50">
                            <td className="p-3 text-white font-bold">{cap.name}</td>
                            <td className="p-3">{getRiskBadge(cap.risk)}</td>
                            <td className="p-3 text-gray-300 font-sans text-xs">{cap.description}</td>
                            <td className="p-3 text-[#5B8CFF] text-[11px]">{cap.requiredProof}</td>
                          </tr>
                        ))}
                      </tbody>
                    </table>
                  </div>
                </div>
              )}

              {activeInspectorTab === "policy" && (
                <pre className="p-4 rounded-xl bg-[#0A0B0D] border border-[#22262F] text-amber-300 overflow-x-auto">
                  <code>{selectedPack.policyToml}</code>
                </pre>
              )}

              {activeInspectorTab === "manifest" && (
                <pre className="p-4 rounded-xl bg-[#0A0B0D] border border-[#22262F] text-cyan-300 overflow-x-auto">
                  <code>{selectedPack.manifestToml}</code>
                </pre>
              )}

              {activeInspectorTab === "schema" && (
                <pre className="p-4 rounded-xl bg-[#0A0B0D] border border-[#22262F] text-emerald-300 overflow-x-auto">
                  <code>{selectedPack.schemaJson}</code>
                </pre>
              )}
            </div>
          </div>
        </div>
      )}
    </section>
  );
}
