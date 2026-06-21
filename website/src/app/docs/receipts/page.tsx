import Image from 'next/image';
import { Database, ShieldCheck, History } from 'lucide-react';
import { Card, CardHeader, CardTitle, CardContent, CardDescription } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";

export default function ReceiptsPage() {
  return (
    <div className="space-y-8 font-sans">
      <div>
        <h1 className="text-3xl md:text-4xl font-extrabold tracking-tight mb-2 text-white">Auditing & Signed Receipts</h1>
        <p className="text-zinc-400 text-lg">Create cryptographically auditable, tamper-evident logs of all processed action transactions.</p>
      </div>

      {/* Hero Visual Card */}
      <Card className="group bg-zinc-950/40 hover:bg-zinc-950/60 border border-zinc-850 hover:border-emerald-500/20 transition-all duration-500 overflow-hidden rounded-2xl shadow-xl hover:shadow-2xl hover:shadow-emerald-500/5 not-prose p-0">
        <div className="grid grid-cols-1 md:grid-cols-12 items-stretch">
          {/* Image Column */}
          <div className="md:col-span-5 relative bg-gradient-to-br from-zinc-950 to-black/80 flex items-center justify-center p-6 border-b md:border-b-0 md:border-r border-zinc-900 overflow-hidden min-h-[260px] md:min-h-0">
            {/* Ambient Glow Effects */}
            <div className="absolute inset-0 bg-[radial-gradient(circle_at_center,rgba(16,185,129,0.08)_0%,transparent_70%)] pointer-events-none" />
            <div className="absolute -inset-10 bg-[radial-gradient(circle_at_bottom_left,rgba(59,130,246,0.04)_0%,transparent_50%)] pointer-events-none" />
            
            {/* Image Wrapper */}
            <div className="relative w-full aspect-square max-w-[220px] md:max-w-[240px] transform group-hover:scale-[1.03] transition-transform duration-500 ease-out">
              <Image 
                src="/images/zap_receipt_chain.png" 
                alt="ZAP Cryptographic Receipt Chain Visual" 
                fill
                style={{ objectFit: 'contain' }}
                className="opacity-90 group-hover:opacity-100 transition-opacity duration-500"
                priority
              />
            </div>
          </div>
          {/* Content Column */}
          <div className="md:col-span-7 p-6 md:p-8 flex flex-col justify-center space-y-3 bg-zinc-950/20">
            <span className="text-xs font-semibold uppercase tracking-wider text-emerald-400 block">Audit Ledger</span>
            <h3 className="text-xl font-bold text-white tracking-tight">Hash-Chained Execution Receipts</h3>
            <p className="text-sm text-zinc-400 leading-relaxed">
              Every action execution appends a signed receipt to a local tamper-evident log. Receipts embed base64-encoded identities, execution hashes, and Proof-of-Action validator threshold consensus chains.
            </p>
          </div>
        </div>
      </Card>

      <div className="grid grid-cols-1 md:grid-cols-2 gap-6">
        {/* Audit Log Configuration Card */}
        <Card className="bg-zinc-950/40 border-zinc-850">
          <CardHeader>
            <div className="flex items-center gap-3">
              <div className="w-8 h-8 rounded-lg bg-zinc-900 border border-zinc-800 flex items-center justify-center">
                <Database className="w-4 h-4 text-emerald-400" />
              </div>
              <div>
                <CardTitle className="text-white text-base">Audit Journal Configuration</CardTitle>
                <CardDescription className="text-xs">Configure local action receipt journaling</CardDescription>
              </div>
            </div>
          </CardHeader>
          <CardContent className="space-y-3">
            <p className="text-xs text-zinc-400">
              Configure ZAP to append a signed receipt for each processed action envelope to the binary receipt journal:
            </p>
            <pre className="text-xs bg-[#050505] p-3.5 rounded-lg border border-zinc-900 font-mono text-zinc-350">
              <code>{`[receipts]
dir = "logs/receipts"`}</code>
            </pre>
          </CardContent>
        </Card>

        {/* Receipt Payload Schema Card */}
        <Card className="bg-zinc-950/40 border-zinc-850">
          <CardHeader>
            <div className="flex items-center gap-3">
              <div className="w-8 h-8 rounded-lg bg-zinc-900 border border-zinc-800 flex items-center justify-center">
                <ShieldCheck className="w-4 h-4 text-blue-400" />
              </div>
              <div>
                <CardTitle className="text-white text-base">Receipt Payload Schema</CardTitle>
                <CardDescription className="text-xs">Immutable signed receipt fields</CardDescription>
              </div>
            </div>
          </CardHeader>
          <CardContent className="space-y-3 text-xs text-zinc-400">
            <p>Each action execution appends a single binary journal record containing:</p>
            <ul className="space-y-1.5 list-disc pl-5 text-zinc-300">
              <li><strong>Identity Context:</strong> Source, target, and processing node IDs.</li>
              <li><strong>Action Details:</strong> Subject namespace, parent, and correlation IDs.</li>
              <li><strong>Payload Hash:</strong> Cryptographic BLAKE3 digest of headers and payload.</li>
              <li><strong>PoA Consensus Summary:</strong> Signatures validating quorum authority.</li>
              <li><strong>PACT Reference:</strong> Optional PACT id, intent, status, and canonical hash for verified <code>zap.pact.record</code> messages.</li>
              <li><strong>Signature:</strong> Ed25519 signature over <code>ZAP-ACTION-RECEIPT-v1</code>.</li>
            </ul>
          </CardContent>
        </Card>
      </div>

      {/* CLI Management Card */}
      <Card className="bg-zinc-950/40 border-zinc-850">
        <CardHeader>
          <div className="flex items-center gap-3">
            <div className="w-8 h-8 rounded-lg bg-zinc-900 border border-zinc-800 flex items-center justify-center">
              <History className="w-4 h-4 text-purple-400" />
            </div>
            <div>
              <CardTitle className="text-white text-base">CLI Management & Journal Auditing</CardTitle>
              <CardDescription className="text-xs">Commands to verify, compact, and pull peer journals</CardDescription>
            </div>
          </div>
        </CardHeader>
        <CardContent className="space-y-6">
          <div className="space-y-2">
            <div className="flex justify-between items-center">
              <span className="text-xs font-semibold text-zinc-300">Verify Journal Integrity</span>
              <Badge variant="outline" className="text-[9px] bg-zinc-900 text-zinc-400 border-zinc-850">receipts verify</Badge>
            </div>
            <pre className="text-xs bg-[#050505] p-3 rounded-lg border border-zinc-900 font-mono text-zinc-350 overflow-x-auto">
              <code>zap receipts verify --dir logs/receipts</code>
            </pre>
          </div>

          <div className="space-y-2 pt-4 border-t border-zinc-900">
            <div className="flex justify-between items-center">
              <span className="text-xs font-semibold text-zinc-300">Compact Journal Safely</span>
              <Badge variant="outline" className="text-[9px] bg-zinc-900 text-zinc-400 border-zinc-850">receipts compact</Badge>
            </div>
            <pre className="text-xs bg-[#050505] p-3 rounded-lg border border-zinc-900 font-mono text-zinc-350 overflow-x-auto">
              <code>{`zap receipts compact \\
  --dir logs/receipts \\
  --out logs/receipts.compacted`}</code>
            </pre>
          </div>

          <div className="space-y-2 pt-4 border-t border-zinc-900">
            <div className="flex justify-between items-center">
              <span className="text-xs font-semibold text-zinc-300">Pull Peer Journal</span>
              <Badge variant="outline" className="text-[9px] bg-zinc-900 text-zinc-400 border-zinc-850">receipts pull</Badge>
            </div>
            <pre className="text-xs bg-[#050505] p-3 rounded-lg border border-zinc-900 font-mono text-zinc-350 overflow-x-auto">
              <code>{`zap receipts pull \\
  --config zap.toml \\
  --target <peer-id> \\
  --limit 100 \\
  --out-dir logs/peer-receipts`}</code>
            </pre>
          </div>
        </CardContent>
      </Card>
    </div>
  );
}
