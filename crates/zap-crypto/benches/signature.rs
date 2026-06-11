use bytes::Bytes;
use criterion::{Criterion, criterion_group, criterion_main};
use uuid::Uuid;
use zap_core::{ZapFlags, ZapFrame};
use zap_crypto::{Keypair, sign_frame, verify_frame};

fn verify_signature(c: &mut Criterion) {
    let keypair = Keypair::generate();
    let frame = ZapFrame::with_timestamp(
        keypair.node_id(),
        Uuid::from_bytes([2; 16]),
        ZapFlags::ENCRYPTED,
        42,
        Bytes::from_static(b"benchmark-payload"),
    )
    .unwrap();
    let signed = sign_frame(&keypair, &frame).unwrap();
    let public_key = keypair.verifying_key();

    c.bench_function("ed25519_verify_frame", |b| {
        b.iter(|| verify_frame(&public_key, &signed).unwrap())
    });
}

criterion_group!(benches, verify_signature);
criterion_main!(benches);
