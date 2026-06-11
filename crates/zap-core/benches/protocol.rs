use bytes::Bytes;
use criterion::{Criterion, criterion_group, criterion_main};
use uuid::Uuid;
use zap_core::{ZapFlags, ZapFrame, ZapHeader};

fn protocol(c: &mut Criterion) {
    let frame = ZapFrame::with_timestamp(
        Uuid::from_bytes([1; 16]),
        Uuid::from_bytes([2; 16]),
        ZapFlags::PRIORITY,
        42,
        Bytes::from_static(b"benchmark-payload"),
    )
    .unwrap();
    let header_bytes = frame.header.to_bytes();

    c.bench_function("parse_header_64_bytes", |b| {
        b.iter(|| ZapHeader::parse(&header_bytes).unwrap())
    });
    c.bench_function("encode_frame", |b| b.iter(|| frame.encode()));
}

criterion_group!(benches, protocol);
criterion_main!(benches);
