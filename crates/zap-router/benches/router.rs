use criterion::{BatchSize, Criterion, criterion_group, criterion_main};
use std::hint::black_box;
use uuid::Uuid;
use zap_router::{RouteMatch, RouteMessage, RouteRule, RouteTable, RouteTarget};

fn route_rules(count: usize, peer: Uuid) -> Vec<RouteRule> {
    let mut routes = Vec::with_capacity(count);
    for index in 0..count.saturating_sub(1) {
        routes.push(RouteRule {
            name: Some(format!("sensor-{index}")),
            description: None,
            requires_peer_grant: None,
            matches: RouteMatch {
                kind: Some("action".to_string()),
                subject: Some(format!("sensor.{index}.*")),
                content_type: Some("application/json".to_string()),
                ..RouteMatch::default()
            },
            target: RouteTarget::local_driver(format!("sensor.{index}")),
        });
    }
    routes.push(RouteRule {
        name: Some("safety-peer".to_string()),
        description: Some("forward safety actions to the safety peer".to_string()),
        requires_peer_grant: None,
        matches: RouteMatch {
            kind: Some("action".to_string()),
            subject: Some("safety.*".to_string()),
            content_type: Some("application/json".to_string()),
            ..RouteMatch::default()
        },
        target: RouteTarget::peer(peer),
    });
    routes
}

fn router(c: &mut Criterion) {
    let peer = Uuid::from_bytes([9; 16]);
    let routes = route_rules(64, peer);
    let table = RouteTable::new(routes.clone()).unwrap();
    let message = RouteMessage {
        source_node: Uuid::from_bytes([1; 16]),
        target_node: Uuid::from_bytes([2; 16]),
        kind: "action".to_string(),
        subject: "safety.emergency_stop".to_string(),
        content_type: Some("application/json".to_string()),
    };

    c.bench_function("router_decide_64_routes_last_match", |b| {
        b.iter(|| black_box(table.decide(black_box(&message))))
    });
    c.bench_function("router_validate_64_routes", |b| {
        b.iter_batched(
            || routes.clone(),
            |routes| black_box(RouteTable::new(black_box(routes)).unwrap()),
            BatchSize::SmallInput,
        )
    });
}

criterion_group!(benches, router);
criterion_main!(benches);
