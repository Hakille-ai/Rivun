# Machine Connections

`zap-machine` adds a hardware-neutral layer for machine profiles and protocol adapters. It is designed so roadmap work can build against stable concepts now while real serial, TCP, PLC, or fieldbus drivers arrive later behind the same trait.

## Scope

- Device profiles declare a profile id, transport, protocol, health policy, and capabilities.
- Capability mapping converts profile commands, health, and state keys into ZAP `CapabilityId` values.
- `MachineConnection` validates declared commands and payload limits before dispatch.
- `MachineBus` owns multiple device connections and routes commands by device id.
- Built-in adapters are deterministic placeholders: mock, serial-line, TCP length-prefixed frames, and Modbus-like register operations.
- No adapter in this crate opens a real hardware port or socket.

## Profile Shape

A profile pairs an adapter kind with a compatible transport/protocol:

| Adapter | Transport | Protocol |
| --- | --- | --- |
| `mock` | `mock` | `mock` |
| `serial` | `serial` | `serial_line` |
| `tcp` | `tcp` | `tcp_frames` |
| `modbus_like` | `industrial_bus`, `serial`, or `tcp` | `modbus_like` |

Commands are represented as `driver.execute:<command>` capabilities, state keys as `machine.state:<key>`, and health as `machine.health:<profile_id>`.

## Example

```rust
use zap_machine::{
    AdapterKind, CommandSpec, DeviceCapability, DeviceProfile, MachineBus,
    MachineCommand, MachineConnection, MachineId, MockAdapter, ProtocolProfile,
    TransportProfile,
};

let profile = DeviceProfile::new(
    "lab.thermostat",
    "Lab Thermostat",
    AdapterKind::Mock,
    TransportProfile::Mock { channel: "demo".into() },
    ProtocolProfile::Mock,
)?
.with_capability(DeviceCapability::health("lab.thermostat")?)
.with_capability(DeviceCapability::state("temperature.celsius")?)
.with_capability(DeviceCapability::command_spec(
    CommandSpec::new("thermostat.setpoint.write")?.with_max_payload_bytes(16),
)?);

let adapter = MockAdapter::new()
    .with_response("thermostat.setpoint.write", b"accepted".to_vec())?;
let connection = MachineConnection::new(
    "lab.thermostat.1",
    profile,
    Box::new(adapter),
)?;

let mut bus = MachineBus::new();
bus.attach(connection)?;
bus.connect_all()?;
let outcome = bus.execute(
    &MachineId::new("lab.thermostat.1")?,
    MachineCommand::new("thermostat.setpoint.write", b"22.0".to_vec())?,
)?;
assert_eq!(outcome.response, b"accepted");
# Ok::<(), zap_machine::ZapMachineError>(())
```

## Testing Without Hardware

Use `MockAdapter` for business logic tests. It records command history, stores the last payload in machine state, and returns scripted responses.

Use `SerialAdapter::scripted` to verify line framing and response handling without touching a COM device. It writes outbound frames into memory.

Use `TcpAdapter::scripted` to verify length-prefixed command frames without opening a socket.

Use `ModbusLikeAdapter` to model register reads/writes with explicit command-to-operation mappings:

```rust
let adapter = ModbusLikeAdapter::new(7)
    .with_register(40_001, 120)
    .map_command(
        "plc.speed.write",
        ModbusOperation::WritePayloadU16 { register: 40_001 },
    )?;
```

## Current Limits

- Real OS serial ports, TCP sockets, Modbus RTU/TCP, and OPC UA are intentionally out of scope for this crate.
- Adapter execution is synchronous.
- Payloads are opaque bytes. Higher-level command schemas can be layered on top once registry/device manifests need them.
- Health is explicit adapter state, not yet backed by timers or automatic heartbeat polling.
