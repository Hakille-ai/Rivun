'use client';

import React, { useState } from 'react';
import Link from 'next/link';
import { Terminal, Layers, Cpu, ShieldCheck, ArrowRight } from 'lucide-react';
import { Header } from '@/components/layout/Header';
import { Footer } from '@/components/layout/Footer';
import { SearchModal } from '@/components/ui/SearchModal';
import { WireFrameSandbox } from '@/components/interactive/WireFrameSandbox';
import { Badge } from '@/components/ui/Badge';

export default function SandboxPage() {
  const [isSearchOpen, setIsSearchOpen] = useState(false);

  return (
    <div className="min-h-screen flex flex-col bg-bg-base">
      <Header onOpenSearch={() => setIsSearchOpen(true)} />
      <SearchModal isOpen={isSearchOpen} onClose={() => setIsSearchOpen(false)} />

      <main className="flex-1 max-w-7xl w-full mx-auto px-4 sm:px-6 lg:px-8 py-10">
        {/* Page Header */}
        <div className="mb-8">
          <div className="flex items-center gap-2 mb-2">
            <Badge variant="cyan">Interactive Protocol Sandbox</Badge>
            <Badge variant="outline">Big-Endian Binary Framing</Badge>
          </div>
          <h1 className="text-3xl font-extrabold tracking-tight text-text-primary">
            Live 64-Byte Wire Frame Sandbox
          </h1>
          <p className="mt-2 text-sm text-text-secondary max-w-3xl">
            Inspect the fixed 64-byte <code>RivunHeader</code> wire protocol, toggle bitflags, inspect byte
            offset allocations, and visualize Ed25519 <code>ZSIG</code> and Proof-of-Action <code>ZPOA</code> trailers in real-time.
          </p>
        </div>

        {/* Sub-navigation Tabs */}
        <div className="flex items-center gap-2 border-b border-border-subtle pb-4 mb-8 overflow-x-auto">
          <Link
            href="/sandbox"
            className="flex items-center gap-2 px-3 py-1.5 rounded-lg bg-accent-primary/10 border border-accent-primary/30 text-accent-primary text-xs font-semibold"
          >
            <Terminal className="w-3.5 h-3.5" />
            <span>Wire Frame Encoder</span>
          </Link>
          <Link
            href="/sandbox/poa-quorum"
            className="flex items-center gap-2 px-3 py-1.5 rounded-lg bg-bg-surface hover:bg-bg-surface-raised border border-border-subtle text-text-secondary hover:text-text-primary text-xs font-medium transition-colors"
          >
            <Cpu className="w-3.5 h-3.5" />
            <span>PoA Quorum Simulator</span>
          </Link>
          <Link
            href="/sandbox/pact"
            className="flex items-center gap-2 px-3 py-1.5 rounded-lg bg-bg-surface hover:bg-bg-surface-raised border border-border-subtle text-text-secondary hover:text-text-primary text-xs font-medium transition-colors"
          >
            <ShieldCheck className="w-3.5 h-3.5" />
            <span>PACT Canonicalizer</span>
          </Link>
        </div>

        {/* Interactive Component */}
        <WireFrameSandbox />
      </main>

      <Footer />
    </div>
  );
}
