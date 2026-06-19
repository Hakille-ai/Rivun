# Healthcare Domain Pack

This preview pack models privacy-sensitive healthcare coordination as typed ZAP
subjects. It is designed for care teams that need auditable automation around
patient-record access, alert routing, documentation proposals, device-command
proposals, and evidence-preserving privacy workflows.

## Safety Posture

- Default policy is fail-closed.
- Read access requires scoped grants.
- Alert routing and care-task proposals require explicit capability grants.
- Record writes and device proposals require human approval or simulation.
- Clinical orders and privacy exports require Proof-of-Action.
- Raw protected health information should be referenced by hash or evidence id
  rather than embedded directly in routeable messages.

## Validate

```powershell
cargo run -p zap-cli -- pack validate --pack examples/domain-packs/healthcare --json
```
