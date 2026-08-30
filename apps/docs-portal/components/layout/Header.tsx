'use client';

import React from 'react';
import Link from 'next/link';
import { usePathname } from 'next/navigation';
import {
  Search,
  BookOpen,
  PlayCircle,
  Terminal,
  Layers,
  Menu,
  X,
  ExternalLink,
  ShieldCheck,
} from 'lucide-react';

interface HeaderProps {
  onOpenSearch: () => void;
  onToggleSidebar?: () => void;
  isSidebarOpen?: boolean;
}

export function Header({ onOpenSearch, onToggleSidebar, isSidebarOpen }: HeaderProps) {
  const pathname = usePathname();

  const navLinks = [
    { label: 'Documentation', href: '/docs/getting-started/overview', icon: BookOpen },
    { label: 'Wire Sandbox', href: '/sandbox', icon: Terminal },
    { label: 'PoA Quorum', href: '/sandbox/poa-quorum', icon: Layers },
    { label: 'API Explorer', href: '/api-explorer', icon: PlayCircle },
  ];

  return (
    <header className="sticky top-0 z-40 w-full glass-header">
      <div className="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8 h-16 flex items-center justify-between gap-4">
        {/* Left: Brand & Mobile Sidebar Toggle */}
        <div className="flex items-center gap-3">
          {onToggleSidebar && (
            <button
              onClick={onToggleSidebar}
              aria-label="Toggle navigation menu"
              className="lg:hidden p-2 rounded-lg text-text-secondary hover:text-text-primary hover:bg-bg-subtle border border-border-subtle"
            >
              {isSidebarOpen ? <X className="w-5 h-5" /> : <Menu className="w-5 h-5" />}
            </button>
          )}

          <Link href="/" className="flex items-center gap-2.5 group">
            <div className="w-8 h-8 rounded-lg bg-gradient-to-br from-cyan-400 to-indigo-600 flex items-center justify-center shadow-glow group-hover:scale-105 transition-transform">
              <ShieldCheck className="w-5 h-5 text-bg-base" />
            </div>
            <div className="flex flex-col">
              <div className="flex items-center gap-1.5">
                <span className="font-extrabold text-base tracking-tight text-text-primary">
                  RIVUN
                </span>
                <span className="text-[10px] font-mono px-1.5 py-0.2 rounded bg-cyan-500/10 text-cyan-400 border border-cyan-500/30">
                  DOCS
                </span>
              </div>
              <span className="text-[10px] font-mono text-text-muted">
                ZAP Universal Protocol
              </span>
            </div>
          </Link>
        </div>

        {/* Center: Search Trigger */}
        <div className="flex-1 max-w-md hidden sm:block">
          <button
            onClick={onOpenSearch}
            className="w-full flex items-center justify-between px-3.5 py-2 rounded-xl bg-bg-surface border border-border-subtle hover:border-accent-primary/50 text-text-muted hover:text-text-secondary transition-all shadow-card group"
          >
            <div className="flex items-center gap-2 text-xs">
              <Search className="w-4 h-4 text-text-muted group-hover:text-accent-primary transition-colors" />
              <span>Search docs, crates, wire format...</span>
            </div>
            <kbd className="flex items-center gap-0.5 px-2 py-0.5 rounded border border-border-subtle bg-bg-subtle text-[10px] font-mono text-text-muted">
              <span>⌘</span>K
            </kbd>
          </button>
        </div>

        {/* Right: Nav Links & Version */}
        <div className="flex items-center gap-2 sm:gap-4">
          <button
            onClick={onOpenSearch}
            aria-label="Open search modal"
            className="sm:hidden p-2 rounded-lg text-text-secondary hover:text-text-primary hover:bg-bg-subtle border border-border-subtle"
          >
            <Search className="w-4 h-4" />
          </button>

          <nav className="hidden md:flex items-center gap-1">
            {navLinks.map((link) => {
              const isActive = pathname.startsWith(link.href);
              const Icon = link.icon;
              return (
                <Link
                  key={link.href}
                  href={link.href}
                  className={`flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-xs font-medium transition-all ${
                    isActive
                      ? 'text-accent-primary bg-accent-primary/10 border border-accent-primary/30'
                      : 'text-text-secondary hover:text-text-primary hover:bg-bg-subtle'
                  }`}
                >
                  <Icon className="w-3.5 h-3.5" />
                  <span>{link.label}</span>
                </Link>
              );
            })}
          </nav>

          <div className="hidden lg:flex items-center pl-2 border-l border-border-subtle">
            <span className="text-[11px] font-mono text-text-muted bg-bg-surface px-2.5 py-1 rounded-md border border-border-subtle">
              v1.0.0-PROD
            </span>
          </div>
        </div>
      </div>
    </header>
  );
}
