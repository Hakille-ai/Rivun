import Image from 'next/image';
import { Key, Wrench, GitBranch, ShieldAlert } from 'lucide-react';
import { Card, CardHeader, CardTitle, CardContent, CardDescription } from "@/components/ui/card";
import { Alert, AlertTitle, AlertDescription } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";

export default function GettingStartedPage() {
  return (
    <div className="space-y-8 font-sans">
      <div>
        <h1 className="text-3xl md:text-4xl font-extrabold tracking-tight mb-2 text-white">Getting Started</h1>
        <p className="text-zinc-400 text-lg">Build the CLI, verify fixtures, validate packs, and run the local smoke test in minutes.</p>
      </div>

      {/* Hero Visual Card */}
      <Card className="group bg-zinc-950/40 hover:bg-zinc-950/60 border border-zinc-850 hover:border-blue-500/20 transition-all duration-500 overflow-hidden rounded-2xl shadow-xl hover:shadow-2xl hover:shadow-blue-500/5 not-prose p-0">
        <div className="grid grid-cols-1 md:grid-cols-12 items-stretch">
          {/* Image Column */}
          <div className="md:col-span-5 relative bg-gradient-to-br from-zinc-950 to-black/80 flex items-center justify-center p-6 border-b md:border-b-0 md:border-r border-zinc-900 overflow-hidden min-h-[260px] md:min-h-0">
            {/* Ambient Glow Effects */}
            <div className="absolute inset-0 bg-[radial-gradient(circle_at_center,rgba(59,130,246,0.08)_0%,transparent_70%)] pointer-events-none" />
            <div className="absolute -inset-10 bg-[radial-gradient(circle_at_bottom_left,rgba(168,85,247,0.04)_0%,transparent_50%)] pointer-events-none" />
            
            {/* Image Wrapper */}
            <div className="relative w-full aspect-square max-w-[220px] md:max-w-[240px] transform group-hover:scale-[1.03] transition-transform duration-500 ease-out">
              <Image 
                src="/images/zap_getting_started_flow.png" 
                alt="ZAP Node Onboarding Setup Flow" 
                fill
                style={{ objectFit: 'contain' }}
                className="opacity-90 group-hover:opacity-100 transition-opacity duration-500"
                priority
              />
            </div>
          </div>
          {/* Content Column */}
          <div className="md:col-span-7 p-6 md:p-8 flex flex-col justify-center space-y-3 bg-zinc-950/20">
            <span className="text-xs font-semibold uppercase tracking-wider text-blue-400 block">Onboarding Pipeline</span>
            <h3 className="text-xl font-bold text-white tracking-tight">Five-Minute Source Check</h3>
            <p className="text-sm text-zinc-400 leading-relaxed">
              Go from cloning the repository to a verified local toolchain. Build the CLI, check the shared protocol fixtures, validate domain packs, and use the smoke test for live dispatch coverage.
            </p>
          </div>
        </div>
      </Card>

      <Alert className="border-blue-500/20 bg-blue-500/5 text-blue-300">
        <ShieldAlert className="w-4 h-4 text-blue-400" />
        <AlertTitle className="font-semibold text-blue-400">Pre-1.0 Alpha Release</AlertTitle>
        <AlertDescription className="text-xs">
          ZAP is in active development. APIs, CLI flags, and wire framing specs may evolve. Ensure you follow standard security precautions.
        </AlertDescription>
      </Alert>

      <div className="grid grid-cols-1 md:grid-cols-2 gap-6">
        <Card className="bg-zinc-950/40 border-zinc-850">
          <CardHeader>
            <div className="flex items-center gap-3">
              <div className="w-8 h-8 rounded-lg bg-zinc-900 border border-zinc-800 flex items-center justify-center">
                <Wrench className="w-4 h-4 text-zinc-400" />
              </div>
              <div>
                <CardTitle className="text-white text-base">Prerequisites</CardTitle>
                <CardDescription className="text-xs">Required compilation tools</CardDescription>
              </div>
            </div>
          </CardHeader>
          <CardContent className="space-y-3">
            <div className="flex items-center justify-between p-3 rounded-lg bg-[#050505] border border-zinc-900">
              <span className="text-sm text-zinc-300">Rust Toolchain</span>
              <Badge variant="outline" className="bg-zinc-900 border-zinc-800 text-blue-400 font-mono text-[10px]">1.93+ (2024 Edition)</Badge>
            </div>
            <div className="flex items-center justify-between p-3 rounded-lg bg-[#050505] border border-zinc-900">
              <span className="text-sm text-zinc-300">Version Control</span>
              <Badge variant="outline" className="bg-zinc-900 border-zinc-800 text-zinc-400 font-mono text-[10px]">Git CLI</Badge>
            </div>
          </CardContent>
        </Card>

        <Card className="bg-zinc-950/40 border-zinc-850">
          <CardHeader>
            <div className="flex items-center gap-3">
              <div className="w-8 h-8 rounded-lg bg-zinc-900 border border-zinc-800 flex items-center justify-center">
                <GitBranch className="w-4 h-4 text-zinc-400" />
              </div>
              <div>
                <CardTitle className="text-white text-base">Clone & Build</CardTitle>
                <CardDescription className="text-xs">Compile CLI binary from source</CardDescription>
              </div>
            </div>
          </CardHeader>
          <CardContent className="space-y-2">
            <pre className="text-xs bg-[#050505] p-3 rounded-lg border border-zinc-900 font-mono text-zinc-300">
              <code>{`git clone https://github.com/Hakille-ai/ZAP.git
cd ZAP
cargo build --locked -p zap-cli`}</code>
            </pre>
          </CardContent>
        </Card>
      </div>

      <div className="space-y-6">
        <h2 className="text-xl font-bold text-white flex items-center gap-2">
          <Key className="w-5 h-5 text-blue-500" /> Setup & Verification
        </h2>

        <div className="space-y-4">
          <div className="flex flex-col md:flex-row gap-6 p-6 rounded-xl border border-zinc-850 bg-zinc-950/20">
            <div className="flex-1 space-y-2">
              <div className="flex items-center gap-2">
                <Badge className="bg-blue-500/10 text-blue-400 border-blue-500/20 text-[10px] px-2 py-0.5">Step 1</Badge>
                <h3 className="text-white font-semibold">Verify Protocol Fixtures</h3>
              </div>
              <p className="text-sm text-zinc-400">
                Shared fixtures keep Rust, Python, TypeScript, Go, and operator tooling aligned on stable protocol contracts, including PACT records and bundles.
              </p>
            </div>
            <div className="flex-1">
              <pre className="text-xs bg-[#050505] p-3.5 rounded-lg border border-zinc-900 font-mono text-zinc-300">
                <code>{`cargo run --locked -p zap-cli -- \\
  fixtures verify --fixtures fixtures`}</code>
              </pre>
            </div>
          </div>

          <div className="flex flex-col md:flex-row gap-6 p-6 rounded-xl border border-zinc-850 bg-zinc-950/20">
            <div className="flex-1 space-y-2">
              <div className="flex items-center gap-2">
                <Badge className="bg-blue-500/10 text-blue-400 border-blue-500/20 text-[10px] px-2 py-0.5">Step 2</Badge>
                <h3 className="text-white font-semibold">Verify a PACT Record</h3>
              </div>
              <p className="text-sm text-zinc-400">
                PACT records prove intent, consent, proof, terms, and revocation state using the same ZAP key files and offline verification model.
              </p>
            </div>
            <div className="flex-1">
              <pre className="text-xs bg-[#050505] p-3.5 rounded-lg border border-zinc-900 font-mono text-zinc-300">
                <code>{`cargo run --locked -p zap-cli -- pact create \\
  --actor agent.alpha --target driver.valve --intent valve.open \\
  --out pact-unsigned.json --force
cargo run --locked -p zap-cli -- pact sign \\
  --input pact-unsigned.json --key .zap/node.key \\
  --out pact-signed.json --force
cargo run --locked -p zap-cli -- pact verify \\
  --input pact-signed.json --json`}</code>
              </pre>
            </div>
          </div>

          <div className="flex flex-col md:flex-row gap-6 p-6 rounded-xl border border-zinc-850 bg-zinc-950/20">
            <div className="flex-1 space-y-2">
              <div className="flex items-center gap-2">
                <Badge className="bg-blue-500/10 text-blue-400 border-blue-500/20 text-[10px] px-2 py-0.5">Step 3</Badge>
                <h3 className="text-white font-semibold">Validate Domain Packs</h3>
              </div>
              <p className="text-sm text-zinc-400">
                Domain packs package capabilities, schemas, policies, and examples without weakening the core safety model.
              </p>
            </div>
            <div className="flex-1">
              <pre className="text-xs bg-[#050505] p-3.5 rounded-lg border border-zinc-900 font-mono text-zinc-300">
                <code>{`cargo run --locked -p zap-cli -- \\
  pack list --root examples/domain-packs`}</code>
              </pre>
            </div>
          </div>

          <div className="flex flex-col md:flex-row gap-6 p-6 rounded-xl border border-zinc-850 bg-zinc-950/20">
            <div className="flex-1 space-y-2">
              <div className="flex items-center gap-2">
                <Badge className="bg-blue-500/10 text-blue-400 border-blue-500/20 text-[10px] px-2 py-0.5">Step 4</Badge>
                <h3 className="text-white font-semibold">Run Live Dispatch Smoke</h3>
              </div>
              <p className="text-sm text-zinc-400">
                The smoke test launches a local node, sends an action, and verifies the receipt path without manual peer editing.
              </p>
            </div>
            <div className="flex-1">
              <pre className="text-xs bg-[#050505] p-3.5 rounded-lg border border-zinc-900 font-mono text-zinc-300">
                <code>{`cargo ci-smoke`}</code>
              </pre>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}
