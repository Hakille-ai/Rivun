use criterion::{Criterion, criterion_group, criterion_main};
use zap_runtime::{ExecutionLimits, WasmExecutor};

fn echo_driver() -> Vec<u8> {
    wat::parse_str(
        r#"
        (module
          (memory (export "memory") 1)
          (global $heap (mut i32) (i32.const 1024))
          (func (export "zap_alloc") (param $len i32) (result i32)
            global.get $heap
            global.get $heap
            local.get $len
            i32.add
            global.set $heap)
          (func (export "zap_dealloc") (param i32 i32))
          (func (export "zap_execute")
            (param $action_ptr i32) (param $action_len i32)
            (param $payload_ptr i32) (param $payload_len i32)
            (result i64)
            local.get $payload_ptr
            i64.extend_i32_u
            i64.const 32
            i64.shl
            local.get $payload_len
            i64.extend_i32_u
            i64.or))
        "#,
    )
    .unwrap()
}

fn runtime(c: &mut Criterion) {
    let wasm = echo_driver();
    let executor = WasmExecutor::new().unwrap();
    let driver = executor.compile_and_validate(&wasm).unwrap();
    let limits = ExecutionLimits {
        fuel: 100_000,
        ..ExecutionLimits::default()
    };

    c.bench_function("wasm_compile_and_validate_echo", |b| {
        b.iter(|| executor.compile_and_validate(&wasm).unwrap())
    });
    c.bench_function("wasm_execute_echo", |b| {
        b.iter(|| {
            executor
                .execute(&driver, "echo", b"benchmark-payload", limits)
                .unwrap()
        })
    });
}

criterion_group!(benches, runtime);
criterion_main!(benches);
