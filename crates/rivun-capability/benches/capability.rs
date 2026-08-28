use criterion::{Criterion, criterion_group, criterion_main};
use std::hint::black_box;
use tempfile::TempDir;
use uuid::Uuid;
use rivun_capability::{
    CapabilityAdvertisement, CapabilityGrant, CapabilityId, CapabilityRequirement,
    DriverPermissions, JsonlCapabilityCache, capabilities_for_driver,
};

struct CapabilityCacheBench {
    _temp: TempDir,
    cache: JsonlCapabilityCache,
}

fn capability_at(index: usize) -> CapabilityId {
    CapabilityId::new(format!("driver.execute:bench.{index}")).unwrap()
}

fn advertisement(node_id: Uuid, count: usize) -> CapabilityAdvertisement {
    let mut advertisement = CapabilityAdvertisement::new(node_id);
    for index in 0..count {
        let capability = capability_at(index);
        advertisement.capabilities.insert(capability.clone());
        advertisement.grants.push(CapabilityGrant {
            capability: capability.clone(),
            reason: Some("benchmark grant".to_string()),
        });
        advertisement.requirements.push(CapabilityRequirement {
            capability,
            required: index % 2 == 0,
            reason: None,
        });
    }
    advertisement
}

fn cache_fixture(entries: usize) -> CapabilityCacheBench {
    let temp = tempfile::tempdir().unwrap();
    let cache = JsonlCapabilityCache::open(temp.path().join("capabilities.jsonl"));

    for index in 0..entries {
        let peer = Uuid::from_bytes([index as u8; 16]);
        cache.put(peer, advertisement(peer, 8)).unwrap();
    }

    CapabilityCacheBench { _temp: temp, cache }
}

fn capability(c: &mut Criterion) {
    let mut permissions = DriverPermissions::none();
    permissions.network = true;
    permissions.filesystem = true;
    permissions.clock = true;
    permissions.environment = true;
    permissions.emit_event = true;
    permissions.memory_read = true;
    permissions.memory_write = true;
    permissions.device_call = true;
    permissions.max_host_call_bytes = 8192;

    let advertisement = advertisement(Uuid::from_bytes([7; 16]), 64);
    let requested = (0..64).step_by(8).map(capability_at).collect::<Vec<_>>();
    let cache = cache_fixture(64);

    c.bench_function("capability_permissions_to_set", |b| {
        b.iter(|| {
            black_box(
                capabilities_for_driver(black_box("thermostat.setpoint"), permissions).unwrap(),
            )
        })
    });
    c.bench_function("capability_advertisement_filter_64", |b| {
        b.iter(|| black_box(advertisement.filtered(black_box(&requested))))
    });
    c.bench_function("capability_cache_verify_64_entries", |b| {
        b.iter(|| black_box(cache.cache.verify().unwrap()))
    });
}

criterion_group!(benches, capability);
criterion_main!(benches);
