# Agentic Development Pack

This preview domain pack describes auditable coding-agent workflows over ZAP.
It is intentionally conservative: read-only inspection is low risk, source
patching and test execution require explicit grants, and pull-request creation
is treated as a high-risk publication action.

## Capabilities

| Capability | Risk | Purpose |
| --- | --- | --- |
| `repo.read` | low | Read repository files and metadata. |
| `repo.patch` | medium | Prepare or apply scoped patches. |
| `test.run` | medium | Run bounded test or lint commands. |
| `ci.inspect` | low | Inspect CI status and logs. |
| `pr.create` | high | Create or update pull requests. |

## Recommended Policy

Use `policies/action-policy.toml` as a starting point. It requires explicit
grants for repository mutation and test execution, and requires human approval
for pull-request creation.

## Suggested Subjects

The pack reserves these action subjects:

- `repo.read`
- `repo.patch`
- `test.run`
- `ci.inspect`
- `pr.create`

Agent protocol messages such as `zap.agent.intent`, `zap.agent.status`, and
`zap.agent.result` should be used to link a coding task to the actions above.

## Future Work

- add JSON schemas for each action payload;
- add a gateway adapter for local git and test commands;
- add expected receipt fixtures;
- add an end-to-end demo: planner -> patcher -> tester -> reviewer -> PR.
