import Link from "next/link";
import { Archive, CheckCircle2, FileCheck2, GitBranch, ShieldCheck } from "lucide-react";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";

const readinessChecks = [
  "Protocol and PACT fixtures verify with zap fixtures verify.",
  "Domain pack catalog validates with zap pack list.",
  "Python, TypeScript, Rust, and Go SDK conformance run in release CI, including PACT hash and bundle checks.",
  "Website documentation lint passes before packaging.",
  "Checksums, Sigstore bundles, and release manifest are attached to artifacts.",
];

export default function ReleasePage() {
  return (
    <div className="space-y-8">
      <div>
        <div className="mb-3 flex items-center gap-2 text-sm font-medium text-blue-300">
          <Archive className="h-4 w-4" />
          Phase 7 Gate
        </div>
        <h1>Release Readiness</h1>
        <p className="lead">
          The release process turns roadmap evidence into a repeatable gate for packaged ZAP builds, SDK conformance, website quality, and supply-chain proof.
        </p>
      </div>

      <Card className="border-zinc-850 bg-zinc-950/40">
        <CardHeader>
          <CardTitle className="flex items-center gap-2 text-white">
            <FileCheck2 className="h-5 w-5 text-emerald-300" />
            Readiness Command
          </CardTitle>
        </CardHeader>
        <CardContent className="space-y-4 text-sm text-zinc-400">
          <p>Stable release candidates are expected to pass the strict readiness gate in CI:</p>
          <pre className="overflow-x-auto rounded-lg border border-zinc-800 bg-black/40 p-4 text-xs text-zinc-200">
            <code>cargo run --locked -p xtask -- release readiness --require-go</code>
          </pre>
          <p>
            Local preparation may omit Go only when the machine lacks that toolchain, but release CI must run the full command with Go enabled.
          </p>
        </CardContent>
      </Card>

      <div className="grid gap-4 md:grid-cols-2">
        <Card className="border-zinc-850 bg-zinc-950/40">
          <CardHeader>
            <CardTitle className="flex items-center gap-2 text-base text-white">
              <CheckCircle2 className="h-4 w-4 text-emerald-300" />
              Required Gates
            </CardTitle>
          </CardHeader>
          <CardContent>
            <ul className="space-y-3 text-sm text-zinc-400">
              {readinessChecks.map((item) => (
                <li key={item} className="flex gap-2">
                  <CheckCircle2 className="mt-0.5 h-4 w-4 flex-shrink-0 text-emerald-300" />
                  <span>{item}</span>
                </li>
              ))}
            </ul>
          </CardContent>
        </Card>

        <Card className="border-zinc-850 bg-zinc-950/40">
          <CardHeader>
            <CardTitle className="flex items-center gap-2 text-base text-white">
              <ShieldCheck className="h-4 w-4 text-blue-300" />
              Release Evidence
            </CardTitle>
          </CardHeader>
          <CardContent className="space-y-3 text-sm text-zinc-400">
            <p>Each release should preserve the workflow URL, commit SHA, command output, checksums, signatures, SBOM status, and migration notes.</p>
            <p>
              Protocol, PACT, CLI, SDK, policy, domain-pack, and website route changes all require explicit migration notes before tagging.
            </p>
          </CardContent>
        </Card>
      </div>

      <Card className="border-blue-500/20 bg-blue-500/5">
        <CardContent className="flex flex-col gap-3 pt-6 text-sm text-zinc-300 sm:flex-row sm:items-center sm:justify-between">
          <span>Track release maturity against roadmap evidence and governance review.</span>
          <div className="flex flex-wrap gap-3">
            <Link href="/docs/roadmap-status" className="inline-flex items-center gap-2 text-blue-300 hover:text-blue-200">
              <GitBranch className="h-4 w-4" />
              Roadmap Status
            </Link>
            <Link href="/docs/rfc-process" className="inline-flex items-center gap-2 text-blue-300 hover:text-blue-200">
              <FileCheck2 className="h-4 w-4" />
              RFC / ZEP
            </Link>
          </div>
        </CardContent>
      </Card>
    </div>
  );
}
