# Cloud Operations Subjects

This preview catalog documents action subjects used by the cloud-ops pack for
SRE automation, incident response, and controlled production operations.
Concrete JSON schemas will be added once domain-pack validation includes payload
schema enforcement.

## `telemetry.read`

Read bounded operational telemetry from approved systems.

Expected payload fields:

- `environment`: production, staging, development, or sandbox environment;
- `scope`: service, cluster, region, cell, tenant, account, or namespace;
- `signals`: metrics, traces, logs, events, synthetics, health, or alerts;
- `window`: current reading or bounded historical interval;
- `filters`: optional query filters, label selectors, or log predicates;
- `reason`: human-readable purpose for the read.

## `incident.declare`

Create, update, escalate, or resolve an incident record.

Expected payload fields:

- `incident_id`: existing incident id or requested new incident reference;
- `severity`: sev0, sev1, sev2, sev3, sev4, or informational;
- `service`: affected service or dependency;
- `status`: investigating, identified, monitoring, resolved, or cancelled;
- `summary`: operator-readable incident summary;
- `routing`: on-call team, escalation policy, or communication channel;
- `evidence`: alert, dashboard, ticket, receipt, or timeline references.

## `service.scale`

Adjust bounded capacity or concurrency for a cloud service.

Expected payload fields:

- `environment`: target environment;
- `service`: service, workload, deployment, job, or queue worker id;
- `region`: optional cloud region, zone, or cell;
- `dimension`: replicas, min_replicas, max_replicas, concurrency, rate_limit, or capacity_units;
- `target`: requested bounded target value;
- `duration_minutes`: optional time-bound override duration;
- `rollback_plan`: previous value or automated revert reference;
- `reason`: incident mitigation, load management, maintenance, or cost-control rationale.

## `traffic.shift`

Shift bounded traffic between versions, regions, cells, or failover targets.

Expected payload fields:

- `environment`: target environment;
- `service`: service or gateway affected by the shift;
- `from_target`: current region, cell, version, or upstream target;
- `to_target`: destination region, cell, version, or upstream target;
- `percent`: traffic percentage to move;
- `duration_minutes`: bounded hold period before review or revert;
- `health_checks`: checks that must remain healthy during the shift;
- `approver`: human approval or change-management reference.

## `deploy.rollback`

Rollback a service deployment to a previously approved release artifact.

Expected payload fields:

- `environment`: target environment;
- `service`: service being rolled back;
- `current_release`: currently deployed version or artifact digest;
- `target_release`: approved previous version or artifact digest;
- `incident_id`: linked incident or change record;
- `blast_radius`: expected customers, regions, cells, or tenants affected;
- `verification`: smoke tests, synthetic checks, or monitoring gates;
- `approver`: human approver or approval workflow reference.

## `secret.rotate`

Rotate a production secret, credential, token, certificate, or signing key.

Expected payload fields:

- `environment`: target environment;
- `secret_ref`: vault path, key id, certificate id, or credential alias;
- `rotation_type`: regenerate, revoke, reissue, import, or expire;
- `consumers`: services, jobs, or integrations expected to reload the value;
- `rollout_plan`: staged rollout, dual-write, restart, or reload strategy;
- `poa_ref`: proof-of-authority reference;
- `verification`: health checks and credential-use validation after rotation.

## `data.purge`

Delete, purge, or irreversibly expire cloud data.

Expected payload fields:

- `environment`: target environment;
- `resource`: storage bucket, object prefix, table, partition, queue, cache, or topic;
- `selector`: bounded key, partition, query, object prefix, or retention rule;
- `retention_basis`: policy, legal, privacy, test cleanup, or operational reason;
- `dry_run`: whether a preview count was generated before execution;
- `estimated_records`: approximate object, row, message, or key count;
- `poa_ref`: proof-of-authority reference;
- `evidence`: ticket, approval, receipt, export, or dry-run artifact.
