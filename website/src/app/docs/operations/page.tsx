import { Terminal, ShieldCheck, Users, Send, Key } from 'lucide-react';
import { Card, CardHeader, CardTitle, CardContent, CardDescription } from "@/components/ui/card";
import { Alert, AlertTitle, AlertDescription } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";

export default function OperationsPage() {
  return (
    <div className="space-y-8 font-sans">
      <div>
        <h1 className="text-3xl md:text-4xl font-extrabold tracking-tight mb-2 text-white">CLI Operations Reference</h1>
        <p className="text-zinc-400 text-lg">A complete reference of CLI operator workflows for managing ZAP nodes, peer trust, and code execution.</p>
      </div>

      <Alert className="border-blue-500/20 bg-blue-500/5 text-blue-300">
        <Terminal className="w-4 h-4 text-blue-400" />
        <AlertTitle className="font-semibold text-blue-400">CLI Binary Path</AlertTitle>
        <AlertDescription className="text-xs">
          The operations commands below assume the compiled binary <code>zap</code> (or <code>zap-cli</code>) is located in your system&apos;s PATH.
        </AlertDescription>
      </Alert>

      {/* Node Identity */}
      <Card className="bg-zinc-950/40 border-zinc-850">
        <CardHeader>
          <div className="flex items-center gap-3">
            <div className="w-8 h-8 rounded-lg bg-zinc-900 border border-zinc-800 flex items-center justify-center">
              <Key className="w-4 h-4 text-blue-400" />
            </div>
            <div>
              <CardTitle className="text-white text-base">Node Identity & Integrity Checks</CardTitle>
              <CardDescription className="text-xs">Generate node keypairs and check configuration status</CardDescription>
            </div>
          </div>
        </CardHeader>
        <CardContent className="space-y-6">
          <div className="space-y-2">
            <div className="flex items-center justify-between">
              <span className="text-sm font-semibold text-zinc-300">Generate Identity Key</span>
              <Badge variant="outline" className="text-[10px] text-zinc-500 border-zinc-850">keygen</Badge>
            </div>
            <p className="text-xs text-zinc-500">Create a secure Ed25519 identity key file used for signing all node communications:</p>
            <pre className="text-xs bg-[#050505] p-3 rounded-lg border border-zinc-900 font-mono text-zinc-300">
              <code>zap keygen --out .zap/node.key</code>
            </pre>
          </div>

          <div className="space-y-2 pt-2 border-t border-zinc-900">
            <div className="flex items-center justify-between">
              <span className="text-sm font-semibold text-zinc-300">Config & Health Doctor</span>
              <Badge variant="outline" className="text-[10px] text-zinc-500 border-zinc-850">doctor</Badge>
            </div>
            <p className="text-xs text-zinc-500">Analyze configuration variables, compile schemas, and evaluate security scoring before starting daemons:</p>
            <pre className="text-xs bg-[#050505] p-3 rounded-lg border border-zinc-900 font-mono text-zinc-300">
              <code>{`# Run simple configuration check
zap check-config --strict --config zap.toml

# Run advanced node diagnostics
zap doctor --config zap.toml`}</code>
            </pre>
          </div>
        </CardContent>
      </Card>

      {/* Peer Onboarding */}
      <Card className="bg-zinc-950/40 border-zinc-850">
        <CardHeader>
          <div className="flex items-center gap-3">
            <div className="w-8 h-8 rounded-lg bg-zinc-900 border border-zinc-800 flex items-center justify-center">
              <Users className="w-4 h-4 text-purple-400" />
            </div>
            <div>
              <CardTitle className="text-white text-base">Peer Onboarding & Mesh Trust</CardTitle>
              <CardDescription className="text-xs">Manage trusted connections in the brokerless grid</CardDescription>
            </div>
          </div>
        </CardHeader>
        <CardContent className="space-y-6">
          <div className="space-y-2">
            <div className="flex items-center justify-between">
              <span className="text-sm font-semibold text-zinc-300">Generate Trust Invitation</span>
              <Badge variant="outline" className="text-[10px] text-zinc-500 border-zinc-850">peer invite</Badge>
            </div>
            <p className="text-xs text-zinc-500">Compile a signed trust advertisement invitation to welcome remote peer endpoints:</p>
            <pre className="text-xs bg-[#050505] p-3 rounded-lg border border-zinc-900 font-mono text-zinc-300 overflow-x-auto">
              <code>{`zap peer invite \\
  --node-id <peer-node-id> \\
  --addr 10.0.0.12:7777 \\
  --public-key <peer-public-key> \\
  --transport-key <64-hex-chars> \\
  --out .zap/peer-invite.toml`}</code>
            </pre>
          </div>

          <div className="space-y-2 pt-2 border-t border-zinc-900">
            <div className="flex items-center justify-between">
              <span className="text-sm font-semibold text-zinc-300">Accept Peer Trust Connection</span>
              <Badge variant="outline" className="text-[10px] text-zinc-550 border-zinc-850">peer accept</Badge>
            </div>
            <p className="text-xs text-zinc-500">Import an onboarding invitation, verify signatures, and register the remote peer in local trust files:</p>
            <pre className="text-xs bg-[#050505] p-3 rounded-lg border border-zinc-900 font-mono text-zinc-300">
              <code>zap peer accept --config zap.toml --invite .zap/peer-invite.toml</code>
            </pre>
          </div>
        </CardContent>
      </Card>

      {/* Action Dispatching */}
      <Card className="bg-zinc-950/40 border-zinc-850">
        <CardHeader>
          <div className="flex items-center gap-3">
            <div className="w-8 h-8 rounded-lg bg-zinc-900 border border-zinc-800 flex items-center justify-center">
              <Send className="w-4 h-4 text-emerald-400" />
            </div>
            <div>
              <CardTitle className="text-white text-base">Action Dispatching</CardTitle>
              <CardDescription className="text-xs">Submit payload packets directly onto the grid</CardDescription>
            </div>
          </div>
        </CardHeader>
        <CardContent className="space-y-3">
          <p className="text-sm text-zinc-400">
            Submit cryptographically signed event messages and commands to specific targets over the encrypted UDP mesh:
          </p>
          <pre className="text-xs bg-[#050505] p-4 rounded-lg border border-zinc-900 font-mono text-zinc-300 overflow-x-auto">
            <code>{`zap send \\
  --config zap.toml \\
  --target <peer-uuid> \\
  --kind action \\
  --subject thermostat.setpoint \\
  --payload '{"value": 22.5}'`}</code>
          </pre>
        </CardContent>
      </Card>
    </div>
  );
}
