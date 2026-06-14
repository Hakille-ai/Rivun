import Image from 'next/image';
import { Key, Terminal, Archive, ShieldCheck, Cpu } from 'lucide-react';
import { Card, CardHeader, CardTitle, CardContent, CardDescription } from "@/components/ui/card";
import { Alert, AlertTitle, AlertDescription } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";

export default function ZapStorePage() {
  return (
    <div className="space-y-8 font-sans">
      <div>
        <h1 className="text-3xl md:text-4xl font-extrabold tracking-tight mb-2 text-white">ZapStore & Registries</h1>
        <p className="text-zinc-400 text-lg">Manage driver provenance, validation registries, mirror syncs, and artifact bundles in ZAP.</p>
      </div>

      {/* Hero Visual Card */}
      <Card className="group bg-zinc-950/40 hover:bg-zinc-950/60 border border-zinc-850 hover:border-blue-500/20 transition-all duration-500 overflow-hidden rounded-2xl shadow-xl hover:shadow-2xl hover:shadow-blue-500/5 not-prose p-0">
        <div className="grid grid-cols-1 md:grid-cols-12 items-stretch">
          {/* Image Column */}
          <div className="md:col-span-5 relative bg-gradient-to-br from-zinc-950 to-black/80 flex items-center justify-center p-6 border-b md:border-b-0 md:border-r border-zinc-900 overflow-hidden min-h-[260px] md:min-h-0">
            {/* Ambient Glow Effects */}
            <div className="absolute inset-0 bg-[radial-gradient(circle_at_center,rgba(79,70,229,0.08)_0%,transparent_70%)] pointer-events-none" />
            <div className="absolute -inset-10 bg-[radial-gradient(circle_at_bottom_left,rgba(6,182,212,0.04)_0%,transparent_50%)] pointer-events-none" />
            
            {/* Image Wrapper */}
            <div className="relative w-full aspect-square max-w-[220px] md:max-w-[240px] transform group-hover:scale-[1.03] transition-transform duration-500 ease-out">
              <Image 
                src="/images/zap_registry_manifest.png" 
                alt="ZapStore Registry Manifest Visual" 
                fill
                style={{ objectFit: 'contain' }}
                className="opacity-90 group-hover:opacity-100 transition-opacity duration-500"
                priority
              />
            </div>
          </div>
          {/* Content Column */}
          <div className="md:col-span-7 p-6 md:p-8 flex flex-col justify-center space-y-3 bg-zinc-950/20">
            <span className="text-xs font-semibold uppercase tracking-wider text-blue-400 block">Provenance Management</span>
            <h3 className="text-xl font-bold text-white tracking-tight">Attested Driver Indexes</h3>
            <p className="text-sm text-zinc-400 leading-relaxed">
              ZapStore registers and distributes signed driver manifests. A manifest binds target action metadata to a BLAKE3 cryptographic hash of compiled WASM bytecode, guaranteeing that drivers cannot be altered during transport.
            </p>
          </div>
        </div>
      </Card>

      <div className="grid grid-cols-1 md:grid-cols-2 gap-6">
        {/* Driver Manifests Card */}
        <Card className="bg-zinc-950/40 border-zinc-850">
          <CardHeader>
            <div className="flex items-center gap-3">
              <div className="w-8 h-8 rounded-lg bg-zinc-900 border border-zinc-800 flex items-center justify-center">
                <Archive className="w-4 h-4 text-blue-400" />
              </div>
              <div>
                <CardTitle className="text-white text-base">Driver Manifests</CardTitle>
                <CardDescription className="text-xs">Immutable cryptographic driver descriptors</CardDescription>
              </div>
            </div>
          </CardHeader>
          <CardContent className="space-y-3 text-sm text-zinc-400">
            <p>A signed driver manifest securely binds the following values:</p>
            <ul className="space-y-1 text-xs text-zinc-300 list-disc pl-5">
              <li><strong>Action Identifier:</strong> Action binding name (e.g. <code>"echo"</code>).</li>
              <li><strong>Artifact Hash:</strong> BLAKE3 binary fingerprint of compiled WASM bytecode.</li>
              <li><strong>Permissions:</strong> Declarations of sandbox syscall exceptions.</li>
              <li><strong>Signature:</strong> Ed25519 payload attestation.</li>
            </ul>
          </CardContent>
        </Card>

        {/* Local Registries Card */}
        <Card className="bg-zinc-950/40 border-zinc-850">
          <CardHeader>
            <div className="flex items-center gap-3">
              <div className="w-8 h-8 rounded-lg bg-zinc-900 border border-zinc-800 flex items-center justify-center">
                <ShieldCheck className="w-4 h-4 text-emerald-400" />
              </div>
              <div>
                <CardTitle className="text-white text-base">Local Registries</CardTitle>
                <CardDescription className="text-xs">Operator-signed catalog validation</CardDescription>
              </div>
            </div>
          </CardHeader>
          <CardContent className="space-y-3 text-sm text-zinc-400">
            <p>
              The local <code>registry.index.toml</code> catalogs all approved manifests, mirror locations, and driver revocations. The entire registry index can be cryptographically signed by an operator key:
            </p>
            <pre className="text-[10px] bg-[#050505] p-2.5 rounded-lg border border-zinc-900 font-mono text-zinc-350">
              <code>zap registry sign --registry index.toml --operator-key .zap/node.key</code>
            </pre>
          </CardContent>
        </Card>
      </div>

      {/* Manifest Schema Example */}
      <Card className="bg-zinc-950/40 border-zinc-850">
        <CardHeader>
          <div className="flex items-center gap-3">
            <div className="w-8 h-8 rounded-lg bg-zinc-900 border border-zinc-800 flex items-center justify-center">
              <Cpu className="w-4 h-4 text-purple-400" />
            </div>
            <div>
              <CardTitle className="text-white text-base">Manifest Schema Example</CardTitle>
              <CardDescription className="text-xs">Declared permissions and metadata bindings</CardDescription>
            </div>
          </div>
        </CardHeader>
        <CardContent className="space-y-4">
          <pre className="text-xs bg-[#050505] p-4 rounded-lg border border-zinc-900 font-mono text-zinc-300 overflow-x-auto">
            <code>{`schema_version = 1
name = "echo-driver"
version = "0.1.0"
action = "echo"
abi_version = 1
wasm_hash = "blake3:f68a73..."
author_node_id = "00000000-0000-4000-8000-000000000000"
author_public_key = "base64-pubkey..."
signature = "signature-bytes..."

[permissions]
network = false
filesystem = false
clock = false
environment = false`}</code>
          </pre>
        </CardContent>
      </Card>

      {/* CLI Utilities */}
      <Card className="bg-zinc-950/40 border-zinc-850">
        <CardHeader>
          <div className="flex items-center gap-3">
            <div className="w-8 h-8 rounded-lg bg-zinc-900 border border-zinc-800 flex items-center justify-center">
              <Terminal className="w-4 h-4 text-blue-400" />
            </div>
            <div>
              <CardTitle className="text-white text-base">CLI Manifest Operations</CardTitle>
              <CardDescription className="text-xs">Create and verify manifests from the terminal</CardDescription>
            </div>
          </div>
        </CardHeader>
        <CardContent className="space-y-6">
          <div className="space-y-2">
            <div className="flex justify-between items-center">
              <span className="text-xs font-semibold text-zinc-300">Create Driver Manifest</span>
              <Badge variant="outline" className="text-[9px] bg-zinc-900 text-zinc-400 border-zinc-850">driver-manifest create</Badge>
            </div>
            <pre className="text-xs bg-[#050505] p-3 rounded-lg border border-zinc-900 font-mono text-zinc-350 overflow-x-auto">
              <code>{`zap driver-manifest create \\
  --driver drivers/echo.wat \\
  --action echo \\
  --author-key .zap/node.key \\
  --out drivers/echo.manifest.toml`}</code>
            </pre>
          </div>

          <div className="space-y-2 pt-2 border-t border-zinc-900">
            <div className="flex justify-between items-center">
              <span className="text-xs font-semibold text-zinc-300">Verify Manifest Integrity</span>
              <Badge variant="outline" className="text-[9px] bg-zinc-900 text-zinc-400 border-zinc-850">driver-manifest verify</Badge>
            </div>
            <pre className="text-xs bg-[#050505] p-3 rounded-lg border border-zinc-900 font-mono text-zinc-350 overflow-x-auto">
              <code>{`zap driver-manifest verify \\
  --driver drivers/echo.wat \\
  --manifest drivers/echo.manifest.toml`}</code>
            </pre>
          </div>
        </CardContent>
      </Card>
    </div>
  );
}
