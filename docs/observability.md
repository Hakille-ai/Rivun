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

Recommended metric names:

- `zap_node_health_status`: gauge where `0=healthy`, `1=degraded`,
  `2=critical`;
- `zap_frames_sent_total`, `zap_frames_received_total`,
  `zap_frames_rejected_total`;
- `zap_driver_execution_seconds_bucket` and
  `zap_driver_execution_errors_total`;
- `zap_peer_trust_status`: gauge by peer and status;
- `zap_registry_signature_valid`: gauge, `1` only when local registry signature
  verification succeeds;
- `zap_receipt_log_verify_failures_total`;
- `zap_capability_cache_age_seconds`;
- `zap_poa_attestation_failures_total`.

Keep high-cardinality values out of labels. Use logs or receipts for request
ids, install plan hashes, and detailed rejection reasons.

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

### ZapHealthCritical

At least one critical health check is failing. Run:

```bash
cargo run -p zap-cli -- doctor --config /etc/zap/zap.toml --strict --json
```

Then verify key files, receipt paths, registry signature policy, and capability
cache freshness.

### ZapReceiptAuditFailing

Receipt verification failed. Stop pruning receipt logs and archive the current
files before restarting the node:

```bash
cargo run -p zap-cli -- receipts verify --path /var/lib/zap/receipts.jsonl
```

If verification fails after a host or disk incident, preserve the broken log for
forensics and cut over to a new receipt file only after security review.

### ZapRegistrySignatureInvalid

The local ZapStore registry is missing a valid operator signature. Pulling or
mirroring a registry clears trust until it is reviewed and re-signed:

```bash
cargo run -p zap-cli -- registry verify-signature --registry /var/lib/zap/registry.index.toml
```

Do not install new drivers from an unsigned production registry.

### ZapCapabilityCacheStale

The capability cache is older than the deployment policy. Refresh and verify it:

```bash
cargo run -p zap-cli -- capability cache refresh --config /etc/zap/zap.toml --strict --json
cargo run -p zap-cli -- capability cache verify --path /var/lib/zap/capabilities.jsonl
```

Routes with `requires_peer_grant` should remain blocked until the cache is
fresh.

### ZapDriverErrorRateHigh

Driver failures are elevated. Compare recent release changes, registry bundle
hashes, and runtime limits. If only one action is affected, quarantine that
driver version through registry deprecation or revocation.

### ZapPoaAttestationFailures

Validators are failing to attest or responses do not verify. Check validator
set epoch, peer trust status, clock skew, and network reachability before
retrying critical actions.
