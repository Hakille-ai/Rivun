# Milestone 3 Investigation Report: `zap-driver-sdk` Specification & API Design

**Agent**: `explorer_m3_2`  
**Milestone**: Milestone 3 — Async WASM Driver Pipeline & Inter-Driver IPC  
**Date**: 2026-08-15  
**Target Crate**: `crates/zap-driver-sdk` (and interfaces to `crates/zap-runtime`, `crates/zap-capability`, `crates/zap-core`)  

---

## 1. Executive Summary

Milestone 3 transforms ZAP's driver architecture from synchronous, single-invocation action execution into an **asynchronous, streaming, zero-copy inter-driver pipeline**. 

In this new architecture:
- Drivers can stream data continuously (e.g. sensor telemetry, video frames, Modbus/TCP streams) without intermediate heap allocation copies between WASM guest and host.
- Drivers can communicate through deterministic zero-copy inter-driver IPC channels (`PerceptionDriver -> SafetyPolicyDriver -> ActuatorDriver`) with strict aggregate fuel budgeting.
- Legacy synchronous `ZapDriver` implementations remain 100% backward compatible and seamlessly adapt into the async runtime via `SyncDriverAdapter`.
- All guest-host pointer translations, memory slices, and buffer views are strictly bounds-checked and encapsulated in safe Rust abstractions (`PinnedBuffer`, `BufferSlice`, `BufferSliceMut`, `GuestMemoryView`).

This report provides the exact architectural specification, API design, data structures, trait definitions, safety invariants, and implementation plan for `crates/zap-driver-sdk`.

---

## 2. Existing State vs Milestone 3 Gap Analysis

### 2.1 Current Implementation in `crates/zap-driver-sdk`
Currently, `crates/zap-driver-sdk` (`src/lib.rs`) contains a minimal synchronous ABI (v1):
- ABI constants: `DRIVER_ABI_VERSION = 1`, `MEMORY_EXPORT = "memory"`, `ALLOC_EXPORT = "zap_alloc"`, `DEALLOC_EXPORT = "zap_dealloc"`, `EXECUTE_EXPORT = "zap_execute"`.
- `PackedResult`: 64-bit packed pointer/length (`(ptr << 32) | len`).
- `DriverInput<'a>`: Immutable reference to `action: &'a str` and `payload: &'a [u8]`.
- `ZapDriver` trait: Single synchronous method `fn execute(&self, input: DriverInput<'_>) -> Result<Vec<u8>, DriverError>`.
- `DriverError`: Basic error wrapper holding a `message: String`.

### 2.2 Milestone 3 Requirements & Gaps
| Feature Requirement | Current State | Milestone 3 Required Specification |
|---|---|---|
| **Driver Lifecycle** | Single `execute()` method | 4-stage lifecycle: `init()`, `process_stream()`, `handle_event()`, `shutdown()` |
| **Execution Model** | Synchronous only | Async execution with Rust 2024 `async fn` in trait (AFIT) and Send+Sync futures |
| **Buffer Management** | Allocates `Vec<u8>` per call | Zero-copy `PinnedBuffer`, `BufferSlice`, `BufferSliceMut` over guest/host memory |
| **Pointer Safety** | Manual pointer packing only | Safe pointer translation, alignment validation, non-overlapping borrow guarantees |
| **Inter-Driver IPC** | None | `IpcChannel`, `IpcEndpoint`, `IpcMessage`, `IpcPipe` abstractions with backpressure |
| **Causal Tracking** | None | Cryptographic BLAKE3 causal hash chaining across IPC pipe transfers |
| **Fuel Metering** | Host-only external tracking | Driver context access to fuel balance, explicit yield points, and IPC fuel charges |
| **Sync Compatibility** | Baseline | Seamless bi-directional bridging (`SyncDriverAdapter` / `AsyncDriverAdapter`) |

---

## 3. `AsyncZapDriver` Trait & Async Driver Lifecycle

### 3.1 Lifecycle Design
The asynchronous driver lifecycle models real-world cyber-physical, perception, and stream processing workloads:

```
                  +-----------------------------------+
                  |           Uninitialized           |
                  +-----------------------------------+
                                    |
                                    | init(ctx)
                                    v
                  +-----------------------------------+
                  |            Active / Ready         |
                  +-----------------------------------+
                     /              |              \
                    /               |               \
process_stream(ctx, in, out)  handle_event(ctx, evt)  yield_execution()
                    \               |               /
                     \              |              /
                  +-----------------------------------+
                  |           Running Pipeline        |
                  +-----------------------------------+
                                    |
                                    | shutdown(ctx)
                                    v
                  +-----------------------------------+
                  |             Terminated            |
                  +-----------------------------------+
```

