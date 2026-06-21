import Image from 'next/image';
import { ShieldCheck, Server, Cpu, EyeOff } from 'lucide-react';
import { Card, CardHeader, CardTitle, CardContent, CardDescription } from "@/components/ui/card";
import { Alert, AlertTitle, AlertDescription } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";

export default function DeploymentPage() {
  return (
    <div className="space-y-8 font-sans">
      <div>
        <h1 className="text-3xl md:text-4xl font-extrabold tracking-tight mb-2 text-white">Deployment & Hardening</h1>
        <p className="text-zinc-400 text-lg">Deploy secure ZAP nodes using containers and follow production hardening blueprints.</p>
      </div>

      {/* Hero Visual Card */}
      <Card className="group bg-zinc-950/40 hover:bg-zinc-950/60 border border-zinc-850 hover:border-blue-500/20 transition-all duration-500 overflow-hidden rounded-2xl shadow-xl hover:shadow-2xl hover:shadow-blue-500/5 not-prose p-0">
        <div className="grid grid-cols-1 md:grid-cols-12 items-stretch">
          {/* Image Column */}
          <div className="md:col-span-5 relative bg-gradient-to-br from-zinc-950 to-black/80 flex items-center justify-center p-6 border-b md:border-b-0 md:border-r border-zinc-900 overflow-hidden min-h-[260px] md:min-h-0">
            {/* Ambient Glow Effects */}
            <div className="absolute inset-0 bg-[radial-gradient(circle_at_center,rgba(6,182,212,0.08)_0%,transparent_70%)] pointer-events-none" />
            <div className="absolute -inset-10 bg-[radial-gradient(circle_at_bottom_left,rgba(168,85,247,0.04)_0%,transparent_50%)] pointer-events-none" />
            
            {/* Image Wrapper */}
            <div className="relative w-full aspect-square max-w-[220px] md:max-w-[240px] transform group-hover:scale-[1.03] transition-transform duration-500 ease-out">
              <Image 
                src="/images/zap_deployment_topology.png" 
                alt="ZAP Node Peer Deployment Topology" 
                fill
                style={{ objectFit: 'contain' }}
                className="opacity-90 group-hover:opacity-100 transition-opacity duration-500"
                priority
              />
            </div>
          </div>
          {/* Content Column */}
          <div className="md:col-span-7 p-6 md:p-8 flex flex-col justify-center space-y-3 bg-zinc-950/20">
            <span className="text-xs font-semibold uppercase tracking-wider text-purple-400 block">Deployment Model</span>
            <h3 className="text-xl font-bold text-white tracking-tight">Containerized Peer Architecture</h3>
            <p className="text-sm text-zinc-400 leading-relaxed">
              Deploy ZAP nodes inside Docker containers connected via secure overlay VPN networks. Volume mounts secure persistent state ledgers, while private keys are loaded dynamically to sign transactions and verify peer-to-peer data integrity.
            </p>
          </div>
        </div>
      </Card>

      <Alert className="border-amber-500/20 bg-amber-500/5 text-amber-300">
        <EyeOff className="w-4 h-4 text-amber-400" />
        <AlertTitle className="font-semibold text-amber-400">Credential Warning</AlertTitle>
        <AlertDescription className="text-xs">
          Node private keys represent the sole identity signature for a ZAP node. Never commit private keys to code repositories or expose them in container environment variables.
        </AlertDescription>
      </Alert>

      {/* Step 1 */}
      <Card className="bg-zinc-950/40 border-zinc-850">
        <CardHeader>
          <div className="flex items-center gap-3">
            <div className="w-8 h-8 rounded-lg bg-zinc-900 border border-zinc-800 flex items-center justify-center">
              <Cpu className="w-4 h-4 text-blue-400" />
            </div>
            <div>
              <CardTitle className="text-white text-base">1. Multi-Stage Container Build</CardTitle>
              <CardDescription className="text-xs">Minimize production runtime dependencies and image size</CardDescription>
            </div>
          </div>
        </CardHeader>
        <CardContent className="space-y-4">
          <p className="text-sm text-zinc-400">
            The ZAP container image uses a multi-stage Docker build to keep images lightweight and secure. The final runtime lacks build-essential tools and compiles entirely in a clean context:
          </p>
          <pre className="text-xs bg-[#050505] p-4 rounded-lg border border-zinc-900 font-mono text-zinc-300">
            <code>{`docker build -t zap:local .`}</code>
          </pre>
          <div className="grid grid-cols-1 sm:grid-cols-2 gap-4">
            <div className="p-4 rounded-lg bg-[#050505] border border-zinc-900">
              <span className="text-xs font-semibold text-zinc-350 block mb-1">Builder Phase</span>
              <span className="text-xs text-zinc-500">Compiles Rust binaries with optimization flags and strips symbols.</span>
            </div>
            <div className="p-4 rounded-lg bg-[#050505] border border-zinc-900">
              <span className="text-xs font-semibold text-zinc-350 block mb-1">Runtime Phase</span>
              <span className="text-xs text-zinc-500">Utilizes minimal Debian slim image, includes <code>tini</code>, runs as non-root user.</span>
            </div>
          </div>
        </CardContent>
      </Card>

      {/* Step 2 */}
      <Card className="bg-zinc-950/40 border-zinc-850">
        <CardHeader>
          <div className="flex items-center gap-3">
            <div className="w-8 h-8 rounded-lg bg-zinc-900 border border-zinc-800 flex items-center justify-center">
              <Server className="w-4 h-4 text-purple-400" />
            </div>
            <div>
              <CardTitle className="text-white text-base">2. Docker Compose Deployment</CardTitle>
              <CardDescription className="text-xs">Spin up an orchestrated peer cluster locally</CardDescription>
            </div>
          </div>
        </CardHeader>
        <CardContent className="space-y-4">
          <p className="text-sm text-zinc-400">
            Bootstrap local networks and mount state directories securely. Generate private credentials first:
          </p>
          <pre className="text-xs bg-[#050505] p-4 rounded-lg border border-zinc-900 font-mono text-zinc-300">
            <code>{`# Generate state folder and keys
mkdir -p .zap/container
docker compose run --rm node keygen --out /var/lib/zap/node.key

# Start the cluster in detached mode
docker compose up -d`}</code>
          </pre>
        </CardContent>
      </Card>

      {/* Step 3 */}
      <Card className="bg-zinc-950/40 border-zinc-850">
        <CardHeader>
          <div className="flex items-center gap-3">
            <div className="w-8 h-8 rounded-lg bg-zinc-900 border border-zinc-800 flex items-center justify-center">
              <ShieldCheck className="w-4 h-4 text-emerald-400" />
            </div>
            <div>
              <CardTitle className="text-white text-base">3. Hardening Checklist</CardTitle>
              <CardDescription className="text-xs">Production security requirements</CardDescription>
            </div>
          </div>
        </CardHeader>
        <CardContent className="space-y-4 text-sm text-zinc-400">
          <div className="space-y-3">
            <div className="flex flex-col sm:flex-row gap-4 p-3.5 rounded-lg bg-[#050505] border border-zinc-900 justify-between items-start sm:items-center">
              <div>
                <span className="font-semibold text-white text-xs block">ReadOnly Root Filesystem</span>
                <span className="text-xs text-zinc-500">Ensure containers run with a read-only root FS except for state paths.</span>
              </div>
              <Badge variant="outline" className="bg-zinc-900 border-zinc-800 text-blue-400 font-mono text-[10px]">--read-only</Badge>
            </div>

            <div className="flex flex-col sm:flex-row gap-4 p-3.5 rounded-lg bg-[#050505] border border-zinc-900 justify-between items-start sm:items-center">
              <div>
                <span className="font-semibold text-white text-xs block">Strict Signature Enforcement</span>
                <span className="text-xs text-zinc-500">Require cryptographically signed manifests for all drivers loaded on runtime.</span>
              </div>
              <Badge variant="outline" className="bg-zinc-900 border-zinc-800 text-emerald-400 font-mono text-[10px]">require_signature = true</Badge>
            </div>

            <div className="flex flex-col sm:flex-row gap-4 p-3.5 rounded-lg bg-[#050505] border border-zinc-900 justify-between items-start sm:items-center">
              <div>
                <span className="font-semibold text-white text-xs block">Network Constraint Isolation</span>
                <span className="text-xs text-zinc-500">Restrict UDP message interfaces to explicit overlay VPN tunnels.</span>
              </div>
              <Badge variant="outline" className="bg-zinc-900 border-zinc-800 text-zinc-400 font-mono text-[10px]">WireGuard / Overlay</Badge>
            </div>
          </div>
        </CardContent>
      </Card>
    </div>
  );
}
