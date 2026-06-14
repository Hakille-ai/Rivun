use zap_machine::{
    AdapterKind, CommandSpec, DeviceCapability, DeviceProfile, MachineBus, MachineCommand,
    MachineConnection, MachineId, MockAdapter, ProtocolProfile, Result, TransportProfile,
};

fn main() -> Result<()> {
    let profile = DeviceProfile::new(
        "lab.thermostat",
        "Lab Thermostat",
        AdapterKind::Mock,
        TransportProfile::Mock {
            channel: "demo".to_string(),
        },
        ProtocolProfile::Mock,
    )?
    .with_capability(DeviceCapability::health("lab.thermostat")?)
    .with_capability(DeviceCapability::state("temperature.celsius")?)
    .with_capability(DeviceCapability::command_spec(
        CommandSpec::new("thermostat.setpoint.write")?.with_max_payload_bytes(16),
    )?);

    let adapter =
        MockAdapter::new().with_response("thermostat.setpoint.write", b"accepted".to_vec())?;
    let connection = MachineConnection::new("lab.thermostat.1", profile, Box::new(adapter))?;
    let mut bus = MachineBus::new();
    bus.attach(connection)?;
    bus.connect_all()?;

    let outcome = bus.execute(
        &MachineId::new("lab.thermostat.1")?,
        MachineCommand::new("thermostat.setpoint.write", b"22.0".to_vec())?,
    )?;

    println!("{}", String::from_utf8_lossy(&outcome.response));
    Ok(())
}
