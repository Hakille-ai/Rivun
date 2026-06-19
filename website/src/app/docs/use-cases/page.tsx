import Image from 'next/image';
import { Cpu, Activity, ShieldAlert, Bot, Cpu as RoboticsIcon, Layers } from 'lucide-react';
import { Card, CardHeader, CardTitle, CardContent, CardDescription } from "@/components/ui/card";

export default function UseCasesPage() {
  return (
    <div className="space-y-8 font-sans">
      <div>
        <h1 className="text-3xl md:text-4xl font-extrabold tracking-tight mb-2 text-white">Real-World Use Cases</h1>
        <p className="text-zinc-400 text-lg">Primary industry scenarios where ZAP adds significant security and performance advantages compared to traditional protocols like MQTT or gRPC.</p>
      </div>

      {/* Hero Visual Card */}
      <Card className="group bg-zinc-950/40 hover:bg-zinc-950/60 border border-zinc-850 hover:border-purple-500/20 transition-all duration-500 overflow-hidden rounded-2xl shadow-xl hover:shadow-2xl hover:shadow-purple-500/5 not-prose p-0">
        <div className="grid grid-cols-1 md:grid-cols-12 items-stretch">
          {/* Image Column */}
          <div className="md:col-span-5 relative bg-gradient-to-br from-zinc-950 to-black/80 flex items-center justify-center p-6 border-b md:border-b-0 md:border-r border-zinc-900 overflow-hidden min-h-[260px] md:min-h-0">
            {/* Ambient Glow Effects */}
            <div className="absolute inset-0 bg-[radial-gradient(circle_at_center,rgba(217,70,239,0.08)_0%,transparent_70%)] pointer-events-none" />
            <div className="absolute -inset-10 bg-[radial-gradient(circle_at_bottom_left,rgba(59,130,246,0.04)_0%,transparent_50%)] pointer-events-none" />
            
            {/* Image Wrapper */}
            <div className="relative w-full aspect-square max-w-[220px] md:max-w-[240px] transform group-hover:scale-[1.03] transition-transform duration-500 ease-out">
              <Image 
                src="/images/zap_use_cases.png" 
                alt="ZAP Protocol Real-World Use Cases Visual" 
                fill
                style={{ objectFit: 'contain' }}
                className="opacity-90 group-hover:opacity-100 transition-opacity duration-500"
                priority
              />
            </div>
          </div>
          {/* Content Column */}
          <div className="md:col-span-7 p-6 md:p-8 flex flex-col justify-center space-y-3 bg-zinc-950/20">
            <span className="text-xs font-semibold uppercase tracking-wider text-purple-400 block">Applications</span>
            <h3 className="text-xl font-bold text-white tracking-tight">Zero-Trust Edge & Swarm Architectures</h3>
            <p className="text-sm text-zinc-400 leading-relaxed">
              ZAP provides a secure, low-latency foundation for decentralized networks, enabling tamper-evident AI agent coordination, consensus-gated physical machine control, and secure sandboxing for smart home plugins.
            </p>
          </div>
        </div>
      </Card>

      <div className="grid grid-cols-1 md:grid-cols-3 gap-6">
        {/* Use Case 1 */}
        <Card className="bg-zinc-950/40 border-zinc-850">
          <CardHeader>
            <div className="flex items-center gap-3">
              <div className="w-8 h-8 rounded-lg bg-zinc-900 border border-zinc-800 flex items-center justify-center">
                <Bot className="w-4 h-4 text-purple-400" />
              </div>
              <div>
                <CardTitle className="text-white text-base">1. Multi-Agent AI Swarms</CardTitle>
                <CardDescription className="text-xs">Secure autonomous agent coordination</CardDescription>
              </div>
            </div>
          </CardHeader>
          <CardContent className="space-y-3 text-xs text-zinc-400">
            <p>
              Autonomous AI agents need to orchestrate complex tasks, pass contexts, and request actions from other tools. ZAP secures this loop:
            </p>
            <ul className="space-y-1.5 list-disc pl-5 text-zinc-300">
              <li><strong>Cryptographic Attribution:</strong> Every instruction carries the agent node&apos;s Ed25519 signature, preventing prompt injection or payload tampering.</li>
              <li><strong>Chain Audits:</strong> Causal correlation IDs chain agent operations. Processing outputs generate signed receipts committed to the hash ledger.</li>
            </ul>
          </CardContent>
        </Card>

        {/* Use Case 2 */}
        <Card className="bg-zinc-950/40 border-zinc-850">
          <CardHeader>
            <div className="flex items-center gap-3">
              <div className="w-8 h-8 rounded-lg bg-zinc-900 border border-zinc-800 flex items-center justify-center">
                <RoboticsIcon className="w-4 h-4 text-blue-400" />
              </div>
              <div>
                <CardTitle className="text-white text-base">2. Robotics & Smart Edge</CardTitle>
                <CardDescription className="text-xs">Microsecond loops for hardware control</CardDescription>
              </div>
            </div>
          </CardHeader>
          <CardContent className="space-y-3 text-xs text-zinc-400">
            <p>
              Industrial PLC controllers, robotic arms, and telemetry sensors require microsecond reaction loops without compromise.
            </p>
            <ul className="space-y-1.5 list-disc pl-5 text-zinc-300">
              <li><strong>No Head-of-Line Blocking:</strong> Direct UDP datagram routing over ChaCha20-Poly1305 skips the TCP handshake and HOL blocking of TLS tunnels.</li>
              <li><strong>Validator Consensus Quorum:</strong> High-risk device commands (e.g. <code>&quot;arm.rotate_degrees&quot;</code>) are blocked unless accompanied by threshold validation attestations (Proof-of-Action).</li>
            </ul>
          </CardContent>
        </Card>

        {/* Use Case 3 */}
        <Card className="bg-zinc-950/40 border-zinc-850">
          <CardHeader>
            <div className="flex items-center gap-3">
              <div className="w-8 h-8 rounded-lg bg-zinc-900 border border-zinc-800 flex items-center justify-center">
                <Layers className="w-4 h-4 text-emerald-400" />
              </div>
              <div>
                <CardTitle className="text-white text-base">3. Gateway Sandboxing</CardTitle>
                <CardDescription className="text-xs">Third-party plug-in isolation</CardDescription>
              </div>
            </div>
          </CardHeader>
          <CardContent className="space-y-3 text-xs text-zinc-400">
            <p>
              Smart home gateways require extensible third-party plugins (drivers) to talk to Zigbee, Z-Wave, or Modbus devices.
            </p>
            <ul className="space-y-1.5 list-disc pl-5 text-zinc-300">
              <li><strong>VM Isolation:</strong> Compiled WASM drivers run in a strict, bounded Wasmtime container (max 16 MB memory, instruction fuel caps).</li>
              <li><strong>Zero Ambient Authority:</strong> Drivers cannot open network connections, read files, or check environment variables unless explicitly allowed by manifests.</li>
            </ul>
          </CardContent>
        </Card>
      </div>
    </div>
  );
}
