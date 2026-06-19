use zap_ops::{GovernanceConfig, ObservabilityConfig};

const EMITTED_ZAP_NODE_METRICS: &[&str] = &[
    "zap_frames_sent_total",
    "zap_frames_received_total",
    "zap_frames_rejected_total",
    "zap_driver_execution_errors_total",
    "zap_peer_trust_status",
    "zap_registry_signature_valid",
    "zap_capability_cache_age_seconds",
    "zap_receipt_log_verify_failures_total",
    "zap_poa_attestation_failures_total",
];

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

#[test]
fn prometheus_and_grafana_only_reference_emitted_zap_metrics() {
    let prometheus_rules = include_str!("../config/prometheus/zap-rules.yml");
    let grafana_dashboard = include_str!("../config/grafana/zap-production-dashboard.json");

    serde_json::from_str::<serde_json::Value>(grafana_dashboard).unwrap();

    let referenced = collect_zap_metric_names(&[prometheus_rules, grafana_dashboard]);
    let mut unknown = referenced
        .into_iter()
        .filter(|metric| !EMITTED_ZAP_NODE_METRICS.contains(&metric.as_str()))
        .collect::<Vec<_>>();
    unknown.sort();
    unknown.dedup();

    assert!(
        unknown.is_empty(),
        "ops configs reference metrics not emitted by ZapNode::metrics_prometheus_text(): {unknown:?}"
    );
}

fn collect_zap_metric_names(inputs: &[&str]) -> Vec<String> {
    inputs
        .iter()
        .flat_map(|input| {
            input
                .split(|ch: char| !ch.is_ascii_alphanumeric() && ch != '_')
                .filter(|token| token.starts_with("zap_"))
                .map(str::to_string)
        })
        .collect()
}