### 3.2 Trait Definition
```rust
/// Asynchronous WASM driver contract for ZAP pipelines.
///
/// Enables non-blocking streaming I/O, asynchronous event handling,
/// zero-copy memory processing, and inter-driver IPC.
pub trait AsyncZapDriver: Send + Sync + 'static {
    /// Initialize driver state, allocate pinned buffers, and register IPC endpoints.
    ///
    /// Invoked once by the host before streaming or event handling begins.
    fn init(&mut self, _ctx: &mut DriverContext) -> impl std::future::Future<Output = Result<(), DriverError>> + Send {
        async { Ok(()) }
    }

    /// Process a continuous streaming data chunk with zero-copy input and output buffers.
    ///
    /// - `input`: Zero-copy immutable view into the incoming data buffer.
    /// - `output`: Pre-allocated pinned buffer to receive transformed/processed bytes.
    /// - Returns: Number of bytes written to `output`.
    fn process_stream(
        &mut self,
        ctx: &mut DriverContext,
        input: &BufferSlice<'_>,
        output: &mut PinnedBuffer,
    ) -> impl std::future::Future<Output = Result<usize, DriverError>> + Send;

    /// Handle an asynchronous discrete event or IPC message from an upstream stage or host.
    ///
    /// - `event`: The incoming structured IPC message.
    /// - Returns: Optional response message to route downstream or back to host.
    fn handle_event(
        &mut self,
        _ctx: &mut DriverContext,
        _event: &IpcMessage,
    ) -> impl std::future::Future<Output = Result<Option<IpcMessage>, DriverError>> + Send {
        async { Ok(None) }
    }

    /// Gracefully shut down the driver, releasing resources and flushing state.
    fn shutdown(&mut self, _ctx: &mut DriverContext) -> impl std::future::Future<Output = Result<(), DriverError>> + Send {
        async { Ok(()) }
    }
}
```

### 3.3 `DriverContext` Specification
```rust
/// Execution context and environment provided to async drivers.
#[derive(Debug, Clone)]
pub struct DriverContext {
    pub driver_id: String,
    pub stage_index: usize,
    pub permissions: DriverPermissions,
    pub fuel_limit: u64,
    pub fuel_consumed: u64,
    pub endpoints: HashMap<String, IpcEndpointHandle>,
    pub metadata: HashMap<String, String>,
}

impl DriverContext {
    pub fn new(driver_id: impl Into<String>, stage_index: usize, permissions: DriverPermissions) -> Self {
        Self {
            driver_id: driver_id.into(),
            stage_index,
            permissions,
            fuel_limit: 10_000_000,
            fuel_consumed: 0,
            endpoints: HashMap::new(),
            metadata: HashMap::new(),
        }
    }

    pub fn with_fuel_limit(mut self, limit: u64) -> Self {
        self.fuel_limit = limit;
        self
    }

    pub fn remaining_fuel(&self) -> u64 {
        self.fuel_limit.saturating_sub(self.fuel_consumed)
    }

    pub fn consume_fuel(&mut self, amount: u64) -> Result<(), DriverError> {
        let new_total = self.fuel_consumed.saturating_add(amount);
        if new_total > self.fuel_limit {
            return Err(DriverError::FuelLimitExceeded {
                consumed: new_total,
                limit: self.fuel_limit,
            });
        }
        self.fuel_consumed = new_total;
        Ok(())
    }

    pub fn register_endpoint(&mut self, name: impl Into<String>, handle: IpcEndpointHandle) {
        self.endpoints.insert(name.into(), handle);
    }

    pub fn get_endpoint(&self, name: &str) -> Option<&IpcEndpointHandle> {
        self.endpoints.get(name)
    }
}
```

---

## 4. Zero-Copy Buffer Architecture & Memory Slice Utilities

Zero-copy performance is critical for high-throughput perception pipelines (e.g. 100+ MB/s sensor streams). The SDK provides three core types:
1. `PinnedBuffer`: Fixed-address contiguous memory block for DMA/host-guest sharing.
2. `BufferSlice<'a>`: Immutable zero-copy window with strict bounds checking.
3. `BufferSliceMut<'a>`: Mutable zero-copy window with exclusive access guarantees.

