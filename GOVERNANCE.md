# Governance

rivun is currently maintained as a small, security-focused open-source project.
The governance model is intentionally lightweight until the contributor base is
larger.

## Roles

- **Maintainers** steward protocol compatibility, releases, security responses,
  and project direction.
- **Reviewers** provide technical review in areas they understand.
- **Contributors** propose patches, docs, tests, examples, and issue reports.

## Decision Principles

1. Preserve security invariants before adding features.
2. Keep the wire protocol stable and versioned.
3. Separate layers: wire, envelope, transport, node policy, runtime, adapters.
4. Prefer simple, inspectable designs over opaque automation.
5. Do not add financial, billing, reward, or settlement behavior to protocol
   primitives.

## Change Approval

Protocol, crypto, runtime isolation, Docker hardening, and release-process
changes require maintainer review. Small docs, examples, or test-only changes
can be accepted with lighter review once CI is green.

## Security Authority

Maintainers may temporarily embargo details, revert risky changes, or cut a
security patch release when a vulnerability affects users.
