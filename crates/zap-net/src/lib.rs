//! Encrypted UDP transport for ZAP.
//!
//! V1 intentionally uses static peer discovery: every peer has a stable node id,
//! UDP address, and 32-byte transport key. The module also exposes a Noise
//! handshake helper that can be used by higher layers to derive those transport
//! keys dynamically.

use bytes::{Bytes, BytesMut};
use chacha20poly1305::{
    ChaCha20Poly1305, Nonce,
    aead::{Aead, KeyInit, Payload},
};
use rand_core::{OsRng, RngCore};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet, VecDeque},
    net::SocketAddr,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};
use thiserror::Error;
use tokio::{net::UdpSocket, sync::RwLock};
use uuid::Uuid;
use zap_core::{ZapError as CoreError, ZapFlags, ZapFrame};

pub mod noise {
    use super::{Result, ZapNetError};
    use snow::{Builder, params::NoiseParams};

    pub const NOISE_PATTERN: &str = "Noise_NN_25519_ChaChaPoly_BLAKE2s";

    pub struct NoiseHandshake {
        state: snow::HandshakeState,
    }

    impl NoiseHandshake {
        pub fn initiator() -> Result<Self> {
            let params = NOISE_PATTERN
                .parse::<NoiseParams>()
                .map_err(|err| ZapNetError::Noise(err.to_string()))?;
            Ok(Self {
                state: Builder::new(params)
                    .build_initiator()
                    .map_err(|err| ZapNetError::Noise(err.to_string()))?,
            })
        }

        pub fn responder() -> Result<Self> {
            let params = NOISE_PATTERN
                .parse::<NoiseParams>()
                .map_err(|err| ZapNetError::Noise(err.to_string()))?;
            Ok(Self {
                state: Builder::new(params)
                    .build_responder()
                    .map_err(|err| ZapNetError::Noise(err.to_string()))?,
            })
        }

        pub fn write_message(&mut self, payload: &[u8]) -> Result<Vec<u8>> {
            let mut out = vec![0_u8; payload.len() + 128];
            let len = self
                .state
                .write_message(payload, &mut out)
                .map_err(|err| ZapNetError::Noise(err.to_string()))?;
            out.truncate(len);
            Ok(out)
        }

        pub fn read_message(&mut self, message: &[u8]) -> Result<Vec<u8>> {
            let mut out = vec![0_u8; message.len() + 128];
            let len = self
                .state
                .read_message(message, &mut out)
                .map_err(|err| ZapNetError::Noise(err.to_string()))?;
            out.truncate(len);
            Ok(out)
        }

        pub fn into_transport_key(self) -> Result<[u8; 32]> {
            let mut transport = self
                .state
                .into_transport_mode()
                .map_err(|err| ZapNetError::Noise(err.to_string()))?;
            let mut key = [0_u8; 32];
            let mut ciphertext = [0_u8; 64];
            let len = transport
                .write_message(b"zap-transport-key-v1", &mut ciphertext)
                .map_err(|err| ZapNetError::Noise(err.to_string()))?;
            let digest = blake3::hash(&ciphertext[..len]);
            key.copy_from_slice(digest.as_bytes());
            Ok(key)
        }
    }
}

const DATAGRAM_MAGIC: [u8; 4] = *b"ZAPD";
const DATAGRAM_VERSION: u8 = 1;
const DATAGRAM_HEADER_LEN: usize = 52;
const DEFAULT_MAX_DATAGRAM_SIZE: usize = 65_507;
const DEFAULT_INBOUND_NONCE_CACHE_CAPACITY: usize = 4096;
const NONCE_LEN: usize = 12;
const NONCE_PREFIX_LEN: usize = 4;
const NONCE_COUNTER_RANDOM_MASK: u64 = u64::MAX >> 1;
const KEY_LEN: usize = 32;

