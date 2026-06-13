use criterion::{Criterion, criterion_group, criterion_main};
use std::hint::black_box;
use zap_driver_sdk::{
    DriverError, DriverInput, PackedResult, ZapDriver, execute_driver, pack_result, unpack_result,
};

struct EchoDriver;

impl ZapDriver for EchoDriver {
    fn execute(&self, input: DriverInput<'_>) -> Result<Vec<u8>, DriverError> {
        if input.action != "echo" {
            return Err(DriverError::new("unsupported action"));
        }
        Ok(input.payload.to_vec())
    }
}

fn sdk(c: &mut Criterion) {
    let driver = EchoDriver;
    let payload = [0x42_u8; 64];

    c.bench_function("driver_sdk_pack_unpack_result", |b| {
        b.iter(|| {
            let packed = pack_result(black_box(0x1020_3040), black_box(0x5060_7080));
            black_box(unpack_result(black_box(packed)))
        })
    });
    c.bench_function("driver_sdk_execute_trait_echo", |b| {
        b.iter(|| black_box(execute_driver(&driver, "echo", black_box(&payload)).unwrap()))
    });
    c.bench_function("driver_sdk_packed_result_methods", |b| {
        b.iter(|| {
            let packed = PackedResult::new(black_box(0x1020_3040), black_box(0x5060_7080)).pack();
            black_box(PackedResult::unpack(black_box(packed)))
        })
    });
}

criterion_group!(benches, sdk);
criterion_main!(benches);
