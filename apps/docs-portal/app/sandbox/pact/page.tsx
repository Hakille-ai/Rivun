'use client';

import React, { useState } from 'react';
import Link from 'next/link';
import { Terminal, Cpu, ShieldCheck } from 'lucide-react';
import { Header } from '@/components/layout/Header';
import { Footer } from '@/components/layout/Footer';
import { SearchModal } from '@/components/ui/SearchModal';
import { PactVisualizer } from '@/components/interactive/PactVisualizer';
import { Badge } from '@/components/ui/Badge';

export default function PactPage() {
  const [isSearchOpen, setIsSearchOpen] = useState(false);

  return (
    <div className="min-h-screen flex flex-col bg-bg-base">
      <Header onOpenSearch={() => setIsSearchOpen(true)} />
      <SearchModal isOpen={isSearchOpen} onClose={() => setIsSearchOpen(false)} />

      <main className="flex-1 max-w-7xl w-full mx-auto px-4 sm:px-6 lg:px-8 py-10">
        {/* Page Header */}
        <div className="mb-8">
          <div className="flex items-center gap-2 mb-2">
            <Badge variant="purple">Multi-Party Contract Engine</Badge>
            <Badge variant="outline">RFC 8785 Canonical JSON</Badge>
          </div>
          <h1 className="text-3xl font-extrabold tracking-tight text-text-primary">
            PACT Record Canonicalizer & Signer
          </h1>
          <p className="mt-2 text-sm text-text-secondary max-w-3xl">
            Construct deterministic multi-party PACT records, sort JSON dictionary keys alphabetically according to
            RFC 8785, calculate BLAKE3 digests under <code>ZAP-PACT-v1</code>, and inspect detached Ed25519 signatures.
          </p>
        </div>

        {/* Sub-navigation Tabs */}
        <div className="flex items-center gap-2 border-b border-border-subtle pb-4 mb-8 overflow-x-auto">
          <Link
            href="/sandbox"
            className="flex items-center gap-2 px-3 py-1.5 rounded-lg bg-bg-surface hover:bg-bg-surface-raised border border-border-subtle text-text-secondary hover:text-text-primary text-xs font-medium transition-colors"
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
            className="flex items-center gap-2 px-3 py-1.5 rounded-lg bg-accent-primary/10 border border-accent-primary/30 text-accent-primary text-xs font-semibold"
          >
            <ShieldCheck className="w-3.5 h-3.5" />
            <span>PACT Canonicalizer</span>
          </Link>
        </div>

        {/* Interactive Component */}
        <PactVisualizer />
      </main>

      <Footer />
    </div>
  );
}
