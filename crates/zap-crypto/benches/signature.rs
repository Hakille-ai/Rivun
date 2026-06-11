use bytes::Bytes;
use criterion::{Criterion, criterion_group, criterion_main};
use uuid::Uuid;
use zap_core::{ZapFlags, ZapFrame};
use zap_crypto::{Keypair, certify_frame, sign_frame, verify_frame, verify_poa_certificate};

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
    let validator = Keypair::generate();
    let consensus_frame = ZapFrame::with_timestamp(
        keypair.node_id(),
        Uuid::from_bytes([2; 16]),
        ZapFlags::ENCRYPTED | ZapFlags::REQUIRES_CONSENSUS,
        42,
        Bytes::from_static(b"benchmark-payload"),
    )
    .unwrap();
    let signed_consensus = sign_frame(&keypair, &consensus_frame).unwrap();
    let certified = certify_frame(&signed_consensus, 1, std::slice::from_ref(&validator)).unwrap();
    let validators = vec![(validator.node_id(), validator.verifying_key())];

    c.bench_function("ed25519_sign_frame", |b| {
        b.iter(|| sign_frame(&keypair, &frame).unwrap())
    });
    c.bench_function("ed25519_verify_frame", |b| {
        b.iter(|| verify_frame(&public_key, &signed).unwrap())
    });
    c.bench_function("poa_verify_certificate", |b| {
        b.iter(|| verify_poa_certificate(&certified, &validators, 1).unwrap())
    });
}

criterion_group!(benches, verify_signature);
criterion_main!(benches);
