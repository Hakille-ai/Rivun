# Finance Domain Pack

This preview pack models finance workflows as typed, auditable Rivun subjects. It
is for systems that need to separate proposals, risk checks, approvals,
execution, and reconciliation evidence.

## Safety Posture

- Default policy is fail-closed.
- Account reads, risk checks, and proposals require scoped grants.
- Approval records require human or compliance approval.
- Trade execution, payment execution, and final reconciliation require
  Proof-of-Action.
- Every execution references a deterministic proposal hash and evidence trail.

## Validate

```powershell
cargo run -p rivun-cli -- pack validate --pack examples/domain-packs/finance --json
```
