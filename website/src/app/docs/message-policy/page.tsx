import { AlertTriangle, CheckCircle2, FileCode2, Lock, Route, ShieldCheck, Terminal, UserCheck } from 'lucide-react';
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";

const decisions = [
  ["allow", "Accepts the message and continues to routing or local dispatch.", "Use only for explicit low-risk traffic."],
  ["deny", "Rejects before routing, forwarding, or driver execution.", "Use for blocked subjects and production defaults."],
  ["require_poa", "Requires a consensus-protected frame with a valid Proof-of-Action certificate.", "Use for safety, money movement, physical control, and critical automation."],
  ["require_grant", "Requires the configured capability grant before the message can continue.", "Set required_capability on the rule."],
  ["require_human", "Operator label for a human approval gate.", "Current TOML token is human_approval."],
  ["require_simulation", "Operator label for a successful simulation gate.", "Current TOML token is simulate_first."],
];

const ruleFields = [
  ["kind", "Optional exact message kind such as action or data."],
  ["subject", "Optional subject match. Supports * and suffix wildcards such as safety.*."],
  ["source_node", "Optional source node UUID match."],
  ["target_node", "Optional target node UUID match."],
  ["content_type", "Optional exact content type match for typed payloads."],
  ["reason", "Operator-readable explanation returned in evaluation output."],
];

const checks = [
  "Set message_policy.default_decision = \"deny\" on production receivers.",
  "Add explicit allow, require_poa, require_grant, human_approval, or simulate_first rules for expected traffic.",
  "Run policy dry-runs with --strict before deploying a new policy file.",
  "Use zap doctor --strict --json to surface allow-by-default configs during readiness checks.",
];

