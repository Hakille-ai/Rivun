import Image from 'next/image';
import { CheckCircle2, Circle, Milestone, Cpu, Layers, Archive, Shield, Rocket } from 'lucide-react';
import { Card, CardHeader, CardTitle, CardContent, CardDescription } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";

export default function RoadmapPage() {
  return (
    <div className="space-y-8 font-sans">
      <div>
        <h1 className="text-3xl md:text-4xl font-extrabold tracking-tight mb-2 text-white">Project Roadmap</h1>
        <p className="text-zinc-400 text-lg">Understand the current status, stable features, and future phases of the ZAP protocol.</p>
      </div>

      {/* Hero Visual Card */}
      <Card className="group bg-zinc-950/40 hover:bg-zinc-950/60 border border-zinc-850 hover:border-blue-500/20 transition-all duration-500 overflow-hidden rounded-2xl shadow-xl hover:shadow-2xl hover:shadow-blue-500/5 not-prose p-0">
        <div className="grid grid-cols-1 md:grid-cols-12 items-stretch">
          {/* Image Column */}
          <div className="md:col-span-5 relative bg-gradient-to-br from-zinc-950 to-black/80 flex items-center justify-center p-6 border-b md:border-b-0 md:border-r border-zinc-900 overflow-hidden min-h-[260px] md:min-h-0">
            {/* Ambient Glow Effects */}
            <div className="absolute inset-0 bg-[radial-gradient(circle_at_center,rgba(59,130,246,0.08)_0%,transparent_70%)] pointer-events-none" />
            <div className="absolute -inset-10 bg-[radial-gradient(circle_at_bottom_left,rgba(6,182,212,0.04)_0%,transparent_50%)] pointer-events-none" />
            
            {/* Image Wrapper */}
            <div className="relative w-full aspect-square max-w-[220px] md:max-w-[240px] transform group-hover:scale-[1.03] transition-transform duration-500 ease-out">
              <Image 
                src="/images/zap_roadmap_timeline.png" 
                alt="ZAP Protocol Milestones Roadmap Visual" 
                fill
                style={{ objectFit: 'contain' }}
                className="opacity-90 group-hover:opacity-100 transition-opacity duration-500"
                priority
              />
            </div>
          </div>
          {/* Content Column */}
          <div className="md:col-span-7 p-6 md:p-8 flex flex-col justify-center space-y-3 bg-zinc-950/20">
            <span className="text-xs font-semibold uppercase tracking-wider text-blue-400 block">Milestones</span>
            <h3 className="text-xl font-bold text-white tracking-tight">Evolution of the ZAP Stack</h3>
            <p className="text-sm text-zinc-400 leading-relaxed">
              The ZAP protocol&apos;s core execution sandbox, message validation layers, driver registry manifests, and Proof-of-Action consensus are fully implemented. Next milestones focus on dynamic network routing tables and peer orchestration VPNs.
            </p>
          </div>
        </div>
      </Card>

      {/* Phase Cards */}
      <div className="space-y-6">
        {/* Phase 1 */}
        <Card className="bg-zinc-950/40 border-zinc-850">
          <CardHeader className="pb-2">
            <div className="flex items-center justify-between">
              <div className="flex items-center gap-3">
                <div className="w-8 h-8 rounded-lg bg-zinc-900 border border-zinc-800 flex items-center justify-center">
                  <Cpu className="w-4 h-4 text-blue-400" />
                </div>
                <div>
                  <CardTitle className="text-white text-base">Phase 1: Kernel Alpha</CardTitle>
                  <CardDescription className="text-xs">Core engine primitives and sandboxing</CardDescription>
                </div>
              </div>
              <Badge className="bg-emerald-500/10 border-emerald-500/20 text-emerald-400 gap-1 text-[10px]">
                <CheckCircle2 className="w-3 h-3" /> Implemented
              </Badge>
            </div>
          </CardHeader>
          <CardContent className="text-sm text-zinc-400 pt-2">
            <ul className="space-y-2 text-xs text-zinc-300 grid grid-cols-1 md:grid-cols-2 gap-x-6 gap-y-2 pl-0 list-none">
              <li className="flex items-start gap-2">
                <span className="text-blue-400 font-semibold">•</span>
                <span><strong>Framing:</strong> Big-endian 64-byte binary ZAP-Wire framing.</span>
              </li>
              <li className="flex items-start gap-2">
                <span className="text-blue-400 font-semibold">•</span>
                <span><strong>Identity:</strong> Ed25519 signature validation and BLAKE3 node derivation.</span>
              </li>
              <li className="flex items-start gap-2">
                <span className="text-blue-400 font-semibold">•</span>
                <span><strong>Transport:</strong> Encrypted UDP peer-to-peer tunnels via ChaCha20-Poly1305.</span>
              </li>
              <li className="flex items-start gap-2">
                <span className="text-blue-400 font-semibold">•</span>
                <span><strong>Sandboxing:</strong> Zero-trust WASM driver isolation via Wasmtime.</span>
              </li>
            </ul>
          </CardContent>
        </Card>

        {/* Phase 2 */}
        <Card className="bg-zinc-950/40 border-zinc-850">
          <CardHeader className="pb-2">
            <div className="flex items-center justify-between">
              <div className="flex items-center gap-3">
                <div className="w-8 h-8 rounded-lg bg-zinc-900 border border-zinc-800 flex items-center justify-center">
                  <Layers className="w-4 h-4 text-purple-400" />
                </div>
                <div>
                  <CardTitle className="text-white text-base">Phase 2: Agent Gateway</CardTitle>
                  <CardDescription className="text-xs">Contract validation and packet evaluation layers</CardDescription>
                </div>
              </div>
              <Badge className="bg-emerald-500/10 border-emerald-500/20 text-emerald-400 gap-1 text-[10px]">
                <CheckCircle2 className="w-3 h-3" /> Implemented
              </Badge>
            </div>
          </CardHeader>
          <CardContent className="text-sm text-zinc-400 pt-2">
            <ul className="space-y-2 text-xs text-zinc-300 grid grid-cols-1 md:grid-cols-2 gap-x-6 gap-y-2 pl-0 list-none">
              <li className="flex items-start gap-2">
                <span className="text-purple-400 font-semibold">•</span>
                <span><strong>Payload Validation:</strong> Schema contract compilation via <code>zap-schema</code>.</span>
              </li>
              <li className="flex items-start gap-2">
                <span className="text-purple-400 font-semibold">•</span>
                <span><strong>Message Policy:</strong> Access control evaluations via <code>zap-policy</code>.</span>
              </li>
              <li className="flex items-start gap-2">
                <span className="text-purple-400 font-semibold">•</span>
                <span><strong>Consensus Request:</strong> Supplying validation signatures for consensus-protected action frames.</span>
              </li>
            </ul>
          </CardContent>
        </Card>

        {/* Phase 3 */}
        <Card className="bg-zinc-950/40 border-zinc-850">
          <CardHeader className="pb-2">
            <div className="flex items-center justify-between">
              <div className="flex items-center gap-3">
                <div className="w-8 h-8 rounded-lg bg-zinc-900 border border-zinc-800 flex items-center justify-center">
                  <Archive className="w-4 h-4 text-blue-400" />
                </div>
                <div>
                  <CardTitle className="text-white text-base">Phase 3: Driver Registry</CardTitle>
                  <CardDescription className="text-xs">ZapStore manifest indexing and bundle distribution</CardDescription>
                </div>
              </div>
              <Badge className="bg-emerald-500/10 border-emerald-500/20 text-emerald-400 gap-1 text-[10px]">
                <CheckCircle2 className="w-3 h-3" /> Implemented
              </Badge>
            </div>
          </CardHeader>
          <CardContent className="text-sm text-zinc-400 pt-2">
            <ul className="space-y-2 text-xs text-zinc-300 grid grid-cols-1 md:grid-cols-2 gap-x-6 gap-y-2 pl-0 list-none">
              <li className="flex items-start gap-2">
                <span className="text-blue-400 font-semibold">•</span>
                <span><strong>ZapStore Manifests:</strong> Signed driver manifests binding hashes, ABI, and authors.</span>
              </li>
              <li className="flex items-start gap-2">
                <span className="text-blue-400 font-semibold">•</span>
                <span><strong>Revocation Index:</strong> Approved indices and manifest version control tables.</span>
              </li>
              <li className="flex items-start gap-2">
                <span className="text-blue-400 font-semibold">•</span>
                <span><strong>Bundles:</strong> Export, offline verification, and import of registry bundle directories.</span>
              </li>
            </ul>
          </CardContent>
        </Card>

        {/* Phase 4 */}
        <Card className="bg-zinc-950/40 border-zinc-850">
          <CardHeader className="pb-2">
            <div className="flex items-center justify-between">
              <div className="flex items-center gap-3">
                <div className="w-8 h-8 rounded-lg bg-zinc-900 border border-zinc-800 flex items-center justify-center">
                  <Shield className="w-4 h-4 text-amber-400" />
                </div>
                <div>
                  <CardTitle className="text-white text-base">Phase 4: Proof-of-Action Network</CardTitle>
                  <CardDescription className="text-xs">Quorum threshold validation and audit logging</CardDescription>
                </div>
              </div>
              <Badge className="bg-emerald-500/10 border-emerald-500/20 text-emerald-400 gap-1 text-[10px]">
                <CheckCircle2 className="w-3 h-3" /> Implemented
              </Badge>
            </div>
          </CardHeader>
          <CardContent className="text-sm text-zinc-400 pt-2">
            <ul className="space-y-2 text-xs text-zinc-300 grid grid-cols-1 md:grid-cols-2 gap-x-6 gap-y-2 pl-0 list-none">
              <li className="flex items-start gap-2">
                <span className="text-amber-400 font-semibold">•</span>
                <span><strong>ZPOA Trailers:</strong> Attestation validation of frames marked with threshold signatures.</span>
              </li>
              <li className="flex items-start gap-2">
                <span className="text-amber-400 font-semibold">•</span>
                <span><strong>Validator Sets:</strong> Operator-signed versioned validator sets.</span>
              </li>
              <li className="flex items-start gap-2">
                <span className="text-amber-400 font-semibold">•</span>
                <span><strong>Receipt Ledgers:</strong> Tamper-evident signed JSONL audit logs.</span>
              </li>
            </ul>
          </CardContent>
        </Card>

        {/* Phase 5 */}
        <Card className="bg-zinc-950/40 border-zinc-850">
          <CardHeader className="pb-2">
            <div className="flex items-center justify-between">
              <div className="flex items-center gap-3">
                <div className="w-8 h-8 rounded-lg bg-zinc-900 border border-zinc-800 flex items-center justify-center">
                  <Rocket className="w-4 h-4 text-blue-400" />
                </div>
                <div>
                  <CardTitle className="text-white text-base">Phase 5: Future Core Interfaces</CardTitle>
                  <CardDescription className="text-xs">VPN overlay tunnels and dynamic enrollment</CardDescription>
                </div>
              </div>
              <Badge className="bg-blue-500/10 border-blue-500/20 text-blue-400 gap-1 text-[10px]">
                <Circle className="w-2.5 h-2.5 fill-blue-400/30 animate-pulse" /> In Progress
              </Badge>
            </div>
          </CardHeader>
          <CardContent className="text-sm text-zinc-400 pt-2">
            <p className="text-xs text-zinc-300 leading-relaxed">
              Active development focuses on live peer enrollment handshakes, dynamic revocation table propagation, fleet orchestration tools, and low-overhead stream mesh protocols utilizing multiplexed UDP transport.
            </p>
          </CardContent>
        </Card>
      </div>
    </div>
  );
}
