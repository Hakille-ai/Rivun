use criterion::{Criterion, criterion_group, criterion_main};
use std::hint::black_box;
use rivun_schema::{
    BodyContract, BodyFormat, MessageContract, MessageContractSet, MessageParts, MetadataContract,
};

fn json_contract(name: &str, subject: &str) -> MessageContract {
    MessageContract {
        schema_version: 1,
        name: Some(name.to_string()),
        kind: "action".to_string(),
        subject: subject.to_string(),
        content_type: Some("application/json".to_string()),
        max_body_bytes: Some(256),
        body: BodyContract {
            format: BodyFormat::JsonObject,
            required_json_fields: vec!["temperature_c".to_string()],
            allowed_json_fields: vec!["temperature_c".to_string(), "mode".to_string()],
        },
        metadata: MetadataContract {
            max_bytes: Some(128),
            json_object: true,
            required_json_fields: vec!["source".to_string()],
        },
    }
}

fn contract_set(count: usize) -> MessageContractSet {
    let mut contracts = Vec::with_capacity(count);
    for index in 0..count.saturating_sub(1) {
        contracts.push(json_contract(
            &format!("sensor-{index}"),
            &format!("sensor.{index}.*"),
        ));
    }
    contracts.push(json_contract("setpoint", "thermostat.*"));
    MessageContractSet::new(true, contracts).unwrap()
}

fn contract_set_toml(count: usize) -> String {
    let mut input = "require_match = true\n\n".to_string();
    for index in 0..count {
        input.push_str("[[contracts]]\n");
        input.push_str("schema_version = 1\n");
        input.push_str(&format!("name = \"sensor-{index}\"\n"));
        input.push_str("kind = \"action\"\n");
        input.push_str(&format!("subject = \"sensor.{index}.*\"\n"));
        input.push_str("content_type = \"application/json\"\n");
        input.push_str("max_body_bytes = 256\n\n");
        input.push_str("[contracts.body]\n");
        input.push_str("format = \"json_object\"\n");
        input.push_str("required_json_fields = [\"temperature_c\"]\n");
        input.push_str("allowed_json_fields = [\"temperature_c\", \"mode\"]\n\n");
        input.push_str("[contracts.metadata]\n");
        input.push_str("max_bytes = 128\n");
        input.push_str("json_object = true\n");
        input.push_str("required_json_fields = [\"source\"]\n\n");
    }
    input
}

fn schema(c: &mut Criterion) {
    let contract = json_contract("setpoint", "thermostat.*");
    let set = contract_set(32);
    let parts = MessageParts {
        kind: "action",
        subject: "thermostat.setpoint",
        content_type: Some("application/json"),
        metadata: br#"{"source":"criterion"}"#,
        body: br#"{"temperature_c":20,"mode":"heat"}"#,
    };
    let toml = contract_set_toml(16);

    c.bench_function("schema_validate_json_contract", |b| {
        b.iter(|| {
            contract.validate_message(black_box(&parts)).unwrap();
            black_box(())
        })
    });
    c.bench_function("schema_contract_set_match_32", |b| {
        b.iter(|| {
            set.validate_message(black_box(&parts)).unwrap();
            black_box(())
        })
    });
    c.bench_function("schema_parse_toml_contract_set_16", |b| {
        b.iter(|| black_box(MessageContractSet::from_toml_str(black_box(&toml)).unwrap()))
    });
}

criterion_group!(benches, schema);
criterion_main!(benches);
