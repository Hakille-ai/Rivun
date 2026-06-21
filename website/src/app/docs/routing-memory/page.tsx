import Image from 'next/image';
import { Network, Activity, FileCode, Search, Database } from 'lucide-react';
import { Card, CardHeader, CardTitle, CardContent, CardDescription } from "@/components/ui/card";
import { Alert, AlertTitle, AlertDescription } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";

export default function RoutingMemoryPage() {
  return (
    <div className="space-y-8 font-sans">
      <div>
        <h1 className="text-3xl md:text-4xl font-extrabold tracking-tight mb-2 text-white">Capabilities & Routing</h1>
        <p className="text-zinc-400 text-lg">Understand capability discovery, packet routing policies, and the local auditable memory system.</p>
      </div>

      {/* Hero Visual Card */}
      <Card className="group bg-zinc-950/40 hover:bg-zinc-950/60 border border-zinc-850 hover:border-purple-500/20 transition-all duration-500 overflow-hidden rounded-2xl shadow-xl hover:shadow-2xl hover:shadow-purple-500/5 not-prose p-0">
        <div className="grid grid-cols-1 md:grid-cols-12 items-stretch">
          {/* Image Column */}
          <div className="md:col-span-5 relative bg-gradient-to-br from-zinc-950 to-black/80 flex items-center justify-center p-6 border-b md:border-b-0 md:border-r border-zinc-900 overflow-hidden min-h-[260px] md:min-h-0">
            {/* Ambient Glow Effects */}
            <div className="absolute inset-0 bg-[radial-gradient(circle_at_center,rgba(99,102,241,0.08)_0%,transparent_70%)] pointer-events-none" />
            <div className="absolute -inset-10 bg-[radial-gradient(circle_at_bottom_left,rgba(59,130,246,0.04)_0%,transparent_50%)] pointer-events-none" />
            
            {/* Image Wrapper */}
            <div className="relative w-full aspect-square max-w-[220px] md:max-w-[240px] transform group-hover:scale-[1.03] transition-transform duration-500 ease-out">
              <Image 
                src="/images/zap_capability_routing.png" 
                alt="ZAP Network Capability Routing Visual" 
                fill
                style={{ objectFit: 'contain' }}
                className="opacity-90 group-hover:opacity-100 transition-opacity duration-500"
                priority
              />
            </div>
          </div>
          {/* Content Column */}
          <div className="md:col-span-7 p-6 md:p-8 flex flex-col justify-center space-y-3 bg-zinc-950/20">
            <span className="text-xs font-semibold uppercase tracking-wider text-purple-400 block">Mesh Tunneling</span>
            <h3 className="text-xl font-bold text-white tracking-tight">Capability-Based Dispatching</h3>
            <p className="text-sm text-zinc-400 leading-relaxed">
              ZAP nodes advertise capabilities over the brokerless UDP mesh. Inbound envelopes are processed against deterministic routing rules, matching action namespaces to local driver execution units or forwarding destinations.
            </p>
          </div>
        </div>
      </Card>

      <div className="grid grid-cols-1 md:grid-cols-2 gap-6">
        {/* Capability Discovery Card */}
        <Card className="bg-zinc-950/40 border-zinc-850">
          <CardHeader>
            <div className="flex items-center gap-3">
              <div className="w-8 h-8 rounded-lg bg-zinc-900 border border-zinc-800 flex items-center justify-center">
                <Search className="w-4 h-4 text-blue-400" />
              </div>
              <div>
                <CardTitle className="text-white text-base">Capability Discovery</CardTitle>
                <CardDescription className="text-xs">Query and register mesh functions</CardDescription>
              </div>
            </div>
          </CardHeader>
          <CardContent className="space-y-3 text-sm text-zinc-400">
            <p>
              Nodes dynamically broadcast and answer capability queries over the mesh via the following signed ZENV control envelopes:
            </p>
            <div className="flex flex-wrap gap-2 pt-1">
              <Badge variant="outline" className="font-mono text-[9px] bg-zinc-900 text-zinc-400 border-zinc-850">zap.capability.query</Badge>
              <Badge variant="outline" className="font-mono text-[9px] bg-zinc-900 text-zinc-400 border-zinc-850">zap.capability.response</Badge>
              <Badge variant="outline" className="font-mono text-[9px] bg-zinc-900 text-zinc-400 border-zinc-850">zap.capability.announce</Badge>
            </div>
            <pre className="text-[10px] bg-[#050505] p-2.5 rounded-lg border border-zinc-900 font-mono text-zinc-350 mt-2">
              <code>zap capability query --target &lt;peer-id&gt;</code>
            </pre>
          </CardContent>
        </Card>

        {/* Auditable Memory Ledgers */}
        <Card className="bg-zinc-950/40 border-zinc-850">
          <CardHeader>
            <div className="flex items-center gap-3">
              <div className="w-8 h-8 rounded-lg bg-zinc-900 border border-zinc-800 flex items-center justify-center">
                <Database className="w-4 h-4 text-emerald-400" />
              </div>
              <div>
                <CardTitle className="text-white text-base">Auditable Memory Ledgers</CardTitle>
                <CardDescription className="text-xs">Hash-chained, tamper-evident log storage</CardDescription>
              </div>
            </div>
          </CardHeader>
          <CardContent className="space-y-3 text-sm text-zinc-400">
            <p>
              The <code>zap-memory</code> storage engine maintains append-only binary journals. Each record maps its own body hash and a reference digest linking to the previous entry, establishing a strict sequence validation structure:
            </p>
            <pre className="text-[10px] bg-[#050505] p-2.5 rounded-lg border border-zinc-900 font-mono text-zinc-350">
              <code>zap memory verify --dir .zap/memory</code>
            </pre>
          </CardContent>
        </Card>
      </div>

      {/* Deterministic Routing */}
      <Card className="bg-zinc-950/40 border-zinc-850">
        <CardHeader>
          <div className="flex items-center gap-3">
            <div className="w-8 h-8 rounded-lg bg-zinc-900 border border-zinc-800 flex items-center justify-center">
              <Network className="w-4 h-4 text-purple-400" />
            </div>
            <div>
              <CardTitle className="text-white text-base">Deterministic Routing Rules</CardTitle>
              <CardDescription className="text-xs">Bind action namespaces to targets</CardDescription>
            </div>
          </div>
        </CardHeader>
        <CardContent className="space-y-4">
          <p className="text-xs text-zinc-400">
            Routing matching targets action subjects (e.g. <code>thermostat.*</code>) and restricts forwarding to peers carrying valid Capability Grants:
          </p>
          <pre className="text-xs bg-[#050505] p-4 rounded-lg border border-zinc-900 font-mono text-zinc-300 overflow-x-auto">
            <code>{`[[routes]]
name = "thermostat-peer"
requires_peer_grant = "driver.execute:thermostat.setpoint"

[routes.match]
kind = "action"
subject = "thermostat.*"

[routes.target]
peer = "00000000-0000-4000-8000-000000000000"`}</code>
          </pre>
        </CardContent>
      </Card>
    </div>
  );
}
