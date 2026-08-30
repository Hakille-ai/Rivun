"use client";

import React, { useState, useEffect } from "react";
import Link from "next/link";
import { 
  ShieldCheck, 
  Terminal, 
  Cpu, 
  Layers, 
  Cloud, 
  Lock, 
  Calculator, 
  Menu, 
  X, 
  ExternalLink,
  ChevronRight,
  Sparkles
} from "lucide-react";

export function Navbar() {
  const [isScrolled, setIsScrolled] = useState(false);
  const [mobileMenuOpen, setMobileMenuOpen] = useState(false);

  useEffect(() => {
    const handleScroll = () => {
      setIsScrolled(window.scrollY > 20);
    };
    window.addEventListener("scroll", handleScroll);
    return () => window.removeEventListener("scroll", handleScroll);
  }, []);

  const navLinks = [
    { name: "Protocol", href: "#innovations", icon: Cpu },
    { name: "P2P Swarm", href: "#swarm", icon: Layers },
    { name: "Domain Packs", href: "#domain-packs", icon: ShieldCheck },
    { name: "Cloud & Station", href: "#cloud", icon: Cloud },
    { name: "Security & Proofs", href: "#security", icon: Lock },
    { name: "Pricing & ROI", href: "#pricing", icon: Calculator },
  ];

  return (
    <header
      className={`fixed top-0 left-0 right-0 z-50 transition-all duration-300 ${
        isScrolled
          ? "bg-[#0A0B0D]/85 backdrop-blur-xl border-b border-[#22262F]/80 shadow-2xl py-3"
          : "bg-transparent py-5"
      }`}
    >
      <div className="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8 flex items-center justify-between">
        {/* Brand Logo */}
        <Link href="/" className="flex items-center gap-3 group">
          <div className="relative flex items-center justify-center w-10 h-10 rounded-xl bg-gradient-to-br from-[#5B8CFF] to-[#3B72F2] p-[1px] shadow-glow">
            <div className="w-full h-full bg-[#0A0B0D] rounded-[11px] flex items-center justify-center group-hover:bg-[#111318] transition-colors">
              <span className="font-mono font-bold text-lg text-white tracking-wider flex items-center">
                <span className="text-[#5B8CFF]">R</span>
                <span className="text-xs text-[#3DD68C] ml-0.5">▪</span>
              </span>
            </div>
            {/* Live Pulse Indicator */}
            <span className="absolute -top-1 -right-1 flex h-2.5 w-2.5">
              <span className="animate-ping absolute inline-flex h-full w-full rounded-full bg-[#3DD68C] opacity-75"></span>
              <span className="relative inline-flex rounded-full h-2.5 w-2.5 bg-[#3DD68C]"></span>
            </span>
          </div>
          <div>
            <div className="flex items-center gap-1.5">
              <span className="font-bold text-lg text-white tracking-tight">RIVUN</span>
              <span className="px-1.5 py-0.5 text-[10px] font-mono font-semibold bg-[#5B8CFF]/15 text-[#5B8CFF] border border-[#5B8CFF]/30 rounded">
                ZAP v1
              </span>
            </div>
            <p className="text-[11px] text-[#9AA1AE] hidden sm:block font-mono">Zero-Trust Agent Protocol</p>
          </div>
        </Link>

        {/* Desktop Navigation Links */}
        <nav className="hidden lg:flex items-center gap-1 bg-[#111318]/70 border border-[#22262F] px-3 py-1.5 rounded-full backdrop-blur-md">
          {navLinks.map((link) => (
            <a
              key={link.name}
              href={link.href}
              className="px-3.5 py-1.5 text-xs font-medium text-[#9AA1AE] hover:text-white hover:bg-white/5 rounded-full transition-all duration-200"
            >
              {link.name}
            </a>
          ))}
          <Link
            href="/sandbox"
            className="px-3.5 py-1.5 text-xs font-semibold text-[#5B8CFF] hover:text-white hover:bg-[#5B8CFF]/20 rounded-full transition-all duration-200 flex items-center gap-1.5"
          >
            <Terminal className="w-3.5 h-3.5" />
            <span>Sandbox</span>
          </Link>
        </nav>

        {/* Action Buttons */}
        <div className="hidden sm:flex items-center gap-3">
          <a
            href="http://localhost:3002"
            target="_blank"
            rel="noopener noreferrer"
            className="px-3.5 py-2 text-xs font-medium text-[#9AA1AE] hover:text-white border border-[#22262F] hover:border-[#3A4150] bg-[#111318]/60 hover:bg-[#181B22] rounded-lg transition-all duration-200 flex items-center gap-1.5"
          >
            <span>Docs Portal</span>
            <ExternalLink className="w-3 h-3 text-[#9AA1AE]" />
          </a>

          <Link
            href="/sandbox"
            className="relative group overflow-hidden px-4 py-2 text-xs font-semibold text-white bg-gradient-to-r from-[#5B8CFF] to-[#3B72F2] hover:from-[#4378F0] hover:to-[#2B60E0] rounded-lg shadow-glow transition-all duration-200 flex items-center gap-1.5"
          >
            <Sparkles className="w-3.5 h-3.5 text-white animate-pulse" />
            <span>Launch Sandbox</span>
            <ChevronRight className="w-3 h-3 group-hover:translate-x-0.5 transition-transform" />
          </Link>
        </div>

        {/* Mobile Menu Button */}
        <div className="flex lg:hidden items-center gap-2">
          <button
            onClick={() => setMobileMenuOpen(!mobileMenuOpen)}
            className="p-2 text-[#9AA1AE] hover:text-white bg-[#111318] border border-[#22262F] rounded-lg transition-colors"
            aria-label="Toggle navigation menu"
          >
            {mobileMenuOpen ? <X className="w-5 h-5" /> : <Menu className="w-5 h-5" />}
          </button>
        </div>
      </div>

      {/* Mobile Dropdown Drawer */}
      {mobileMenuOpen && (
        <div className="lg:hidden fixed inset-x-0 top-[60px] bg-[#0A0B0D]/95 backdrop-blur-2xl border-b border-[#22262F] px-6 py-6 transition-all duration-300">
          <div className="flex flex-col gap-2">
            {navLinks.map((link) => {
              const Icon = link.icon;
              return (
                <a
                  key={link.name}
                  href={link.href}
                  onClick={() => setMobileMenuOpen(false)}
                  className="flex items-center gap-3 px-4 py-3 text-sm font-medium text-[#9AA1AE] hover:text-white hover:bg-[#181B22] rounded-lg border border-transparent hover:border-[#22262F] transition-all"
                >
                  <Icon className="w-4 h-4 text-[#5B8CFF]" />
                  <span>{link.name}</span>
                </a>
              );
            })}

            <Link
              href="/sandbox"
              onClick={() => setMobileMenuOpen(false)}
              className="flex items-center gap-3 px-4 py-3 text-sm font-semibold text-[#5B8CFF] hover:text-white hover:bg-[#5B8CFF]/15 rounded-lg border border-[#5B8CFF]/30 transition-all"
            >
              <Terminal className="w-4 h-4 text-[#5B8CFF]" />
              <span>Interactive Sandbox</span>
            </Link>

            <div className="pt-4 mt-2 border-t border-[#22262F] flex flex-col gap-3">
              <a
                href="http://localhost:3002"
                target="_blank"
                rel="noopener noreferrer"
                className="flex items-center justify-center gap-2 px-4 py-2.5 text-xs font-medium text-white bg-[#111318] border border-[#22262F] rounded-lg"
              >
                <span>Explore Developer Docs</span>
                <ExternalLink className="w-3.5 h-3.5" />
              </a>

              <Link
                href="/sandbox"
                onClick={() => setMobileMenuOpen(false)}
                className="flex items-center justify-center gap-2 px-4 py-2.5 text-xs font-semibold text-white bg-[#5B8CFF] rounded-lg shadow-glow"
              >
                <Sparkles className="w-3.5 h-3.5" />
                <span>Launch Protocol Sandbox</span>
              </Link>
            </div>
          </div>
        </div>
      )}
    </header>
  );
}
