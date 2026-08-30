'use client';

import React, { useState } from 'react';
import Link from 'next/link';
import {
  Rocket,
  Layers,
  Cpu,
  Boxes,
  Cloud,
  Package,
  Code,
  Store,
  ShieldCheck,
  PlayCircle,
  ArrowRight,
  Terminal,
  Search,
  Lock,
  Zap,
  Sparkles,
} from 'lucide-react';
import { Header } from '@/components/layout/Header';
import { Footer } from '@/components/layout/Footer';
import { SearchModal } from '@/components/ui/SearchModal';
import { Badge } from '@/components/ui/Badge';
import { CardGrid, CardItem } from '@/components/ui/CardGrid';

export default function HomePage() {
  const [isSearchOpen, setIsSearchOpen] = useState(false);

  const coreFeatures: CardItem[] = [
    {
      title: 'Fixed 64-Byte Wire Header',
      description:
        'Sub-microsecond binary parsing with 0x5A41505F ("ZAP_") magic, bitflags, Ed25519 ZSIG, and ZPOA trailers.',
      href: '/docs/architecture/wire-format',
      icon: <Terminal className="w-5 h-5" />,
      badge: '0x5A41505F',
    },
    {
      title: 'Universal Envelope (ZENV)',
      description:
        '74-byte zero-copy messaging envelope carrying 8 discrete message kinds with causal tracking.',
      href: '/docs/architecture/universal-envelope',
      icon: <Layers className="w-5 h-5" />,
      badge: '74-Byte',
    },
    {
      title: 'Proof-of-Action (PoA) Consensus',
      description:
        'BFT 2-phase commit quorum mesh (T <= N) without proof-of-work waste or token staking friction.',
      href: '/docs/consensus/bft-consensus',
      icon: <Cpu className="w-5 h-5" />,
      badge: 'T <= N',
    },
    {
      title: 'Sandboxed WASM Runtime',
      description:
        'Hardware-isolated execution via Wasmtime with fuel metering, epoch timeouts, and SPSC ring buffers.',
      href: '/docs/runtime/wasm-sandboxing',
      icon: <Boxes className="w-5 h-5" />,
      badge: 'ABI v1',
    },
    {
      title: 'Sovereign Operator Workstation',
      description:
        'Zero-trust sovereign identity where private keys remain strictly isolated on local workstations.',
      href: '/docs/cloud/sovereign-architecture',
      icon: <Lock className="w-5 h-5" />,
      badge: 'Zero-Trust',
    },
    {
      title: '7-Point Fleet Doctor Diagnostics',
      description:
        'Continuous deep inspection suite for network reachability, WAL replays, MMR journals, and peer trust.',
      href: '/docs/operations/fleet-doctor',
      icon: <ShieldCheck className="w-5 h-5" />,
      badge: '7 Checks',
    },
  ];

  const sdkCards: CardItem[] = [
    {
      title: 'Rust SDK',
      description: 'Zero-overhead native Rust client, memory-safe builders, and WASM guest driver authoring.',
      href: '/docs/sdks/rust',
      icon: <Code className="w-5 h-5 text-orange-400" />,
      badge: 'Native',
    },
    {
      title: 'TypeScript SDK',
      description: 'Universal Node.js, Bun, and browser client with WebCrypto and Noble Ed25519 signing.',
      href: '/docs/sdks/typescript',
      icon: <Code className="w-5 h-5 text-sky-400" />,
      badge: 'Universal',
    },
    {
      title: 'Python SDK',
      description: 'Python 3.10+ dataclasses, stdlib UDP transport, and AI agent framework integration.',
      href: '/docs/sdks/python',
      icon: <Code className="w-5 h-5 text-yellow-400" />,
      badge: 'Agentic',
    },
    {
      title: 'Go SDK',
      description: 'High-concurrency microservice dispatchers, BLAKE3 hashing, and binary wire codecs.',
      href: '/docs/sdks/go',
      icon: <Code className="w-5 h-5 text-cyan-400" />,
      badge: 'Microservices',
    },
  ];

  return (
    <div className="min-h-screen flex flex-col bg-bg-base">
      <Header onOpenSearch={() => setIsSearchOpen(true)} />
      <SearchModal isOpen={isSearchOpen} onClose={() => setIsSearchOpen(false)} />

      {/* Hero Section */}
      <main className="flex-1">
        <section className="relative pt-20 pb-16 overflow-hidden border-b border-border-subtle">
          {/* Subtle background glow */}
          <div className="absolute top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2 w-[600px] h-[300px] bg-gradient-to-tr from-cyan-500/15 to-indigo-600/15 blur-[120px] pointer-events-none rounded-full" />

          <div className="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8 relative z-10 text-center">
            <div className="inline-flex items-center gap-2 px-3 py-1 rounded-full bg-cyan-500/10 border border-cyan-500/30 text-cyan-300 text-xs font-mono mb-6">
              <Zap className="w-3.5 h-3.5 fill-current" />
              <span>ZAP Universal Protocol &bull; Production v1.0.0</span>
            </div>

            <h1 className="text-4xl sm:text-6xl font-extrabold tracking-tight text-text-primary max-w-4xl mx-auto leading-tight sm:leading-none">
              The Sovereign Protocol for{' '}
              <span className="bg-gradient-to-r from-cyan-400 via-indigo-300 to-emerald-400 bg-clip-text text-transparent">
                Autonomous Verification
              </span>
            </h1>

            <p className="mt-6 text-base sm:text-lg text-text-secondary max-w-2xl mx-auto leading-relaxed">
              Explore the complete architectural specifications, 26 workspace crate API references,
              4 official language SDKs, and interactive protocol sandboxes.
            </p>

            {/* CTA Buttons */}
            <div className="mt-8 flex flex-wrap items-center justify-center gap-4">
              <Link
                href="/docs/getting-started/overview"
                className="flex items-center gap-2 px-6 py-3 rounded-xl bg-accent-primary hover:bg-sky-400 text-bg-base font-bold text-sm transition-all shadow-glow hover:scale-105"
              >
                <Rocket className="w-4 h-4" />
                <span>Get Started in 5 Minutes</span>
              </Link>
              <button
                onClick={() => setIsSearchOpen(true)}
                className="flex items-center gap-2 px-6 py-3 rounded-xl bg-bg-surface hover:bg-bg-surface-raised border border-border-subtle text-text-primary text-sm font-medium transition-all shadow-card hover:border-accent-primary/40"
              >
                <Search className="w-4 h-4 text-accent-primary" />
                <span>Search Docs (⌘K)</span>
              </button>
            </div>

            {/* Quick Stats Bar */}
            <div className="mt-14 max-w-4xl mx-auto grid grid-cols-2 sm:grid-cols-4 gap-4 p-4 rounded-2xl bg-bg-surface/60 border border-border-subtle backdrop-blur-md">
              <div>
                <div className="text-2xl font-extrabold font-mono text-cyan-400">26</div>
                <div className="text-xs text-text-muted mt-0.5">Workspace Crates</div>
              </div>
              <div>
                <div className="text-2xl font-extrabold font-mono text-emerald-400">4 SDKs</div>
                <div className="text-xs text-text-muted mt-0.5">Rust, TS, Python, Go</div>
              </div>
              <div>
                <div className="text-2xl font-extrabold font-mono text-indigo-400">7 Packs</div>
                <div className="text-xs text-text-muted mt-0.5">Domain Foundations</div>
              </div>
              <div>
                <div className="text-2xl font-extrabold font-mono text-purple-400">&lt; 10ms</div>
                <div className="text-xs text-text-muted mt-0.5">Search & Quorum</div>
              </div>
            </div>
          </div>
        </section>

        {/* Section 1: Core Architecture Pillars */}
        <section className="py-16 max-w-7xl mx-auto px-4 sm:px-6 lg:px-8">
          <div className="flex items-center justify-between mb-2">
            <div>
              <h2 className="text-2xl font-bold tracking-tight text-text-primary">
                Protocol Architecture & Pillars
              </h2>
              <p className="text-xs text-text-secondary mt-1">
                Zero-trust sovereign foundations built from the wire up.
              </p>
            </div>
            <Link
              href="/docs/architecture/overview"
              className="hidden sm:flex items-center gap-1 text-xs font-semibold text-accent-primary hover:text-sky-300 transition-colors"
            >
              <span>View Architecture Specs</span>
              <ArrowRight className="w-3.5 h-3.5" />
            </Link>
          </div>

          <CardGrid items={coreFeatures} columns={3} />
        </section>

        {/* Section 2: 4 Official SDKs */}
        <section className="py-12 bg-bg-surface/30 border-y border-border-subtle">
          <div className="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8">
            <div className="flex items-center justify-between mb-2">
              <div>
                <h2 className="text-2xl font-bold tracking-tight text-text-primary">
                  Official Developer SDKs
                </h2>
                <p className="text-xs text-text-secondary mt-1">
                  100% bit-for-bit wire conformance verified across 11 shared JSON test vectors.
                </p>
              </div>
              <Link
                href="/docs/sdks/conformance-matrix"
                className="hidden sm:flex items-center gap-1 text-xs font-semibold text-accent-primary hover:text-sky-300 transition-colors"
              >
                <span>View Conformance Matrix</span>
                <ArrowRight className="w-3.5 h-3.5" />
              </Link>
            </div>

            <CardGrid items={sdkCards} columns={4} />
          </div>
        </section>

        {/* Section 3: Interactive Protocol Tools */}
        <section className="py-16 max-w-7xl mx-auto px-4 sm:px-6 lg:px-8">
          <div className="text-center max-w-2xl mx-auto mb-10">
            <Badge variant="purple">Live In-Browser Playgrounds</Badge>
            <h2 className="text-3xl font-extrabold tracking-tight text-text-primary mt-2">
              Interactive Protocol Sandboxes
            </h2>
            <p className="text-xs text-text-secondary mt-2">
              Test wire encoding, simulate Byzantine quorum meshes, canonicalize PACT contracts, and
              test REST APIs live in your browser.
            </p>
          </div>

          <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-6">
            <Link
              href="/sandbox"
              className="p-6 rounded-2xl border border-border-subtle bg-bg-surface hover:bg-bg-surface-raised transition-all hover:border-accent-primary/40 shadow-card group"
            >
              <Terminal className="w-8 h-8 text-accent-primary mb-4 group-hover:scale-110 transition-transform" />
              <h3 className="text-base font-bold text-text-primary group-hover:text-accent-primary transition-colors">
                Live Wire Frame Sandbox
              </h3>
              <p className="text-xs text-text-secondary mt-2 leading-relaxed">
                Toggle bitflags, inspect byte offsets, and view live 64-byte hex header encoding.
              </p>
            </Link>

            <Link
              href="/sandbox/poa-quorum"
              className="p-6 rounded-2xl border border-border-subtle bg-bg-surface hover:bg-bg-surface-raised transition-all hover:border-emerald-500/40 shadow-card group"
            >
              <Cpu className="w-8 h-8 text-emerald-400 mb-4 group-hover:scale-110 transition-transform" />
              <h3 className="text-base font-bold text-text-primary group-hover:text-emerald-400 transition-colors">
                PoA Quorum Calculator
              </h3>
              <p className="text-xs text-text-secondary mt-2 leading-relaxed">
                Simulate validator nodes, network partitions, and verify BFT quorum thresholds ($T \le N$).
              </p>
            </Link>

            <Link
              href="/sandbox/pact"
              className="p-6 rounded-2xl border border-border-subtle bg-bg-surface hover:bg-bg-surface-raised transition-all hover:border-purple-500/40 shadow-card group"
            >
              <ShieldCheck className="w-8 h-8 text-purple-400 mb-4 group-hover:scale-110 transition-transform" />
              <h3 className="text-base font-bold text-text-primary group-hover:text-purple-400 transition-colors">
                PACT Canonicalizer
              </h3>
              <p className="text-xs text-text-secondary mt-2 leading-relaxed">
                Deterministic JSON key sorting, BLAKE3 hashing, and Ed25519 signature generator.
              </p>
            </Link>

            <Link
              href="/api-explorer"
              className="p-6 rounded-2xl border border-border-subtle bg-bg-surface hover:bg-bg-surface-raised transition-all hover:border-sky-500/40 shadow-card group"
            >
              <PlayCircle className="w-8 h-8 text-sky-400 mb-4 group-hover:scale-110 transition-transform" />
              <h3 className="text-base font-bold text-text-primary group-hover:text-sky-400 transition-colors">
                Rivun Cloud REST API
              </h3>
              <p className="text-xs text-text-secondary mt-2 leading-relaxed">
                Execute live requests against SaaS endpoints with schema validation and low latency.
              </p>
            </Link>
          </div>
        </section>
      </main>

      <Footer />
    </div>
  );
}
