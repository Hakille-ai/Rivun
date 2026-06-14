use zap_ops::{GovernanceConfig, ObservabilityConfig};

#[test]
fn production_observability_config_validates() {
    let input = include_str!("../config/observability/production.toml");
    ObservabilityConfig::from_toml_str(input).unwrap();
}

#[test]
fn production_governance_config_validates() {
    let input = include_str!("../config/governance/production-governance.toml");
    let config = GovernanceConfig::from_toml_str(input).unwrap();
    assert!(
        config
            .policy_for_action("registry.publication.create")
            .is_some()
    );
}
