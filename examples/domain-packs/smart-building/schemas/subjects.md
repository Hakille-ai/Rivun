# Smart Building Subjects

This preview catalog documents action subjects used by the smart-building pack.
Concrete JSON schemas will be added once domain-pack validation is implemented.

## `sensor.read`

Read environmental, occupancy, energy, or equipment telemetry from a building
sensor.

Expected payload fields:

- `site_id`: building or campus identifier;
- `zone_id`: room, floor, plant room, or equipment zone;
- `sensor_type`: sensor class such as temperature, humidity, co2, occupancy,
  power, water, or fault;
- `window`: current reading or bounded historical interval;
- `reason`: human-readable purpose for the read.

## `thermostat.setpoint`

Set a bounded HVAC heating or cooling target for a zone.

Expected payload fields:

- `site_id`: building or campus identifier;
- `zone_id`: HVAC zone being changed;
- `mode`: heating, cooling, auto, or hold;
- `setpoint_c`: target temperature in Celsius;
- `duration_minutes`: bounded override duration;
- `reason`: comfort, energy, maintenance, or incident response rationale.

## `light.set`

Set lighting state, scene, or brightness for a room, floor, or zone.

Expected payload fields:

- `site_id`: building or campus identifier;
- `zone_id`: lighting zone being changed;
- `state`: on, off, scene, or brightness;
- `brightness_percent`: optional brightness level from 0 to 100;
- `duration_minutes`: optional bounded override duration;
- `reason`: occupancy, safety, maintenance, or scheduled operation rationale.

## `lock.open`

Open or release an access-controlled door, gate, turnstile, or lock.

Expected payload fields:

- `site_id`: building or campus identifier;
- `access_point_id`: controlled lock or door identifier;
- `release_seconds`: short release duration;
- `requester`: operator, workflow, or incident id requesting access;
- `reason`: emergency, maintenance, visitor access, or security rationale;
- `evidence`: incident, ticket, or approval reference.

## `alarm.arm`

Arm, disarm, or change mode for a building alarm partition.

Expected payload fields:

- `site_id`: building or campus identifier;
- `partition_id`: alarm partition or area identifier;
- `mode`: arm_away, arm_stay, disarm, bypass, or test;
- `effective_at`: immediate or scheduled activation time;
- `reason`: security, maintenance, testing, or incident rationale;
- `approver`: human approver or approval workflow reference.

## `camera.snapshot`

Capture a still image from an approved security camera.

Expected payload fields:

- `site_id`: building or campus identifier;
- `camera_id`: approved camera identifier;
- `purpose`: security, safety, facilities, or incident review purpose;
- `retention_minutes`: bounded retention period for the snapshot;
- `redaction`: requested privacy treatment such as none, faces, or people;
- `approver`: human approver or approval workflow reference.