```
+-------------------------------------------------------------------------------+
|                       Host Linear WASM Memory / Heap                          |
|  [0x0000 ..................... 0x1000 ============ 0x1800 ........... 0xFFFF] |
+-------------------------------------------------------------------------------+
                                    ^                  ^
                                    |  PinnedBuffer    |
                                    |  (base=0x1000)   |
                                    |  (cap=2048)      |
                                    +--------+---------+
                                             |
                         +-------------------+-------------------+
                         |                                       |
                 BufferSlice (0..512)                   BufferSliceMut (512..2048)
                 (Read-only Perception view)            (Write-only Actuator view)
```

### 4.1 `PinnedBuffer` Specification
```rust
/// Contiguous memory buffer pinned at a stable address for zero-copy I/O.
#[derive(Debug)]
pub struct PinnedBuffer {
    ptr: u32,
    capacity: usize,
    len: usize,
    data: Vec<u8>,
}

impl PinnedBuffer {
    /// Create a new pinned buffer with pre-allocated capacity.
    pub fn with_capacity(capacity: usize) -> Self {
        let mut data = vec![0_u8; capacity];
        let ptr = data.as_mut_ptr() as usize as u32;
        Self {
            ptr,
            capacity,
            len: 0,
            data,
        }
    }

    /// Construct a pinned buffer from an existing memory vector.
    pub fn from_vec(data: Vec<u8>) -> Self {
        let capacity = data.capacity();
        let len = data.len();
        let ptr = data.as_ptr() as usize as u32;
        Self {
            ptr,
            capacity,
            len,
            data,
        }
    }

    pub fn ptr(&self) -> u32 {
        self.ptr
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn as_slice(&self) -> &[u8] {
        &self.data[..self.len]
    }

    pub fn as_mut_slice(&mut self) -> &mut [u8] {
        &mut self.data[..self.len]
    }

    pub fn clear(&mut self) {
        self.len = 0;
    }

    pub fn set_len(&mut self, new_len: usize) -> Result<(), BufferError> {
        if new_len > self.capacity {
            return Err(BufferError::CapacityExceeded {
                requested: new_len,
                capacity: self.capacity,
            });
        }
        self.len = new_len;
        Ok(())
    }

    /// Write bytes into the pinned buffer starting from current `len`.
    pub fn write(&mut self, bytes: &[u8]) -> Result<usize, BufferError> {
        let remaining = self.capacity.saturating_sub(self.len);
        if bytes.len() > remaining {
            return Err(BufferError::CapacityExceeded {
                requested: self.len + bytes.len(),
                capacity: self.capacity,
            });
        }
        self.data[self.len..self.len + bytes.len()].copy_from_slice(bytes);
        self.len += bytes.len();
        Ok(bytes.len())
    }

    /// Borrow an immutable sub-slice view.
    pub fn slice(&self, offset: usize, len: usize) -> Result<BufferSlice<'_>, BufferError> {
        let end = offset.checked_add(len).ok_or(BufferError::OutOfBounds {
            offset,
            len,
            bound: self.len,
        })?;
        if end > self.len {
            return Err(BufferError::OutOfBounds {
                offset,
                len,
                bound: self.len,
            });
        }
        Ok(BufferSlice::new(&self.data[offset..end]))
    }

    /// Borrow a mutable sub-slice view.
    pub fn slice_mut(&mut self, offset: usize, len: usize) -> Result<BufferSliceMut<'_>, BufferError> {
        let end = offset.checked_add(len).ok_or(BufferError::OutOfBounds {
            offset,
            len,
            bound: self.capacity,
        })?;
        if end > self.capacity {
            return Err(BufferError::OutOfBounds {
                offset,
                len,
                bound: self.capacity,
            });
        }
        if end > self.len {
            self.len = end;
        }
        Ok(BufferSliceMut::new(&mut self.data[offset..end]))
    }
}
```

