import Image from 'next/image';
import { Network, Key, Cpu, FileCheck, Play, ShieldAlert } from 'lucide-react';
import { Card, CardHeader, CardTitle, CardContent, CardDescription } from "@/components/ui/card";
import { Alert, AlertTitle, AlertDescription } from "@/components/ui/alert";

export default function TutorialPage() {
  return (
    <div className="space-y-8 font-sans">
      <div>
        <h1 className="text-3xl md:text-4xl font-extrabold tracking-tight mb-2 text-white">End-to-End Tutorial</h1>
        <p className="text-zinc-400 text-lg">Configure and deploy a multi-node secure telemetry pipeline with WASM sandboxing.</p>
      </div>

      {/* Hero Visual Card */}
      <Card className="group bg-zinc-950/40 hover:bg-zinc-950/60 border border-zinc-850 hover:border-purple-500/20 transition-all duration-500 overflow-hidden rounded-2xl shadow-xl hover:shadow-2xl hover:shadow-purple-500/5 not-prose p-0">
        <div className="grid grid-cols-1 md:grid-cols-12 items-stretch">
          {/* Image Column */}
          <div className="md:col-span-5 relative bg-gradient-to-br from-zinc-950 to-black/80 flex items-center justify-center p-6 border-b md:border-b-0 md:border-r border-zinc-900 overflow-hidden min-h-[260px] md:min-h-0">
            {/* Ambient Glow Effects */}
            <div className="absolute inset-0 bg-[radial-gradient(circle_at_center,rgba(168,85,247,0.08)_0%,transparent_70%)] pointer-events-none" />
            <div className="absolute -inset-10 bg-[radial-gradient(circle_at_bottom_left,rgba(59,130,246,0.04)_0%,transparent_50%)] pointer-events-none" />
            
            {/* Image Wrapper */}
            <div className="relative w-full aspect-square max-w-[220px] md:max-w-[240px] transform group-hover:scale-[1.03] transition-transform duration-500 ease-out">
              <Image 
                src="/images/zap_tutorial_flow.png" 
                alt="ZAP Developer compilation and signing pipeline" 
                fill
                style={{ objectFit: 'contain' }}
                className="opacity-90 group-hover:opacity-100 transition-opacity duration-500"
                priority
              />
            </div>
          </div>
          {/* Content Column */}
          <div className="md:col-span-7 p-6 md:p-8 flex flex-col justify-center space-y-3 bg-zinc-950/20">
            <span className="text-xs font-semibold uppercase tracking-wider text-purple-400 block">Developer Tutorial</span>
            <h3 className="text-xl font-bold text-white tracking-tight">Driver Lifecycle: Build, Sign, and Run</h3>
            <p className="text-sm text-zinc-400 leading-relaxed">
              Follow the complete developer pipeline. Write raw WebAssembly Text logic, compile it to binary bytecode, bind authorization parameters to a signed manifest file, and load the driver package into a running ZAP daemon instance.
            </p>
          </div>
        </div>
      </Card>

      <Alert className="border-blue-500/20 bg-blue-500/5 text-blue-300">
        <ShieldAlert className="w-4 h-4 text-blue-400" />
        <AlertTitle className="font-semibold text-blue-400">Setup Goals</AlertTitle>
        <AlertDescription className="text-xs">
          This tutorial walks you through launching two nodes, generating cryptographic keys, compiling a WASM thermostat driver, signing execution manifests, and dispatching a payload securely over UDP.
        </AlertDescription>
      </Alert>

      {/* Step 1 */}
      <Card className="bg-zinc-950/40 border-zinc-850">
        <CardHeader>
          <div className="flex items-center gap-3">
            <div className="w-8 h-8 rounded-lg bg-zinc-900 border border-zinc-800 flex items-center justify-center">
              <Network className="w-4 h-4 text-blue-400" />
            </div>
            <div>
              <CardTitle className="text-white text-base">1. Architecture Overview</CardTitle>
              <CardDescription className="text-xs">Establish the network layout</CardDescription>
            </div>
          </div>
        </CardHeader>
        <CardContent className="space-y-4">
          <p className="text-sm text-zinc-400">
            We will construct a smart factory control system with two distinct nodes communicating in a zero-trust mesh layout:
          </p>
          <div className="grid grid-cols-1 sm:grid-cols-2 gap-4">
            <div className="p-4 rounded-lg bg-[#050505] border border-zinc-900">
              <span className="text-xs font-semibold text-blue-400 uppercase tracking-widest block mb-1">Node A (Gateway)</span>
              <span className="text-xs text-zinc-400">Runs the sandboxed WASM driver to process actions locally.</span>
            </div>
            <div className="p-4 rounded-lg bg-[#050505] border border-zinc-900">
              <span className="text-xs font-semibold text-purple-400 uppercase tracking-widest block mb-1">Node B (Terminal)</span>
              <span className="text-xs text-zinc-400">Submits action triggers via secure UDP requests to Node A.</span>
            </div>
          </div>
        </CardContent>
      </Card>

      {/* Step 2 */}
      <Card className="bg-zinc-950/40 border-zinc-850">
        <CardHeader>
          <div className="flex items-center gap-3">
            <div className="w-8 h-8 rounded-lg bg-zinc-900 border border-zinc-800 flex items-center justify-center">
              <Key className="w-4 h-4 text-purple-400" />
            </div>
            <div>
              <CardTitle className="text-white text-base">2. Generate Key Identities</CardTitle>
              <CardDescription className="text-xs">Create cryptographic private credentials</CardDescription>
            </div>
          </div>
        </CardHeader>
        <CardContent className="space-y-3">
          <p className="text-sm text-zinc-400">
            Generate separate identities for each participant. These private keys will be used to sign and verify wire packets:
          </p>
          <pre className="text-xs bg-[#050505] p-4 rounded-lg border border-zinc-900 font-mono text-zinc-300">
            <code>{`# Generate Node A identity
zap keygen --out .zap/node-a.key

# Generate Node B identity
zap keygen --out .zap/node-b.key`}</code>
          </pre>
        </CardContent>
      </Card>

      {/* Step 3 */}
      <Card className="bg-zinc-950/40 border-zinc-850">
        <CardHeader>
          <div className="flex items-center gap-3">
            <div className="w-8 h-8 rounded-lg bg-zinc-900 border border-zinc-800 flex items-center justify-center">
              <Cpu className="w-4 h-4 text-emerald-400" />
            </div>
            <div>
              <CardTitle className="text-white text-base">3. Create the Sandboxed WASM Driver</CardTitle>
              <CardDescription className="text-xs">Author low-level execution logic</CardDescription>
            </div>
          </div>
        </CardHeader>
        <CardContent className="space-y-3">
          <p className="text-sm text-zinc-400">
            Write a WebAssembly Text (WAT) driver representing the device control loop. Save this code as <code>drivers/thermostat.wat</code>:
          </p>
          <pre className="text-xs bg-[#050505] p-4 rounded-lg border border-zinc-900 font-mono text-zinc-300 overflow-x-auto">
            <code>{`(module
  (memory (export "memory") 1)
  (global $heap (mut i32) (i32.const 1024))
  
  (func (export "zap_alloc") (param $len i32) (result i32)
    global.get $heap
{{ ... }}
    i64.extend_i32_u
    i64.const 32
    i64.shl
    local.get $payload_len
    i64.extend_i32_u
    i64.or))`}</code>
          </pre>
        </CardContent>
      </Card>

      {/* Step 4 */}
      <Card className="bg-zinc-950/40 border-zinc-850">
        <CardHeader>
          <div className="flex items-center gap-3">
            <div className="w-8 h-8 rounded-lg bg-zinc-900 border border-zinc-800 flex items-center justify-center">
              <FileCheck className="w-4 h-4 text-amber-400" />
            </div>
            <div>
              <CardTitle className="text-white text-base">4. Create and Sign the Driver Manifest</CardTitle>
              <CardDescription className="text-xs">Publish execution metadata with signatures</CardDescription>
            </div>
          </div>
        </CardHeader>
        <CardContent className="space-y-3">
          <p className="text-sm text-zinc-400">
            Sign the driver manifest to authorize the execution of specific actions (e.g. <code>thermostat.set</code>) using Node A&apos;s private identity key:
          </p>
          <pre className="text-xs bg-[#050505] p-4 rounded-lg border border-zinc-900 font-mono text-zinc-300">
            <code>{`zap driver-manifest create \\
  --driver drivers/thermostat.wat \\
  --action thermostat.set \\
  --author-key .zap/node-a.key \\
  --out drivers/thermostat.manifest.toml`}</code>
          </pre>
        </CardContent>
      </Card>

      {/* Step 5 */}
      <Card className="bg-zinc-950/40 border-zinc-850">
        <CardHeader>
          <div className="flex items-center gap-3">
            <div className="w-8 h-8 rounded-lg bg-zinc-900 border border-zinc-800 flex items-center justify-center">
              <Play className="w-4 h-4 text-emerald-400" />
            </div>
            <div>
              <CardTitle className="text-white text-base">5. Launch and Test Node Tunnels</CardTitle>
              <CardDescription className="text-xs">Establish the tunnels and dispatch payloads</CardDescription>
            </div>
          </div>
        </CardHeader>
        <CardContent className="space-y-3">
          <p className="text-sm text-zinc-400">
            Start Node A and Node B using their config files. From Node B, dispatch action payloads over the encrypted UDP mesh network:
          </p>
          <pre className="text-xs bg-[#050505] p-4 rounded-lg border border-zinc-900 font-mono text-zinc-300">
            <code>{`# Run daemon node
zap daemon --config .zap/node-a.toml`}</code>
          </pre>
        </CardContent>
      </Card>
    </div>
  );
}
