# Industrial Automation Subjects

Subjects in this pack should use explicit asset identifiers and bounded payloads. Drivers should reject requests that omit site, cell, line, asset, protocol endpoint, safety state, or execution window where those fields are relevant.

## `telemetry.read`

Reads industrial telemetry from PLCs, OPC UA servers, Modbus devices, robot controllers, safety relays, historians, or SCADA adapters.

Expected payload fields:

- `site_id`
- `area_id`
- `asset_id`
- `protocol`
- `points`
- `window`
- `purpose`

## `simulation.run`

Runs a bounded offline, staging, or digital-twin simulation before physical control actions.

Expected payload fields:

- `site_id`
- `asset_id`
- `scenario_id`
- `proposed_actions`
- `expected_state`
- `rollback_plan`
- `simulation_target`

## `opcua.node.write`

Writes a bounded value to an approved OPC UA node.

Expected payload fields:

- `site_id`
- `asset_id`
- `endpoint`
- `namespace`
- `node_id`
- `value`
- `bounds`
- `simulation_receipt`
- `operator_approval`

## `modbus.register.write`

Writes a bounded coil or register value to an approved Modbus device.

Expected payload fields:

- `site_id`
- `asset_id`
- `unit_id`
- `register_type`
- `address`
- `value`
- `bounds`
- `simulation_receipt`
- `operator_approval`

## `robot.motion.command`

Commands bounded robot motion, jog, speed, or program-step execution.

Expected payload fields:

- `site_id`
- `cell_id`
- `robot_id`
- `motion_type`
- `target`
- `speed_limit`
- `safety_state`
- `exclusion_zone_clear`
- `simulation_receipt`
- `proof_of_authority`

## `plc.program.update`

Updates or activates PLC program logic.

Expected payload fields:

- `site_id`
- `line_id`
- `plc_id`
- `program_artifact`
- `change_ticket`
- `diff_summary`
- `simulation_receipt`
- `rollback_plan`
- `proof_of_authority`

## `safety.interlock.override`

Overrides, resets, bypasses, or changes a safety interlock, guard, relay, or e-stop condition.

Expected payload fields:

- `site_id`
- `area_id`
- `safety_device_id`
- `override_type`
- `hazard_analysis`
- `lockout_tagout_evidence`
- `duration`
- `restoration_plan`
- `proof_of_authority`
