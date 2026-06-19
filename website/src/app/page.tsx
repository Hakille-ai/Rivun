import Link from 'next/link';
import Image from 'next/image';
import { ArrowRight, ShieldCheck, Zap, Cpu, Lock, Network, Database, Activity } from 'lucide-react';
import { Button } from "@/components/ui/button";
import { Card, CardHeader, CardTitle, CardDescription, CardContent } from "@/components/ui/card";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import Footer from "@/components/layout/Footer";

export default function Home() {
  return (
    <div className="flex-1 overflow-y-auto w-full">
      <div className="flex flex-col items-center justify-center min-h-screen">
      <div className="glow-bg top-0 left-1/2 -translate-x-1/2"></div>
      
      {/* Hero Section */}
      <section className="w-full relative min-h-[90vh] flex items-center justify-center pt-24 pb-12 overflow-hidden">
        <div className="absolute inset-0 z-0">
          <Image 
            src="/images/zap_hero.png" 
            alt="ZAP Hero Network Mesh" 
            fill
            style={{ objectFit: 'cover' }}
            className="opacity-30 mix-blend-screen"
            priority
          />
          <div className="absolute inset-0 bg-gradient-to-b from-transparent via-black/50 to-black"></div>
        </div>

        <div className="w-full max-w-7xl px-6 text-center relative z-10">
          <div className="inline-flex items-center gap-2 px-3 py-1 rounded-full bg-blue-500/10 border border-blue-500/20 text-blue-400 text-sm font-medium mb-8 backdrop-blur-md">
            <span className="w-2 h-2 rounded-full bg-blue-500 animate-pulse"></span>
            Pre-1.0 Alpha Available
          </div>
          <h1 className="text-5xl md:text-7xl font-extrabold tracking-tight mb-8 text-transparent bg-clip-text bg-gradient-to-b from-white to-zinc-400">
            The Universal Low-Latency Protocol<br className="hidden md:block" /> for Typed Message Dispatch
          </h1>
          <p className="text-xl md:text-2xl text-zinc-300 max-w-3xl mx-auto mb-12 drop-shadow-md">
            End-to-end cryptographic provenance, zero-trust WASM sandboxing, and Proof-of-Action consensus. 
            Built for AI agents, robotics, and edge environments.
          </p>
          <div className="flex flex-col sm:flex-row items-center justify-center gap-4">
            <Button render={<Link href="/docs/getting-started" />} size="lg" className="rounded-full bg-white text-black hover:bg-zinc-200 px-8 py-6 text-base font-semibold">
              Get Started <ArrowRight className="ml-2 w-5 h-5" />
            </Button>
            <Button render={<Link href="/docs" />} size="lg" variant="outline" className="rounded-full border-zinc-700 hover:bg-zinc-800 text-white px-8 py-6 text-base font-semibold backdrop-blur-md">
              Read Documentation
            </Button>
          </div>
        </div>
      </section>

      {/* Architecture Showcase */}
      <section className="w-full max-w-7xl px-6 py-24 relative z-10">
        <div className="text-center mb-16">
          <h2 className="text-3xl md:text-5xl font-bold mb-4">Uncompromising Architecture</h2>
          <p className="text-zinc-400 text-lg max-w-2xl mx-auto">
            A secure pipeline orchestrating zero-trust distributed actions.
          </p>
        </div>
        
        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-6 items-stretch">
          {/* Step 1: ZENV Envelope */}
          <Card className="bg-zinc-950/40 border-zinc-800 hover:border-blue-500/30 transition-all duration-300 flex flex-col justify-between">
            <CardHeader className="pb-2">
              <div className="text-xs font-semibold text-blue-500 uppercase tracking-widest mb-1">01. Wire Frame</div>
              <CardTitle className="text-white text-lg">ZENV Envelope</CardTitle>
              <CardDescription className="text-zinc-400 text-xs">Messages prefix every action payload with a typed 74-byte header.</CardDescription>
            </CardHeader>
            <CardContent className="pt-0">
              <div className="bg-[#050505] p-3 rounded-lg border border-zinc-900 font-mono text-[11px] text-zinc-500 space-y-1">
                <div><span className="text-blue-400">magic:</span> 0x5A41505F</div>
                <div><span className="text-blue-400">version:</span> 1</div>
                <div><span className="text-blue-400">flags:</span> SIGNED | ENCRYPT</div>
                <div><span className="text-blue-400">action:</span> &quot;thermostat.set&quot;</div>
              </div>
            </CardContent>
          </Card>

          {/* Step 2: Cryptographic Verification */}
          <Card className="bg-zinc-950/40 border-zinc-800 hover:border-purple-500/30 transition-all duration-300 flex flex-col justify-between">
            <CardHeader className="pb-2">
              <div className="text-xs font-semibold text-purple-500 uppercase tracking-widest mb-1">02. Security</div>
              <CardTitle className="text-white text-lg">Provenance Check</CardTitle>
              <CardDescription className="text-zinc-400 text-xs">Ed25519 signatures verify sender identity using a BLAKE3 public key hash.</CardDescription>
            </CardHeader>
            <CardContent className="pt-0">
              <div className="bg-[#050505] p-3 rounded-lg border border-zinc-900 font-mono text-[11px] text-zinc-500 space-y-1">
                <div><span className="text-purple-400">pubkey:</span> ed25519_pk...</div>
                <div><span className="text-purple-400">hash:</span> blake3_digest...</div>
                <div className="text-green-500 flex items-center gap-1 mt-1 font-semibold">✓ Verified</div>
              </div>
            </CardContent>
          </Card>

          {/* Step 3: WASM Sandbox */}
          <Card className="bg-zinc-950/40 border-zinc-800 hover:border-emerald-500/30 transition-all duration-300 flex flex-col justify-between">
            <CardHeader className="pb-2">
              <div className="text-xs font-semibold text-emerald-500 uppercase tracking-widest mb-1">03. Execution</div>
              <CardTitle className="text-white text-lg">WASM Sandbox</CardTitle>
              <CardDescription className="text-zinc-400 text-xs">VM execution with strict instruction fuel, 16MB memory bounds, and deny-by-default imports.</CardDescription>
            </CardHeader>
            <CardContent className="pt-0">
              <div className="bg-[#050505] p-3 rounded-lg border border-zinc-900 font-mono text-[11px] text-zinc-500 space-y-1">
                <div><span className="text-emerald-400">sandbox_mem:</span> 16 MB max</div>
                <div><span className="text-emerald-400">fuel_limit:</span> 1,000,000</div>
                <div><span className="text-emerald-400">host_imports:</span> Restricted</div>
              </div>
            </CardContent>
          </Card>

          {/* Step 4: Ledger & Consensus */}
          <Card className="bg-zinc-950/40 border-zinc-800 hover:border-amber-500/30 transition-all duration-300 flex flex-col justify-between">
            <CardHeader className="pb-2">
              <div className="text-xs font-semibold text-amber-500 uppercase tracking-widest mb-1">04. Audit</div>
              <CardTitle className="text-white text-lg">Auditable Ledger</CardTitle>
              <CardDescription className="text-zinc-400 text-xs">Signed receipts are appended to a tamper-evident chain for a complete audit trail.</CardDescription>
            </CardHeader>
            <CardContent className="pt-0">
              <div className="bg-[#050505] p-3 rounded-lg border border-zinc-900 font-mono text-[11px] text-zinc-500 space-y-1">
                <div><span className="text-amber-400">ledger:</span> append_only.jsonl</div>
                <div><span className="text-amber-400">chaining:</span> Hash linked</div>
                <div className="text-emerald-500 flex items-center gap-1 mt-1 font-semibold">✓ Committed</div>
              </div>
            </CardContent>
          </Card>
        </div>

        {/* Visual Pipeline Showcase */}
        <Card className="mt-16 bg-zinc-950/40 border-zinc-850 overflow-hidden p-0">
          <div className="grid grid-cols-1 md:grid-cols-12 items-center">
            {/* Image Column */}
            <div className="md:col-span-5 relative aspect-square w-full bg-[#050505] border-b md:border-b-0 md:border-r border-zinc-900 p-6">
              <div className="relative w-full h-full">
                <Image 
                  src="/images/zap_architecture.png" 
                  alt="ZAP Zero-Trust Message Dispatch Architecture" 
                  fill
                  style={{ objectFit: 'contain' }}
                  className="opacity-90 hover:opacity-100 transition-opacity duration-500"
                />
              </div>
            </div>
            {/* Content Column */}
            <div className="md:col-span-7 p-8 space-y-3">
              <span className="text-xs font-semibold uppercase tracking-wider text-blue-400 block">Visual Pipeline</span>
              <h3 className="text-2xl font-bold text-white">Cryptographically Attested Execution</h3>
              <p className="text-sm text-zinc-400 leading-relaxed">
                Every frame carries BLAKE3 verification hints enabling early DoS filtering before hitting public key signature math or WASM driver engines.
              </p>
            </div>
          </div>
        </Card>
      </section>

      {/* Why ZAP */}
      <section className="w-full max-w-7xl px-6 pb-24 relative z-10">
        <div className="text-center mb-16">
          <h2 className="text-3xl md:text-5xl font-bold mb-4">Beyond the Limits of Legacy</h2>
          <p className="text-zinc-400 text-lg max-w-2xl mx-auto">
            Traditional architectures like MQTT, gRPC, and Kafka weren&apos;t designed for zero-trust edge networks and autonomous agent coordination.
          </p>
        </div>
        
        <div className="grid grid-cols-1 md:grid-cols-3 gap-8">
          <Card className="bg-zinc-950/40 border-zinc-800">
            <CardHeader>
              <CardTitle className="text-xl font-bold flex items-center gap-2">
                <div className="w-8 h-8 rounded-lg bg-red-500/10 flex items-center justify-center"><span className="text-red-500 font-bold text-sm">M</span></div>
                MQTT & Pub/Sub
              </CardTitle>
            </CardHeader>
            <CardContent className="text-zinc-400 text-sm">
              Brokers become central points of failure. No end-to-end cryptographic identity tracking for individual events.
            </CardContent>
          </Card>

          <Card className="bg-zinc-950/40 border-zinc-800">
            <CardHeader>
              <CardTitle className="text-xl font-bold flex items-center gap-2">
                <div className="w-8 h-8 rounded-lg bg-purple-500/10 flex items-center justify-center"><span className="text-purple-500 font-bold text-sm">g</span></div>
                gRPC / TCP
              </CardTitle>
            </CardHeader>
            <CardContent className="text-zinc-400 text-sm">
              Head-of-line blocking and heavy connection setup overhead. Lacks native multi-node consensus for safety-critical actions.
            </CardContent>
          </Card>

          <Card className="bg-zinc-950/40 border-blue-500/30 shadow-[0_0_30px_rgba(59,130,246,0.15)]">
            <CardHeader>
              <CardTitle className="text-xl font-bold flex items-center gap-2 text-white">
                <Zap className="text-blue-500 w-6 h-6 animate-pulse" />
                ZAP Architecture
              </CardTitle>
            </CardHeader>
            <CardContent className="text-zinc-300 text-sm">
              Decentralized P2P UDP with ChaCha20 encryption. Every 64-byte frame is Ed25519-signed. Native WASM sandboxing and Proof-of-Action.
            </CardContent>
          </Card>
        </div>
      </section>

      {/* Developer Experience with Tabs */}
      <section className="w-full bg-zinc-950/50 border-y border-white/5 py-24 relative z-10">
        <div className="max-w-4xl mx-auto px-6">
          <div className="text-center mb-12">
            <div className="inline-flex items-center gap-2 px-3 py-1 rounded-full bg-purple-500/10 border border-purple-500/20 text-purple-400 text-sm font-medium mb-4">
              Developer Experience
            </div>
            <h2 className="text-3xl md:text-4xl font-bold mb-4">Radically Simple Integration</h2>
            <p className="text-zinc-400 text-base max-w-2xl mx-auto">
              Despite the profound low-level complexity, the ZAP SDK exposes a beautiful, minimalist API interface for WebAssembly drivers and Rust nodes.
            </p>
          </div>

          <Tabs defaultValue="wat" className="w-full">
            <TabsList className="grid w-full grid-cols-2 bg-zinc-900 border border-zinc-850 p-1 rounded-xl mb-6">
              <TabsTrigger value="wat" className="rounded-lg py-2.5 text-sm font-medium text-zinc-400 data-[state=active]:bg-zinc-950 data-[state=active]:text-white data-[state=active]:shadow-sm">
                echo.wat (WASM Driver)
              </TabsTrigger>
              <TabsTrigger value="rust" className="rounded-lg py-2.5 text-sm font-medium text-zinc-400 data-[state=active]:bg-zinc-950 data-[state=active]:text-white data-[state=active]:shadow-sm">
                Rust SDK Integration
              </TabsTrigger>
            </TabsList>
            <TabsContent value="wat">
              <pre className="text-xs md:text-sm text-zinc-300 bg-[#050505] p-5 rounded-xl overflow-x-auto border border-zinc-800 font-mono leading-relaxed">
                <code>{`(module
  (memory (export "memory") 1)
  (func (export "zap_alloc") (param $len i32) (result i32) ...)
  (func (export "zap_dealloc") (param i32 i32))
  
  (func (export "zap_execute")
    (param $action_ptr i32) (param $action_len i32)
    (param $payload_ptr i32) (param $payload_len i32)
    (result i64)
    ;; Pack the resulting pointer and length into an i64
    local.get $payload_ptr
    i64.extend_i32_u
    i64.const 32
    i64.shl
    local.get $payload_len
    i64.extend_i32_u
    i64.or))`}</code>
              </pre>
            </TabsContent>
            <TabsContent value="rust">
              <pre className="text-xs md:text-sm text-zinc-300 bg-[#050505] p-5 rounded-xl overflow-x-auto border border-zinc-800 font-mono leading-relaxed">
                <code>{`use zap_driver_sdk::{
  ZapDriver, DriverInput, DriverError
};

struct EchoDriver;

impl ZapDriver for EchoDriver {
  fn execute(
    &self,
    input: DriverInput<'_>
  ) -> Result<Vec<u8>, DriverError> {
    if input.action != "echo" {
      return Err(
        DriverError::new("unsupported action")
      );
    }
    
    // ZAP handles memory allocation
    // across the sandbox boundary safely
    Ok(input.payload.to_vec())
  }
}`}</code>
              </pre>
            </TabsContent>
          </Tabs>
        </div>
      </section>

      {/* Core Features */}
      <section className="w-full py-24">
        <div className="max-w-7xl mx-auto px-6">
          <div className="text-center mb-16">
            <h2 className="text-3xl md:text-5xl font-bold mb-4">A Complete Protocol Stack</h2>
            <p className="text-zinc-400 text-lg max-w-2xl mx-auto">Everything you need to orchestrate distributed systems safely.</p>
          </div>
          
          <div className="grid grid-cols-1 md:grid-cols-2 xl:grid-cols-3 gap-6">
            {[
              { icon: ShieldCheck, title: "Ed25519 Signatures", desc: "Every frame is cryptographically signed and verified end-to-end. ZAP_SIGN provides a fast 8-byte hint for DoS rejection." },
              { icon: Cpu, title: "WASM Sandboxing", desc: "Driver execution with strict instruction fuel, 16MB memory bounds, and wall-clock time limits. Deny-by-default host ABI." },
              { icon: Activity, title: "Proof-of-Action", desc: "Multi-validator consensus for critical operations. Actions requiring consensus are gated by signed PoA certificates." },
              { icon: Lock, title: "Encrypted UDP", desc: "ChaCha20-Poly1305 authenticated encryption with Noise helpers. 96-bit nonces prevent replay attacks at the network layer." },
              { icon: Network, title: "Capability Routing", desc: "Explicit capability advertisements, queries, and grants. Routes can require peer grants before forwarding." },
              { icon: Database, title: "Auditable Ledgers", desc: "Append-only JSONL memory and signed action receipts. Tamper-evident hash chaining for full operational audit trails." }
            ].map((f, i) => (
              <Card key={i} className="bg-zinc-950/40 border-zinc-800 hover:border-blue-500/30 transition-all duration-300 group">
                <CardHeader>
                  <f.icon className="w-8 h-8 text-blue-500/70 group-hover:text-blue-400 mb-2 transition-colors" />
                  <CardTitle className="text-lg font-bold text-white">{f.title}</CardTitle>
                </CardHeader>
                <CardContent className="text-sm text-zinc-400">
                  {f.desc}
                </CardContent>
              </Card>
            ))}
          </div>
        </div>
      </section>

      {/* Benchmarks */}
      <section className="w-full max-w-7xl mx-auto px-6 py-24 relative z-10">
        <div className="flex flex-col md:flex-row items-center gap-16">
          <div className="flex-1">
            <div className="inline-flex items-center gap-2 px-3 py-1 rounded-full bg-blue-500/10 border border-blue-500/20 text-blue-400 text-sm font-medium mb-4">
              Performance
            </div>
            <h2 className="text-3xl md:text-5xl font-bold mb-6">Built for Nanosecond Latency</h2>
            <p className="text-zinc-400 text-lg mb-6">
              ZAP&apos;s hot-path packet processing is rigorously benchmarked. Regression thresholds strictly enforce a 7% max regression on the 64-byte header parser and frame encoding.
            </p>
            <ul className="space-y-4">
              <li className="flex items-center gap-3"><div className="w-2 h-2 rounded-full bg-blue-500"></div> <span className="text-zinc-300">Zero-copy wire frame parsing</span></li>
              <li className="flex items-center gap-3"><div className="w-2 h-2 rounded-full bg-blue-500"></div> <span className="text-zinc-300">36+ automated Criterion benchmarks</span></li>
              <li className="flex items-center gap-3"><div className="w-2 h-2 rounded-full bg-blue-500"></div> <span className="text-zinc-300">Fast path DoS rejection via signature hints</span></li>
            </ul>
          </div>
          <Card className="flex-1 w-full bg-zinc-950/40 border-zinc-800 p-8 relative overflow-hidden">
            <div className="absolute top-0 right-0 w-48 h-48 bg-blue-500/10 blur-3xl rounded-full"></div>
            <h3 className="text-lg font-mono text-zinc-350 mb-6 relative z-10">Benchmark Targets</h3>
            <div className="space-y-6 relative z-10">
              <div>
                <div className="flex justify-between text-sm mb-2"><span className="text-zinc-400">Header Parse</span><span className="text-blue-400 font-mono">1 ns threshold</span></div>
                <div className="h-2 w-full bg-zinc-800 rounded-full overflow-hidden"><div className="h-full bg-blue-500 w-[5%]"></div></div>
              </div>
              <div>
                <div className="flex justify-between text-sm mb-2"><span className="text-zinc-400">Ed25519 Sign</span><span className="text-blue-400 font-mono">~µs scale</span></div>
                <div className="h-2 w-full bg-zinc-800 rounded-full overflow-hidden"><div className="h-full bg-blue-500 w-[40%]"></div></div>
              </div>
              <div>
                <div className="flex justify-between text-sm mb-2"><span className="text-zinc-400">WASM Execute</span><span className="text-blue-400 font-mono">~ms scale</span></div>
                <div className="h-2 w-full bg-zinc-800 rounded-full overflow-hidden"><div className="h-full bg-blue-500 w-[80%]"></div></div>
              </div>
            </div>
          </Card>
        </div>
      </section>

      {/* CTA */}
      <section className="w-full bg-gradient-to-t from-blue-900/10 to-transparent py-24 text-center border-t border-white/5">
        <h2 className="text-3xl md:text-5xl font-bold mb-6">Ready to shape the future?</h2>
        <p className="text-zinc-400 max-w-2xl mx-auto mb-10 text-lg">Integrate ZAP in your next AI agent swarm, robotics project, or distributed application.</p>
        <Button render={<Link href="/docs/getting-started" />} size="lg" className="rounded-full bg-white text-black hover:bg-zinc-200 px-8 py-6 font-semibold">
          Start Building with ZAP
        </Button>
      </section>
      </div>
      <Footer />
    </div>
  );
}