export default function MessagePolicyPage() {
  return (
    <div className="space-y-8 font-sans">
      <div>
        <h1 className="text-3xl md:text-4xl font-extrabold tracking-tight mb-2 text-white">Message Policy</h1>
        <p className="text-zinc-400 text-lg">Deterministic receiver-side gates for typed ZAP messages before routing, forwarding, or driver execution.</p>
      </div>

      <Alert className="border-amber-500/20 bg-amber-500/5 text-amber-300">
        <AlertTriangle className="w-4 h-4 text-amber-400" />
        <AlertTitle className="font-semibold text-amber-400">Production Defaults Fail Closed</AlertTitle>
        <AlertDescription className="text-xs">
          Omitted configs default to <code>allow</code> for backward compatibility. Production receivers should set <code>message_policy.default_decision = &quot;deny&quot;</code> and describe every expected path with explicit rules.
        </AlertDescription>
      </Alert>

      <Card className="group bg-zinc-950/40 hover:bg-zinc-950/60 border border-zinc-850 hover:border-amber-500/20 transition-all duration-500 overflow-hidden rounded-2xl shadow-xl hover:shadow-2xl hover:shadow-amber-500/5 not-prose p-0">
        <div className="grid grid-cols-1 md:grid-cols-12 items-stretch">
          <div className="md:col-span-5 relative bg-gradient-to-br from-zinc-950 to-black/80 flex items-center justify-center p-6 border-b md:border-b-0 md:border-r border-zinc-900 overflow-hidden min-h-[260px] md:min-h-0">
            <div className="absolute inset-0 bg-[radial-gradient(circle_at_center,rgba(245,158,11,0.08)_0%,transparent_70%)] pointer-events-none" />
            <div className="absolute -inset-10 bg-[radial-gradient(circle_at_bottom_left,rgba(34,197,94,0.04)_0%,transparent_50%)] pointer-events-none" />
            <div className="relative w-full max-w-[300px] space-y-3">
              {["default deny", "safety.* require_poa", "debug.* deny"].map((rule, index) => (
                <div key={rule} className="flex items-center justify-between rounded-xl border border-zinc-800 bg-black/40 p-3 shadow-lg shadow-black/20">
                  <div className="flex items-center gap-3">
                    <div className="h-9 w-9 rounded-lg bg-zinc-900 border border-zinc-800 flex items-center justify-center">
                      {index === 0 ? <Lock className="h-4 w-4 text-amber-400" /> : <ShieldCheck className="h-4 w-4 text-emerald-400" />}
                    </div>
                    <p className="font-mono text-xs text-zinc-200">{rule}</p>
                  </div>
                  <Badge variant="outline" className="font-mono text-[10px] bg-zinc-900 text-zinc-400 border-zinc-850">rule</Badge>
                </div>
              ))}
            </div>
          </div>
          <div className="md:col-span-7 p-6 md:p-8 flex flex-col justify-center space-y-3 bg-zinc-950/20">
            <span className="text-xs font-semibold uppercase tracking-wider text-amber-400 block">Receiver Safety Boundary</span>
            <h3 className="text-xl font-bold text-white tracking-tight">Typed Messages, Deterministic Gates</h3>
            <p className="text-sm text-zinc-400 leading-relaxed">
              AI planners, gateways, and operators can propose typed actions, but ZAP evaluates kind, subject, nodes, content type, grants, PoA, human approval, and simulation evidence before the message continues.
            </p>
          </div>
        </div>
      </Card>

      <div className="grid grid-cols-1 md:grid-cols-2 gap-6">
        <Card className="bg-zinc-950/40 border-zinc-850">
          <CardHeader>
            <div className="flex items-center gap-3">
              <div className="w-8 h-8 rounded-lg bg-zinc-900 border border-zinc-800 flex items-center justify-center">
                <Lock className="w-4 h-4 text-amber-400" />
              </div>
              <div>
                <CardTitle className="text-white text-base">Default Decision</CardTitle>
                <CardDescription className="text-xs">Fallback behavior when no rule matches</CardDescription>
              </div>
            </div>
          </CardHeader>
          <CardContent className="space-y-3 text-sm text-zinc-400">
            <p><code>default_decision = &quot;allow&quot;</code> accepts unmatched messages. It preserves existing behavior and is useful for local migration or controlled development.</p>
            <p><code>default_decision = &quot;deny&quot;</code> rejects unmatched messages. It is the expected production posture for fail-closed receivers.</p>
          </CardContent>
        </Card>

        <Card className="bg-zinc-950/40 border-zinc-850">
          <CardHeader>
            <div className="flex items-center gap-3">
              <div className="w-8 h-8 rounded-lg bg-zinc-900 border border-zinc-800 flex items-center justify-center">
                <Route className="w-4 h-4 text-blue-400" />
              </div>
              <div>
                <CardTitle className="text-white text-base">Rule Matching</CardTitle>
                <CardDescription className="text-xs">Rules are evaluated in file order</CardDescription>
              </div>
            </div>
          </CardHeader>
          <CardContent className="space-y-2">
            {ruleFields.map(([field, description]) => (
              <div key={field} className="flex items-start gap-2 text-xs text-zinc-400">
                <CheckCircle2 className="mt-0.5 h-4 w-4 flex-shrink-0 text-emerald-400" />
                <span><code>{field}</code>: {description}</span>
              </div>
            ))}
          </CardContent>
        </Card>
      </div>

      <Card className="bg-zinc-950/40 border-zinc-850 overflow-hidden">
        <CardHeader>
          <div className="flex items-center gap-3">
            <div className="w-8 h-8 rounded-lg bg-zinc-900 border border-zinc-800 flex items-center justify-center">
              <ShieldCheck className="w-4 h-4 text-emerald-400" />
            </div>
            <div>
              <CardTitle className="text-white text-base">Rule Decisions</CardTitle>
              <CardDescription className="text-xs">What a matched rule requires before the message can continue</CardDescription>
            </div>
          </div>
        </CardHeader>
        <CardContent className="p-0">
          <Table>
            <TableHeader className="bg-zinc-900/40 border-zinc-850">
              <TableRow>
                <TableHead className="px-6 py-3 text-zinc-400 font-semibold text-xs">Decision</TableHead>
                <TableHead className="px-6 py-3 text-zinc-400 font-semibold text-xs">Effect</TableHead>
                <TableHead className="px-6 py-3 text-zinc-400 font-semibold text-xs">Operator Note</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody className="divide-y divide-zinc-900">
              {decisions.map(([decision, effect, note]) => (
                <TableRow key={decision} className="hover:bg-zinc-950/20">
                  <TableCell className="px-6 py-4 font-mono text-xs text-zinc-300">{decision}</TableCell>
                  <TableCell className="px-6 py-4 text-xs text-zinc-400">{effect}</TableCell>
                  <TableCell className="px-6 py-4 text-xs text-zinc-500">{note}</TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        </CardContent>
      </Card>

      <Card className="bg-zinc-950/40 border-zinc-850">
        <CardHeader>
          <div className="flex items-center gap-3">
            <div className="w-8 h-8 rounded-lg bg-zinc-900 border border-zinc-800 flex items-center justify-center">
              <FileCode2 className="w-4 h-4 text-cyan-400" />
            </div>
            <div>
              <CardTitle className="text-white text-base">Production TOML</CardTitle>
              <CardDescription className="text-xs">Fail-closed baseline with explicit gates</CardDescription>
            </div>
          </div>
        </CardHeader>
        <CardContent>
          <pre className="text-xs bg-[#050505] p-4 rounded-lg border border-zinc-900 font-mono text-zinc-300 overflow-x-auto">
            <code>{`[message_policy]
default_decision = "deny"

[[message_policy.rules]]
name = "read telemetry"
kind = "data"
subject = "telemetry.*"
decision = "allow"
reason = "read-only telemetry is allowed"

[[message_policy.rules]]
name = "safety quorum"
kind = "action"
subject = "safety.*"
decision = "require_poa"
reason = "safety actions require validator quorum"

[[message_policy.rules]]
name = "driver grant"
kind = "action"
subject = "driver.execute"
decision = "require_grant"
required_capability = "driver.execute:echo"

[[message_policy.rules]]
name = "operator approval"
kind = "action"
subject = "ops.change.*"
decision = "human_approval"

[[message_policy.rules]]
name = "simulation gate"
kind = "action"
subject = "building.hvac.*"
decision = "simulate_first"`}</code>
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
              <CardTitle className="text-white text-base">CLI Commands</CardTitle>
              <CardDescription className="text-xs">Dry-run policies and verify receiver readiness before rollout</CardDescription>
            </div>
          </div>
        </CardHeader>
        <CardContent className="space-y-4">
          <pre className="text-xs bg-[#050505] p-4 rounded-lg border border-zinc-900 font-mono text-zinc-300 overflow-x-auto">
            <code>{`cargo run -p zap-cli -- policy evaluate --policy policy.toml \
  --kind action --subject safety.emergency_stop \
  --requires-consensus --strict --json

cargo run -p zap-cli -- policy evaluate --policy policy.toml \
  --kind action --subject driver.execute \
  --grant driver.execute:echo --strict --json

cargo run -p zap-cli -- policy evaluate --policy policy.toml \
  --kind action --subject ops.change.restart \
  --human-approved --strict --json

cargo run -p zap-cli -- policy evaluate --policy policy.toml \
  --kind action --subject building.hvac.setpoint \
  --simulation-passed --strict --json

cargo run -p zap-cli -- doctor --config /etc/zap/zap.toml --strict --json`}</code>
          </pre>
          <div className="flex flex-wrap gap-2">
            <Badge variant="outline" className="font-mono text-[10px] bg-zinc-900 text-zinc-400 border-zinc-850">policy evaluate</Badge>
            <Badge variant="outline" className="font-mono text-[10px] bg-zinc-900 text-zinc-400 border-zinc-850">--strict</Badge>
            <Badge variant="outline" className="font-mono text-[10px] bg-zinc-900 text-zinc-400 border-zinc-850">--json</Badge>
            <Badge variant="outline" className="font-mono text-[10px] bg-zinc-900 text-zinc-400 border-zinc-850">doctor</Badge>
          </div>
        </CardContent>
      </Card>

      <Card className="bg-zinc-950/40 border-zinc-850">
        <CardHeader>
          <div className="flex items-center gap-3">
            <div className="w-8 h-8 rounded-lg bg-zinc-900 border border-zinc-800 flex items-center justify-center">
              <UserCheck className="w-4 h-4 text-purple-400" />
            </div>
            <div>
              <CardTitle className="text-white text-base">Rollout Checklist</CardTitle>
              <CardDescription className="text-xs">Minimum checks before accepting production actions</CardDescription>
            </div>
          </div>
        </CardHeader>
        <CardContent className="space-y-2">
          {checks.map((check) => (
            <div key={check} className="flex items-start gap-2 text-xs text-zinc-400">
              <CheckCircle2 className="mt-0.5 h-4 w-4 flex-shrink-0 text-emerald-400" />
              <span>{check}</span>
            </div>
          ))}
        </CardContent>
      </Card>
    </div>
  );
}
