use bytes::Bytes;
use criterion::{Criterion, criterion_group, criterion_main};
use uuid::Uuid;
use rivun_core::{PoaAttestation, PoaTrailer, RivunFlags, RivunFrame, RivunHeader};

fn protocol(c: &mut Criterion) {
    let frame = RivunFrame::with_timestamp(
        Uuid::from_bytes([1; 16]),
        Uuid::from_bytes([2; 16]),
        RivunFlags::PRIORITY,
        42,
        Bytes::from_static(b"benchmark-payload"),
    )
    .unwrap();
    let header_bytes = frame.header.to_bytes();
    let mut certified_frame = RivunFrame::with_timestamp(
        Uuid::from_bytes([1; 16]),
        Uuid::from_bytes([2; 16]),
        RivunFlags::SIGNED | RivunFlags::REQUIRES_CONSENSUS,
        42,
        Bytes::from_static(b"benchmark-payload"),
    )
    .unwrap();
    certified_frame.set_auth([3; 64], [4; 8]);
    certified_frame.set_poa(
        PoaTrailer::new(
            1,
            [5; 32],
            vec![PoaAttestation {
                validator_node: Uuid::from_bytes([6; 16]),
                signature: [7; 64],
            }],
        )
        .unwrap(),
    );
    let certified_bytes = certified_frame.encode();

    c.bench_function("parse_header_64_bytes", |b| {
        b.iter(|| RivunHeader::parse(&header_bytes).unwrap())
    });
    c.bench_function("encode_frame", |b| b.iter(|| frame.encode()));
    c.bench_function("decode_signed_poa_frame", |b| {
        b.iter(|| RivunFrame::decode(&certified_bytes).unwrap())
    });
}

criterion_group!(benches, protocol);
criterion_main!(benches);
