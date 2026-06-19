# Smart Building Pack

This preview domain pack describes auditable smart-building workflows over ZAP.
It focuses on common commercial building integrations: environmental telemetry,
HVAC setpoints, lighting, access control, alarms, and camera snapshots.

The pack is intentionally fail-closed. Informational reads and reversible
comfort changes require explicit grants. High and critical actions that affect
security, privacy, or physical access are gated by human approval or
Proof-of-Action.

## Capabilities

| Capability | Risk | Purpose |
| --- | --- | --- |
| `sensor.read` | low | Read building telemetry and occupancy signals. |
| `thermostat.setpoint` | medium | Apply bounded HVAC setpoint overrides. |
| `light.set` | medium | Set lighting state, scene, or brightness. |
| `lock.open` | critical | Release an access-controlled lock or door. |
| `alarm.arm` | high | Arm, disarm, or change alarm partition mode. |
| `camera.snapshot` | high | Capture a still image from a security camera. |

## Recommended Policy

Use `policies/action-policy.toml` as a starting point. It requires explicit
grants for sensor, thermostat, and lighting actions. It requires Proof-of-Action
for `lock.open`, and human approval for `alarm.arm` and `camera.snapshot`.

Operators should narrow grants by site, zone, device, time window, and workflow
purpose in their gateway or node configuration. Door release, alarm, and camera
workflows should attach receipts that identify the requester, approver, target
device, and reason.

## Suggested Subjects

The pack reserves these action subjects:

- `sensor.read`
- `thermostat.setpoint`
- `light.set`
- `lock.open`
- `alarm.arm`
- `camera.snapshot`

Agent protocol messages such as `zap.agent.intent`, `zap.agent.status`, and
`zap.agent.result` should be used to link building automations to the actions
above.

## Operating Notes

- Keep HVAC and lighting overrides time-bounded so normal schedules resume.
- Treat camera snapshots as sensitive data and set short retention windows.
- Require local safety and emergency procedures to override automation when
  building occupants are at risk.
- Record lock and alarm actions in the building access-control or security log.

## Future Work

- add JSON schemas for each action payload;
- add route templates for common BAS, BMS, and access-control gateways;
- add expected receipt fixtures for security-sensitive actions;
- add an end-to-end demo: sensor read -> comfort decision -> bounded actuator
  change -> receipt audit.