#[derive(Debug, Error)]
pub enum ZapNetError {
    #[error(transparent)]
    Core(#[from] CoreError),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("unknown peer {0}")]
    UnknownPeer(Uuid),
    #[error("peer address mismatch for {node_id}: expected {expected}, got {actual}")]
    PeerAddressMismatch {
        node_id: Uuid,
        expected: SocketAddr,
        actual: SocketAddr,
    },
    #[error("invalid datagram magic")]
    InvalidDatagramMagic,
    #[error("unsupported datagram version {0}")]
    UnsupportedDatagramVersion(u8),
    #[error("nonzero datagram reserved bytes: {0:02x?}")]
    NonzeroDatagramReserved([u8; 3]),
    #[error("datagram too short: expected at least {expected} bytes, got {actual}")]
    DatagramTooShort { expected: usize, actual: usize },
    #[error("datagram exceeds maximum size {max}: {actual}")]
    DatagramTooLarge { max: usize, actual: usize },
    #[error("AEAD encryption or decryption failed")]
    Aead,
    #[error("transport nonce counter exhausted; rotate the transport key")]
    NonceCounterExhausted,
    #[error("replayed datagram nonce from peer {node_id}")]
    ReplayedDatagramNonce { node_id: Uuid },
    #[error("decrypted frame source mismatch: envelope {envelope}, frame {frame}")]
    SourceMismatch { envelope: Uuid, frame: Uuid },
    #[error("decrypted frame target mismatch: envelope {envelope}, frame {frame}")]
    TargetMismatch { envelope: Uuid, frame: Uuid },
    #[error("outbound frame source {frame} does not match endpoint {endpoint}")]
    OutboundSourceMismatch { endpoint: Uuid, frame: Uuid },
    #[error("outbound frame target {frame} does not match requested peer {target}")]
    OutboundTargetMismatch { target: Uuid, frame: Uuid },
    #[error("broadcast frame target must be nil, got {0}")]
    InvalidBroadcastTarget(Uuid),
    #[error("frame target {0} is not this endpoint")]
    NotForThisNode(Uuid),
    #[error("invalid transport key length: expected 32 bytes, got {0}")]
    InvalidTransportKeyLength(usize),
    #[error("invalid transport key hex: {0}")]
    InvalidTransportKeyHex(#[from] hex::FromHexError),
    #[error("Noise handshake failed: {0}")]
    Noise(String),
}

pub type Result<T> = std::result::Result<T, ZapNetError>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransportKey(pub [u8; KEY_LEN]);

impl TransportKey {
    pub fn from_hex(input: &str) -> Result<Self> {
        let bytes = hex::decode(input)?;
        if bytes.len() != KEY_LEN {
            return Err(ZapNetError::InvalidTransportKeyLength(bytes.len()));
        }
        Ok(Self(bytes.try_into().unwrap()))
    }

    pub fn to_hex(&self) -> String {
        hex::encode(self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Peer {
    pub node_id: Uuid,
    pub addr: SocketAddr,
    pub transport_key: TransportKey,
}

impl Peer {
    pub fn new(node_id: Uuid, addr: SocketAddr, transport_key: [u8; KEY_LEN]) -> Self {
        Self {
            node_id,
            addr,
            transport_key: TransportKey(transport_key),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ZapEndpointConfig {
    pub bind: SocketAddr,
    pub node_id: Uuid,
    pub peers: Vec<Peer>,
    pub max_datagram_size: usize,
    pub inbound_nonce_cache_capacity: usize,
    nonce_prefix: Option<[u8; NONCE_PREFIX_LEN]>,
}

impl ZapEndpointConfig {
    pub fn new(bind: SocketAddr, node_id: Uuid) -> Self {
        Self {
            bind,
            node_id,
            peers: Vec::new(),
            max_datagram_size: DEFAULT_MAX_DATAGRAM_SIZE,
            inbound_nonce_cache_capacity: DEFAULT_INBOUND_NONCE_CACHE_CAPACITY,
            nonce_prefix: None,
        }
    }

    #[cfg(test)]
    fn with_nonce_prefix(mut self, nonce_prefix: [u8; NONCE_PREFIX_LEN]) -> Self {
        self.nonce_prefix = Some(nonce_prefix);
        self
    }
}

#[derive(Debug)]
pub struct InboundZap {
    pub peer: Peer,
    pub from_addr: SocketAddr,
    pub frame: ZapFrame,
}

#[derive(Debug, Default)]
struct PeerTables {
    by_id: HashMap<Uuid, Peer>,
    by_addr: HashMap<SocketAddr, Uuid>,
    inbound_nonces: HashMap<Uuid, NonceReplayCache>,
}

#[derive(Debug)]
pub struct ZapEndpoint {
    socket: Arc<UdpSocket>,
    node_id: Uuid,
    peers: Arc<RwLock<PeerTables>>,
    nonce_prefix: [u8; NONCE_PREFIX_LEN],
    next_nonce: AtomicU64,
    max_datagram_size: usize,
    inbound_nonce_cache_capacity: usize,
}

impl ZapEndpoint {
    pub async fn bind(config: ZapEndpointConfig) -> Result<Self> {
        let socket = UdpSocket::bind(config.bind).await?;
        let endpoint = Self {
            socket: Arc::new(socket),
            node_id: config.node_id,
            peers: Arc::new(RwLock::new(PeerTables::default())),
            nonce_prefix: config.nonce_prefix.unwrap_or_else(random_nonce_prefix),
            next_nonce: AtomicU64::new(random_nonce_counter()),
            max_datagram_size: config.max_datagram_size.max(DATAGRAM_HEADER_LEN + 16),
            inbound_nonce_cache_capacity: config.inbound_nonce_cache_capacity,
        };

        for peer in config.peers {
            endpoint.add_peer(peer).await;
        }

        Ok(endpoint)
    }

    pub fn node_id(&self) -> Uuid {
        self.node_id
    }

    pub fn local_addr(&self) -> Result<SocketAddr> {
        Ok(self.socket.local_addr()?)
    }

    pub fn nonce_prefix(&self) -> [u8; NONCE_PREFIX_LEN] {
        self.nonce_prefix
    }

    pub async fn add_peer(&self, peer: Peer) {
        let mut peers = self.peers.write().await;
        peers.by_addr.insert(peer.addr, peer.node_id);
        peers
            .inbound_nonces
            .entry(peer.node_id)
            .or_insert_with(|| NonceReplayCache::new(self.inbound_nonce_cache_capacity));
        peers.by_id.insert(peer.node_id, peer);
    }

    pub async fn send(&self, target: Uuid, payload: Bytes) -> Result<()> {
        let frame = ZapFrame::new(self.node_id, target, ZapFlags::ENCRYPTED, payload)?;
        self.send_frame(target, &frame).await
    }

    pub async fn send_frame(&self, target: Uuid, frame: &ZapFrame) -> Result<()> {
        self.validate_outbound_frame(target, frame)?;
        let peer = self.peer_by_id(target).await?;
        let encoded = frame.encode();
        let datagram = encrypt_datagram(
            self.node_id,
            target,
            self.nonce_prefix,
            self.next_nonce_counter()?,
            &peer.transport_key,
            &encoded,
        )?;

        if datagram.len() > self.max_datagram_size {
            return Err(ZapNetError::DatagramTooLarge {
                max: self.max_datagram_size,
                actual: datagram.len(),
            });
        }

        self.socket.send_to(&datagram, peer.addr).await?;
        Ok(())
    }

    fn validate_outbound_frame(&self, target: Uuid, frame: &ZapFrame) -> Result<()> {
        if frame.header.source_node != self.node_id {
            return Err(ZapNetError::OutboundSourceMismatch {
                endpoint: self.node_id,
                frame: frame.header.source_node,
            });
        }
        if frame.header.flags.contains(ZapFlags::BROADCAST) {
            if frame.header.target_node != Uuid::nil() {
                return Err(ZapNetError::InvalidBroadcastTarget(
                    frame.header.target_node,
                ));
            }
        } else if frame.header.target_node != target {
            return Err(ZapNetError::OutboundTargetMismatch {
                target,
                frame: frame.header.target_node,
            });
        }
        Ok(())
    }

    pub async fn broadcast(&self, payload: Bytes) -> Result<()> {
        let peers = self.peers.read().await;
        let targets: Vec<Uuid> = peers.by_id.keys().copied().collect();
        drop(peers);

        let frame = ZapFrame::new(
            self.node_id,
            Uuid::nil(),
            ZapFlags::ENCRYPTED | ZapFlags::BROADCAST,
            payload,
        )?;
        for target in targets {
            self.send_frame(target, &frame).await?;
        }
        Ok(())
    }

    pub async fn recv(&self) -> Result<InboundZap> {
        let mut buf = vec![0_u8; self.max_datagram_size];
        let (len, from_addr) = self.socket.recv_from(&mut buf).await?;
        buf.truncate(len);
        self.decode_inbound(&buf, from_addr).await
    }

    async fn peer_by_id(&self, node_id: Uuid) -> Result<Peer> {
        self.peers
            .read()
            .await
            .by_id
            .get(&node_id)
            .cloned()
            .ok_or(ZapNetError::UnknownPeer(node_id))
    }

    async fn decode_inbound(&self, datagram: &[u8], from_addr: SocketAddr) -> Result<InboundZap> {
        if datagram.len() > self.max_datagram_size {
            return Err(ZapNetError::DatagramTooLarge {
                max: self.max_datagram_size,
                actual: datagram.len(),
            });
        }

        let envelope = DatagramEnvelope::parse(datagram)?;
        if envelope.target != self.node_id {
            return Err(ZapNetError::NotForThisNode(envelope.target));
        }
        let peer = self.peer_by_id(envelope.source).await?;
        if peer.addr != from_addr {
            return Err(ZapNetError::PeerAddressMismatch {
                node_id: peer.node_id,
                expected: peer.addr,
                actual: from_addr,
            });
        }

        let plaintext = decrypt_datagram(envelope, &peer.transport_key, datagram)?;
        let frame = ZapFrame::decode(&plaintext)?;

        if frame.header.source_node != envelope.source {
            return Err(ZapNetError::SourceMismatch {
                envelope: envelope.source,
                frame: frame.header.source_node,
            });
        }
        if frame.header.flags.contains(ZapFlags::BROADCAST) {
            if frame.header.target_node != Uuid::nil() {
                return Err(ZapNetError::InvalidBroadcastTarget(
                    frame.header.target_node,
                ));
            }
        } else if frame.header.target_node != envelope.target {
            return Err(ZapNetError::TargetMismatch {
                envelope: envelope.target,
                frame: frame.header.target_node,
            });
        }

        self.remember_inbound_nonce(envelope.source, envelope.nonce)
            .await?;

        Ok(InboundZap {
            peer,
            from_addr,
            frame,
        })
    }

    fn next_nonce_counter(&self) -> Result<u64> {
        self.next_nonce
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
                current.checked_add(1)
            })
            .map_err(|_| ZapNetError::NonceCounterExhausted)
    }

    async fn remember_inbound_nonce(&self, node_id: Uuid, nonce: [u8; NONCE_LEN]) -> Result<()> {
        let mut peers = self.peers.write().await;
        let cache = peers
            .inbound_nonces
            .entry(node_id)
            .or_insert_with(|| NonceReplayCache::new(self.inbound_nonce_cache_capacity));
        cache.remember(node_id, nonce)
    }
}

#[derive(Debug, Clone, Copy)]
struct DatagramEnvelope {
    source: Uuid,
    target: Uuid,
    nonce: [u8; NONCE_LEN],
}

impl DatagramEnvelope {
    fn parse(datagram: &[u8]) -> Result<Self> {
        if datagram.len() < DATAGRAM_HEADER_LEN {
            return Err(ZapNetError::DatagramTooShort {
                expected: DATAGRAM_HEADER_LEN,
                actual: datagram.len(),
            });
        }
        if datagram[0..4] != DATAGRAM_MAGIC {
            return Err(ZapNetError::InvalidDatagramMagic);
        }
        if datagram[4] != DATAGRAM_VERSION {
            return Err(ZapNetError::UnsupportedDatagramVersion(datagram[4]));
        }
        let reserved = [datagram[5], datagram[6], datagram[7]];
        if reserved != [0_u8; 3] {
            return Err(ZapNetError::NonzeroDatagramReserved(reserved));
        }

        let source = Uuid::from_bytes(datagram[8..24].try_into().unwrap());
        let target = Uuid::from_bytes(datagram[24..40].try_into().unwrap());
        let mut nonce = [0_u8; NONCE_LEN];
        nonce.copy_from_slice(&datagram[40..52]);
        Ok(Self {
            source,
            target,
            nonce,
        })
    }
}

#[derive(Debug)]
struct NonceReplayCache {
    capacity: usize,
    seen: HashSet<[u8; NONCE_LEN]>,
    order: VecDeque<[u8; NONCE_LEN]>,
}

impl NonceReplayCache {
    fn new(capacity: usize) -> Self {
        Self {
            capacity,
            seen: HashSet::with_capacity(capacity.min(DEFAULT_INBOUND_NONCE_CACHE_CAPACITY)),
            order: VecDeque::with_capacity(capacity.min(DEFAULT_INBOUND_NONCE_CACHE_CAPACITY)),
        }
    }

    fn remember(&mut self, node_id: Uuid, nonce: [u8; NONCE_LEN]) -> Result<()> {
        if self.capacity == 0 {
            return Ok(());
        }
        if self.seen.contains(&nonce) {
            return Err(ZapNetError::ReplayedDatagramNonce { node_id });
        }

        self.seen.insert(nonce);
        self.order.push_back(nonce);
        while self.order.len() > self.capacity {
            if let Some(expired) = self.order.pop_front() {
                self.seen.remove(&expired);
            }
        }
        Ok(())
    }
}

fn encrypt_datagram(
    source: Uuid,
    target: Uuid,
    nonce_prefix: [u8; NONCE_PREFIX_LEN],
    nonce_counter: u64,
    key: &TransportKey,
    plaintext: &[u8],
) -> Result<Bytes> {
    let mut header = [0_u8; DATAGRAM_HEADER_LEN];
    header[0..4].copy_from_slice(&DATAGRAM_MAGIC);
    header[4] = DATAGRAM_VERSION;
    header[8..24].copy_from_slice(source.as_bytes());
    header[24..40].copy_from_slice(target.as_bytes());
    header[40..44].copy_from_slice(&nonce_prefix);
    header[44..52].copy_from_slice(&nonce_counter.to_be_bytes());

    let cipher = ChaCha20Poly1305::new_from_slice(&key.0).map_err(|_| ZapNetError::Aead)?;
    let ciphertext = cipher
        .encrypt(
            Nonce::from_slice(&header[40..52]),
            Payload {
                msg: plaintext,
                aad: &header,
            },
        )
        .map_err(|_| ZapNetError::Aead)?;

    let mut out = BytesMut::with_capacity(header.len() + ciphertext.len());
    out.extend_from_slice(&header);
    out.extend_from_slice(&ciphertext);
    Ok(out.freeze())
}

fn random_nonce_prefix() -> [u8; NONCE_PREFIX_LEN] {
    let mut prefix = [0_u8; NONCE_PREFIX_LEN];
    OsRng.fill_bytes(&mut prefix);
    prefix
}

fn random_nonce_counter() -> u64 {
    OsRng.next_u64() & NONCE_COUNTER_RANDOM_MASK
}

fn decrypt_datagram(
    envelope: DatagramEnvelope,
    key: &TransportKey,
    datagram: &[u8],
) -> Result<Bytes> {
    let header = &datagram[..DATAGRAM_HEADER_LEN];
    let ciphertext = &datagram[DATAGRAM_HEADER_LEN..];
    let cipher = ChaCha20Poly1305::new_from_slice(&key.0).map_err(|_| ZapNetError::Aead)?;
    let plaintext = cipher
        .decrypt(
            Nonce::from_slice(&envelope.nonce),
            Payload {
                msg: ciphertext,
                aad: header,
            },
        )
        .map_err(|_| ZapNetError::Aead)?;
    Ok(Bytes::from(plaintext))
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::Bytes;
    use tokio::time::{Duration, timeout};

    fn id(byte: u8) -> Uuid {
        Uuid::from_bytes([byte; 16])
    }

    #[tokio::test]
    async fn endpoints_exchange_encrypted_frames() {
        let key = [42_u8; 32];
        let a = ZapEndpoint::bind(ZapEndpointConfig::new(
            "127.0.0.1:0".parse().unwrap(),
            id(1),
        ))
        .await
        .unwrap();
        let b = ZapEndpoint::bind(ZapEndpointConfig::new(
            "127.0.0.1:0".parse().unwrap(),
            id(2),
        ))
        .await
        .unwrap();

        a.add_peer(Peer::new(id(2), b.local_addr().unwrap(), key))
            .await;
        b.add_peer(Peer::new(id(1), a.local_addr().unwrap(), key))
            .await;

        a.send(id(2), Bytes::from_static(b"ping")).await.unwrap();
        let inbound = timeout(Duration::from_secs(2), b.recv())
            .await
            .unwrap()
            .unwrap();

        assert_eq!(inbound.peer.node_id, id(1));
        assert_eq!(inbound.frame.header.source_node, id(1));
        assert_eq!(inbound.frame.header.target_node, id(2));
        assert_eq!(inbound.frame.payload, Bytes::from_static(b"ping"));
    }

    #[tokio::test]
    async fn broadcast_sends_nil_target_frames_to_all_peers() {
        let ab_key = [42_u8; 32];
        let ac_key = [43_u8; 32];
        let a = ZapEndpoint::bind(ZapEndpointConfig::new(
            "127.0.0.1:0".parse().unwrap(),
            id(1),
        ))
        .await
        .unwrap();
        let b = ZapEndpoint::bind(ZapEndpointConfig::new(
            "127.0.0.1:0".parse().unwrap(),
            id(2),
        ))
        .await
        .unwrap();
        let c = ZapEndpoint::bind(ZapEndpointConfig::new(
            "127.0.0.1:0".parse().unwrap(),
            id(3),
        ))
        .await
        .unwrap();

        a.add_peer(Peer::new(id(2), b.local_addr().unwrap(), ab_key))
            .await;
        a.add_peer(Peer::new(id(3), c.local_addr().unwrap(), ac_key))
            .await;
        b.add_peer(Peer::new(id(1), a.local_addr().unwrap(), ab_key))
            .await;
        c.add_peer(Peer::new(id(1), a.local_addr().unwrap(), ac_key))
            .await;

        a.broadcast(Bytes::from_static(b"announce")).await.unwrap();
        let inbound_b = timeout(Duration::from_secs(2), b.recv())
            .await
            .unwrap()
            .unwrap();
        let inbound_c = timeout(Duration::from_secs(2), c.recv())
            .await
            .unwrap()
            .unwrap();

        for inbound in [inbound_b, inbound_c] {
            assert_eq!(inbound.peer.node_id, id(1));
            assert_eq!(inbound.frame.header.source_node, id(1));
            assert_eq!(inbound.frame.header.target_node, Uuid::nil());
            assert!(inbound.frame.header.flags.contains(ZapFlags::BROADCAST));
            assert_eq!(inbound.frame.payload, Bytes::from_static(b"announce"));
        }
    }

    #[test]
    fn nonce_prefix_is_encoded_into_datagram_nonce() {
        let key = TransportKey([42_u8; 32]);
        let source = id(1);
        let target = id(2);
        let datagram =
            encrypt_datagram(source, target, [0xAA, 0xBB, 0xCC, 0xDD], 7, &key, b"hello").unwrap();
        let envelope = DatagramEnvelope::parse(&datagram).unwrap();

        assert_eq!(envelope.nonce[0..4], [0xAA, 0xBB, 0xCC, 0xDD]);
        assert_eq!(envelope.nonce[4..12], 7_u64.to_be_bytes());
        assert_eq!(
            decrypt_datagram(envelope, &key, &datagram).unwrap(),
            b"hello".as_slice()
        );
    }

    #[test]
    fn datagram_parse_rejects_nonzero_reserved_bytes() {
        let key = TransportKey([42_u8; 32]);
        let mut datagram =
            encrypt_datagram(id(1), id(2), [0xAA, 0xBB, 0xCC, 0xDD], 7, &key, b"hello")
                .unwrap()
                .to_vec();
        datagram[5] = 1;

        assert!(matches!(
            DatagramEnvelope::parse(&datagram),
            Err(ZapNetError::NonzeroDatagramReserved([1, 0, 0]))
        ));
    }

    #[tokio::test]
    async fn endpoint_rejects_replayed_datagram_nonce() {
        let key = TransportKey([42_u8; 32]);
        let source = id(1);
        let target = id(2);
        let peer_addr = "127.0.0.1:9001".parse().unwrap();
        let endpoint = ZapEndpoint::bind(ZapEndpointConfig::new(
            "127.0.0.1:0".parse().unwrap(),
            target,
        ))
        .await
        .unwrap();
        endpoint.add_peer(Peer::new(source, peer_addr, key.0)).await;

        let frame = ZapFrame::with_timestamp(
            source,
            target,
            ZapFlags::ENCRYPTED,
            42,
            Bytes::from_static(b"ping"),
        )
        .unwrap();
        let datagram = encrypt_datagram(
            source,
            target,
            [0xAA, 0xBB, 0xCC, 0xDD],
            7,
            &key,
            &frame.encode(),
        )
        .unwrap();

        endpoint.decode_inbound(&datagram, peer_addr).await.unwrap();
        assert!(matches!(
            endpoint.decode_inbound(&datagram, peer_addr).await,
            Err(ZapNetError::ReplayedDatagramNonce { node_id }) if node_id == source
        ));
    }

    #[tokio::test]
    async fn endpoint_rejects_broadcast_frame_with_non_nil_target() {
        let key = TransportKey([42_u8; 32]);
        let source = id(1);
        let target = id(2);
        let peer_addr = "127.0.0.1:9001".parse().unwrap();
        let endpoint = ZapEndpoint::bind(ZapEndpointConfig::new(
            "127.0.0.1:0".parse().unwrap(),
            target,
        ))
        .await
        .unwrap();
        endpoint.add_peer(Peer::new(source, peer_addr, key.0)).await;

        let frame = ZapFrame::with_timestamp(
            source,
            target,
            ZapFlags::ENCRYPTED | ZapFlags::BROADCAST,
            42,
            Bytes::from_static(b"bad-broadcast"),
        )
        .unwrap();
        let datagram = encrypt_datagram(
            source,
            target,
            [0xAA, 0xBB, 0xCC, 0xDD],
            7,
            &key,
            &frame.encode(),
        )
        .unwrap();

        assert!(matches!(
            endpoint.decode_inbound(&datagram, peer_addr).await,
            Err(ZapNetError::InvalidBroadcastTarget(node_id)) if node_id == target
        ));
    }

    #[tokio::test]
    async fn endpoint_rejects_datagram_envelope_targeting_another_node() {
        let key = TransportKey([42_u8; 32]);
        let source = id(1);
        let target = id(2);
        let wrong_target = id(3);
        let peer_addr = "127.0.0.1:9001".parse().unwrap();
        let endpoint = ZapEndpoint::bind(ZapEndpointConfig::new(
            "127.0.0.1:0".parse().unwrap(),
            target,
        ))
        .await
        .unwrap();
        endpoint.add_peer(Peer::new(source, peer_addr, key.0)).await;

        let frame = ZapFrame::with_timestamp(
            source,
            Uuid::nil(),
            ZapFlags::ENCRYPTED | ZapFlags::BROADCAST,
            42,
            Bytes::from_static(b"broadcast"),
        )
        .unwrap();
        let datagram = encrypt_datagram(
            source,
            wrong_target,
            [0xAA, 0xBB, 0xCC, 0xDD],
            7,
            &key,
            &frame.encode(),
        )
        .unwrap();

        assert!(matches!(
            endpoint.decode_inbound(&datagram, peer_addr).await,
            Err(ZapNetError::NotForThisNode(node_id)) if node_id == wrong_target
        ));
    }

    #[tokio::test]
    async fn inbound_nonce_cache_can_be_disabled_for_specialized_tests() {
        let key = TransportKey([42_u8; 32]);
        let source = id(1);
        let target = id(2);
        let peer_addr = "127.0.0.1:9001".parse().unwrap();
        let mut config = ZapEndpointConfig::new("127.0.0.1:0".parse().unwrap(), target);
        config.inbound_nonce_cache_capacity = 0;
        let endpoint = ZapEndpoint::bind(config).await.unwrap();
        endpoint.add_peer(Peer::new(source, peer_addr, key.0)).await;

        let frame = ZapFrame::with_timestamp(
            source,
            target,
            ZapFlags::ENCRYPTED,
            42,
            Bytes::from_static(b"ping"),
        )
        .unwrap();
        let datagram = encrypt_datagram(
            source,
            target,
            [0xAA, 0xBB, 0xCC, 0xDD],
            7,
            &key,
            &frame.encode(),
        )
        .unwrap();

        endpoint.decode_inbound(&datagram, peer_addr).await.unwrap();
        endpoint.decode_inbound(&datagram, peer_addr).await.unwrap();
    }

    #[tokio::test]
    async fn endpoint_uses_configured_nonce_prefix_for_datagrams() {
        let key = [42_u8; 32];
        let a = ZapEndpoint::bind(
            ZapEndpointConfig::new("127.0.0.1:0".parse().unwrap(), id(1))
                .with_nonce_prefix([1, 2, 3, 4]),
        )
        .await
        .unwrap();
        let b = ZapEndpoint::bind(ZapEndpointConfig::new(
            "127.0.0.1:0".parse().unwrap(),
            id(2),
        ))
        .await
        .unwrap();

        a.add_peer(Peer::new(id(2), b.local_addr().unwrap(), key))
            .await;
        b.add_peer(Peer::new(id(1), a.local_addr().unwrap(), key))
            .await;

        a.send(id(2), Bytes::from_static(b"ping")).await.unwrap();
        let inbound = timeout(Duration::from_secs(2), b.recv())
            .await
            .unwrap()
            .unwrap();

        assert_eq!(a.nonce_prefix(), [1, 2, 3, 4]);
        assert_eq!(inbound.frame.payload, Bytes::from_static(b"ping"));
    }

    #[tokio::test]
    async fn endpoint_randomizes_initial_nonce_counter_with_headroom() {
        let endpoint = ZapEndpoint::bind(ZapEndpointConfig::new(
            "127.0.0.1:0".parse().unwrap(),
            id(1),
        ))
        .await
        .unwrap();

        assert!(endpoint.next_nonce.load(Ordering::Relaxed) <= NONCE_COUNTER_RANDOM_MASK);
    }

    #[tokio::test]
    async fn refuses_to_send_after_nonce_counter_exhaustion() {
        let endpoint = ZapEndpoint::bind(ZapEndpointConfig::new(
            "127.0.0.1:0".parse().unwrap(),
            id(1),
        ))
        .await
        .unwrap();
        endpoint
            .add_peer(Peer::new(
                id(2),
                "127.0.0.1:9".parse().unwrap(),
                [42_u8; 32],
            ))
            .await;
        endpoint.next_nonce.store(u64::MAX, Ordering::Relaxed);

        assert!(matches!(
            endpoint.send(id(2), Bytes::from_static(b"ping")).await,
            Err(ZapNetError::NonceCounterExhausted)
        ));
    }

    #[tokio::test]
    async fn rejects_outbound_frame_with_wrong_source() {
        let endpoint = ZapEndpoint::bind(ZapEndpointConfig::new(
            "127.0.0.1:0".parse().unwrap(),
            id(1),
        ))
        .await
        .unwrap();
        let frame = ZapFrame::with_timestamp(
            id(9),
            id(2),
            ZapFlags::ENCRYPTED,
            42,
            Bytes::from_static(b"ping"),
        )
        .unwrap();

        assert!(matches!(
            endpoint.send_frame(id(2), &frame).await,
            Err(ZapNetError::OutboundSourceMismatch { endpoint: endpoint_id, frame: frame_id })
                if endpoint_id == id(1) && frame_id == id(9)
        ));
    }

    #[tokio::test]
    async fn rejects_outbound_frame_with_wrong_unicast_target() {
        let endpoint = ZapEndpoint::bind(ZapEndpointConfig::new(
            "127.0.0.1:0".parse().unwrap(),
            id(1),
        ))
        .await
        .unwrap();
        let frame = ZapFrame::with_timestamp(
            id(1),
            id(3),
            ZapFlags::ENCRYPTED,
            42,
            Bytes::from_static(b"ping"),
        )
        .unwrap();

        assert!(matches!(
            endpoint.send_frame(id(2), &frame).await,
            Err(ZapNetError::OutboundTargetMismatch { target, frame: frame_id })
                if target == id(2) && frame_id == id(3)
        ));
    }

    #[tokio::test]
    async fn rejects_outbound_broadcast_frame_with_non_nil_target() {
        let endpoint = ZapEndpoint::bind(ZapEndpointConfig::new(
            "127.0.0.1:0".parse().unwrap(),
            id(1),
        ))
        .await
        .unwrap();
        let frame = ZapFrame::with_timestamp(
            id(1),
            id(2),
            ZapFlags::ENCRYPTED | ZapFlags::BROADCAST,
            42,
            Bytes::from_static(b"broadcast"),
        )
        .unwrap();

        assert!(matches!(
            endpoint.send_frame(id(2), &frame).await,
            Err(ZapNetError::InvalidBroadcastTarget(node_id)) if node_id == id(2)
        ));
    }

    #[tokio::test]
    async fn rejects_unknown_peer() {
        let endpoint = ZapEndpoint::bind(ZapEndpointConfig::new(
            "127.0.0.1:0".parse().unwrap(),
            id(1),
        ))
        .await
        .unwrap();

        assert!(matches!(
            endpoint.send(id(9), Bytes::from_static(b"ping")).await,
            Err(ZapNetError::UnknownPeer(_))
        ));
    }

    #[test]
    fn noise_handshake_can_derive_transport_material() {
        let mut initiator = noise::NoiseHandshake::initiator().unwrap();
        let mut responder = noise::NoiseHandshake::responder().unwrap();

        let msg1 = initiator.write_message(b"hello").unwrap();
        assert_eq!(responder.read_message(&msg1).unwrap(), b"hello");
        let msg2 = responder.write_message(b"world").unwrap();
        assert_eq!(initiator.read_message(&msg2).unwrap(), b"world");

        let initiator_key = initiator.into_transport_key().unwrap();
        let responder_key = responder.into_transport_key().unwrap();
        assert_ne!(initiator_key, [0_u8; 32]);
        assert_ne!(responder_key, [0_u8; 32]);
    }
}
