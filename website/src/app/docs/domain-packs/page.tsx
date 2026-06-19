import { Archive, ClipboardCheck, Package, Route, ShieldAlert, Terminal } from 'lucide-react';
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";

const riskLevels = [
  { level: "low", label: "Read-only or informational", tone: "text-emerald-400" },
  { level: "medium", label: "Reversible write or bounded automation", tone: "text-blue-400" },
  { level: "high", label: "Service, data, cost, or availability impact", tone: "text-amber-400" },
  { level: "critical", label: "Safety, access, money movement, destructive change, or physical effect", tone: "text-red-400" },
];

const firstPacks = [
  "zap-pack-agentic-dev",
  "zap-pack-smart-building",
  "zap-pack-industrial",
  "zap-pack-cloud-ops",
  "zap-pack-personal-ai",
];

export default function DomainPacksPage() {
  return (
    <div className="space-y-8 font-sans">
      <div>
        <h1 className="text-3xl md:text-4xl font-extrabold tracking-tight mb-2 text-white">Domain Packs</h1>
        <p className="text-zinc-400 text-lg">Signed, reviewable bundles that adapt ZAP capabilities, policies, schemas, and routes to a specific operating field.</p>
      </div>

      <Card className="group bg-zinc-950/40 hover:bg-zinc-950/60 border border-zinc-850 hover:border-emerald-500/20 transition-all duration-500 overflow-hidden rounded-2xl shadow-xl hover:shadow-2xl hover:shadow-emerald-500/5 not-prose p-0">
        <div className="grid grid-cols-1 md:grid-cols-12 items-stretch">
          <div className="md:col-span-5 relative bg-gradient-to-br from-zinc-950 to-black/80 flex items-center justify-center p-6 border-b md:border-b-0 md:border-r border-zinc-900 overflow-hidden min-h-[260px] md:min-h-0">
            <div className="absolute inset-0 bg-[radial-gradient(circle_at_center,rgba(16,185,129,0.08)_0%,transparent_70%)] pointer-events-none" />
            <div className="absolute -inset-10 bg-[radial-gradient(circle_at_bottom_left,rgba(59,130,246,0.04)_0%,transparent_50%)] pointer-events-none" />
            <div className="relative grid grid-cols-2 gap-3 w-full max-w-[260px]">
              {["schemas", "policies", "routes", "drivers"].map((item) => (
                <div key={item} className="rounded-xl border border-zinc-800 bg-black/40 px-4 py-5 text-center shadow-lg shadow-black/20">
                  <Package className="mx-auto mb-3 h-6 w-6 text-emerald-400" />
                  <span className="font-mono text-[11px] text-zinc-300">{item}/</span>
                </div>
              ))}
            </div>
          </div>
          <div className="md:col-span-7 p-6 md:p-8 flex flex-col justify-center space-y-3 bg-zinc-950/20">
            <span className="text-xs font-semibold uppercase tracking-wider text-emerald-400 block">Reusable Domain Trust</span>
            <h3 className="text-xl font-bold text-white tracking-tight">Capabilities Without Protocol Drift</h3>
            <p className="text-sm text-zinc-400 leading-relaxed">
              Domain packs keep ZAP&apos;s core protocol narrow while giving teams a repeatable starting point for domain capability names, message subjects, policy templates, route templates, drivers, tests, and operating notes.
            </p>
          </div>
        </div>
      </Card>

      <Alert className="border-amber-500/20 bg-amber-500/5 text-amber-300">
        <ShieldAlert className="w-4 h-4 text-amber-400" />
        <AlertTitle className="font-semibold text-amber-400">Preview Trust Boundary</AlertTitle>
        <AlertDescription className="text-xs">
          Early pack manifests are documentation and planning artifacts. Future ZapStore workflows can sign, publish, verify, revoke, and install full capability packs.
        </AlertDescription>
      </Alert>

      <div className="grid grid-cols-1 md:grid-cols-2 gap-6">
        <Card className="bg-zinc-950/40 border-zinc-850">
          <CardHeader>
            <div className="flex items-center gap-3">
              <div className="w-8 h-8 rounded-lg bg-zinc-900 border border-zinc-800 flex items-center justify-center">
                <Archive className="w-4 h-4 text-blue-400" />
              </div>
              <div>
                <CardTitle className="text-white text-base">Pack Layout</CardTitle>
                <CardDescription className="text-xs">Directory contract for preview bundles</CardDescription>
              </div>
            </div>
          </CardHeader>
          <CardContent className="space-y-4 text-sm text-zinc-400">
            <p>Only <code>pack.toml</code> and <code>README.md</code> are required for early preview packs. Executable artifacts can add these directories as the pack matures:</p>
            <pre className="text-xs bg-[#050505] p-4 rounded-lg border border-zinc-900 font-mono text-zinc-300 overflow-x-auto">
              <code>{`pack.toml
README.md
schemas/
policies/
routes/
drivers/
dashboards/
tests/`}</code>
            </pre>
          </CardContent>
        </Card>

        <Card className="bg-zinc-950/40 border-zinc-850">
          <CardHeader>
            <div className="flex items-center gap-3">
              <div className="w-8 h-8 rounded-lg bg-zinc-900 border border-zinc-800 flex items-center justify-center">
                <ClipboardCheck className="w-4 h-4 text-emerald-400" />
              </div>
              <div>
                <CardTitle className="text-white text-base">Risk Vocabulary</CardTitle>
                <CardDescription className="text-xs">Fail-closed defaults for domain actions</CardDescription>
              </div>
            </div>
          </CardHeader>
          <CardContent className="space-y-3">
            {riskLevels.map((risk) => (
              <div key={risk.level} className="flex items-start justify-between gap-4 rounded-lg border border-zinc-900 bg-black/20 px-3 py-2">
                <Badge variant="outline" className={`font-mono text-[10px] bg-zinc-900 border-zinc-850 ${risk.tone}`}>{risk.level}</Badge>
                <p className="text-xs text-zinc-400 flex-1">{risk.label}</p>
              </div>
            ))}
          </CardContent>
        </Card>
      </div>

      <Card className="bg-zinc-950/40 border-zinc-850">
        <CardHeader>
          <div className="flex items-center gap-3">
            <div className="w-8 h-8 rounded-lg bg-zinc-900 border border-zinc-800 flex items-center justify-center">
              <Route className="w-4 h-4 text-purple-400" />
            </div>
            <div>
              <CardTitle className="text-white text-base">Manifest Contract</CardTitle>
              <CardDescription className="text-xs">Plain TOML for security and operations review</CardDescription>
            </div>
          </div>
        </CardHeader>
        <CardContent className="space-y-4">
          <pre className="text-xs bg-[#050505] p-4 rounded-lg border border-zinc-900 font-mono text-zinc-300 overflow-x-auto">
            <code>{`schema_version = 1
id = "zap-pack-agentic-dev"
name = "Agentic Development"
version = "0.1.0"
status = "preview"
description = "Auditable coding-agent workflows over ZAP."

[compatibility]
zap_protocol = ">=0.1.0,<1.0.0"
driver_abi = ">=1,<=2"

[[capabilities]]
id = "repo.patch"
risk = "medium"
requires = ["repo.read"]
description = "Prepare or apply scoped source patches."`}</code>
          </pre>
        </CardContent>
      </Card>

      <Card className="bg-zinc-950/40 border-zinc-850">
        <CardHeader>
          <div className="flex items-center gap-3">
            <div className="w-8 h-8 rounded-lg bg-zinc-900 border border-zinc-800 flex items-center justify-center">
              <Terminal className="w-4 h-4 text-blue-400" />
            </div>
            <div>
              <CardTitle className="text-white text-base">Lifecycle Direction</CardTitle>
              <CardDescription className="text-xs">From preview folders to signed ZapStore artifacts</CardDescription>
            </div>
          </div>
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-5 gap-2">
            {firstPacks.map((pack) => (
              <div key={pack} className="rounded-lg border border-zinc-900 bg-black/20 px-3 py-3">
                <p className="font-mono text-[11px] text-zinc-300">{pack}</p>
              </div>
            ))}
          </div>
          <p className="text-xs text-zinc-500">
            Preview packs should define capability ids, policies, schemas, routes, example drivers, expected receipts, and tests before being promoted toward signed build, install, audit, and revoke workflows.
          </p>
        </CardContent>
      </Card>
    </div>
  );
}
