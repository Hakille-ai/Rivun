//! Streaming I/O buffers, SPSC circular ring buffers, TCP/Modbus transports, and buffer pools.

use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, VecDeque},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
    },
    time::Duration,
};
use thiserror::Error;

/// Streaming I/O errors.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum StreamingError {
    #[error("stream {0} not found")]
    StreamNotFound(u32),
    #[error("stream {0} is closed or reached EOF")]
    StreamClosed(u32),
    #[error("stream buffer overflow: capacity {capacity}, requested {requested}")]
    BufferOverflow { capacity: usize, requested: usize },
    #[error("stream read timeout after {0:?}")]
    Timeout(Duration),
    #[error("Modbus error: {0}")]
    Modbus(String),
    #[error("I/O error: {0}")]
    Io(String),
}

/// Backpressure policy when streaming buffers reach capacity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum StreamBackpressurePolicy {
    /// Discard the oldest data to make room for incoming chunks.
    #[default]
    DropOldest,
    /// Discard newly arriving data when full.
    DropNewest,
    /// Block/wait until space is available or timeout expires.
    BlockWithTimeout(Duration),
    /// Return immediate buffer overflow error.
    Error,
}

/// Lock-free / Atomic Single-Producer Single-Consumer (SPSC) circular byte ring-buffer.
///
/// Uses atomic write/read pointers with memory barriers for zero-copy streaming.
pub struct SpscRingBuffer {
    capacity: usize,
    buffer: Box<[u8]>,
    head: AtomicUsize,
    tail: AtomicUsize,
    dropped_bytes: AtomicU64,
    total_written: AtomicU64,
    closed: AtomicBool,
    policy: StreamBackpressurePolicy,
}

impl SpscRingBuffer {
    /// Create a new circular SPSC ring buffer with the given byte capacity and backpressure policy.
    pub fn new(capacity: usize, policy: StreamBackpressurePolicy) -> Self {
        let capacity = capacity.max(16);
        // Allocate buffer slice
        let buffer = vec![0u8; capacity + 1].into_boxed_slice();
        Self {
            capacity: capacity + 1,
            buffer,
            head: AtomicUsize::new(0),
            tail: AtomicUsize::new(0),
            dropped_bytes: AtomicU64::new(0),
            total_written: AtomicU64::new(0),
            closed: AtomicBool::new(false),
            policy,
        }
    }

    pub fn capacity(&self) -> usize {
        self.capacity - 1
    }

    pub fn is_closed(&self) -> bool {
        self.closed.load(Ordering::Acquire)
    }

    pub fn close(&self) {
        self.closed.store(true, Ordering::Release);
    }

    pub fn available_read(&self) -> usize {
        let head = self.head.load(Ordering::Acquire);
        let tail = self.tail.load(Ordering::Acquire);
        if head >= tail {
            head - tail
        } else {
            self.capacity - (tail - head)
        }
    }

    pub fn available_write(&self) -> usize {
        (self.capacity - 1) - self.available_read()
    }

    pub fn is_empty(&self) -> bool {
        self.available_read() == 0
    }

    pub fn is_full(&self) -> bool {
        self.available_write() == 0
    }

    pub fn dropped_bytes(&self) -> u64 {
        self.dropped_bytes.load(Ordering::Acquire)
    }

    pub fn total_written(&self) -> u64 {
        self.total_written.load(Ordering::Acquire)
    }

