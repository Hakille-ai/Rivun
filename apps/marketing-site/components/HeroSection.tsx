"use client";

import React from "react";
import Link from "next/link";
import {
  Sparkles,
  ArrowRight,
  ShieldCheck,
  Zap,
  Lock,
  Cpu,
  Github,
  Terminal,
  Activity,
  CheckCircle2,
} from "lucide-react";
import { HeroFrameVisualizer } from "./HeroFrameVisualizer";

export function HeroSection() {
  return (
    <section className="relative pt-32 pb-20 lg:pt-40 lg:pb-32 overflow-hidden">
      {/* Background ambient lighting and grid */}
      <div className="absolute inset-0 grid-bg opacity-30 pointer-events-none" />
      <div className="absolute top-1/4 left-1/2 -translate-x-1/2 -translate-y-1/2 w-[800px] h-[500px] bg-gradient-to-b from-[#5B8CFF]/15 to-transparent rounded-full blur-[120px] pointer-events-none" />
      <div className="absolute top-1/3 right-10 w-[400px] h-[400px] bg-purple-500/10 rounded-full blur-[100px] pointer-events-none" />

      <div className="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8 relative z-10">
        {/* Announcement Pill */}
        <div className="flex justify-center mb-8">
          <div className="inline-flex items-center gap-2 px-3.5 py-1.5 rounded-full bg-[#111318] border border-[#22262F] hover:border-[#5B8CFF]/40 text-xs text-[#9AA1AE] shadow-lg transition-all">
            <span className="flex h-2 w-2 relative">
              <span className="animate-ping absolute inline-flex h-full w-full rounded-full bg-[#5B8CFF] opacity-75" />
              <span className="relative inline-flex rounded-full h-2 w-2 bg-[#5B8CFF]" />
            </span>
            <span className="font-semibold text-white">Rivun Protocol v1.0.0</span>
            <span className="text-[#6B7280]">|</span>
            <span className="text-[#5B8CFF] font-medium flex items-center gap-1">
              Zero-Trust Agent Fabric
              <ArrowRight className="w-3 h-3" />
            </span>
          </div>
        </div>

        {/* Hero Headline & Subtitle */}
        <div className="text-center max-w-4xl mx-auto space-y-6 mb-12">
          <h1 className="text-4xl sm:text-6xl lg:text-7xl font-extrabold tracking-tight leading-[1.1]">
            <span className="text-gradient-apple block">The Zero-Trust Execution Fabric</span>
            <span className="text-gradient-blue block mt-1">for Autonomous AI Agents</span>
          </h1>

          <p className="text-base sm:text-lg lg:text-xl text-[#9AA1AE] max-w-3xl mx-auto leading-relaxed">
            Eliminate prompt injection risks and rogue agent actions. Rivun enforces 
            <span className="text-white font-medium"> 64-byte binary signed wire frames</span>, 
            <span className="text-[#3DD68C] font-medium"> 2-phase BFT Proof-of-Action consensus</span>, and 
            <span className="text-[#5B8CFF] font-medium"> air-gapped Merkle Mountain Range</span> receipts for every agent decision.
          </p>

          {/* Action CTAs */}
          <div className="flex flex-wrap items-center justify-center gap-4 pt-2">
            <Link
              href="/sandbox"
              className="px-6 py-3.5 text-sm font-semibold text-white bg-gradient-to-r from-[#5B8CFF] to-[#3B72F2] hover:from-[#4378F0] hover:to-[#2B60E0] rounded-xl shadow-glow transition-all flex items-center gap-2 group"
            >
              <Terminal className="w-4 h-4 text-white" />
              <span>Launch Protocol Sandbox</span>
              <ArrowRight className="w-4 h-4 group-hover:translate-x-0.5 transition-transform" />
            </Link>

            <a
              href="#innovations"
              className="px-6 py-3.5 text-sm font-semibold text-[#F4F5F7] bg-[#111318] hover:bg-[#181B22] border border-[#22262F] hover:border-[#3A4150] rounded-xl transition-all flex items-center gap-2"
            >
              <Cpu className="w-4 h-4 text-[#5B8CFF]" />
              <span>Explore Protocol Specs</span>
            </a>

            <a
              href="https://github.com/Hakille-ai/ZAP"
              target="_blank"
              rel="noopener noreferrer"
              className="px-4 py-3.5 text-sm font-medium text-[#9AA1AE] hover:text-white bg-[#111318] hover:bg-[#181B22] border border-[#22262F] rounded-xl transition-all flex items-center gap-2"
            >
              <Github className="w-4 h-4" />
              <span className="hidden sm:inline">GitHub</span>
            </a>
          </div>
        </div>

        {/* Live Metrics Strip */}
        <div className="grid grid-cols-2 md:grid-cols-4 gap-4 max-w-5xl mx-auto mb-16">
          <div className="p-4 rounded-xl bg-[#111318]/70 border border-[#22262F] backdrop-blur-md flex items-center gap-3.5">
            <div className="p-2.5 rounded-lg bg-[#3DD68C]/10 text-[#3DD68C] border border-[#3DD68C]/20">
              <Zap className="w-5 h-5" />
            </div>
            <div>
              <div className="text-xl font-bold font-mono text-white">&lt; 0.8ms</div>
              <div className="text-xs text-[#9AA1AE]">p99 Round-Trip SLA</div>
            </div>
          </div>

          <div className="p-4 rounded-xl bg-[#111318]/70 border border-[#22262F] backdrop-blur-md flex items-center gap-3.5">
            <div className="p-2.5 rounded-lg bg-[#5B8CFF]/10 text-[#5B8CFF] border border-[#5B8CFF]/20">
              <ShieldCheck className="w-5 h-5" />
            </div>
            <div>
              <div className="text-xl font-bold font-mono text-white">100% Signed</div>
              <div className="text-xs text-[#9AA1AE]">Ed25519 Wire Frames</div>
            </div>
          </div>

          <div className="p-4 rounded-xl bg-[#111318]/70 border border-[#22262F] backdrop-blur-md flex items-center gap-3.5">
            <div className="p-2.5 rounded-lg bg-purple-500/10 text-purple-400 border border-purple-500/20">
              <Cpu className="w-5 h-5" />
            </div>
            <div>
              <div className="text-xl font-bold font-mono text-white">7 Official Packs</div>
              <div className="text-xs text-[#9AA1AE]">Domain Policies & WASM</div>
            </div>
          </div>

          <div className="p-4 rounded-xl bg-[#111318]/70 border border-[#22262F] backdrop-blur-md flex items-center gap-3.5">
            <div className="p-2.5 rounded-lg bg-amber-500/10 text-amber-400 border border-amber-500/20">
              <Lock className="w-5 h-5" />
            </div>
            <div>
              <div className="text-xl font-bold font-mono text-white">Zero-Trust</div>
              <div className="text-xs text-[#9AA1AE]">Sovereign Operator Keys</div>
            </div>
          </div>
        </div>

        {/* Embedded Interactive Signed Frame Visualizer */}
        <div className="max-w-6xl mx-auto">
          <HeroFrameVisualizer />
        </div>
      </div>
    </section>
  );
}
