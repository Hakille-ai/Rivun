use criterion::{BatchSize, Criterion, criterion_group, criterion_main};
use std::hint::black_box;
use rivun_capability::DriverPermissions;
use rivun_crypto::Keypair;
use rivun_store::{
    DriverManifest, DriverRegistry, RegistryPublication, artifact_hash, registry_hash,
};

fn wasm() -> &'static [u8] {
    b"(module (memory (export \"memory\") 1))"
}

fn driver_manifest(author: &Keypair, action: &str) -> DriverManifest {
    DriverManifest::new(
        action,
        "0.1.0",
        action,
        wasm(),
        DriverPermissions::none(),
        Some("Benchmark driver".to_string()),
        author,
    )
    .unwrap()
}

fn signed_registry(entries: usize, author: &Keypair, operator: &Keypair) -> DriverRegistry {
    let mut registry = DriverRegistry::empty(Some("criterion".to_string()));
    for index in 0..entries {
        registry
            .add_manifest(
                &driver_manifest(author, &format!("driver.{index}")),
                Some(format!("manifests/driver-{index}.toml")),
            )
            .unwrap();
    }
    registry.sign(operator).unwrap();
    registry
}

fn store(c: &mut Criterion) {
    let author = Keypair::generate();
    let operator = Keypair::generate();
    let publisher = Keypair::generate();
    let manifest = driver_manifest(&author, "echo");
    let registry = signed_registry(16, &author, &operator);
    let publication = RegistryPublication::new(
        &registry,
        &publisher,
        123,
        Some("stable".to_string()),
        vec![],
    )
    .unwrap();
    let left = signed_registry(16, &author, &operator);
    let mut right = signed_registry(16, &author, &operator);
    for index in 16..32 {
        right
            .add_manifest(&driver_manifest(&author, &format!("driver.{index}")), None)
            .unwrap();
    }

    c.bench_function("store_manifest_sign", |b| {
        b.iter(|| {
            black_box(
                DriverManifest::new(
                    "echo",
                    "0.1.0",
                    "echo",
                    black_box(wasm()),
                    DriverPermissions::none(),
                    Some("Benchmark driver".to_string()),
                    &author,
                )
                .unwrap(),
            )
        })
    });
    c.bench_function("store_manifest_verify_driver", |b| {
        b.iter(|| {
            manifest
                .verify_for_driver("echo", black_box(wasm()))
                .unwrap();
            black_box(())
        })
    });
    c.bench_function("store_registry_verify_signature_16_entries", |b| {
        b.iter(|| {
            registry.verify_signature().unwrap();
            black_box(())
        })
    });
    c.bench_function("store_registry_hash_16_entries", |b| {
        b.iter(|| black_box(registry_hash(black_box(&registry)).unwrap()))
    });
    c.bench_function("store_registry_merge_32_entries", |b| {
        b.iter_batched(
            || left.clone(),
            |mut registry| black_box(registry.merge_from(black_box(&right)).unwrap()),
            BatchSize::SmallInput,
        )
    });
    c.bench_function("store_publication_verify", |b| {
        b.iter(|| {
            publication
                .verify_for_registry(black_box(&registry), None)
                .unwrap();
            black_box(())
        })
    });
    c.bench_function("store_artifact_hash_4kb", |b| {
        let bytes = vec![0xA5_u8; 4096];
        b.iter(|| black_box(artifact_hash(black_box(&bytes))))
    });
}

criterion_group!(benches, store);
criterion_main!(benches);