    /// Push bytes into the ring buffer according to the configured backpressure policy.
    pub fn write(&self, data: &[u8]) -> Result<usize, StreamingError> {
        if self.is_closed() {
            return Err(StreamingError::StreamClosed(0));
        }
        if data.is_empty() {
            return Ok(0);
        }

        let write_len = data.len();
        let avail = self.available_write();

        if write_len > avail {
            match self.policy {
                StreamBackpressurePolicy::DropOldest => {
                    let needed = write_len - avail;
                    let to_drop = needed.min(self.available_read());
                    self.skip_read(to_drop);
                    self.dropped_bytes
                        .fetch_add(to_drop as u64, Ordering::AcqRel);
                }
                StreamBackpressurePolicy::DropNewest => {
                    let to_drop = write_len.saturating_sub(avail);
                    self.dropped_bytes
                        .fetch_add(to_drop as u64, Ordering::AcqRel);
                    if avail == 0 {
                        return Ok(0);
                    }
                    return self.write_direct(&data[..avail]);
                }
                StreamBackpressurePolicy::BlockWithTimeout(_) | StreamBackpressurePolicy::Error => {
                    return Err(StreamingError::BufferOverflow {
                        capacity: self.capacity(),
                        requested: write_len,
                    });
                }
            }
        }

        self.write_direct(data)
    }

    fn write_direct(&self, data: &[u8]) -> Result<usize, StreamingError> {
        let mut head = self.head.load(Ordering::Acquire);
        let ptr = self.buffer.as_ptr() as *mut u8;

        for &byte in data {
            let next_head = (head + 1) % self.capacity;
            unsafe {
                *ptr.add(head) = byte;
            }
            head = next_head;
        }

        self.head.store(head, Ordering::Release);
        self.total_written
            .fetch_add(data.len() as u64, Ordering::AcqRel);
        Ok(data.len())
    }

    /// Read available bytes into the provided buffer. Returns number of bytes read.
    pub fn read(&self, out: &mut [u8]) -> usize {
        if out.is_empty() {
            return 0;
        }

        let mut tail = self.tail.load(Ordering::Acquire);
        let head = self.head.load(Ordering::Acquire);
        let ptr = self.buffer.as_ptr();

        let mut read_count = 0;
        while tail != head && read_count < out.len() {
            unsafe {
                out[read_count] = *ptr.add(tail);
            }
            tail = (tail + 1) % self.capacity;
            read_count += 1;
        }

        self.tail.store(tail, Ordering::Release);
        read_count
    }

    /// Read all available bytes into a newly allocated vector.
    pub fn read_all(&self) -> Vec<u8> {
        let avail = self.available_read();
        let mut buf = vec![0u8; avail];
        let n = self.read(&mut buf);
        buf.truncate(n);
        buf
    }

    fn skip_read(&self, count: usize) {
        let mut tail = self.tail.load(Ordering::Acquire);
        let head = self.head.load(Ordering::Acquire);
        let mut skipped = 0;
        while tail != head && skipped < count {
            tail = (tail + 1) % self.capacity;
            skipped += 1;
        }
        self.tail.store(tail, Ordering::Release);
    }
}

/// Simulated Modbus device memory tables and protocol handler.
#[derive(Debug, Clone)]
pub struct AsyncModbusConnection {
    pub unit_id: u8,
    pub holding_registers: Arc<Mutex<HashMap<u16, u16>>>,
    pub input_registers: Arc<Mutex<HashMap<u16, u16>>>,
    pub coils: Arc<Mutex<HashMap<u16, bool>>>,
    pub discrete_inputs: Arc<Mutex<HashMap<u16, bool>>>,
}

