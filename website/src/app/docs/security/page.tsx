import Image from 'next/image';
import { ShieldAlert, Key, Lock, Users, FileSignature, CheckCircle } from 'lucide-react';
import { Card, CardHeader, CardTitle, CardContent, CardDescription } from "@/components/ui/card";
import { Alert, AlertTitle, AlertDescription } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";

export default function SecurityPage() {
  return (
    <div className="space-y-8 font-sans">
      <div>
        <h1 className="text-3xl md:text-4xl font-extrabold tracking-tight mb-2 text-white">ZAP Security Model</h1>
        <p className="text-zinc-400 text-lg">ZAP treats cryptographic identity, transport confidentiality, and sandbox execution isolation as independent, separate layers.</p>
      </div>

      {/* Hero Visual Card */}
      <Card className="group bg-zinc-950/40 hover:bg-zinc-950/60 border border-zinc-850 hover:border-purple-500/20 transition-all duration-500 overflow-hidden rounded-2xl shadow-xl hover:shadow-2xl hover:shadow-purple-500/5 not-prose p-0">
        <div className="grid grid-cols-1 md:grid-cols-12 items-stretch">
          {/* Image Column */}
          <div className="md:col-span-5 relative bg-gradient-to-br from-zinc-950 to-black/80 flex items-center justify-center p-6 border-b md:border-b-0 md:border-r border-zinc-900 overflow-hidden min-h-[260px] md:min-h-0">
            {/* Ambient Glow Effects */}
            <div className="absolute inset-0 bg-[radial-gradient(circle_at_center,rgba(99,102,241,0.08)_0%,transparent_70%)] pointer-events-none" />
            <div className="absolute -inset-10 bg-[radial-gradient(circle_at_bottom_left,rgba(236,72,153,0.04)_0%,transparent_50%)] pointer-events-none" />
            
            {/* Image Wrapper */}
            <div className="relative w-full aspect-square max-w-[220px] md:max-w-[240px] transform group-hover:scale-[1.03] transition-transform duration-500 ease-out">
              <Image 
                src="/images/zap_security_model.png" 
                alt="Cryptographic Security Model Visual" 
                fill
                style={{ objectFit: 'contain' }}
                className="opacity-90 group-hover:opacity-100 transition-opacity duration-500"
                priority
              />
            </div>
          </div>
          {/* Content Column */}
          <div className="md:col-span-7 p-6 md:p-8 flex flex-col justify-center space-y-3 bg-zinc-950/20">
            <span className="text-xs font-semibold uppercase tracking-wider text-purple-400 block">Security Hierarchy</span>
            <h3 className="text-xl font-bold text-white tracking-tight">Zero-Trust Security Infrastructure</h3>
            <p className="text-sm text-zinc-400 leading-relaxed">
              ZAP assumes a compromised transport channel. Rather than relying on boundary firewalls, every payload is individually encrypted, signed, and gated by validator consensus contracts.
            </p>
          </div>
        </div>
      </Card>

      <div className="grid grid-cols-1 md:grid-cols-2 gap-6">
        {/* Node Identity */}
        <Card className="bg-zinc-950/40 border-zinc-850">
          <CardHeader>
            <div className="flex items-center gap-3">
              <div className="w-8 h-8 rounded-lg bg-zinc-900 border border-zinc-800 flex items-center justify-center">
                <Key className="w-4 h-4 text-blue-400" />
              </div>
              <div>
                <CardTitle className="text-white text-base">1. Node Identity</CardTitle>
                <CardDescription className="text-xs">Ed25519-derived cryptographic UUIDs</CardDescription>
              </div>
            </div>
          </CardHeader>
          <CardContent className="space-y-3 text-sm text-zinc-400">
            <p>
              Each node generates an Ed25519 identity key. The public key is hashed using BLAKE3 to derive a unique UUID v8.
            </p>
            <div className="p-3.5 rounded-lg bg-[#050505] border border-zinc-900 text-xs font-mono text-zinc-500">
              <span className="text-blue-400">ZAP_SIGN:</span> Fast 8-byte signature validation hint in the wire header to reject DoS traffic early.
            </div>
          </CardContent>
        </Card>

        {/* Encrypted Transport */}
        <Card className="bg-zinc-950/40 border-zinc-850">
          <CardHeader>
            <div className="flex items-center gap-3">
              <div className="w-8 h-8 rounded-lg bg-zinc-900 border border-zinc-800 flex items-center justify-center">
                <Lock className="w-4 h-4 text-purple-400" />
              </div>
              <div>
                <CardTitle className="text-white text-base">2. Encrypted Transport</CardTitle>
                <CardDescription className="text-xs">ChaCha20-Poly1305 AEAD over UDP</CardDescription>
              </div>
            </div>
          </CardHeader>
          <CardContent className="space-y-3 text-sm text-zinc-400">
            <p>
              Nonces are 96-bit (32-bit random bound + 64-bit counter) to prevent replay vulnerabilities across process lifecycles.
            </p>
            <div className="flex justify-between items-center p-3 rounded-lg bg-[#050505] border border-zinc-900 text-xs">
              <span className="text-zinc-300 font-semibold">Replay Cache Cap</span>
              <Badge className="bg-zinc-900 border-zinc-800 text-purple-400 text-[10px]">security.replay_cache_capacity</Badge>
            </div>
          </CardContent>
        </Card>
      </div>

      <div className="grid grid-cols-1 md:grid-cols-2 gap-6">
        {/* Peer Trust Contracts */}
        <Card className="bg-zinc-950/40 border-zinc-850">
          <CardHeader>
            <div className="flex items-center gap-3">
              <div className="w-8 h-8 rounded-lg bg-zinc-900 border border-zinc-800 flex items-center justify-center">
                <Users className="w-4 h-4 text-blue-400" />
              </div>
              <div>
                <CardTitle className="text-white text-base">3. Peer Trust Contracts</CardTitle>
                <CardDescription className="text-xs">Granular authorization boundaries</CardDescription>
              </div>
            </div>
          </CardHeader>
          <CardContent className="space-y-4">
            <p className="text-xs text-zinc-400">
              Decrypting transport packages does not authorize actions. Peer relations carry explicit trust configs:
            </p>
            <pre className="text-xs bg-[#050505] p-3.5 rounded-lg border border-zinc-900 font-mono text-zinc-350 overflow-x-auto">
              <code>{`[peers.trust]
status = "trusted"
allow_send = true
allow_receive = true
allow_forward = false
allow_poa_attestation = true`}</code>
            </pre>
          </CardContent>
        </Card>

        {/* PoA Consensus */}
        <Card className="bg-zinc-950/40 border-zinc-850">
          <CardHeader>
            <div className="flex items-center gap-3">
              <div className="w-8 h-8 rounded-lg bg-zinc-900 border border-zinc-800 flex items-center justify-center">
                <FileSignature className="w-4 h-4 text-amber-400" />
              </div>
              <div>
                <CardTitle className="text-white text-base">4. Proof-of-Action Consensus</CardTitle>
                <CardDescription className="text-xs">Cryptographic validator quorum</CardDescription>
              </div>
            </div>
          </CardHeader>
          <CardContent className="space-y-4">
            <p className="text-xs text-zinc-400">
              High-risk triggers carry attested PoA certificates verified against validator threshold limits:
            </p>
            <pre className="text-xs bg-[#050505] p-3.5 rounded-lg border border-zinc-900 font-mono text-zinc-350 overflow-x-auto">
              <code>{`[poa]
required_threshold = 2
validator_set = "poa-validators.v4.json"
validator_set_authority = "operator-pubkey"`}</code>
            </pre>
          </CardContent>
        </Card>
      </div>

      <Card className="bg-zinc-950/40 border-zinc-850">
        <CardHeader>
          <div className="flex items-center gap-3">
            <div className="w-8 h-8 rounded-lg bg-zinc-900 border border-zinc-800 flex items-center justify-center">
              <CheckCircle className="w-4 h-4 text-emerald-400" />
            </div>
            <div>
              <CardTitle className="text-white text-base">5. PACT Signed Action Records</CardTitle>
              <CardDescription className="text-xs">Intent, consent, proof, terms, revocation, and offline verification</CardDescription>
            </div>
          </div>
        </CardHeader>
        <CardContent className="space-y-3 text-sm text-zinc-400">
          <p>
            PACT records are <code>application/zap-pact+json</code> payloads carried in <code>ZENV</code>. They reuse ZAP BLAKE3 hashing and Ed25519 domain signatures instead of introducing a parallel trust stack.
          </p>
          <p className="text-xs text-zinc-500">
            Mutable fields such as status, signatures, verification results, revocation evidence, and timeline entries are excluded from the canonical signing payload.
          </p>
        </CardContent>
      </Card>
    </div>
  );
}
