import Link from "next/link";
import { AlertTriangle, CheckCircle2, CircleDashed, Milestone, PackageCheck, ShieldCheck } from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";

const phaseRows = [
  {
    phase: "Phase 0",
    title: "Promise, Packaging, and Adoption",
    status: "partial",
    done: "Install docs, governance flow, and first-class website pages for major operator guides.",
    remaining: "Release/community coverage and link hygiene must stay enforced as pages evolve.",
  },
  {
    phase: "Phase 1",
    title: "Production Hardening",
    status: "partial",
    done: "Prometheus/Grafana assets, fail-closed policy defaults, receipt fsync modes, and ops runbooks.",
    remaining: "HTTP metrics/health adapters, segment manifests, and persistent replay windows.",
  },
  {
    phase: "Phase 2",
    title: "Protocol Spec and SDK Conformance",
    status: "partial",
    done: "Shared fixtures, CLI fixture verification, schema export, and PACT record/bundle conformance across Rust, TypeScript, Python, and Go.",
    remaining: "More binary golden vectors and broader SDK coverage for frames, receipts, capabilities, and agents.",
  },
  {
    phase: "Phase 3",
    title: "Agent Gateway",
    status: "partial",
    done: "Intent/status/result plus session, delegation, and negotiation JSON builders.",
    remaining: "Persistent orchestration, receipt linkage, and evidence export bundles.",
  },
  {
    phase: "Phase 4",
    title: "Domain Packs and Marketplace",
    status: "partial",
    done: "Pack validate/inspect/list plus agentic-dev, smart-building, cloud-ops, industrial, and personal-ai packs.",
    remaining: "Signed pack build/install flows and ZapStore marketplace integration.",
  },
  {
    phase: "Phase 5-7",
    title: "Fleet, Modularization, and 1.0 Readiness",
    status: "planned",
    done: "Release readiness command, compatibility docs, and operator release checklist.",
    remaining: "Fleet topology inspection, service extraction, audit plan, and strict example doctor CI.",
  },
];

const statusStyles = {
  done: "bg-emerald-500/10 border-emerald-500/20 text-emerald-300",
  partial: "bg-amber-500/10 border-amber-500/20 text-amber-300",
  planned: "bg-blue-500/10 border-blue-500/20 text-blue-300",
};

export default function RoadmapStatusPage() {
  return (
    <div className="space-y-8">
      <div>
        <div className="mb-3 flex items-center gap-2 text-sm font-medium text-blue-300">
          <Milestone className="h-4 w-4" />
          Implementation Evidence
        </div>
        <h1>Roadmap Status</h1>
        <p className="lead">
          A tighter operator view of what is complete, what is partial, and what still needs implementation before ZAP can claim full roadmap coverage.
        </p>
      </div>

      <div className="grid gap-4 md:grid-cols-3">
        <Card className="border-zinc-850 bg-zinc-950/40">
          <CardHeader className="pb-2">
            <CardTitle className="flex items-center gap-2 text-base text-white">
              <CheckCircle2 className="h-4 w-4 text-emerald-300" />
              Proved
            </CardTitle>
          </CardHeader>
          <CardContent className="text-sm text-zinc-400">
            CLI conformance gates, domain-pack validation, PACT fixture tests, SDK conformance, release readiness, and ops config tests now provide repeatable evidence.
          </CardContent>
        </Card>
        <Card className="border-zinc-850 bg-zinc-950/40">
          <CardHeader className="pb-2">
            <CardTitle className="flex items-center gap-2 text-base text-white">
              <PackageCheck className="h-4 w-4 text-amber-300" />
              Partial
            </CardTitle>
          </CardHeader>
          <CardContent className="text-sm text-zinc-400">
            The current stack has usable primitives for agents, receipts, packs, and observability, but several workflows still stop at local builders or docs.
          </CardContent>
        </Card>
        <Card className="border-zinc-850 bg-zinc-950/40">
          <CardHeader className="pb-2">
            <CardTitle className="flex items-center gap-2 text-base text-white">
              <AlertTriangle className="h-4 w-4 text-blue-300" />
              Next Gates
            </CardTitle>
          </CardHeader>
          <CardContent className="text-sm text-zinc-400">
            Highest leverage remains metrics/health serving, evidence bundles, signed pack install, fleet inspection, and broader schema parity.
          </CardContent>
        </Card>
      </div>

      <div className="space-y-4">
        {phaseRows.map((row) => (
          <Card key={row.phase} className="border-zinc-850 bg-zinc-950/40">
            <CardHeader className="pb-3">
              <div className="flex flex-col gap-3 sm:flex-row sm:items-start sm:justify-between">
                <div>
                  <p className="text-xs font-semibold uppercase text-zinc-500">{row.phase}</p>
                  <CardTitle className="mt-1 text-lg text-white">{row.title}</CardTitle>
                </div>
                <Badge className={`${statusStyles[row.status as keyof typeof statusStyles]} w-fit capitalize`}>
                  {row.status}
                </Badge>
              </div>
            </CardHeader>
            <CardContent className="grid gap-4 text-sm md:grid-cols-2">
              <div>
                <h2 className="mb-2 flex items-center gap-2 text-sm font-semibold text-zinc-200">
                  <ShieldCheck className="h-4 w-4 text-emerald-300" />
                  Evidence
                </h2>
                <p className="text-zinc-400">{row.done}</p>
              </div>
              <div>
                <h2 className="mb-2 flex items-center gap-2 text-sm font-semibold text-zinc-200">
                  <CircleDashed className="h-4 w-4 text-amber-300" />
                  Remaining
                </h2>
                <p className="text-zinc-400">{row.remaining}</p>
              </div>
            </CardContent>
          </Card>
        ))}
      </div>

      <Card className="border-blue-500/20 bg-blue-500/5">
        <CardContent className="pt-6 text-sm text-zinc-300">
          Source of truth: keep this page aligned with <Link href="/docs/roadmap" className="text-blue-300 hover:text-blue-200">Roadmap</Link>,{" "}
          <Link href="/docs/release" className="text-blue-300 hover:text-blue-200">Release Readiness</Link>, and repository evidence such as CLI tests,
          fixtures, workflows, and ops assets.
        </CardContent>
      </Card>
    </div>
  );
}
