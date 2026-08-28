# RFC and ZEP Process

rivun uses lightweight RFCs and rivun Enhancement Proposals (ZEPs) for changes that
affect long-lived contracts. The goal is to make protocol, crypto, ABI, config,
SDK, and domain pack decisions reviewable before implementation details harden.

## When a ZEP Is Required

Open a Security / Protocol Change issue first, then write a ZEP for any change
that affects:

- @@@@rivun_HEADER@@WIRE@@ or `ZENV` binary layout, versioning, or negotiation;
- cryptography, signatures, trust roots, replay protection, or key handling;
- driver ABI, host imports, Wasm permissions, or sandbox boundaries;
- node config defaults, policy defaults, governance controls, or release
  authority;
- agent intent, status, action, result, or fixture semantics;
- domain pack manifest fields, schema rules, risk levels, or policy semantics;
- SDK behavior that changes compatibility for existing applications.

Small bug fixes, documentation improvements, internal refactors, and new
examples usually do not need a ZEP unless they redefine a public contract.

## ZEP States

- `draft`: idea is being shaped and may change substantially.
- `review`: maintainers are actively reviewing the proposal.
- `accepted`: design is approved for implementation.
- `implemented`: code, fixtures, docs, and migration notes are merged.
- `deferred`: useful, but not scheduled.
- `rejected`: decision recorded with rationale.
- `superseded`: replaced by a later ZEP.

## Required Sections

Each ZEP should include:

- title, authors, status, creation date, and target release;
- summary of the problem and desired outcome;
- affected contracts and compatibility impact;
- threat model for security, crypto, permissions, or governance changes;
- detailed design, including message shapes, config keys, or manifest fields;
- migration and rollback plan;
- validation plan with fixtures, golden vectors, tests, or interop checks;
- alternatives considered;
- open questions and unresolved risks.

## Review Rules

Protocol, crypto, ABI, config-default, and domain pack contract changes require:

- one maintainer review from the relevant code owner group;
- one security review when trust, keys, sandboxing, permissions, policy, or
  governance is affected;
- updated docs and fixtures before implementation is considered complete;
- explicit migration notes for operators and SDK users when behavior changes.

A ZEP can be accepted before code exists, but it is not implemented until tests,
fixtures, docs, and release notes land with the code.

## Implementation Checklist

- Link the ZEP from the issue and pull request.
- Add or update protocol fixtures and golden vectors where applicable.
- Update `docs/security.md`, `docs/operations.md`, SDK docs, or domain pack docs
  when their contracts change.
- Validate example domain packs with `rivun pack validate`.
- Run workspace tests and targeted SDK or website checks for touched areas.
- Record breaking changes in release notes and migration guidance.

## Template

```markdown
# ZEP-NNN: Short Title

- Status: draft
- Authors:
- Created:
- Target release:
- Related issue:

## Summary

## Motivation

## Affected Contracts

## Threat Model

## Detailed Design

## Compatibility and Migration

## Validation Plan

## Alternatives

## Open Questions
```

