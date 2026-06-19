# Observability

This runbook defines the production telemetry contract for ZAP nodes. The
runtime can evolve, but production deployments should keep the same operator
signals: scrapeability, node health, driver execution, peer trust, registry
integrity, receipt verification, capability cache freshness, and PoA failures.

Reference assets live under `crates/zap-ops/config`:

- `observability/production.toml`: typed ZAP ops contract validated by
  `zap-ops`;
- `prometheus/zap-scrape.yml`: example Prometheus scrape job;
- `prometheus/zap-rules.yml`: alert rules and runbook links;
- `otel/collector.yml`: OpenTelemetry Collector pipeline;
- `grafana/zap-production-dashboard.json`: dashboard import.

## Required Labels

Every metric and span should include:

- `service.name`: `zap-node`;
- `deployment.environment`: `production`, `staging`, or `development`;
- `cluster`: stable cluster name;
- `node_id`: local ZAP node id when the signal is node-specific;
- `peer`: peer node id when the signal describes a remote peer;
- `action`: envelope subject or driver action when dispatching work.

Payload bodies, secrets, private keys, transport keys, and signed install plan
contents must not be exported as labels or span attributes.

## Metrics Contract

Metrics emitted by `ZapNode::metrics_prometheus_text()`:

- `zap_frames_sent_total`, `zap_frames_received_total`,
  `zap_frames_rejected_total`: counters labelled by `node_id` and peer or
  rejection reason;
- `zap_driver_execution_errors_total`: counter labelled by `node_id` and
  `action`;
- `zap_peer_trust_status`: gauge by peer and status;
- `zap_registry_signature_valid`: gauge, `1` only when local registry signature
  verification succeeds;
- `zap_receipt_log_verify_failures_total`;
- `zap_capability_cache_age_seconds`;
- `zap_poa_attestation_failures_total`.

Keep high-cardinality values out of labels. Use logs or receipts for request
ids, install plan hashes, and detailed rejection reasons.

Alert routing should preserve the incident class as a low-cardinality label,
for example `policy_default_allow`, `registry_signature_invalid`,
`capability_cache_stale`, `receipt_verification_failure`,
`poa_attestation_failure`, `driver_runtime_errors`, or `replay_spike`. Use the
incident id in logs and receipt metadata rather than metric labels.

Prometheus `up{job="zap-node"}` is the scrapeability signal used by the
production rules. Health-check details are still produced by the ops health
configuration and `zap doctor`, but `zap-node` does not currently emit a
dedicated node-health status gauge. Driver latency histograms are also not
emitted yet; use driver error rate and receipt/PoA failures for production
paging.

`zap-node` exposes this contract as an in-process readiness surface through
`ZapNode::metrics_snapshot()` and `ZapNode::metrics_prometheus_text()`. The
daemon does not open a metrics HTTP endpoint by itself; deployments that need
Prometheus scraping should mount the text output behind their existing sidecar,
supervisor, or embedding service and apply the bind/path policy from
`crates/zap-ops/config/observability/production.toml`.

## Health Checks

Use `crates/zap-ops/config/observability/production.toml` as the canonical
shape. The minimum production set is:

- UDP bind or listener reachability;
- receipt log path mounted and writable by the daemon user;
- registry bundle manifest present when `[registry.bundle_path]` is configured;
- `zap doctor --strict --json` for config readiness.

Health reports should be stale after 60 seconds unless the deployment uses a
slower control loop.

## Alerts

### ZapNodeDown

Prometheus cannot scrape the node. Check network policy, process status, and
the metrics bind address. If the daemon is healthy but metrics are unavailable,
treat this as an observability incident and keep traffic changes frozen until
scraping is restored.

### ZapReceiptAuditFailing

Receipt verification failed. Stop pruning receipt logs and archive the current
files before restarting the node:

```bash
cargo run -p zap-cli -- receipts verify --path /var/lib/zap/receipts.jsonl
```

If verification fails after a host or disk incident, preserve the broken log for
forensics and cut over to a new receipt file only after security review.

Rollback only to a deployment whose receipt log verifies for the full audit
window. Preserve local and pulled peer receipts as described in
[Operations](operations.md#receipt-verification-failure).

### ZapRegistrySignatureInvalid

The local ZapStore registry is missing a valid operator signature. Pulling or
mirroring a registry clears trust until it is reviewed and re-signed:

```bash
cargo run -p zap-cli -- registry verify-signature --registry /var/lib/zap/registry.index.toml
```

Do not install new drivers from an unsigned production registry.

Rollback to the last signed registry/publication/bundle set that verifies, and
preserve the invalid index. See
[Operations](operations.md#registry-signature-invalid).

### ZapCapabilityCacheStale

The capability cache is older than the deployment policy. Refresh and verify it:

```bash
cargo run -p zap-cli -- capability cache refresh --config /etc/zap/zap.toml --strict --json
cargo run -p zap-cli -- capability cache verify --path /var/lib/zap/capabilities.jsonl
```

Routes with `requires_peer_grant` should remain blocked until the cache is
fresh.

Rollback route changes when the refreshed cache no longer grants the required
capability. Preserve stale and refreshed JSONL caches as described in
[Operations](operations.md#stale-capability-cache).

### ZapDriverErrorRateHigh

Driver failures are elevated. Compare recent release changes, registry bundle
hashes, and runtime limits. If only one action is affected, quarantine that
driver version through registry deprecation or revocation.

Rollback to the last manifest and registry entry that verify and have successful
receipts under the same runtime limits. See
[Operations](operations.md#driver-runtime-errors).

### ZapFrameRejectRateHigh

Inbound frame rejections are elevated by reason. Start with peer trust,
configuration drift, signature failures, and schema/policy changes before
assuming transport loss:

```bash
cargo run -p zap-cli -- trust inspect --config /etc/zap/zap.toml --json
cargo run -p zap-cli -- check-config --config /etc/zap/zap.toml --json
```

Preserve sender and receiver logs plus any receipt entries covering the same
window. If the reason points at replay, nonce, freshness, or stale-frame
validation, follow the replay spike runbook below.

### ZapPoaAttestationFailures

Validators are failing to attest or responses do not verify. Check validator
set epoch, peer trust status, clock skew, and network reachability before
retrying critical actions.

Do not lower the effective threshold as an emergency shortcut. Use the last
signed validator set that verifies, and preserve PoA request/response JSON as
described in [Operations](operations.md#poa-attestation-failure).

### ZapReplaySpike

Replay or nonce-related frame rejections are rising. First verify local config
and peer trust:

```bash
cargo run -p zap-cli -- doctor --config /etc/zap/zap.toml --strict --json
cargo run -p zap-cli -- trust inspect --config /etc/zap/zap.toml --json
cargo run -p zap-cli -- check-config --config /etc/zap/zap.toml --json
```

Freeze peer key rotation and topology changes until the spike is explained.
Preserve sender and receiver logs, then follow
[Operations](operations.md#replay-spikes).
