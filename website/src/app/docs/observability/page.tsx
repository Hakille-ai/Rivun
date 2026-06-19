import { Activity, AlertTriangle, BarChart3, CheckCircle2, HeartPulse, RadioTower, ShieldOff, Terminal } from 'lucide-react';
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";

const labels = [
  "service.name",
  "deployment.environment",
  "cluster",
  "node_id",
  "peer",
  "action",
];

const metrics = [
  ["zap_node_health_status", "0 healthy, 1 degraded, 2 critical"],
  ["zap_frames_sent_total", "Outbound frame volume"],
  ["zap_frames_received_total", "Inbound frame volume"],
  ["zap_frames_rejected_total", "Policy, schema, transport, or trust rejects"],
  ["zap_driver_execution_seconds_bucket", "Driver latency histogram"],
  ["zap_driver_execution_errors_total", "Driver failures by action"],
  ["zap_peer_trust_status", "Peer trust state by status"],
  ["zap_registry_signature_valid", "1 only when registry verification succeeds"],
  ["zap_receipt_log_verify_failures_total", "Receipt audit failures"],
  ["zap_capability_cache_age_seconds", "Freshness of local capability cache"],
  ["zap_poa_attestation_failures_total", "Proof-of-Action attestation failures"],
];

const healthChecks = [
  "UDP bind or listener reachability",
  "Receipt log path mounted and writable by the daemon user",
  "Registry bundle manifest present when bundle_path is configured",
  "zap doctor --strict --json for config readiness",
];

const alerts = [
  ["ZapNodeDown", "Prometheus cannot scrape the node. Freeze traffic changes until scraping is restored."],
  ["ZapHealthCritical", "At least one critical health check is failing. Run doctor and verify key files, receipts, registry signatures, and cache freshness."],
  ["ZapReceiptAuditFailing", "Receipt verification failed. Preserve current logs for audit before restart or pruning."],
  ["ZapRegistrySignatureInvalid", "The local ZapStore registry is missing a valid operator signature. Block new production installs."],
  ["ZapCapabilityCacheStale", "Capability data is older than policy. Refresh and verify before grant-protected routes continue."],
  ["ZapDriverErrorRateHigh", "Driver failures are elevated. Compare release changes, registry bundles, runtime limits, and affected actions."],
  ["ZapPoaAttestationFailures", "Validators are failing to attest or responses do not verify. Check epoch, trust, clock skew, and reachability."],
];

