use bytes::Bytes;
use criterion::{Criterion, criterion_group, criterion_main};
use tokio::runtime::Runtime;
use uuid::Uuid;
use zap_net::{Peer, ZapEndpoint, ZapEndpointConfig};

fn id(byte: u8) -> Uuid {
    Uuid::from_bytes([byte; 16])
}

fn udp_round_trip(c: &mut Criterion) {
    let runtime = Runtime::new().unwrap();
    let (a, b) = runtime.block_on(async {
        let key = [7_u8; 32];
        let a = ZapEndpoint::bind(ZapEndpointConfig::new(
            "127.0.0.1:0".parse().unwrap(),
            id(1),
        ))
        .await
        .unwrap();
        let b = ZapEndpoint::bind(ZapEndpointConfig::new(
            "127.0.0.1:0".parse().unwrap(),
            id(2),
        ))
        .await
        .unwrap();
        a.add_peer(Peer::new(id(2), b.local_addr().unwrap(), key))
            .await;
        b.add_peer(Peer::new(id(1), a.local_addr().unwrap(), key))
            .await;
        (a, b)
    });

    c.bench_function("encrypted_udp_round_trip_local", |bench| {
        bench.to_async(&runtime).iter(|| async {
            a.send(id(2), Bytes::from_static(b"ping")).await.unwrap();
            let inbound = b.recv().await.unwrap();
            assert_eq!(inbound.frame.payload, Bytes::from_static(b"ping"));
        })
    });
}

criterion_group!(benches, udp_round_trip);
criterion_main!(benches);