### 4.2 `BufferSlice` & `BufferSliceMut` Specification
```rust
/// Immutable zero-copy slice over a memory region.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BufferSlice<'a> {
    data: &'a [u8],
}

impl<'a> BufferSlice<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Self { data }
    }

    pub fn as_bytes(&self) -> &'a [u8] {
        self.data
    }

    pub fn len(&self) -> usize {
        self.data.len()
    }

    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    pub fn subslice(&self, offset: usize, len: usize) -> Result<Self, BufferError> {
        let end = offset.checked_add(len).ok_or(BufferError::OutOfBounds {
            offset,
            len,
            bound: self.data.len(),
        })?;
        if end > self.data.len() {
            return Err(BufferError::OutOfBounds {
                offset,
                len,
                bound: self.data.len(),
            });
        }
        Ok(Self::new(&self.data[offset..end]))
    }
}

impl<'a> std::ops::Deref for BufferSlice<'a> {
    type Target = [u8];
    fn deref(&self) -> &Self::Target {
        self.data
    }
}

/// Mutable zero-copy slice over a memory region.
#[derive(Debug)]
pub struct BufferSliceMut<'a> {
    data: &'a mut [u8],
}

impl<'a> BufferSliceMut<'a> {
    pub fn new(data: &'a mut [u8]) -> Self {
        Self { data }
    }

    pub fn as_mut_bytes(&mut self) -> &mut [u8] {
        self.data
    }

    pub fn len(&self) -> usize {
        self.data.len()
    }

    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    pub fn write_slice(&mut self, src: &[u8]) -> Result<usize, BufferError> {
        if src.len() > self.data.len() {
            return Err(BufferError::CapacityExceeded {
                requested: src.len(),
                capacity: self.data.len(),
            });
        }
        self.data[..src.len()].copy_from_slice(src);
        Ok(src.len())
    }

    pub fn split_at_mut(self, mid: usize) -> Result<(BufferSliceMut<'a>, BufferSliceMut<'a>), BufferError> {
        if mid > self.data.len() {
            return Err(BufferError::OutOfBounds {
                offset: mid,
                len: 0,
                bound: self.data.len(),
            });
        }
        let (first, second) = self.data.split_at_mut(mid);
        Ok((BufferSliceMut::new(first), BufferSliceMut::new(second)))
    }
}

impl<'a> std::ops::Deref for BufferSliceMut<'a> {
    type Target = [u8];
    fn deref(&self) -> &Self::Target {
        self.data
    }
}

impl<'a> std::ops::DerefMut for BufferSliceMut<'a> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.data
    }
}
```

### 4.3 Safe Pointer Translation Helpers
```rust
/// Guest-Host memory pointer translation utilities.
pub struct MemoryMapper;

impl MemoryMapper {
    /// Validates that a guest pointer and length fit entirely inside memory bounds.
    pub fn validate_range(guest_ptr: u32, len: usize, total_mem_size: usize) -> Result<(), BufferError> {
        let start = guest_ptr as usize;
        let end = start.checked_add(len).ok_or(BufferError::InvalidPointer {
            ptr: guest_ptr,
            len: len as u32,
        })?;
        if end > total_mem_size {
            return Err(BufferError::OutOfBounds {
                offset: start,
                len,
                bound: total_mem_size,
            });
        }
        Ok(())
    }

    /// Safely translates a guest pointer into an immutable byte slice.
    pub fn translate_slice<'a>(memory: &'a [u8], guest_ptr: u32, len: usize) -> Result<&'a [u8], BufferError> {
        Self::validate_range(guest_ptr, len, memory.len())?;
        let start = guest_ptr as usize;
        Ok(&memory[start..start + len])
    }

    /// Safely translates a guest pointer into a mutable byte slice.
    pub fn translate_slice_mut<'a>(memory: &'a mut [u8], guest_ptr: u32, len: usize) -> Result<&'a mut [u8], BufferError> {
        Self::validate_range(guest_ptr, len, memory.len())?;
        let start = guest_ptr as usize;
        Ok(&mut memory[start..start + len])
    }
}
```

---

## 5. Inter-Driver IPC Primitives

The SDK defines first-class IPC primitives allowing drivers to pass messages, stream through pipes, and cryptographically link execution stages.

