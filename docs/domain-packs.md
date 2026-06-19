# Domain Packs

Domain packs are signed, reviewable bundles that make ZAP useful in a specific
field without changing the core protocol. A pack collects capability names,
message schemas, policy templates, route templates, driver references,
simulation guidance, and operating notes for one domain.

The intent is simple: ZAP stays a narrow trust boundary, while packs provide
the reusable domain knowledge that lets teams adopt it quickly.

## Goals

- give operators a repeatable starting point for a domain;
- keep capabilities, schemas, policies, and routes consistent;
- make high-risk actions fail closed by default;
- attach threat-model notes and audit expectations to domain workflows;
- allow future ZapStore workflows to sign, publish, verify, revoke, and install
  whole capability packs, not only individual drivers.

## Pack Layout

A pack should be a directory with this shape:

```text
pack.toml
README.md
schemas/
policies/
routes/
drivers/
dashboards/
tests/
```

Only `pack.toml` and `README.md` are required for early preview packs. Other
directories are added when the pack includes concrete executable artifacts.

## CLI Workflows

Preview packs can already be checked locally with the CLI.

Validate a pack:

```powershell
cargo run -p zap-cli -- pack validate --pack examples/domain-packs/agentic-dev --json
```

Inspect a pack summary:

```powershell
cargo run -p zap-cli -- pack inspect --pack examples/domain-packs/cloud-ops --json
```

`pack validate` checks that:

- `pack.toml` exists and parses as TOML;
- `schema_version` is supported;
- `id`, `name`, `version`, and `status` are present;
- capability ids are non-empty and unique;
- risk values use `low`, `medium`, `high`, or `critical`;
- referenced policy and schema files exist;
- referenced policy TOML is accepted by the message-policy parser.

`pack inspect` returns the validation result plus a stable summary containing
the pack id, name, version, status, capability count, policy count, schema
count, and risk-level distribution. This is intended for CI, website generation,
pack catalogs, and future ZapStore pack workflows.

## Manifest Contract

`pack.toml` is the domain-pack manifest. It is intentionally plain TOML so it
can be reviewed during security and operations work.

```toml
schema_version = 1
id = "zap-pack-agentic-dev"
name = "Agentic Development"
version = "0.1.0"
status = "preview"
description = "Auditable coding-agent workflows over ZAP."

[compatibility]
zap_protocol = ">=0.1.0,<1.0.0"
driver_abi = ">=1,<=2"

[[capabilities]]
id = "repo.read"
risk = "low"
description = "Read repository files and metadata."

[[capabilities]]
id = "repo.patch"
risk = "medium"
requires = ["repo.read"]
description = "Prepare or apply scoped source patches."

[[policies]]
path = "policies/action-policy.toml"
description = "Baseline fail-closed policy for pack actions."
```

Future signed manifests should also include author identity, operator approval,
bundle hashes, revocation metadata, and install-plan bindings. Until that is
implemented, pack manifests are documentation and planning artifacts, not a
node-enforced trust source.

## Risk Levels

Packs use a small risk vocabulary:

- `low`: read-only or local informational action;
- `medium`: reversible write or bounded automation;
- `high`: action with service, data, cost, or availability impact;
- `critical`: safety, access, money movement, destructive change, or physical
  effect.

Recommended defaults:

- `low` actions may be allowed when explicitly granted;
- `medium` actions should require scoped grants and receipts;
- `high` actions should require simulation or human approval;
- `critical` actions should require Proof-of-Action and, where appropriate,
  human approval.

## Naming Rules

Capability ids should be short, stable, and domain-oriented:

- use lowercase ASCII;
- use dot-separated resources and verbs;
- avoid vendor-specific names in shared packs;
- prefer `resource.verb` such as `repo.read`, `test.run`, `deploy.rollout`;
- use vendor prefixes only for private extensions, such as
  `acme.robot.arm.move`.

Message subjects should follow the same domain vocabulary and remain stable
across SDKs:

```text
repo.read
repo.patch
test.run
ci.inspect
pr.create
```

## Pack Lifecycle

Preview lifecycle:

1. create the pack directory and manifest;
2. define capability ids and risk levels;
3. add policy templates and example configs;
4. add schemas and route templates;
5. add example drivers or gateway adapters;
6. add tests and expected receipts;
7. promote from `preview` to `beta` once an end-to-end example passes.

Future lifecycle:

1. `zap pack build`;
2. `zap pack sign`;
3. `zap pack publish`;
4. `zap pack install`;
5. `zap pack audit`;
6. `zap pack revoke`.

## First Official Packs

Current preview packs in `examples/domain-packs/`:

- `zap-pack-agentic-dev`: auditable software development agents;
- `zap-pack-smart-building`: smart building sensors and actuators;
- `zap-pack-cloud-ops`: deployment and incident automation;
- `zap-pack-industrial`: industrial control with simulation and PoA defaults;
- `zap-pack-personal-ai`: personal assistant actions with approval gates.

Recommended next packs:

- `zap-pack-healthcare`: privacy-first care coordination and strict audit;
- `zap-pack-finance`: proposal, risk check, approval, execute, and
  reconciliation flows.

These packs should start as docs and examples, then become signed ZapStore
artifacts once pack installation exists.

## CI Expectations

Official preview packs should pass validation in CI. A pack should not be added
to the website, README, or marketplace planning docs until these are true:

- `zap pack validate --json` returns `valid: true`;
- every referenced schema and policy path exists;
- risk levels are assigned deliberately, not left as low by default;
- high and critical capabilities have an explicit safety gate in the policy
  template;
- the README names the operational boundary, expected grants, audit evidence,
  and actions that remain out of scope.
