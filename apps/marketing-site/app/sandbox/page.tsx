import React from "react";
import { Navbar } from "../../components/Navbar";
import { ProtocolSandbox } from "../../components/ProtocolSandbox";
import { HeroFrameVisualizer } from "../../components/HeroFrameVisualizer";
import { Footer } from "../../components/Footer";
import { Binary, Terminal, ShieldCheck } from "lucide-react";

export const metadata = {
  title: "Protocol Sandbox | Rivun Protocol Playground",
  description:
    "Author, sign, and test Rivun binary wire frames, evaluate policy rules, and generate multi-language SDK code in Rust, TypeScript, Python, and Go.",
};

export default function SandboxPage() {
  return (
    <main className="min-h-screen bg-[#0A0B0D] flex flex-col pt-24">
      <Navbar />
      <div className="flex-1 max-w-7xl mx-auto px-4 sm:px-6 lg:px-8 py-10 w-full space-y-12">
        <div className="text-center max-w-3xl mx-auto space-y-3">
          <div className="inline-flex items-center gap-2 px-3 py-1 rounded-full bg-[#5B8CFF]/15 border border-[#5B8CFF]/30 text-[#5B8CFF] text-xs font-mono font-bold">
            <Terminal className="w-3.5 h-3.5" />
            <span>INTERACTIVE DEVELOPER PLAYGROUND</span>
          </div>
          <h1 className="text-3xl sm:text-5xl font-extrabold text-white tracking-tight">
            Rivun Binary Frame & Policy Sandbox
          </h1>
          <p className="text-sm sm:text-base text-[#9AA1AE]">
            Explore the binary wire header layout, test fail-closed AST policies, and generate production-ready SDK client code.
          </p>
        </div>

        {/* Embedded Live Encoder */}
        <HeroFrameVisualizer />

        {/* Embedded Code Generator & Policy Sandbox */}
        <ProtocolSandbox />
      </div>
      <Footer />
    </main>
  );
}