### 5.1 `IpcMessage`
```rust
bitflags::bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct IpcFlags: u32 {
        const NONE = 0;
        const PRIORITY = 1 << 0;
        const STREAM_CHUNK = 1 << 1;
        const END_OF_STREAM = 1 << 2;
        const REQUIRES_ACK = 1 << 3;
    }
}

/// A structured inter-driver IPC message frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IpcMessage {
    pub message_id: u64,
    pub channel_id: u32,
    pub topic: String,
    pub source_stage: String,
    pub target_stage: String,
    pub sequence: u64,
    pub timestamp_micros: u64,
    pub flags: IpcFlags,
    pub payload: Vec<u8>,
}

impl IpcMessage {
    pub fn new(topic: impl Into<String>, payload: Vec<u8>) -> Self {
        Self {
            message_id: 0,
            channel_id: 0,
            topic: topic.into(),
            source_stage: String::new(),
            target_stage: String::new(),
            sequence: 0,
            timestamp_micros: 0,
            flags: IpcFlags::NONE,
            payload,
        }
    }

    pub fn with_routing(
        mut self,
        source_stage: impl Into<String>,
        target_stage: impl Into<String>,
        channel_id: u32,
        sequence: u64,
    ) -> Self {
        self.source_stage = source_stage.into();
        self.target_stage = target_stage.into();
        self.channel_id = channel_id;
        self.sequence = sequence;
        self
    }

    /// Compute cryptographic BLAKE3 digest of the message for causal chain linking.
    pub fn digest(&self) -> [u8; 32] {
        let mut hasher = blake3::Hasher::new();
        hasher.update(&self.message_id.to_be_bytes());
        hasher.update(&self.channel_id.to_be_bytes());
        hasher.update(self.topic.as_bytes());
        hasher.update(self.source_stage.as_bytes());
        hasher.update(self.target_stage.as_bytes());
        hasher.update(&self.sequence.to_be_bytes());
        hasher.update(&self.payload);
        *hasher.finalize().as_bytes()
    }
}
```

### 5.2 `IpcChannel` & Backpressure Strategies
```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackpressureStrategy {
    Block,
    DropOldest,
    DropNewest,
    ErrorOnFull,
}

/// Channel abstraction for IPC between pipeline stages.
#[derive(Debug, Clone)]
pub struct IpcChannelConfig {
    pub channel_id: u32,
    pub name: String,
    pub capacity_bytes: usize,
    pub max_messages: usize,
    pub backpressure: BackpressureStrategy,
}

impl Default for IpcChannelConfig {
    fn default() -> Self {
        Self {
            channel_id: 0,
            name: "default_channel".to_string(),
            capacity_bytes: 1024 * 1024,
            max_messages: 256,
            backpressure: BackpressureStrategy::Block,
        }
    }
}
```

### 5.3 `IpcPipe` & Causal Chain Tracking
```rust
/// Deterministic zero-copy pipeline pipe connecting Stage A to Stage B.
#[derive(Debug)]
pub struct IpcPipe {
    pub name: String,
    pub channel_id: u32,
    causal_hasher: blake3::Hasher,
    sequence_counter: u64,
    total_bytes_transferred: u64,
}

impl IpcPipe {
    pub fn new(name: impl Into<String>, channel_id: u32) -> Self {
        let mut causal_hasher = blake3::Hasher::new();
        causal_hasher.update(b"ZAP-IPC-PIPE-v1:");
        let name_str = name.into();
        causal_hasher.update(name_str.as_bytes());
        Self {
            name: name_str,
            channel_id,
            causal_hasher,
            sequence_counter: 0,
            total_bytes_transferred: 0,
        }
    }

    /// Transfer a message through the pipe, updating the causal transcript and sequence ID.
    pub fn transfer(&mut self, mut msg: IpcMessage) -> Result<IpcMessage, IpcError> {
        self.sequence_counter += 1;
        msg.channel_id = self.channel_id;
        msg.sequence = self.sequence_counter;
        let digest = msg.digest();
        self.causal_hasher.update(&digest);
        self.total_bytes_transferred += msg.payload.len() as u64;
        Ok(msg)
    }

    /// Retrieve the current hex-encoded causal chain hash.
    pub fn current_causal_hash(&self) -> String {
        self.causal_hasher.finalize().to_hex().to_string()
    }
}
```

---

## 6. Sync `ZapDriver` <-> `AsyncZapDriver` Interoperability

To guarantee strict backward compatibility with Milestone 2 and existing sync driver code, `zap-driver-sdk` provides transparent adapter wrappers:

```
+-------------------------------------------------------------+
|                         ZapDriver                           |
|       (Sync: execute(DriverInput) -> Result<Vec<u8>>)       |
+-------------------------------------------------------------+
                              |
                              | wrapped by SyncDriverAdapter<D>
                              v
+-------------------------------------------------------------+
|                      AsyncZapDriver                         |
|     (Async: init, process_stream, handle_event, shutdown)   |
+-------------------------------------------------------------+
```

