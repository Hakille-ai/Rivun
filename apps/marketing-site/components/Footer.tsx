"use client";

import React, { useState } from "react";
import Link from "next/link";
import {
  Github,
  Twitter,
  Disc as Discord,
  Terminal,
  ShieldCheck,
  Cpu,
  Layers,
  ArrowRight,
  Check,
  ExternalLink,
  BookOpen,
  FileCode,
} from "lucide-react";

export function Footer() {
  const [email, setEmail] = useState("");
  const [subscribed, setSubscribed] = useState(false);

  const handleSubscribe = (e: React.FormEvent) => {
    e.preventDefault();
    if (email) {
      setSubscribed(true);
      setEmail("");
      setTimeout(() => setSubscribed(false), 3000);
    }
  };

  const CRATES_LIST = [
    "rivun-core",
    "rivun-crypto",
    "rivun-envelope",
    "rivun-net",
    "rivun-journal",
    "rivun-ledger",
    "rivun-capability",
    "rivun-driver-sdk",
    "rivun-runtime",
    "rivun-agent",
    "rivun-pact",
    "rivun-policy",
    "rivun-pack",
    "rivun-store",
    "rivun-router",
    "rivun-schema",
    "rivun-machine",
    "rivun-memory",
    "rivun-telemetry",
    "rivun-node",
    "rivun-gateway",
    "rivun-ops",
    "rivun-cli",
    "rivun-cloud-api",
    "rivun-cloud-bridge",
    "rivun-control",
  ];

  return (
    <footer className="bg-[#0A0B0D] border-t border-[#22262F] pt-20 pb-12 relative overflow-hidden">
      {/* Background glow */}
      <div className="absolute bottom-0 left-1/2 -translate-x-1/2 w-[800px] h-[300px] bg-[#5B8CFF]/5 rounded-full blur-[140px] pointer-events-none" />

      <div className="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8 relative z-10">
        {/* Main 4-Column Footer Grid */}
        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-5 gap-10 pb-16 border-b border-[#22262F]">
          {/* Col 1 & 2: Brand & Newsletter (2 cols) */}
          <div className="lg:col-span-2 space-y-6">
            <div className="flex items-center gap-3">
              <div className="flex items-center justify-center w-9 h-9 rounded-xl bg-[#5B8CFF] shadow-glow">
                <span className="font-mono font-bold text-base text-black tracking-wider">R</span>
              </div>
              <span className="font-bold text-lg text-white tracking-tight">RIVUN PROTOCOL</span>
            </div>

            <p className="text-xs text-[#9AA1AE] leading-relaxed max-w-sm">
              The zero-trust execution and verification fabric for autonomous AI agents.
              64-byte binary wire frames, 2-phase BFT consensus, and air-gapped Merkle Mountain Range receipts.
            </p>

            {/* Newsletter Subscription */}
            <form onSubmit={handleSubscribe} className="space-y-2 max-w-sm">
              <label className="text-xs font-semibold text-white block">
                Stay updated with Protocol RFCs & Releases
              </label>
              <div className="flex gap-2">
                <input
                  type="email"
                  required
                  placeholder="operator@company.com"
                  value={email}
                  onChange={(e) => setEmail(e.target.value)}
                  className="flex-1 px-3.5 py-2 text-xs bg-[#111318] border border-[#22262F] focus:border-[#5B8CFF] rounded-xl text-white outline-none font-mono"
                />
                <button
                  type="submit"
                  className="px-4 py-2 text-xs font-semibold text-white bg-[#5B8CFF] hover:bg-[#4378F0] rounded-xl transition-all flex items-center gap-1 shrink-0"
                >
                  {subscribed ? <Check className="w-4 h-4 text-white" /> : <span>Subscribe</span>}
                </button>
              </div>
              {subscribed && (
                <p className="text-[11px] text-[#3DD68C] font-mono">
                  Subscribed to Rivun Protocol RFC updates!
                </p>
              )}
            </form>
          </div>

          {/* Col 3: Protocol Specs & Architecture */}
          <div className="space-y-3">
            <h4 className="text-xs font-bold text-white uppercase tracking-wider">
              Protocol Specs
            </h4>
            <ul className="space-y-2 text-xs text-[#9AA1AE]">
              <li>
                <a href="#innovations" className="hover:text-white transition-colors">
                  RivunHeader Wire Format (64B)
                </a>
              </li>
              <li>
                <a href="#innovations" className="hover:text-white transition-colors">
                  ZENV Universal Envelope (74B)
                </a>
              </li>
              <li>
                <a href="#innovations" className="hover:text-white transition-colors">
                  ChaCha20-Poly1305 (ZAPD)
                </a>
              </li>
              <li>
                <a href="#innovations" className="hover:text-white transition-colors">
                  Proof-of-Action BFT (T ≤ N)
                </a>
              </li>
              <li>
                <a href="#innovations" className="hover:text-white transition-colors">
                  Wasmtime Fuel Sandboxing
                </a>
              </li>
              <li>
                <a href="#innovations" className="hover:text-white transition-colors">
                  Merkle Mountain Ranges (MMR)
                </a>
              </li>
            </ul>
          </div>

          {/* Col 4: Official SDKs & Tooling */}
          <div className="space-y-3">
            <h4 className="text-xs font-bold text-white uppercase tracking-wider">
              Official SDKs
            </h4>
            <ul className="space-y-2 text-xs text-[#9AA1AE]">
              <li>
                <a href="http://localhost:3002" target="_blank" rel="noopener noreferrer" className="hover:text-white transition-colors flex items-center gap-1">
                  <span>Rust SDK (crates.io)</span>
                  <ExternalLink className="w-3 h-3 text-[#6B7280]" />
                </a>
              </li>
              <li>
                <a href="http://localhost:3002" target="_blank" rel="noopener noreferrer" className="hover:text-white transition-colors flex items-center gap-1">
                  <span>TypeScript SDK (npm)</span>
                  <ExternalLink className="w-3 h-3 text-[#6B7280]" />
                </a>
              </li>
              <li>
                <a href="http://localhost:3002" target="_blank" rel="noopener noreferrer" className="hover:text-white transition-colors flex items-center gap-1">
                  <span>Python SDK (PyPI)</span>
                  <ExternalLink className="w-3 h-3 text-[#6B7280]" />
                </a>
              </li>
              <li>
                <a href="http://localhost:3002" target="_blank" rel="noopener noreferrer" className="hover:text-white transition-colors flex items-center gap-1">
                  <span>Go SDK (pkg.go.dev)</span>
                  <ExternalLink className="w-3 h-3 text-[#6B7280]" />
                </a>
              </li>
              <li>
                <Link href="/sandbox" className="hover:text-[#5B8CFF] transition-colors">
                  Interactive Browser Sandbox
                </Link>
              </li>
            </ul>
          </div>

          {/* Col 5: Governance & Ecosystem */}
          <div className="space-y-3">
            <h4 className="text-xs font-bold text-white uppercase tracking-wider">
              Governance & Security
            </h4>
            <ul className="space-y-2 text-xs text-[#9AA1AE]">
              <li>
                <a href="https://github.com/Hakille-ai/ZAP" target="_blank" rel="noopener noreferrer" className="hover:text-white transition-colors">
                  RFC Consensus Process
                </a>
              </li>
              <li>
                <a href="#security" className="hover:text-white transition-colors">
                  Security Bug Bounty
                </a>
              </li>
              <li>
                <a href="#domain-packs" className="hover:text-white transition-colors">
                  RivunStore Domain Registry
                </a>
              </li>
              <li>
                <a href="#cloud" className="hover:text-white transition-colors">
                  Operator Workstation Vault
                </a>
              </li>
              <li>
                <a href="http://localhost:3002" target="_blank" rel="noopener noreferrer" className="hover:text-white transition-colors flex items-center gap-1">
                  <span>Documentation Portal</span>
                  <ExternalLink className="w-3 h-3 text-[#6B7280]" />
                </a>
              </li>
            </ul>
          </div>
        </div>

        {/* 26 Workspace Crates Directory Strip */}
        <div className="py-8 border-b border-[#22262F]">
          <div className="flex items-center gap-2 mb-3">
            <Cpu className="w-4 h-4 text-[#5B8CFF]" />
            <span className="text-xs font-bold text-white uppercase font-mono">
              26 Official Workspace Crates (crates/*)
            </span>
          </div>
          <div className="flex flex-wrap gap-1.5">
            {CRATES_LIST.map((crateName) => (
              <span
                key={crateName}
                className="px-2 py-1 rounded bg-[#111318] text-[11px] font-mono text-[#9AA1AE] border border-[#22262F] hover:border-[#5B8CFF]/50 hover:text-white transition-colors"
              >
                {crateName}
              </span>
            ))}
          </div>
        </div>

        {/* Bottom Attribution & Socials */}
        <div className="pt-8 flex flex-col sm:flex-row items-center justify-between gap-4 text-xs text-[#6B7280]">
          <p>© 2026 Rivun Protocol Architects (Hakille-ai/ZAP). Released under Apache-2.0 / MIT.</p>

          <div className="flex items-center gap-4">
            <a
              href="https://github.com/Hakille-ai/ZAP"
              target="_blank"
              rel="noopener noreferrer"
              className="text-[#9AA1AE] hover:text-white transition-colors"
              aria-label="GitHub Repository"
            >
              <Github className="w-4 h-4" />
            </a>
            <a
              href="https://twitter.com"
              target="_blank"
              rel="noopener noreferrer"
              className="text-[#9AA1AE] hover:text-white transition-colors"
              aria-label="Twitter Community"
            >
              <Twitter className="w-4 h-4" />
            </a>
            <a
              href="https://discord.com"
              target="_blank"
              rel="noopener noreferrer"
              className="text-[#9AA1AE] hover:text-white transition-colors"
              aria-label="Discord Community"
            >
              <Discord className="w-4 h-4" />
            </a>
          </div>
        </div>
      </div>
    </footer>
  );
}
