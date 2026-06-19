# Cloud Operations Domain Pack

This preview domain pack models SRE, cloud operations, and incident response
actions as auditable ZAP subjects. It is designed for automation that needs to
observe systems, coordinate incidents, mitigate production issues, and guard
high-impact operational changes.

## Scope

- Read bounded operational telemetry from approved observability sources.
- Declare, update, escalate, and resolve incidents with evidence links.
- Scale services or capacity within explicit environment and blast-radius
  grants.
- Shift production traffic or rollback deployments only with human approval.
- Rotate secrets and purge data only with proof of authority.

## Capabilities

| Capability | Risk | Purpose |
| --- | --- | --- |
| `telemetry.read` | low | Read metrics, traces, logs, events, alerts, and health signals. |
| `incident.declare` | medium | Create or update incident state with routing metadata. |
| `service.scale` | medium | Change bounded capacity, concurrency, or autoscaling targets. |
| `traffic.shift` | high | Move traffic across versions, regions, cells, or failover targets. |
| `deploy.rollback` | high | Roll back a deployment to an approved release artifact. |
| `secret.rotate` | critical | Rotate credentials, certificates, tokens, or signing keys. |
| `data.purge` | critical | Irreversibly delete or expire bounded cloud data. |

## Policy Model

The baseline policy is fail-closed:

- low and medium operational actions require explicit grants;
- traffic shifts and rollbacks require human approval;
- secret rotation and data purge require proof of authority;
- every unlisted action subject is denied by default.

## Validation

Run this from the repository root:

```powershell
cargo run -p zap-cli -- pack validate --pack examples/domain-packs/cloud-ops --json
```