### 6.1 `SyncDriverAdapter` Implementation
```rust
/// Adapts any synchronous `ZapDriver` into a fully compliant `AsyncZapDriver`.
pub struct SyncDriverAdapter<D> {
    inner: D,
}

impl<D: ZapDriver> SyncDriverAdapter<D> {
    pub fn new(inner: D) -> Self {
        Self { inner }
    }

    pub fn inner(&self) -> &D {
        &self.inner
    }

    pub fn into_inner(self) -> D {
        self.inner
    }
}

impl<D: ZapDriver + Send + Sync + 'static> AsyncZapDriver for SyncDriverAdapter<D> {
    async fn init(&mut self, _ctx: &mut DriverContext) -> Result<(), DriverError> {
        Ok(())
    }

    async fn process_stream(
        &mut self,
        _ctx: &mut DriverContext,
        input: &BufferSlice<'_>,
        output: &mut PinnedBuffer,
    ) -> Result<usize, DriverError> {
        let result = self.inner.execute(DriverInput {
            action: "process_stream",
            payload: input.as_bytes(),
        })?;
        output.write(&result).map_err(|e| DriverError::new(e.to_string()))
    }

    async fn handle_event(
        &mut self,
        _ctx: &mut DriverContext,
        event: &IpcMessage,
    ) -> Result<Option<IpcMessage>, DriverError> {
        let result = self.inner.execute(DriverInput {
            action: &event.topic,
            payload: &event.payload,
        })?;
        Ok(Some(IpcMessage::new(
            format!("{}.response", event.topic),
            result,
        )))
    }

    async fn shutdown(&mut self, _ctx: &mut DriverContext) -> Result<(), DriverError> {
        Ok(())
    }
}

/// Extension trait to convert any `ZapDriver` directly into an `AsyncZapDriver`.
pub trait ZapDriverExt: ZapDriver + Sized {
    fn into_async(self) -> SyncDriverAdapter<Self> {
        SyncDriverAdapter::new(self)
    }
}

impl<D: ZapDriver> ZapDriverExt for D {}
```

---

## 7. Rust Trait System, AFIT, Send + Sync, and Safety Invariants

### 7.1 Async Trait Strategy (AFIT vs Dynamic Dispatch)
- **Rust 2024 / Rust 1.93 Support**: Rust 2024 natively supports `async fn` in traits (AFIT) and RPITIT (`impl Future<Output = ...> + Send`).
- **Dynamic Dispatch (`dyn AsyncZapDriver`)**: While static dispatch (generics) is zero-cost, pipeline stages in `zap-runtime` often require dynamic dispatch to hold heterogenous driver stages in a `Vec<Box<dyn DynamicAsyncDriver>>`.
- **Solution**: The SDK provides both:
  1. Static `AsyncZapDriver` trait for high-performance direct compilation.
  2. `BoxedAsyncDriver` / `DynamicAsyncDriver` helper wrapping boxed futures:
     `fn process_stream_boxed<'a>(&'a mut self, ctx: &'a mut DriverContext, input: &'a BufferSlice<'a>, output: &'a mut PinnedBuffer) -> Pin<Box<dyn Future<Output = Result<usize, DriverError>> + Send + 'a>>`.

### 7.2 Safety Invariants Matrix
| Invariant | Description | Enforcement Mechanism | Failure Guarantee |
|---|---|---|---|
| **Memory Isolation** | Guest pointer cannot read/write outside WASM linear memory | `MemoryMapper::validate_range` checks `ptr + len <= memory_size` | Returns `BufferError::OutOfBounds`, no UB |
| **Data Race Freedom** | Cross-stage memory sharing is strictly serialized or pinned | `BufferSliceMut` requires unique borrow `&mut` | Compile-time aliasing check |
| **Thread Safety** | Drivers & messages can move across Tokio executor threads | `Send + Sync + 'static` trait bounds on all futures and messages | Compile-time enforcement |
| **No Unhandled Panics** | Driver failures must not crash the host execution node | All trait functions return `Result<T, DriverError>` | Host catches errors as `ZapRuntimeError` |
| **Fuel Boundedness** | Drivers cannot run infinite loops or unbounded allocations | `DriverContext::consume_fuel` tracks usage per IPC/stream step | Returns `DriverError::FuelLimitExceeded` |
| **Causal Integrity** | Pipe transfers produce tamper-evident sequential hash chains | `IpcPipe::transfer` strictly updates BLAKE3 transcript | Verification detects stage alteration |

