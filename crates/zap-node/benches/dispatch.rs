use base64::{Engine as _, engine::general_purpose::STANDARD_NO_PAD};
use bytes::Bytes;
use criterion::{Criterion, criterion_group, criterion_main};
use tempfile::TempDir;
use tokio::runtime::Runtime;
use zap_core::{ZapFlags, ZapFrame, now_micros};
use zap_crypto::{Keypair, sign_frame};
use zap_envelope::ZapEnvelope;
use zap_net::{Peer, ZapEndpoint, ZapEndpointConfig};
use zap_node::{
    DiscoveryConfig, DriverConfig, MemoryConfig, PeerConfig, PeerTrustConfig, PoaConfig,
    ReceiptsConfig, RegistryConfig, RuntimeConfig, SecurityConfig, TrustConfig, ZapNode,
    ZapNodeConfig,
};

fn public_key_string(keypair: &Keypair) -> String {
    STANDARD_NO_PAD.encode(keypair.verifying_key().to_bytes())
}

fn echo_driver_wat() -> &'static str {
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
    "#
}

struct NodeBench {
    _temp: TempDir,
    node: ZapNode,
    sender_endpoint: ZapEndpoint,
    receiver_key: Keypair,
    sender_key: Keypair,
}

impl NodeBench {
    async fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let receiver_key = Keypair::generate();
        let sender_key = Keypair::generate();
        let receiver_key_path = temp.path().join("receiver.key");
        let driver_path = temp.path().join("echo.wat");
        std::fs::write(&receiver_key_path, receiver_key.to_key_file_toml().unwrap()).unwrap();
        std::fs::write(&driver_path, echo_driver_wat()).unwrap();

        let sender_endpoint = ZapEndpoint::bind(ZapEndpointConfig::new(
            "127.0.0.1:0".parse().unwrap(),
            sender_key.node_id(),
        ))
        .await
        .unwrap();
        let transport_key = [0x77_u8; 32];
        let config = ZapNodeConfig {
            bind: "127.0.0.1:0".to_string(),
            key_file: receiver_key_path,
            require_signed: true,
            max_datagram_size: None,
            peers: vec![PeerConfig {
                node_id: sender_key.node_id(),
                addr: sender_endpoint.local_addr().unwrap().to_string(),
                public_key: public_key_string(&sender_key),
                transport_key: transport_key
                    .iter()
                    .map(|byte| format!("{byte:02x}"))
                    .collect(),
                transport_key_epoch: None,
                transport_key_rotated_at_micros: None,
                trust: PeerTrustConfig::default(),
            }],
            drivers: vec![DriverConfig {
                action: "echo".to_string(),
                path: driver_path,
                manifest: None,
            }],
            runtime: RuntimeConfig::default(),
            security: SecurityConfig::default(),
            trust: TrustConfig::default(),
            poa: PoaConfig::default(),
            receipts: ReceiptsConfig::default(),
            registry: RegistryConfig::default(),
            discovery: DiscoveryConfig::default(),
            memory: MemoryConfig::default(),
            capability_policy: zap_node::CapabilityPolicyConfig::default(),
            capability_cache: zap_node::CapabilityCacheConfig::default(),
            message_policy: zap_node::MessagePolicyConfig::default(),
            message_schema: zap_node::MessageSchemaConfig::default(),
            routes: Vec::new(),
        };
        let node = ZapNode::from_config(config).await.unwrap();
        sender_endpoint
            .add_peer(Peer::new(
                receiver_key.node_id(),
                node.local_addr().unwrap(),
                transport_key,
            ))
            .await;

        Self {
            _temp: temp,
            node,
            sender_endpoint,
            receiver_key,
            sender_key,
        }
    }

    async fn dispatch_action(&self) {
        let payload = ZapEnvelope::action("echo", Bytes::from_static(b"benchmark-payload"))
            .unwrap()
            .encode();
        let unsigned = ZapFrame::with_timestamp(
            self.sender_key.node_id(),
            self.receiver_key.node_id(),
            ZapFlags::ENCRYPTED,
            now_micros().unwrap(),
            payload,
        )
        .unwrap();
        let signed = sign_frame(&self.sender_key, &unsigned).unwrap();
        self.sender_endpoint
            .send_frame(self.receiver_key.node_id(), &signed)
            .await
            .unwrap();
        let event = self.node.handle_once().await.unwrap();
        assert_eq!(
            event.output.as_deref(),
            Some(b"benchmark-payload".as_slice())
        );
    }
}

fn dispatch(c: &mut Criterion) {
    let runtime = Runtime::new().unwrap();
    let bench = runtime.block_on(NodeBench::new());
    c.bench_function("node_dispatch_zenv_action", |b| {
        b.to_async(&runtime).iter(|| bench.dispatch_action())
    });
}

criterion_group!(benches, dispatch);
criterion_main!(benches);
