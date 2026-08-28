# rivun Telemetry

`rivun-telemetry` is the fleet observability library: diagnostic checks,
incident snapshots, Prometheus metrics, and fleet topology. It complements the
operator contract documented in [Observability](observability.md), which
defines the metrics and alert surface that production deployments should keep
stable.

## Fleet doctor

`rivun fleet doctor` aggregates readiness across **6 core criteria** for the
local node and (optionally) a peer:

```bash
cargo run -p rivun-cli -- fleet doctor --config rivun.toml --json
cargo run -p rivun-cli -- fleet doctor --config rivun.toml --strict --json
cargo run -p rivun-cli -- fleet doctor --config rivun.toml --peer <uuid> --json
```

The underlying `FleetDoctor` runs checks over:

- receipt journals (`SignedReceiptSegmentManifest`);
- driver and domain-pack registries (`DriverRegistry`,
  `DomainPackRegistry`);
- journal segment magic constants (`ZJSEG001`, durable frame magic `ZAPFRM01`);
- ledger and store integrity surfaces.

Each check is a `FleetDoctorCheck { category, name, status, summary, detail }`
with statuses `Passed`/`Warning`/`Failed`; results merge across scopes.
`--strict` exits non-zero on any failure. Local `rivun doctor` remains the
single-node readiness gate; `fleet doctor` is the multi-node view.

## Incident snapshots

`rivun incident snapshot` captures a bounded, redacted evidence bundle for
triage and postmortems:

```bash
cargo run -p rivun-cli -- incident snapshot \
  --config /etc/rivun/rivun.toml \
  --out incidents/$(date +%Y%m%d-%H%M%S)-snapshot.json

# Or as a tar archive
cargo run -p rivun-cli -- incident snapshot \
  --config /etc/rivun/rivun.toml --format tar \
  --out incidents/$(date +%Y%m%d-%H%M%S)-snapshot.tar
```

The snapshot includes:

- doctor output (local checks);
- redacted config readiness counts;
- memory journal verification summaries;
- receipt journal summaries;
- capability-cache verification summaries (when configured);
- live process state where available (`/proc/self/status`: VmRSS, VmSize,
  threads, open fds) via `ProcessState::collect()`.

`SecretRedactor` strips key material, transport keys, raw memory payloads,
memory metadata, raw receipt signatures, and live packet captures by design.
`IncidentCapturer` and `TarBuilder` provide the programmatic API.

## Metrics

`ZapNodeMetricsSnapshot` collects typed counters aligned with the
[observability contract](observability.md):

- frames sent/received/rejected (by reason);
- driver execution errors;
- peer trust state;
- registry signature validity;
- capability cache age;
- receipt verify failures and journal rotations;
- segment manifest errors;
- PoA attestation failures;
- replay rejections/drops;
- pack verification failures;
- gateway requests.

`PrometheusExporter` renders the snapshot as Prometheus text. The CLI-level
gateway metrics (`@@rivun_HEADER@@gateway_*`) are exposed by `rivun-gateway`; the daemon
endpoints `/metrics`, `/healthz`, `/healthz.json` are documented in
[Observability](observability.md).

## Fleet topology

`FleetTopology` tracks fleet membership:

- `FleetNodeState { node_id, addr, trust, health, capabilities, rtt_ms, last_seen_micros }`;
- `FleetNodeHealth`: `Healthy` / `Degraded` / `Critical` / `Unreachable`;
- the local node auto-registers; `FleetNodeHealth` merges peer reports.

Gateway resources expose it as `rivun://fleet/status` and
`rivun://fleet/topology` (see [Gateway](gateway.md)).

## Testing

`crates/rivun-telemetry/tests/` covers doctor status derivation, incident
redaction and tar output, Prometheus rendering (including zero-metric case),
topology merging, and adversarial scenarios (tampered journals, bad manifests,
secret leaks).
