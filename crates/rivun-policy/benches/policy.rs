use criterion::{Criterion, criterion_group, criterion_main};
use std::{collections::BTreeSet, hint::black_box};
use rivun_capability::CapabilityId;
use rivun_policy::{PolicyDecision, PolicyInput, PolicyRule, PolicySet};

fn policy_rules(count: usize, required_capability: &CapabilityId) -> Vec<PolicyRule> {
    let mut rules = Vec::with_capacity(count);
    for index in 0..count.saturating_sub(1) {
        rules.push(PolicyRule {
            name: Some(format!("sensor-{index}")),
            kind: Some("action".to_string()),
            subject: Some(format!("sensor.{index}.*")),
            source_node: None,
            target_node: None,
            content_type: Some("application/json".to_string()),
            decision: PolicyDecision::Allow,
            required_capability: None,
            reason: None,
        });
    }
    rules.push(PolicyRule {
        name: Some("safety-grant".to_string()),
        kind: Some("action".to_string()),
        subject: Some("safety.*".to_string()),
        source_node: None,
        target_node: None,
        content_type: Some("application/json".to_string()),
        decision: PolicyDecision::RequireGrant,
        required_capability: Some(required_capability.clone()),
        reason: Some("safety action requires an explicit grant".to_string()),
    });
    rules
}

fn policy_toml(count: usize) -> String {
    let mut input = String::new();
    for index in 0..count {
        input.push_str("[[rules]]\n");
        input.push_str(&format!("name = \"rule-{index}\"\n"));
        input.push_str("kind = \"action\"\n");
        input.push_str(&format!("subject = \"sensor.{index}.*\"\n"));
        input.push_str("content_type = \"application/json\"\n");
        input.push_str("decision = \"allow\"\n\n");
    }
    input
}

fn policy(c: &mut Criterion) {
    let capability = CapabilityId::new("driver.execute:safety.emergency_stop").unwrap();
    let policy = PolicySet::new(policy_rules(64, &capability)).unwrap();
    let mut grants = BTreeSet::new();
    grants.insert(capability);
    let input = PolicyInput {
        kind: "action",
        subject: "safety.emergency_stop",
        source_node: None,
        target_node: None,
        content_type: Some("application/json"),
        consensus_protected: false,
        granted_capabilities: &grants,
        human_approved: false,
        simulation_passed: false,
    };
    let toml = policy_toml(32);

    c.bench_function("policy_evaluate_64_rules_last_match", |b| {
        b.iter(|| black_box(policy.evaluate(black_box(&input))))
    });
    c.bench_function("policy_parse_toml_32_rules", |b| {
        b.iter(|| black_box(PolicySet::from_toml_str(black_box(&toml)).unwrap()))
    });
}

criterion_group!(benches, policy);
criterion_main!(benches);
