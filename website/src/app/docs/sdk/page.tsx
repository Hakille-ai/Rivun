import Image from 'next/image';
import { Database, Layers, FileCode } from 'lucide-react';
import { Card, CardHeader, CardTitle, CardContent, CardDescription } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";

export default function SDKPage() {
  return (
    <div className="space-y-8 font-sans">
      <div>
        <h1 className="text-3xl md:text-4xl font-extrabold tracking-tight mb-2 text-white">ZAP Driver SDK</h1>
        <p className="text-zinc-400 text-lg">The Rust SDK provides minimal abstractions to build fast, safe WASM drivers.</p>
      </div>

      {/* Hero Visual Card */}
      <Card className="group bg-zinc-950/40 hover:bg-zinc-950/60 border border-zinc-850 hover:border-blue-500/20 transition-all duration-500 overflow-hidden rounded-2xl shadow-xl hover:shadow-2xl hover:shadow-blue-500/5 not-prose p-0">
        <div className="grid grid-cols-1 md:grid-cols-12 items-stretch">
          {/* Image Column */}
          <div className="md:col-span-5 relative bg-gradient-to-br from-zinc-950 to-black/80 flex items-center justify-center p-6 border-b md:border-b-0 md:border-r border-zinc-900 overflow-hidden min-h-[260px] md:min-h-0">
            {/* Ambient Glow Effects */}
            <div className="absolute inset-0 bg-[radial-gradient(circle_at_center,rgba(59,130,246,0.08)_0%,transparent_70%)] pointer-events-none" />
            <div className="absolute -inset-10 bg-[radial-gradient(circle_at_bottom_left,rgba(99,102,241,0.04)_0%,transparent_50%)] pointer-events-none" />
            
            {/* Image Wrapper */}
            <div className="relative w-full aspect-square max-w-[220px] md:max-w-[240px] transform group-hover:scale-[1.03] transition-transform duration-500 ease-out">
              <Image 
                src="/images/zap_driver_sdk.png" 
                alt="ZAP Driver SDK Memory Boundary Visual" 
                fill
                style={{ objectFit: 'contain' }}
                className="opacity-90 group-hover:opacity-100 transition-opacity duration-500"
                priority
              />
            </div>
          </div>
          {/* Content Column */}
          <div className="md:col-span-7 p-6 md:p-8 flex flex-col justify-center space-y-3 bg-zinc-950/20">
            <span className="text-xs font-semibold uppercase tracking-wider text-blue-400 block">WASM Boundary</span>
            <h3 className="text-xl font-bold text-white tracking-tight">Zero-Copy Memory Exchange</h3>
            <p className="text-sm text-zinc-400 leading-relaxed">
              The ZAP Driver SDK manages data serialization and linear memory offsets across the guest WebAssembly boundary. Input structs are written directly to guest memory, and execution results are returned as compact 64-bit packed pointer offsets.
            </p>
          </div>
        </div>
      </Card>

      <div className="grid grid-cols-1 md:grid-cols-2 gap-6">
        {/* The ZapDriver Trait Card */}
        <Card className="bg-zinc-950/40 border-zinc-850">
          <CardHeader>
            <div className="flex items-center gap-3">
              <div className="w-8 h-8 rounded-lg bg-zinc-900 border border-zinc-800 flex items-center justify-center">
                <FileCode className="w-4 h-4 text-blue-400" />
              </div>
              <div>
                <CardTitle className="text-white text-base">The ZapDriver Trait</CardTitle>
                <CardDescription className="text-xs">Expose host execution entrypoint</CardDescription>
              </div>
            </div>
          </CardHeader>
          <CardContent className="space-y-3">
            <p className="text-xs text-zinc-400">
              The core of the SDK is the <code>ZapDriver</code> trait. Implementing this trait automatically sets up the required WASM ABI exports:
            </p>
            <pre className="text-xs bg-[#050505] p-3.5 rounded-lg border border-zinc-900 font-mono text-zinc-300 overflow-x-auto">
              <code>{`pub trait ZapDriver {
    fn execute(
        &self, 
        input: DriverInput<'_>
    ) -> Result<Vec<u8>, DriverError>;
}`}</code>
            </pre>
          </CardContent>
        </Card>

        {/* Memory Management Card */}
        <Card className="bg-zinc-950/40 border-zinc-850">
          <CardHeader>
            <div className="flex items-center gap-3">
              <div className="w-8 h-8 rounded-lg bg-zinc-900 border border-zinc-800 flex items-center justify-center">
                <Database className="w-4 h-4 text-purple-400" />
              </div>
              <div>
                <CardTitle className="text-white text-base">Memory packing</CardTitle>
                <CardDescription className="text-xs">Zero-copy boundary exchange</CardDescription>
              </div>
            </div>
          </CardHeader>
          <CardContent className="space-y-3">
            <p className="text-xs text-zinc-400">
              Output buffers are allocated in guest linear memory and returned as a packed 64-bit integer containing both the pointer and the length:
            </p>
            <pre className="text-xs bg-[#050505] p-3.5 rounded-lg border border-zinc-900 font-mono text-zinc-350 overflow-x-auto">
              <code>{`pub const fn pack_result(ptr: u32, len: u32) -> i64 {
    ((ptr as u64) << 32 | len as u64) as i64
}`}</code>
            </pre>
            <p className="text-xs text-zinc-500 pt-1">
              The host ZAP node unpacks the pointer, reads memory directly from the sandbox, and calls `zap_dealloc` to clear resources.
            </p>
          </CardContent>
        </Card>
      </div>

      {/* Error Handling Card */}
      <Card className="bg-zinc-950/40 border-zinc-850">
        <CardHeader>
          <div className="flex items-center gap-3">
            <div className="w-8 h-8 rounded-lg bg-zinc-900 border border-zinc-800 flex items-center justify-center">
              <Layers className="w-4 h-4 text-emerald-400" />
            </div>
            <div>
              <CardTitle className="text-white text-base">Error Boundary & High Availability</CardTitle>
              <CardDescription className="text-xs">Safe failure captures inside the sandbox</CardDescription>
            </div>
          </div>
        </CardHeader>
        <CardContent className="text-sm text-zinc-400 space-y-2">
          <p className="text-xs">
            The SDK abstracts away error boundaries. If a <code>DriverError</code> is returned, the host catches it gracefully without crashing the ZAP node, ensuring high availability of the underlying protocol engine.
          </p>
          <div className="flex justify-between items-center p-3 rounded-lg bg-[#050505] border border-zinc-900 text-xs">
            <span className="text-zinc-300 font-semibold">Error Recovery Isolation</span>
            <Badge className="bg-zinc-900 border-zinc-800 text-emerald-400 text-[10px]">Zero-Crash Host Guarantee</Badge>
          </div>
        </CardContent>
      </Card>
    </div>
  );
}
