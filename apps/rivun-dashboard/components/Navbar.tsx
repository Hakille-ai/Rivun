"use client";

import React, { useState } from "react";
import { ShieldCheck, Cpu, Terminal, Bell, User, CheckCircle2, ChevronDown } from "lucide-react";

export function Navbar() {
  const [selectedOrg, setSelectedOrg] = useState("Acme Autonomous Systems");

  return (
    <header className="h-16 border-b border-border-subtle bg-bg-surface/80 backdrop-blur-md px-6 flex items-center justify-between sticky top-0 z-40">
      {/* Left brand & Org Selector */}
      <div className="flex items-center space-x-6">
        <div className="flex items-center space-x-3">
          <div className="w-8 h-8 rounded-lg bg-accent-primary flex items-center justify-center shadow-glow">
            <Cpu className="w-5 h-5 text-white" />
          </div>
          <div>
            <div className="flex items-center space-x-2">
              <span className="font-semibold text-text-primary tracking-tight text-base">Rivun Cloud</span>
              <span className="text-[10px] uppercase font-mono tracking-wider px-1.5 py-0.5 rounded bg-accent-glow text-accent-primary border border-accent-primary/20">
                SaaS for ZAP
              </span>
            </div>
          </div>
        </div>

        <div className="h-4 w-px bg-border-subtle" />

        <div className="relative">
          <button className="flex items-center space-x-2 px-3 py-1.5 rounded-lg bg-bg-surface-raised border border-border-subtle text-sm text-text-primary hover:border-border-strong transition">
            <span className="w-2 h-2 rounded-full bg-status-verified shadow-[0_0_8px_#3DD68C]" />
            <span className="font-medium">{selectedOrg}</span>
            <ChevronDown className="w-3.5 h-3.5 text-text-secondary" />
          </button>
        </div>
      </div>

      {/* Center zero-trust banner */}
      <div className="hidden lg:flex items-center space-x-2 px-3 py-1 rounded-full bg-bg-base/60 border border-border-subtle text-xs text-text-secondary">
        <ShieldCheck className="w-4 h-4 text-status-verified" />
        <span>Zero-Trust Invariant: <strong className="text-text-primary font-mono">Ed25519 Keys Stay on Edge</strong></span>
      </div>

      {/* Right controls */}
      <div className="flex items-center space-x-4">
        <div className="flex items-center space-x-2 px-2.5 py-1 rounded-lg bg-bg-surface-raised border border-border-subtle text-xs text-text-secondary font-mono">
          <span className="w-2 h-2 rounded-full bg-status-verified animate-pulse" />
          <span>SSE LIVE</span>
        </div>

        <button className="p-2 rounded-lg bg-bg-surface-raised border border-border-subtle text-text-secondary hover:text-text-primary transition">
          <Bell className="w-4 h-4" />
        </button>

        <div className="flex items-center space-x-3 pl-2 border-l border-border-subtle">
          <div className="w-8 h-8 rounded-full bg-border-strong flex items-center justify-center text-xs font-semibold text-text-primary border border-border-subtle">
            AV
          </div>
          <div className="hidden md:block text-left">
            <div className="text-xs font-medium text-text-primary">Alice Vance</div>
            <div className="text-[10px] text-accent-primary font-mono">Lead Operator</div>
          </div>
        </div>
      </div>
    </header>
  );
}