export default function ObservabilityPage() {
  return (
    <div className="space-y-8 font-sans">
      <div>
        <h1 className="text-3xl md:text-4xl font-extrabold tracking-tight mb-2 text-white">Observability</h1>
        <p className="text-zinc-400 text-lg">Production telemetry contracts for ZAP nodes, driver execution, peer trust, registries, receipts, capability caches, and Proof-of-Action failures.</p>
      </div>

      <Alert className="border-blue-500/20 bg-blue-500/5 text-blue-300">
        <RadioTower className="w-4 h-4 text-blue-400" />
        <AlertTitle className="font-semibold text-blue-400">Scrape Surface</AlertTitle>
        <AlertDescription className="text-xs">
          <code>zap-node</code> exposes <code>ZapNode::metrics_snapshot()</code> and <code>ZapNode::metrics_prometheus_text()</code>. Deployments should mount the Prometheus text behind their existing sidecar, supervisor, or embedding service.
        </AlertDescription>
      </Alert>

      <Card className="group bg-zinc-950/40 hover:bg-zinc-950/60 border border-zinc-850 hover:border-emerald-500/20 transition-all duration-500 overflow-hidden rounded-2xl shadow-xl hover:shadow-2xl hover:shadow-emerald-500/5 not-prose p-0">
        <div className="grid grid-cols-1 md:grid-cols-12 items-stretch">
          <div className="md:col-span-5 relative bg-gradient-to-br from-zinc-950 to-black/80 flex items-center justify-center p-6 border-b md:border-b-0 md:border-r border-zinc-900 overflow-hidden min-h-[260px] md:min-h-0">
            <div className="absolute inset-0 bg-[radial-gradient(circle_at_center,rgba(16,185,129,0.08)_0%,transparent_70%)] pointer-events-none" />
            <div className="absolute -inset-10 bg-[radial-gradient(circle_at_bottom_left,rgba(59,130,246,0.04)_0%,transparent_50%)] pointer-events-none" />
            <div className="relative w-full max-w-[280px] space-y-3">
              {["node health", "driver latency", "receipt audit"].map((signal, index) => (
                <div key={signal} className="flex items-center justify-between rounded-xl border border-zinc-800 bg-black/40 p-3 shadow-lg shadow-black/20">
                  <div className="flex items-center gap-3">
                    <div className="h-9 w-9 rounded-lg bg-zinc-900 border border-zinc-800 flex items-center justify-center">
                      <Activity className={`h-4 w-4 ${index === 0 ? "text-emerald-400" : "text-blue-400"}`} />
                    </div>
                    <p className="font-mono text-xs text-zinc-200">{signal}</p>
                  </div>
                  <Badge variant="outline" className="font-mono text-[10px] bg-zinc-900 text-zinc-400 border-zinc-850">prod</Badge>
                </div>
              ))}
            </div>
          </div>
          <div className="md:col-span-7 p-6 md:p-8 flex flex-col justify-center space-y-3 bg-zinc-950/20">
            <span className="text-xs font-semibold uppercase tracking-wider text-emerald-400 block">Operator Signal Contract</span>
            <h3 className="text-xl font-bold text-white tracking-tight">Health, Trust, and Audit in One View</h3>
            <p className="text-sm text-zinc-400 leading-relaxed">
              ZAP observability treats scraping, node readiness, driver execution, peer trust, registry integrity, receipt verification, capability freshness, and PoA failures as first-class production signals.
            </p>
          </div>
        </div>
      </Card>

      <div className="grid grid-cols-1 md:grid-cols-2 gap-6">
        <Card className="bg-zinc-950/40 border-zinc-850">
          <CardHeader>
            <div className="flex items-center gap-3">
              <div className="w-8 h-8 rounded-lg bg-zinc-900 border border-zinc-800 flex items-center justify-center">
                <BarChart3 className="w-4 h-4 text-blue-400" />
              </div>
              <div>
                <CardTitle className="text-white text-base">Required Labels</CardTitle>
                <CardDescription className="text-xs">Stable attributes for metrics and spans</CardDescription>
              </div>
            </div>
          </CardHeader>
          <CardContent className="space-y-3">
            <div className="flex flex-wrap gap-2">
              {labels.map((label) => (
                <Badge key={label} variant="outline" className="font-mono text-[10px] bg-zinc-900 text-zinc-400 border-zinc-850">{label}</Badge>
              ))}
            </div>
            <p className="text-xs text-zinc-500">Payload bodies, secrets, private keys, transport keys, and signed install plan contents must not be exported as labels or span attributes.</p>
          </CardContent>
        </Card>

        <Card className="bg-zinc-950/40 border-zinc-850">
          <CardHeader>
            <div className="flex items-center gap-3">
              <div className="w-8 h-8 rounded-lg bg-zinc-900 border border-zinc-800 flex items-center justify-center">
                <HeartPulse className="w-4 h-4 text-emerald-400" />
              </div>
              <div>
                <CardTitle className="text-white text-base">Health Checks</CardTitle>
                <CardDescription className="text-xs">Minimum production readiness set</CardDescription>
              </div>
            </div>
          </CardHeader>
          <CardContent className="space-y-2">
            {healthChecks.map((check) => (
              <div key={check} className="flex items-start gap-2 text-xs text-zinc-400">
                <CheckCircle2 className="mt-0.5 h-4 w-4 flex-shrink-0 text-emerald-400" />
                <span>{check}</span>
              </div>
            ))}
          </CardContent>
        </Card>
      </div>

      <Card className="bg-zinc-950/40 border-zinc-850 overflow-hidden">
        <CardHeader>
          <div className="flex items-center gap-3">
            <div className="w-8 h-8 rounded-lg bg-zinc-900 border border-zinc-800 flex items-center justify-center">
              <Activity className="w-4 h-4 text-cyan-400" />
            </div>
            <div>
              <CardTitle className="text-white text-base">Metric Names</CardTitle>
              <CardDescription className="text-xs">Recommended Prometheus contract for ZAP operators</CardDescription>
            </div>
          </div>
        </CardHeader>
        <CardContent className="p-0">
          <div className="divide-y divide-zinc-900">
            {metrics.map(([name, description]) => (
              <div key={name} className="grid grid-cols-1 md:grid-cols-12 gap-2 px-6 py-4 hover:bg-zinc-950/20">
                <code className="md:col-span-5 text-xs text-zinc-300">{name}</code>
                <p className="md:col-span-7 text-xs text-zinc-500">{description}</p>
              </div>
            ))}
          </div>
        </CardContent>
      </Card>

      <Card className="bg-zinc-950/40 border-zinc-850">
        <CardHeader>
          <div className="flex items-center gap-3">
            <div className="w-8 h-8 rounded-lg bg-zinc-900 border border-zinc-800 flex items-center justify-center">
              <Terminal className="w-4 h-4 text-blue-400" />
            </div>
            <div>
              <CardTitle className="text-white text-base">Operator Commands</CardTitle>
              <CardDescription className="text-xs">Diagnostics used by the alert runbooks</CardDescription>
            </div>
          </div>
        </CardHeader>
        <CardContent>
          <pre className="text-xs bg-[#050505] p-4 rounded-lg border border-zinc-900 font-mono text-zinc-300 overflow-x-auto">
            <code>{`cargo run -p zap-cli -- doctor --config /etc/zap/zap.toml --strict --json
cargo run -p zap-cli -- receipts verify --path /var/lib/zap/receipts.jsonl
cargo run -p zap-cli -- registry verify-signature --registry /var/lib/zap/registry.index.toml
cargo run -p zap-cli -- capability cache refresh --config /etc/zap/zap.toml --strict --json
cargo run -p zap-cli -- capability cache verify --path /var/lib/zap/capabilities.jsonl`}</code>
          </pre>
        </CardContent>
      </Card>

      <Card className="bg-zinc-950/40 border-zinc-850">
        <CardHeader>
          <div className="flex items-center gap-3">
            <div className="w-8 h-8 rounded-lg bg-zinc-900 border border-zinc-800 flex items-center justify-center">
              <AlertTriangle className="w-4 h-4 text-amber-400" />
            </div>
            <div>
              <CardTitle className="text-white text-base">Alert Runbooks</CardTitle>
              <CardDescription className="text-xs">Operator response summaries for production incidents</CardDescription>
            </div>
          </div>
        </CardHeader>
        <CardContent className="space-y-3">
          {alerts.map(([name, description]) => (
            <div key={name} className="rounded-lg border border-zinc-900 bg-black/20 px-4 py-3">
              <div className="flex items-start gap-3">
                <ShieldOff className="mt-0.5 h-4 w-4 flex-shrink-0 text-amber-400" />
                <div>
                  <p className="font-mono text-xs text-zinc-300">{name}</p>
                  <p className="mt-1 text-xs text-zinc-500">{description}</p>
                </div>
              </div>
            </div>
          ))}
        </CardContent>
      </Card>
    </div>
  );
}
