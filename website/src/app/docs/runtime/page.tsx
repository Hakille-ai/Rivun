import Image from 'next/image';
import { Database, PlayCircle, Lock } from 'lucide-react';
import { Card, CardHeader, CardTitle, CardContent, CardDescription } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";

export default function RuntimePage() {
  return (
    <div className="space-y-8 font-sans">
      <div>
        <h1 className="text-3xl md:text-4xl font-extrabold tracking-tight mb-2 text-white">Runtime & WASM Isolation</h1>
        <p className="text-zinc-400 text-lg">Understand the secure Wasmtime sandbox used by ZAP to execute message drivers.</p>
      </div>

      {/* Hero Visual Card */}
      <Card className="group bg-zinc-950/40 hover:bg-zinc-950/60 border border-zinc-850 hover:border-emerald-500/20 transition-all duration-500 overflow-hidden rounded-2xl shadow-xl hover:shadow-2xl hover:shadow-emerald-500/5 not-prose p-0">
        <div className="grid grid-cols-1 md:grid-cols-12 items-stretch">
          {/* Image Column */}
          <div className="md:col-span-5 relative bg-gradient-to-br from-zinc-950 to-black/80 flex items-center justify-center p-6 border-b md:border-b-0 md:border-r border-zinc-900 overflow-hidden min-h-[260px] md:min-h-0">
            {/* Ambient Glow Effects */}
            <div className="absolute inset-0 bg-[radial-gradient(circle_at_center,rgba(20,184,166,0.08)_0%,transparent_70%)] pointer-events-none" />
            <div className="absolute -inset-10 bg-[radial-gradient(circle_at_bottom_left,rgba(59,130,246,0.04)_0%,transparent_50%)] pointer-events-none" />
            
            {/* Image Wrapper */}
            <div className="relative w-full aspect-square max-w-[220px] md:max-w-[240px] transform group-hover:scale-[1.03] transition-transform duration-500 ease-out">
              <Image 
                src="/images/zap_wasm_sandbox.png" 
                alt="WASM Sandbox Isolation Visual" 
                fill
                style={{ objectFit: 'contain' }}
                className="opacity-90 group-hover:opacity-100 transition-opacity duration-500"
                priority
              />
            </div>
          </div>
          {/* Content Column */}
          <div className="md:col-span-7 p-6 md:p-8 flex flex-col justify-center space-y-3 bg-zinc-950/20">
            <span className="text-xs font-semibold uppercase tracking-wider text-emerald-400 block">Execution Safety</span>
            <h3 className="text-xl font-bold text-white tracking-tight">Zero-Trust Driver Sandboxing</h3>
            <p className="text-sm text-zinc-400 leading-relaxed">
              When ZAP routes an action payload to a local driver, it compiles the logic to a WebAssembly Text or Binary module and spins up an isolated Wasmtime sandbox. This protects host filesystems, networks, and system processes from malicious or errant code execution.
            </p>
          </div>
        </div>
      </Card>

      <div className="grid grid-cols-1 md:grid-cols-2 gap-6">
        {/* Resource Limits Card */}
        <Card className="bg-zinc-950/40 border-zinc-850">
          <CardHeader>
            <div className="flex items-center gap-3">
              <div className="w-8 h-8 rounded-lg bg-zinc-900 border border-zinc-800 flex items-center justify-center">
                <Database className="w-4 h-4 text-emerald-400" />
              </div>
              <div>
                <CardTitle className="text-white text-base">Resource Bounds</CardTitle>
                <CardDescription className="text-xs">Deterministic limits enforced on each trigger</CardDescription>
              </div>
            </div>
          </CardHeader>
          <CardContent className="space-y-3">
            <div className="flex justify-between items-center p-3 rounded-lg bg-[#050505] border border-zinc-900">
              <span className="text-xs text-zinc-300">Instruction Budget</span>
              <Badge className="bg-zinc-900 border-zinc-800 text-emerald-400 text-[10px]">WASM Fuel Gated</Badge>
            </div>
            <div className="flex justify-between items-center p-3 rounded-lg bg-[#050505] border border-zinc-900">
              <span className="text-xs text-zinc-300">Linear Memory Cap</span>
              <Badge className="bg-zinc-900 border-zinc-800 text-blue-400 text-[10px]">16 MB Max Limit</Badge>
            </div>
            <div className="flex justify-between items-center p-3 rounded-lg bg-[#050505] border border-zinc-900">
              <span className="text-xs text-zinc-300">Real-Time Deadline</span>
              <Badge className="bg-zinc-900 border-zinc-800 text-purple-400 text-[10px]">Epoch Interruption</Badge>
            </div>
          </CardContent>
        </Card>

        {/* Host Imports Card */}
        <Card className="bg-zinc-950/40 border-zinc-850">
          <CardHeader>
            <div className="flex items-center gap-3">
              <div className="w-8 h-8 rounded-lg bg-zinc-900 border border-zinc-800 flex items-center justify-center">
                <Lock className="w-4 h-4 text-purple-400" />
              </div>
              <div>
                <CardTitle className="text-white text-base">Host Imports</CardTitle>
                <CardDescription className="text-xs">Deny-by-default syscall integration</CardDescription>
              </div>
            </div>
          </CardHeader>
          <CardContent className="space-y-3 text-sm text-zinc-400">
            <p>
              By default, compiled modules are barred from importing external functions. In ABI v2, explicit helper APIs exist:
            </p>
            <div className="flex flex-wrap gap-2 pt-2">
              <Badge variant="outline" className="font-mono text-[9px] bg-zinc-900 text-zinc-400 border-zinc-850">zap.emit_event</Badge>
              <Badge variant="outline" className="font-mono text-[9px] bg-zinc-900 text-zinc-400 border-zinc-850">zap.memory_write</Badge>
            </div>
            <p className="text-xs text-zinc-500 pt-2 border-t border-zinc-900/60">
              Access permissions must be declared and signed via security manifests in the registry.
            </p>
          </CardContent>
        </Card>
      </div>

      {/* ABI Exports Section */}
      <Card className="bg-zinc-950/40 border-zinc-850">
        <CardHeader>
          <div className="flex items-center gap-3">
            <div className="w-8 h-8 rounded-lg bg-zinc-900 border border-zinc-800 flex items-center justify-center">
              <PlayCircle className="w-4 h-4 text-blue-400" />
            </div>
            <div>
              <CardTitle className="text-white text-base">ABI v1 Required Exports</CardTitle>
              <CardDescription className="text-xs">Required functions a valid driver must expose</CardDescription>
            </div>
          </div>
        </CardHeader>
        <CardContent className="space-y-4">
          <p className="text-xs text-zinc-400">
            All ZAP modules must export the following function footprints to coordinate allocation and execution with the host engine:
          </p>
          <pre className="text-xs bg-[#050505] p-4 rounded-lg border border-zinc-900 font-mono text-zinc-300 overflow-x-auto">
            <code>{`(memory (export "memory") 1)
(func (export "zap_alloc") (param i32) (result i32))
(func (export "zap_dealloc") (param i32 i32))
(func (export "zap_execute") 
  (param $action_ptr i32) (param $action_len i32) 
  (param $payload_ptr i32) (param $payload_len i32) 
  (result i64))`}</code>
          </pre>
        </CardContent>
      </Card>
    </div>
  );
}
