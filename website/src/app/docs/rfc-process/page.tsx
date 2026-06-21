import { AlertTriangle, CheckCircle2, FileCheck2, FileSignature, GitPullRequest, Lock, Package, Shield, Terminal, type LucideIcon } from 'lucide-react';
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";

const states = [
  ["draft", "The idea is being shaped and may change substantially."],
  ["review", "Maintainers and code owners are actively reviewing the proposal."],
  ["accepted", "The design is approved for implementation."],
  ["implemented", "Code, fixtures, docs, migration notes, and release notes are merged."],
  ["deferred", "The proposal is useful, but not scheduled."],
  ["rejected", "The decision is recorded with rationale."],
  ["superseded", "A later ZEP has replaced the proposal."],
];

const requiredFor = [
  "ZAP-Wire or ZENV binary layout, versioning, or negotiation",
  "Cryptography, signatures, trust roots, replay protection, or key handling",
  "PACT canonical payload fields, signature domains, fixture hashes, or revocation semantics",
  "Driver ABI, host imports, Wasm permissions, or sandbox boundaries",
  "Node config defaults, policy defaults, governance controls, or release authority",
  "Agent intent, status, action, result, or fixture semantics",
  "Domain pack manifest fields, schema rules, risk levels, or policy semantics",
  "SDK behavior that changes compatibility for existing applications",
];

const checklist = [
  "Link the ZEP from the issue and pull request.",
  "Add or update protocol fixtures and golden vectors where applicable.",
  "Update security, operations, SDK, or domain pack docs when their contracts change.",
  "Validate example domain packs with zap pack validate.",
  "Run workspace tests and targeted SDK or website checks for touched areas.",
  "Record breaking changes in release notes and migration guidance.",
];

const links: Array<[string, string, LucideIcon]> = [
  ["Security model", "/docs/security", Shield],
  ["Message policy", "/docs/message-policy", Lock],
  ["PACT profile", "/docs/pact", FileSignature],
  ["Agent protocol", "/docs/agent-protocol", GitPullRequest],
  ["Domain packs", "/docs/domain-packs", Package],
];

