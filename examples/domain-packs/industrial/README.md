# Industrial Automation Domain Pack

The Industrial Automation pack defines a conservative ZAP profile for PLC, OPC UA, Modbus, robotics, and safety-critical plant operations.

It is designed for simulation-first workflows where autonomous agents can inspect telemetry and prepare bounded plans, but physical plant changes remain gated by explicit grants, human approval, or proof of authority.

## Safety posture

- Fail closed by default. Unlisted actions are denied.
- Simulation-first. Write and motion capabilities require `simulation.run`.
- Bounded operations. Grants should scope asset, protocol endpoint, register or node range, plant area, time window, and maximum delta.
- Human gates for high-risk writes. OPC UA and Modbus writes require human approval.
- Proof of authority for critical operations. Robot motion, PLC program updates, and safety interlock overrides require PoA.
- No silent safety bypass. Safety-related operations must include maintenance ticket, lockout/tagout evidence, and named accountable approvers.

## Capabilities

| Capability | Risk | Gate |
| --- | --- | --- |
| `telemetry.read` | low | Grant |
| `simulation.run` | low | Grant |
| `opcua.node.write` | high | Human approval |
| `modbus.register.write` | high | Human approval |
| `robot.motion.command` | critical | Proof of authority |
| `plc.program.update` | critical | Proof of authority |
| `safety.interlock.override` | critical | Proof of authority |

## Expected deployment pattern

1. Read current telemetry and safety state.
2. Produce a bounded plan with affected assets, expected values, and rollback behavior.
3. Run the plan in a simulator or digital twin.
4. Attach approvals or PoA based on capability risk.
5. Execute through a driver that logs receipts, measurements, and final state.

This pack is a starting point for industrial integrations. Production deployments should extend it with site-specific hazard analysis, compliance requirements, and driver-level safeguards.
