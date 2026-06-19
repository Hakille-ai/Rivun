import { AlertCircle, Bot, CheckCircle2, FileJson, GitBranch, MessageSquare, Route, Terminal } from 'lucide-react';
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";

const subjects = [
  ["zap.agent.intent", "AgentMessage::Intent"],
  ["zap.agent.session", "AgentMessage::Session"],
  ["zap.agent.delegation.request", "AgentMessage::DelegationRequest"],
  ["zap.agent.delegation.response", "AgentMessage::DelegationResponse"],
  ["zap.agent.capability_negotiation.request", "AgentMessage::CapabilityNegotiationRequest"],
  ["zap.agent.capability_negotiation.response", "AgentMessage::CapabilityNegotiationResponse"],
  ["zap.agent.status", "AgentMessage::Status"],
  ["zap.agent.result", "AgentMessage::Result"],
  ["zap.agent.error", "AgentMessage::Error"],
];

const validationRules = [
  "schema_version must be 1",
  "protocol UUIDs must not be nil",
  "agent IDs and error codes must use lowercase protocol tokens",
  "capability negotiations cannot be empty",
  "result status must be terminal",
  "failed results must include structured error details",
];

export default function AgentProtocolPage() {
  return (
    <div className="space-y-8 font-sans">
      <div>
        <h1 className="text-3xl md:text-4xl font-extrabold tracking-tight mb-2 text-white">ZAP Agent Protocol</h1>
        <p className="text-zinc-400 text-lg">A high-level JSON contract for model gateways, planners, tools, and operator agents traveling inside ZENV envelopes.</p>
      </div>

      <Card className="group bg-zinc-950/40 hover:bg-zinc-950/60 border border-zinc-850 hover:border-cyan-500/20 transition-all duration-500 overflow-hidden rounded-2xl shadow-xl hover:shadow-2xl hover:shadow-cyan-500/5 not-prose p-0">
        <div className="grid grid-cols-1 md:grid-cols-12 items-stretch">
          <div className="md:col-span-5 relative bg-gradient-to-br from-zinc-950 to-black/80 flex items-center justify-center p-6 border-b md:border-b-0 md:border-r border-zinc-900 overflow-hidden min-h-[260px] md:min-h-0">
            <div className="absolute inset-0 bg-[radial-gradient(circle_at_center,rgba(34,211,238,0.08)_0%,transparent_70%)] pointer-events-none" />
            <div className="absolute -inset-10 bg-[radial-gradient(circle_at_bottom_left,rgba(59,130,246,0.04)_0%,transparent_50%)] pointer-events-none" />
            <div className="relative w-full max-w-[280px] space-y-3">
              {["planner.main", "executor.safety", "operator.console"].map((agent, index) => (
                <div key={agent} className="flex items-center gap-3 rounded-xl border border-zinc-800 bg-black/40 p-3 shadow-lg shadow-black/20">
                  <div className="h-9 w-9 rounded-lg bg-zinc-900 border border-zinc-800 flex items-center justify-center">
                    <Bot className={`h-4 w-4 ${index === 1 ? "text-cyan-400" : "text-blue-400"}`} />
                  </div>
                  <div>
                    <p className="font-mono text-xs text-zinc-200">{agent}</p>
                    <p className="text-[10px] text-zinc-500">schema_version = 1</p>
                  </div>
                </div>
              ))}
            </div>
          </div>
          <div className="md:col-span-7 p-6 md:p-8 flex flex-col justify-center space-y-3 bg-zinc-950/20">
            <span className="text-xs font-semibold uppercase tracking-wider text-cyan-400 block">Agent Message Layer</span>
            <h3 className="text-xl font-bold text-white tracking-tight">Intent Before Execution</h3>
            <p className="text-sm text-zinc-400 leading-relaxed">
              Agent messages express machine-readable intent, sessions, delegation, capability negotiation, status, terminal results, and structured errors before policy evaluation or driver dispatch.
            </p>
          </div>
        </div>
      </Card>

      <Alert className="border-blue-500/20 bg-blue-500/5 text-blue-300">
        <FileJson className="w-4 h-4 text-blue-400" />
        <AlertTitle className="font-semibold text-blue-400">Envelope Content Type</AlertTitle>
        <AlertDescription className="text-xs">
          Agent payloads are designed for <code>ZENV</code> envelopes with <code>content_type = application/zap-agent+json</code>. They do not change the wire frame, transport, signatures, PoA, policy, or runtime layers.
        </AlertDescription>
      </Alert>

      <Card className="bg-zinc-950/40 border-zinc-850 overflow-hidden">
        <CardHeader>
          <div className="flex items-center gap-3">
            <div className="w-8 h-8 rounded-lg bg-zinc-900 border border-zinc-800 flex items-center justify-center">
              <MessageSquare className="w-4 h-4 text-cyan-400" />
            </div>
            <div>
              <CardTitle className="text-white text-base">Envelope Subjects</CardTitle>
              <CardDescription className="text-xs">Subject names mapped to internally tagged agent payloads</CardDescription>
            </div>
          </div>
        </CardHeader>
        <CardContent className="p-0">
          <Table>
            <TableHeader className="bg-zinc-900/40 border-zinc-850">
              <TableRow>
                <TableHead className="px-6 py-3 text-zinc-400 font-semibold text-xs">Subject</TableHead>
                <TableHead className="px-6 py-3 text-zinc-400 font-semibold text-xs">Payload</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody className="divide-y divide-zinc-900">
              {subjects.map(([subject, payload]) => (
                <TableRow key={subject} className="hover:bg-zinc-950/20">
                  <TableCell className="px-6 py-4 font-mono text-xs text-zinc-300">{subject}</TableCell>
                  <TableCell className="px-6 py-4 font-mono text-xs text-zinc-400">{payload}</TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        </CardContent>
      </Card>

      <div className="grid grid-cols-1 md:grid-cols-2 gap-6">
        <Card className="bg-zinc-950/40 border-zinc-850">
          <CardHeader>
            <div className="flex items-center gap-3">
              <div className="w-8 h-8 rounded-lg bg-zinc-900 border border-zinc-800 flex items-center justify-center">
                <GitBranch className="w-4 h-4 text-purple-400" />
              </div>
              <div>
                <CardTitle className="text-white text-base">Core Contracts</CardTitle>
                <CardDescription className="text-xs">Intent, session, delegation, negotiation, and terminal state</CardDescription>
              </div>
            </div>
          </CardHeader>
          <CardContent className="space-y-3 text-sm text-zinc-400">
            <p><code>AgentIntent</code> describes requested work with source and target agents, objective text, structured input, required capabilities, constraints, deadlines, priority, and metadata.</p>
            <p><code>DelegationRequest</code> and <code>DelegationResponse</code> move scoped work between agents. Accepted responses include an assigned agent; rejected responses include a reason.</p>
            <p><code>AgentResult</code> is terminal only: completed, failed, or cancelled.</p>
          </CardContent>
        </Card>

        <Card className="bg-zinc-950/40 border-zinc-850">
          <CardHeader>
            <div className="flex items-center gap-3">
              <div className="w-8 h-8 rounded-lg bg-zinc-900 border border-zinc-800 flex items-center justify-center">
                <CheckCircle2 className="w-4 h-4 text-emerald-400" />
              </div>
              <div>
                <CardTitle className="text-white text-base">Validation Rules</CardTitle>
                <CardDescription className="text-xs">Local checks before policy and dispatch layers</CardDescription>
              </div>
            </div>
          </CardHeader>
          <CardContent className="space-y-2">
            {validationRules.map((rule) => (
              <div key={rule} className="flex items-start gap-2 text-xs text-zinc-400">
                <CheckCircle2 className="mt-0.5 h-4 w-4 flex-shrink-0 text-emerald-400" />
                <span>{rule}</span>
              </div>
            ))}
          </CardContent>
        </Card>
      </div>

      <Card className="bg-zinc-950/40 border-zinc-850">
        <CardHeader>
          <div className="flex items-center gap-3">
            <div className="w-8 h-8 rounded-lg bg-zinc-900 border border-zinc-800 flex items-center justify-center">
              <Route className="w-4 h-4 text-blue-400" />
            </div>
            <div>
              <CardTitle className="text-white text-base">JSON Shape</CardTitle>
              <CardDescription className="text-xs">Internally tagged envelope with deterministic output</CardDescription>
            </div>
          </div>
        </CardHeader>
        <CardContent>
          <pre className="text-xs bg-[#050505] p-4 rounded-lg border border-zinc-900 font-mono text-zinc-300 overflow-x-auto">
            <code>{`{
  "type": "intent",
  "payload": {
    "schema_version": 1,
    "intent_id": "22222222-2222-4222-8222-222222222222",
    "session_id": "11111111-1111-4111-8111-111111111111",
    "source_agent": "planner.main",
    "target_agent": "executor.safety",
    "kind": "act",
    "objective": "open valve",
    "input": { "valve": "v-7" },
    "required_capabilities": ["driver.execute:valve.open"],
    "priority": "high",
    "metadata": {}
  }
}`}</code>
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
              <CardTitle className="text-white text-base">Integration Notes</CardTitle>
              <CardDescription className="text-xs">CLI builders and node/SDK handoff behavior</CardDescription>
            </div>
          </div>
        </CardHeader>
        <CardContent className="space-y-4 text-sm text-zinc-400">
          <p>Future node and SDK integrations should deserialize with <code>AgentMessage::from_json_slice</code> so validation runs before policy evaluation or dispatch.</p>
          <div className="flex flex-wrap gap-2">
            <Badge variant="outline" className="font-mono text-[10px] bg-zinc-900 text-zinc-400 border-zinc-850">zap agent intent</Badge>
            <Badge variant="outline" className="font-mono text-[10px] bg-zinc-900 text-zinc-400 border-zinc-850">zap agent status</Badge>
            <Badge variant="outline" className="font-mono text-[10px] bg-zinc-900 text-zinc-400 border-zinc-850">zap agent result</Badge>
          </div>
        </CardContent>
      </Card>

      <Alert className="border-zinc-800 bg-zinc-950/50 text-zinc-300">
        <AlertCircle className="w-4 h-4 text-zinc-400" />
        <AlertTitle className="font-semibold text-zinc-200">Layer Separation</AlertTitle>
        <AlertDescription className="text-xs">
          Agent capabilities reuse <code>zap-capability::CapabilityId</code>, but negotiated capabilities are descriptive until node policy, manifest, registry, and grant checks authorize execution.
        </AlertDescription>
      </Alert>
    </div>
  );
}
