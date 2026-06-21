import { FileSignature, ShieldCheck, PackageCheck, RotateCcw } from "lucide-react";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";

const subjects = [
  ["zap.pact.record", "action", "Portable signed action record"],
  ["zap.pact.verify", "control", "Offline verification exchange"],
  ["zap.pact.revoke", "control", "Signed revocation evidence"],
  ["zap.pact.bundle", "control", "Portable bundle exchange"],
];

const fields = [
  "pact_id",
  "actor",
  "target",
  "intent",
  "object",
  "terms",
  "consent",
  "proof",
  "created_at_micros",
  "expires_at_micros",
];

export default function PactPage() {
  return (
    <div className="space-y-8 font-sans">
      <div>
        <h1 className="text-3xl md:text-4xl font-extrabold tracking-tight mb-2 text-white">ZAP PACT Profile</h1>
        <p className="text-zinc-400 text-lg">Portable signed action records carried inside native ZAP envelopes.</p>
      </div>

      <Card className="bg-zinc-950/40 border-zinc-850">
        <CardHeader>
          <div className="flex items-center gap-3">
            <div className="w-8 h-8 rounded-lg bg-zinc-900 border border-zinc-800 flex items-center justify-center">
              <FileSignature className="w-4 h-4 text-blue-400" />
            </div>
            <div>
              <CardTitle className="text-white text-base">Native Protocol Profile</CardTitle>
              <CardDescription className="text-xs">No parallel API, database, ledger, or signature stack</CardDescription>
            </div>
          </div>
        </CardHeader>
        <CardContent className="space-y-3 text-sm text-zinc-400">
          <p>
            A PACT record captures identity, intent, target, consent, proof, terms, status, revocation evidence, and offline verification metadata.
            It travels as <code>application/zap-pact+json</code> inside <code>ZENV</code>.
          </p>
          <p>
            PACT hashing uses BLAKE3 and signatures reuse the existing ZAP Ed25519 domain-message transcript with domain <code>ZAP-PACT-v1</code>.
          </p>
        </CardContent>
      </Card>

      <div className="grid grid-cols-1 md:grid-cols-2 gap-6">
        <Card className="bg-zinc-950/40 border-zinc-850">
          <CardHeader>
            <CardTitle className="text-white text-base">Subjects</CardTitle>
            <CardDescription className="text-xs">Record is action; verification, revoke, and bundle are control</CardDescription>
          </CardHeader>
          <CardContent className="space-y-3">
            {subjects.map(([subject, kind, purpose]) => (
              <div key={subject} className="flex items-start justify-between gap-3 border-b border-zinc-900 pb-3 last:border-0 last:pb-0">
                <div>
                  <div className="font-mono text-xs text-zinc-200">{subject}</div>
                  <div className="text-xs text-zinc-500">{purpose}</div>
                </div>
                <Badge variant="outline" className="text-[10px] bg-zinc-900 border-zinc-800 text-zinc-300">{kind}</Badge>
              </div>
            ))}
          </CardContent>
        </Card>

        <Card className="bg-zinc-950/40 border-zinc-850">
          <CardHeader>
            <div className="flex items-center gap-3">
              <ShieldCheck className="w-4 h-4 text-emerald-400" />
              <div>
                <CardTitle className="text-white text-base">Canonical Payload</CardTitle>
                <CardDescription className="text-xs">Fixed ordered fields only</CardDescription>
              </div>
            </div>
          </CardHeader>
          <CardContent className="grid grid-cols-2 gap-2">
            {fields.map((field) => (
              <code key={field} className="text-[11px] bg-zinc-950 border border-zinc-900 rounded px-2 py-1 text-zinc-300">
                {field}
              </code>
            ))}
          </CardContent>
        </Card>
      </div>

      <Card className="bg-zinc-950/40 border-zinc-850">
        <CardHeader>
          <div className="flex items-center gap-3">
            <PackageCheck className="w-4 h-4 text-purple-400" />
            <div>
              <CardTitle className="text-white text-base">CLI Workflow</CardTitle>
              <CardDescription className="text-xs">Create, sign, verify, revoke, and bundle offline</CardDescription>
            </div>
          </div>
        </CardHeader>
        <CardContent>
          <pre className="text-xs bg-[#050505] p-3 rounded-lg border border-zinc-900 font-mono text-zinc-350 overflow-x-auto">
            <code>{`zap pact create --actor agent.alpha --target driver.valve --intent valve.open --out pact-unsigned.json
zap pact sign --input pact-unsigned.json --key .zap/node.key --out pact-signed.json
zap pact verify --input pact-signed.json --json
zap pact revoke --input pact-signed.json --revoked-by ops.lead --reason "operator stop" --key .zap/node.key --out pact-revoked.json
zap pact bundle export --pact pact-signed.json --out pact-bundle.json
zap pact bundle verify --bundle pact-bundle.json --json`}</code>
          </pre>
        </CardContent>
      </Card>

      <Card className="bg-zinc-950/40 border-zinc-850">
        <CardHeader>
          <div className="flex items-center gap-3">
            <RotateCcw className="w-4 h-4 text-amber-400" />
            <div>
              <CardTitle className="text-white text-base">Receipts and SDKs</CardTitle>
              <CardDescription className="text-xs">Audit references and cross-language conformance</CardDescription>
            </div>
          </div>
        </CardHeader>
        <CardContent className="space-y-3 text-sm text-zinc-400">
          <p>
            When a node processes <code>zap.pact.record</code>, it verifies the PACT body before adding an optional PACT reference to the signed receipt.
          </p>
          <p>
            Shared fixtures cover <code>pact-record-v1.json</code>, <code>pact-bundle-v1.json</code>, and a signed PACT ZENV frame. Rust, TypeScript, Python, and Go SDKs load the same fixture and reproduce the same canonical hash.
          </p>
        </CardContent>
      </Card>
    </div>
  );
}