export default function RfcProcessPage() {
  return (
    <div className="space-y-8 font-sans">
      <div>
        <h1 className="text-3xl md:text-4xl font-extrabold tracking-tight mb-2 text-white">RFC / ZEP Process</h1>
        <p className="text-zinc-400 text-lg">A lightweight governance path for ZAP changes that affect long-lived protocol, security, ABI, config, SDK, and domain pack contracts.</p>
      </div>

      <Alert className="border-amber-500/20 bg-amber-500/5 text-amber-300">
        <AlertTriangle className="w-4 h-4 text-amber-400" />
        <AlertTitle className="font-semibold text-amber-400">Contract Changes Need a ZEP</AlertTitle>
        <AlertDescription className="text-xs">
          Open a Security / Protocol Change issue first, then write a ZAP Enhancement Proposal before implementation details harden.
        </AlertDescription>
      </Alert>

      <Card className="group bg-zinc-950/40 hover:bg-zinc-950/60 border border-zinc-850 hover:border-blue-500/20 transition-all duration-500 overflow-hidden rounded-2xl shadow-xl hover:shadow-2xl hover:shadow-blue-500/5 not-prose p-0">
        <div className="grid grid-cols-1 md:grid-cols-12 items-stretch">
          <div className="md:col-span-5 relative bg-gradient-to-br from-zinc-950 to-black/80 flex items-center justify-center p-6 border-b md:border-b-0 md:border-r border-zinc-900 overflow-hidden min-h-[260px] md:min-h-0">
            <div className="absolute inset-0 bg-[radial-gradient(circle_at_center,rgba(59,130,246,0.08)_0%,transparent_70%)] pointer-events-none" />
            <div className="relative w-full max-w-[300px] space-y-3">
              {["draft", "review", "accepted", "implemented"].map((state, index) => (
                <div key={state} className="flex items-center justify-between rounded-xl border border-zinc-800 bg-black/40 p-3 shadow-lg shadow-black/20">
                  <div className="flex items-center gap-3">
                    <div className="h-9 w-9 rounded-lg bg-zinc-900 border border-zinc-800 flex items-center justify-center">
                      <FileCheck2 className={`h-4 w-4 ${index < 2 ? "text-blue-400" : "text-emerald-400"}`} />
                    </div>
                    <p className="font-mono text-xs text-zinc-200">zep: {state}</p>
                  </div>
                  <Badge variant="outline" className="font-mono text-[10px] bg-zinc-900 text-zinc-400 border-zinc-850">state</Badge>
                </div>
              ))}
            </div>
          </div>
          <div className="md:col-span-7 p-6 md:p-8 flex flex-col justify-center space-y-3 bg-zinc-950/20">
            <span className="text-xs font-semibold uppercase tracking-wider text-blue-400 block">Review Before Contracts Harden</span>
            <h3 className="text-xl font-bold text-white tracking-tight">Design, Threat Model, Validate, Then Ship</h3>
            <p className="text-sm text-zinc-400 leading-relaxed">
              ZEPs make protocol, crypto, ABI, config, SDK, and domain pack decisions reviewable before they become compatibility promises for operators and integrators.
            </p>
          </div>
        </div>
      </Card>

      <div className="grid grid-cols-1 md:grid-cols-2 gap-6">
        <Card className="bg-zinc-950/40 border-zinc-850">
          <CardHeader>
            <div className="flex items-center gap-3">
              <div className="w-8 h-8 rounded-lg bg-zinc-900 border border-zinc-800 flex items-center justify-center">
                <Shield className="w-4 h-4 text-amber-400" />
              </div>
              <div>
                <CardTitle className="text-white text-base">When a ZEP Is Required</CardTitle>
                <CardDescription className="text-xs">Public contracts, safety boundaries, and compatibility behavior</CardDescription>
              </div>
            </div>
          </CardHeader>
          <CardContent className="space-y-2">
            {requiredFor.map((item) => (
              <div key={item} className="flex items-start gap-2 text-xs text-zinc-400">
                <CheckCircle2 className="mt-0.5 h-4 w-4 flex-shrink-0 text-emerald-400" />
                <span>{item}</span>
              </div>
            ))}
          </CardContent>
        </Card>

        <Card className="bg-zinc-950/40 border-zinc-850">
          <CardHeader>
            <div className="flex items-center gap-3">
              <div className="w-8 h-8 rounded-lg bg-zinc-900 border border-zinc-800 flex items-center justify-center">
                <FileCheck2 className="w-4 h-4 text-blue-400" />
              </div>
              <div>
                <CardTitle className="text-white text-base">ZEP States</CardTitle>
                <CardDescription className="text-xs">Stable lifecycle labels for maintainers and contributors</CardDescription>
              </div>
            </div>
          </CardHeader>
          <CardContent className="space-y-3">
            {states.map(([state, description]) => (
              <div key={state} className="rounded-lg border border-zinc-900 bg-black/20 px-4 py-3">
                <p className="font-mono text-xs text-zinc-300">{state}</p>
                <p className="mt-1 text-xs text-zinc-500">{description}</p>
              </div>
            ))}
          </CardContent>
        </Card>
      </div>

      <Card className="bg-zinc-950/40 border-zinc-850">
        <CardHeader>
          <div className="flex items-center gap-3">
            <div className="w-8 h-8 rounded-lg bg-zinc-900 border border-zinc-800 flex items-center justify-center">
              <GitPullRequest className="w-4 h-4 text-emerald-400" />
            </div>
            <div>
              <CardTitle className="text-white text-base">Implementation Checklist</CardTitle>
              <CardDescription className="text-xs">A ZEP is implemented only when code, evidence, docs, and migration notes land together</CardDescription>
            </div>
          </div>
        </CardHeader>
        <CardContent className="grid grid-cols-1 md:grid-cols-2 gap-3">
          {checklist.map((item) => (
            <div key={item} className="flex items-start gap-2 rounded-lg border border-zinc-900 bg-black/20 p-3 text-xs text-zinc-400">
              <CheckCircle2 className="mt-0.5 h-4 w-4 flex-shrink-0 text-emerald-400" />
              <span>{item}</span>
            </div>
          ))}
        </CardContent>
      </Card>

      <Card className="bg-zinc-950/40 border-zinc-850">
        <CardHeader>
          <div className="flex items-center gap-3">
            <div className="w-8 h-8 rounded-lg bg-zinc-900 border border-zinc-800 flex items-center justify-center">
              <Terminal className="w-4 h-4 text-cyan-400" />
            </div>
            <div>
              <CardTitle className="text-white text-base">Template</CardTitle>
              <CardDescription className="text-xs">Minimum proposal shape for protocol, security, and compatibility decisions</CardDescription>
            </div>
          </div>
        </CardHeader>
        <CardContent>
          <pre className="text-xs bg-[#050505] p-4 rounded-lg border border-zinc-900 font-mono text-zinc-300 overflow-x-auto">
            <code>{`# ZEP-NNN: Short Title

- Status: draft
- Authors:
- Created:
- Target release:
- Related issue:

## Summary
## Motivation
## Affected Contracts
## Threat Model
## Detailed Design
## Compatibility and Migration
## Validation Plan
## Alternatives
## Open Questions`}</code>
          </pre>
        </CardContent>
      </Card>

      <Card className="bg-zinc-950/40 border-zinc-850">
        <CardHeader>
          <CardTitle className="text-white text-base">Related Docs</CardTitle>
          <CardDescription className="text-xs">Primary surfaces that usually change alongside a ZEP</CardDescription>
        </CardHeader>
          <CardContent className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-5 gap-3">
          {links.map(([title, href, Icon]) => (
            <a key={href} href={href} className="flex items-center gap-3 rounded-lg border border-zinc-900 bg-black/20 p-3 text-xs text-zinc-400 transition-colors hover:border-blue-500/30 hover:text-white">
              <Icon className="h-4 w-4 flex-shrink-0 text-blue-400" />
              <span>{title}</span>
            </a>
          ))}
        </CardContent>
      </Card>
    </div>
  );
}
