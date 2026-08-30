import React from 'react';
import Link from 'next/link';
import { ShieldCheck, ArrowUpRight, Github, Lock } from 'lucide-react';

export function Footer() {
  return (
    <footer className="border-t border-border-subtle bg-bg-surface/60 backdrop-blur-md mt-20">
      <div className="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8 py-12">
        <div className="grid grid-cols-1 md:grid-cols-4 gap-8">
          {/* Brand Info */}
          <div className="space-y-3">
            <div className="flex items-center gap-2">
              <div className="w-7 h-7 rounded-lg bg-gradient-to-br from-cyan-400 to-indigo-600 flex items-center justify-center">
                <ShieldCheck className="w-4 h-4 text-bg-base" />
              </div>
              <span className="font-extrabold text-base tracking-tight text-text-primary">
                RIVUN
              </span>
            </div>
            <p className="text-xs text-text-secondary leading-relaxed">
              The zero-trust, high-throughput autonomous automation and consensus protocol.
            </p>
            <div className="flex items-center gap-2 text-xs text-cyan-400 font-mono">
              <Lock className="w-3.5 h-3.5" />
              <span>Sovereign Identity Protocol</span>
            </div>
          </div>

          {/* Core Protocol */}
          <div className="space-y-2">
            <h4 className="text-xs font-bold uppercase tracking-wider text-text-primary">
              Core Protocol
            </h4>
            <ul className="space-y-1.5 text-xs text-text-secondary">
              <li>
                <Link href="/docs/architecture/wire-format" className="hover:text-text-primary transition-colors">
                  64-Byte Wire Header
                </Link>
              </li>
              <li>
                <Link href="/docs/architecture/universal-envelope" className="hover:text-text-primary transition-colors">
                  Universal Envelope (ZENV)
                </Link>
              </li>
              <li>
                <Link href="/docs/consensus/bft-consensus" className="hover:text-text-primary transition-colors">
                  Proof-of-Action (PoA)
                </Link>
              </li>
              <li>
                <Link href="/docs/runtime/wasm-sandboxing" className="hover:text-text-primary transition-colors">
                  WASM Sandbox & ABI v1
                </Link>
              </li>
            </ul>
          </div>

          {/* SDKs & Tooling */}
          <div className="space-y-2">
            <h4 className="text-xs font-bold uppercase tracking-wider text-text-primary">
              SDKs & Tooling
            </h4>
            <ul className="space-y-1.5 text-xs text-text-secondary">
              <li>
                <Link href="/docs/sdks/rust" className="hover:text-text-primary transition-colors">
                  Rust SDK
                </Link>
              </li>
              <li>
                <Link href="/docs/sdks/typescript" className="hover:text-text-primary transition-colors">
                  TypeScript SDK
                </Link>
              </li>
              <li>
                <Link href="/docs/sdks/python" className="hover:text-text-primary transition-colors">
                  Python SDK
                </Link>
              </li>
              <li>
                <Link href="/docs/sdks/go" className="hover:text-text-primary transition-colors">
                  Go SDK
                </Link>
              </li>
            </ul>
          </div>

          {/* Interactive Tools */}
          <div className="space-y-2">
            <h4 className="text-xs font-bold uppercase tracking-wider text-text-primary">
              Interactive Tools
            </h4>
            <ul className="space-y-1.5 text-xs text-text-secondary">
              <li>
                <Link href="/sandbox" className="hover:text-text-primary transition-colors">
                  Live Wire Frame Sandbox
                </Link>
              </li>
              <li>
                <Link href="/sandbox/poa-quorum" className="hover:text-text-primary transition-colors">
                  PoA Quorum Calculator
                </Link>
              </li>
              <li>
                <Link href="/sandbox/pact" className="hover:text-text-primary transition-colors">
                  PACT Canonicalizer
                </Link>
              </li>
              <li>
                <Link href="/api-explorer" className="hover:text-text-primary transition-colors">
                  Rivun Cloud API Explorer
                </Link>
              </li>
            </ul>
          </div>
        </div>

        <div className="mt-10 pt-6 border-t border-border-subtle flex flex-col sm:flex-row items-center justify-between gap-4 text-xs text-text-muted">
          <div>
            &copy; {new Date().getFullYear()} Rivun Protocol (Hakille-ai/ZAP). Open source under MIT/Apache 2.0.
          </div>
          <div className="flex items-center gap-4">
            <span className="font-mono text-[11px] text-cyan-400">
              Big-Endian Wire Magic: 0x5A41505F
            </span>
          </div>
        </div>
      </div>
    </footer>
  );
}