---

## 8. Proposed Module & File Structure for `crates/zap-driver-sdk`

```
crates/zap-driver-sdk/
├── Cargo.toml
├── benches/
│   └── sdk.rs                       # Benchmarks for sync/async driver execution & zero-copy slices
├── src/
│   ├── lib.rs                       # Top-level exports, ABI constants, packed result helpers
│   ├── async_driver.rs              # AsyncZapDriver, DriverContext, SyncDriverAdapter
│   ├── buffer.rs                    # PinnedBuffer, BufferSlice, BufferSliceMut, MemoryMapper
│   ├── error.rs                     # DriverError, BufferError, IpcError
│   └── ipc.rs                       # IpcMessage, IpcChannelConfig, IpcPipe, BackpressureStrategy
└── tests/
    ├── async_driver_tests.rs        # Integration tests for async driver lifecycle
    ├── buffer_tests.rs              # Zero-copy slicing, memory mapping, bounds validation
    └── ipc_pipe_tests.rs            # Inter-driver IPC chaining, causal hash verification
```

### Updated `Cargo.toml` for `crates/zap-driver-sdk`
```toml
[package]
name = "zap-driver-sdk"
description = "Helpers for authoring synchronous and asynchronous ZAP WASM action drivers."
edition.workspace = true
license.workspace = true
rust-version.workspace = true
version.workspace = true

[dependencies]
bitflags.workspace = true
blake3.workspace = true
bytes.workspace = true
serde = { workspace = true, features = ["derive"] }
thiserror.workspace = true
zap-capability.workspace = true

[dev-dependencies]
criterion.workspace = true
tokio = { workspace = true, features = ["macros", "rt-multi-thread"] }

[[bench]]
name = "sdk"
harness = false
```

---

## 9. Integration with `zap-runtime`

The SDK components seamlessly integrate with host-side runtime modules in `crates/zap-runtime`:
1. **`AsyncWasmExecutor` (`zap-runtime/src/async_engine.rs`)**:
   - Manages Tokio tasks running WASM drivers.
   - Binds host imports `zap::ipc_send`, `zap::ipc_recv`, `zap::yield_execution`.
   - Uses `MemoryMapper` to project host Tokio stream buffers into guest memory.
2. **`DriverPipeline` (`zap-runtime/src/pipeline.rs`)**:
   - Chained multi-stage graphs: `Stage 1 (Perception) -> IpcPipe -> Stage 2 (Policy Filter) -> IpcPipe -> Stage 3 (Actuator)`.
   - Passes `BufferSlice` and `PinnedBuffer` between stages without intermediate copying.
   - Collects `PipelineExecutionReport` with final causal chain hash and aggregate fuel consumption.
3. **`StreamingBufferPool` (`zap-runtime/src/streaming.rs`)**:
   - Manages lock-free circular ring-buffers for high-throughput TCP / Modbus / shared memory streams.

---

## 10. Verification & Test Plan

1. **Unit Tests (`crates/zap-driver-sdk/src/`)**:
   - `test_packed_result_roundtrip`: Validate 64-bit bitshift pack/unpack.
   - `test_pinned_buffer_write_and_slice`: Test zero-copy sub-slicing and bounds enforcement.
   - `test_buffer_slice_mut_split`: Test disjoint mutable slices and capacity limits.
   - `test_memory_mapper_out_of_bounds`: Test guest pointer validation rejects overflows.
   - `test_sync_driver_adapter`: Test sync `EchoDriver` running via `AsyncZapDriver`.
   - `test_ipc_message_digest`: Verify BLAKE3 hashing of IPC frames.
   - `test_ipc_pipe_causal_chain`: Verify multi-stage message chaining produces deterministic causal hashes.

2. **Integration Tests (`tests/`)**:
   - `test_perception_policy_actuator_pipeline`: 3-stage async pipeline test with mock perception data.
   - `test_fuel_limit_enforcement`: Ensure drivers exceeding fuel budgets abort cleanly.

3. **Benchmarks (`benches/sdk.rs`)**:
   - Benchmark zero-copy slice translation vs heap allocation copying.
   - Benchmark IPC pipe message transfer throughput (target: > 1,000,000 msgs/sec in-memory).

---
