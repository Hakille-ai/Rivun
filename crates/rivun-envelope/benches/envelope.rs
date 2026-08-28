use bytes::Bytes;
use criterion::{Criterion, criterion_group, criterion_main};
use rivun_envelope::{RivunEnvelope, RivunEnvelopeRef};

fn envelope(c: &mut Criterion) {
    let envelope = RivunEnvelope::action("device.echo", Bytes::from_static(b"benchmark-payload"))
        .unwrap()
        .with_metadata(Bytes::from_static(br#"{"source":"criterion"}"#))
        .unwrap();
    let encoded = envelope.encode();

    c.bench_function("zenv_action_encode", |b| b.iter(|| envelope.encode()));
    c.bench_function("zenv_action_parse", |b| {
        b.iter(|| RivunEnvelopeRef::parse(&encoded).unwrap())
    });
}

criterion_group!(benches, envelope);
criterion_main!(benches);