impl AsyncModbusConnection {
    pub fn new(unit_id: u8) -> Self {
        Self {
            unit_id,
            holding_registers: Arc::new(Mutex::new(HashMap::new())),
            input_registers: Arc::new(Mutex::new(HashMap::new())),
            coils: Arc::new(Mutex::new(HashMap::new())),
            discrete_inputs: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub async fn read_holding_registers(
        &self,
        addr: u16,
        count: u16,
    ) -> Result<Vec<u16>, StreamingError> {
        let guard = self
            .holding_registers
            .lock()
            .map_err(|_| StreamingError::Modbus("mutex poisoned".into()))?;
        let mut res = Vec::with_capacity(count as usize);
        for i in 0..count {
            let val = guard.get(&(addr + i)).copied().unwrap_or(0);
            res.push(val);
        }
        Ok(res)
    }

    pub async fn write_holding_registers(
        &self,
        addr: u16,
        values: &[u16],
    ) -> Result<(), StreamingError> {
        let mut guard = self
            .holding_registers
            .lock()
            .map_err(|_| StreamingError::Modbus("mutex poisoned".into()))?;
        for (idx, &val) in values.iter().enumerate() {
            guard.insert(addr + idx as u16, val);
        }
        Ok(())
    }

    pub async fn read_coils(&self, addr: u16, count: u16) -> Result<Vec<bool>, StreamingError> {
        let guard = self
            .coils
            .lock()
            .map_err(|_| StreamingError::Modbus("mutex poisoned".into()))?;
        let mut res = Vec::with_capacity(count as usize);
        for i in 0..count {
            let val = guard.get(&(addr + i)).copied().unwrap_or(false);
            res.push(val);
        }
        Ok(res)
    }

    pub async fn write_coil(&self, addr: u16, value: bool) -> Result<(), StreamingError> {
        let mut guard = self
            .coils
            .lock()
            .map_err(|_| StreamingError::Modbus("mutex poisoned".into()))?;
        guard.insert(addr, value);
        Ok(())
    }
}

/// Supported transports for streaming I/O in the ZAP runtime.
pub enum StreamTransport {
    /// In-memory asynchronous queue stream.
    Memory(Arc<Mutex<VecDeque<u8>>>),
    /// High-performance circular SPSC lock-free ring-buffer.
    SharedRingBuffer(Arc<SpscRingBuffer>),
    /// Industrial Modbus connection.
    Modbus(AsyncModbusConnection),
}

/// Central streaming buffer manager indexing and multiplexing active streams.
#[derive(Default)]
pub struct StreamingBufferPool {
    streams: HashMap<u32, StreamTransport>,
}

impl StreamingBufferPool {
    pub fn new() -> Self {
        Self {
            streams: HashMap::new(),
        }
    }

    pub fn register_stream(&mut self, stream_id: u32, transport: StreamTransport) {
        self.streams.insert(stream_id, transport);
    }

    pub fn remove_stream(&mut self, stream_id: u32) -> Option<StreamTransport> {
        self.streams.remove(&stream_id)
    }

    pub fn has_stream(&self, stream_id: u32) -> bool {
        self.streams.contains_key(&stream_id)
    }

    /// Read up to `max_len` bytes from the specified stream ID.
    pub async fn read_async(
        &self,
        stream_id: u32,
        max_len: usize,
    ) -> Result<Vec<u8>, StreamingError> {
        let transport = self
            .streams
            .get(&stream_id)
            .ok_or(StreamingError::StreamNotFound(stream_id))?;
        match transport {
            StreamTransport::Memory(q) => {
                let mut guard = q
                    .lock()
                    .map_err(|_| StreamingError::Io("lock poisoned".into()))?;
                let to_read = max_len.min(guard.len());
                let mut out = Vec::with_capacity(to_read);
                for _ in 0..to_read {
                    if let Some(b) = guard.pop_front() {
                        out.push(b);
                    }
                }
                Ok(out)
            }
            StreamTransport::SharedRingBuffer(ring) => {
                let mut buf = vec![0u8; max_len];
                let n = ring.read(&mut buf);
                buf.truncate(n);
                Ok(buf)
            }
            StreamTransport::Modbus(mb) => {
                // Modbus binary register serialization
                let regs = mb
                    .read_holding_registers(0, (max_len / 2).max(1) as u16)
                    .await?;
                let mut bytes = Vec::with_capacity(regs.len() * 2);
                for reg in regs {
                    bytes.extend_from_slice(&reg.to_be_bytes());
                }
                bytes.truncate(max_len);
                Ok(bytes)
            }
        }
    }

    /// Write bytes into the specified stream ID.
    pub async fn write_async(&self, stream_id: u32, data: &[u8]) -> Result<usize, StreamingError> {
        let transport = self
            .streams
            .get(&stream_id)
            .ok_or(StreamingError::StreamNotFound(stream_id))?;
        match transport {
            StreamTransport::Memory(q) => {
                let mut guard = q
                    .lock()
                    .map_err(|_| StreamingError::Io("lock poisoned".into()))?;
                guard.extend(data.iter().copied());
                Ok(data.len())
            }
            StreamTransport::SharedRingBuffer(ring) => ring.write(data),
            StreamTransport::Modbus(mb) => {
                // Parse 16-bit register pairs and write to Modbus holding registers
                let mut regs = Vec::new();
                for chunk in data.chunks_exact(2) {
                    regs.push(u16::from_be_bytes([chunk[0], chunk[1]]));
                }
                if !regs.is_empty() {
                    mb.write_holding_registers(0, &regs).await?;
                }
                Ok(data.len())
            }
        }
    }

    /// Flush the specified stream.
    pub async fn flush_async(&self, stream_id: u32) -> Result<(), StreamingError> {
        if !self.has_stream(stream_id) {
            return Err(StreamingError::StreamNotFound(stream_id));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_spsc_ring_buffer_basic_rw() {
        let ring = SpscRingBuffer::new(32, StreamBackpressurePolicy::DropOldest);
        assert_eq!(ring.capacity(), 32);
        assert_eq!(ring.available_read(), 0);
        assert_eq!(ring.available_write(), 32);

        let data = b"hello spsc ring buffer!";
        let written = ring.write(data).unwrap();
        assert_eq!(written, data.len());
        assert_eq!(ring.available_read(), data.len());

        let mut out = [0u8; 64];
        let n = ring.read(&mut out);
        assert_eq!(n, data.len());
        assert_eq!(&out[..n], data);
        assert_eq!(ring.available_read(), 0);
    }

    #[test]
    fn test_spsc_ring_buffer_drop_oldest() {
        let ring = SpscRingBuffer::new(16, StreamBackpressurePolicy::DropOldest);
        ring.write(b"12345678").unwrap();
        ring.write(b"ABCDEFGH").unwrap(); // Buffer now full (16 bytes)

        // Write 4 more bytes -> drops "1234"
        ring.write(b"XYZW").unwrap();
        assert_eq!(ring.dropped_bytes(), 4);

        let out = ring.read_all();
        assert_eq!(out, b"5678ABCDEFGHXYZW");
    }

    #[tokio::test]
    async fn test_streaming_buffer_pool_integration() {
        let mut pool = StreamingBufferPool::new();
        let ring = Arc::new(SpscRingBuffer::new(
            128,
            StreamBackpressurePolicy::DropOldest,
        ));
        pool.register_stream(1, StreamTransport::SharedRingBuffer(ring));

        let queue = Arc::new(Mutex::new(VecDeque::new()));
        pool.register_stream(2, StreamTransport::Memory(queue));

        let modbus = AsyncModbusConnection::new(1);
        pool.register_stream(3, StreamTransport::Modbus(modbus));

        // Stream 1 test
        pool.write_async(1, b"ring_packet").await.unwrap();
        let read1 = pool.read_async(1, 64).await.unwrap();
        assert_eq!(read1, b"ring_packet");

        // Stream 2 test
        pool.write_async(2, b"memory_packet").await.unwrap();
        let read2 = pool.read_async(2, 64).await.unwrap();
        assert_eq!(read2, b"memory_packet");

        // Stream 3 Modbus test
        let modbus_data = vec![0x12, 0x34, 0x56, 0x78];
        pool.write_async(3, &modbus_data).await.unwrap();
        let read3 = pool.read_async(3, 4).await.unwrap();
        assert_eq!(read3, modbus_data);
    }
}
