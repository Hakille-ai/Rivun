# Milestone 3 Analysis: Async WASM Driver Pipeline & Inter-Driver IPC

**Author**: `explorer_m3_3`  
**Date**: 2026-08-15  
**Target Crates**: `crates/rivun-runtime` (with interface bindings to `crates/rivun-driver-sdk` and `crates/rivun-capability`)  
**Scope**: Milestone 3 (R3) — Async WASM Driver Execution Engine, Streaming I/O Buffers (TCP, Modbus, Ring-Buffers), Deterministic Inter-Driver IPC, and Chained `DriverPipeline` Orchestrator.

---

## 1. Executive Summary

Milestone 3 transforms rivun from a synchronous, single-driver WebAssembly execution environment into a **high-throughput, non-blocking asynchronous execution fabric** capable of executing concurrent streaming pipelines with microsecond latency, zero-copy data paths, strict sandboxed memory isolation, and deterministic aggregate fuel metering.

### Core Architectural Discoveries:
1. **Existing Runtime State**:
   - `crates/rivun-runtime/src/lib.rs` currently implements synchronous `WasmExecutor` using `wasmtime 45.0.1` with Cranelift, fuel consumption, and an OS thread-based epoch ticker.
   - `crates/rivun-runtime/src/pipeline.rs` currently contains a compilation defect (importing non-existent `WasmActionRuntime`), and uses heuristic fuel estimation rather than exact fuel tracking.
   - `crates/rivun-node` and other components depend on `WasmExecutor`. All M3 enhancements must maintain backward compatibility with `WasmExecutor` while introducing the asynchronous stack.
2. **Async WASM Execution (`async_engine.rs`)**:
   - Enabling Wasmtime's `async` feature allows WASM execution to be scheduled natively on Tokio tasks using async fibers, enabling non-blocking async host calls (e.g. streaming I/O, IPC receive, cooperative yields) without exhausting OS threads.
3. **Lock-Free Streaming I/O (`streaming.rs`)**:
   - High-frequency edge devices (robotics, PLCs, smart meters) stream data via TCP and industrial protocols like Modbus.
   - A cache-line padded Single-Producer Single-Consumer (`SpscRingBuffer`) provides lock-free zero-copy ring slices `(&[u8], &[u8])`, paired with Tokio `Notify` for async backpressure.
   - Protocol adapters for TCP and Modbus (RTU & TCP framing, virtual register banks, CRC16 verification) bridge raw streams into WASM driver buffers.
4. **Deterministic Inter-Driver IPC (`ipc.rs`)**:
   - Isolated WASM instances communicate via point-to-point and routed `IpcPipe` channels.
   - Strict memory sandboxing is maintained: Guest memories remain completely isolated; the host mediates message transfer via zero-copy / single-copy transfers (`bytes::Bytes`).
   - Monotonic sequence numbering and Blake3 causal transcripts guarantee deterministic ordering and auditability.
5. **Chained `DriverPipeline` Orchestrator (`pipeline.rs`)**:
   - Real-world perception $\rightarrow$ safety policy $\rightarrow$ physical actuator workflows are chained in a deterministic pipeline.
   - Strict aggregate fuel budgeting: Total fuel consumed across all stages is tracked atomically, trapping instantly if the global budget is exceeded.
   - End-to-end latency profiling (microsecond precision) and Blake3 rolling causal chain hashes provide tamper-evident cryptographic receipts.

---

## 2. Module Architectural Specifications

```
crates/rivun-runtime/
├── Cargo.toml
├── src/
│   ├── lib.rs              # Crate root, error definitions, re-exports, sync WasmExecutor
│   ├── async_engine.rs     # AsyncWasmExecutor, Tokio task scheduling, async host imports
│   ├── streaming.rs        # SpscRingBuffer, TCP stream adapter, Modbus simulator & adapter
│   ├── ipc.rs              # IpcPipe, IpcRouter, IpcMessage, zero-copy host mediation
│   └── pipeline.rs         # DriverPipeline orchestrator, aggregate fuel budgeting, causal hashes
└── benches/
    └── runtime.rs          # Benchmarks for sync, async, streaming, IPC, and pipelines
```

---

### Module 1: `async_engine.rs` — Asynchronous WASM Driver Execution Engine

#### Purpose
Executes sandboxed WebAssembly drivers asynchronously on Tokio worker threads. Allows drivers to perform asynchronous host calls (I/O, IPC, yielding) without blocking OS threads, while enforcing fuel budgets, memory caps, and wall-clock timeouts.

#### Core Types & Interfaces

```rust
use std::sync::Arc;
use tokio::sync::Mutex;
use wasmtime::{Config, Engine, Instance, Linker, Module, Store, StoreLimits, StoreLimitsBuilder};
use @@rivun_HEADER@@capability::DriverPermissions;
use crate::{ZapRuntimeError, Result, HostCallRecord, HostCallKind};

/// Configuration for async execution limits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AsyncExecutionLimits {
    pub max_memory_bytes: usize,
    pub fuel: u64,
    pub timeout_ms: u64,
    pub max_output_bytes: usize,
    pub permissions: DriverPermissions,
    pub yield_interval_ticks: Option<u64>,
}

impl Default for AsyncExecutionLimits {
    fn default() -> Self {
        Self {
            max_memory_bytes: 16 * 1024 * 1024,
            fuel: 10_000_000,
            timeout_ms: 1_000,
            max_output_bytes: 1024 * 1024,
            permissions: DriverPermissions::none(),
            yield_interval_ticks: Some(10_000),
        }
    }
}

/// Execution output and resource consumption metrics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AsyncWasmExecutionResult {
    pub output: Vec<u8>,
    pub fuel_consumed: u64,
    pub elapsed_micros: u128,
    pub host_calls: Vec<HostCallRecord>,
}

/// Compiled async WASM driver module.
#[derive(Clone)]
pub struct AsyncWasmDriver {
    pub(crate) module: Module,
}

impl AsyncWasmDriver {
    pub fn validate_abi(&self) -> Result<()> {
        // Validates: memory, @@rivun_HEADER@@alloc, @@rivun_HEADER@@dealloc, @@rivun_HEADER@@execute
        crate::validate_driver_module(&self.module)
    }
}

/// Non-blocking async WASM executor.
#[derive(Clone)]
pub struct AsyncWasmExecutor {
    engine: Engine,
    cache: Arc<Mutex<AsyncModuleCache>>,
}

impl AsyncWasmExecutor {
    pub fn new() -> Result<Self> {
        let mut config = Config::new();
        config.async_support(true);
        config.consume_fuel(true);
        config.epoch_interruption(true);
        config.wasm_backtrace_details(wasmtime::WasmBacktraceDetails::Enable);
        let engine = Engine::new(&config)?;
        Ok(Self {
            engine,
            cache: Arc::new(Mutex::new(AsyncModuleCache::new(64))),
        })
    }

    pub fn compile(&self, wasm: impl AsRef<[u8]>) -> Result<AsyncWasmDriver> {
        let module = Module::new(&self.engine, wasm.as_ref())?;
        Ok(AsyncWasmDriver { module })
    }

    pub async fn compile_and_validate_cached(&self, wasm: impl AsRef<[u8]>) -> Result<AsyncWasmDriver> {
        let wasm = wasm.as_ref();
        let key = *blake3::hash(wasm).as_bytes();
        let mut cache = self.cache.lock().await;
        if let Some(driver) = cache.get(&key) {
            return Ok(driver);
        }
        let driver = self.compile(wasm)?;
        driver.validate_abi()?;
        cache.insert(key, driver.clone());
        Ok(driver)
    }

    pub async fn execute_async(
        &self,
        driver: &AsyncWasmDriver,
        action: &str,
        payload: &[u8],
        limits: AsyncExecutionLimits,
    ) -> Result<AsyncWasmExecutionResult> {
        driver.validate_abi()?;
        crate::validate_permissions(limits.permissions)?;

        let state = AsyncStoreState {
            limits: StoreLimitsBuilder::new()
                .memory_size(limits.max_memory_bytes)
                .instances(1)
                .memories(1)
                .tables(1)
                .build(),
            permissions: limits.permissions,
            host_calls: Vec::new(),
        };

        let mut store = Store::new(&self.engine, state);
        store.limiter(|s| &mut s.limits);
        store.set_fuel(limits.fuel)?;

        let mut linker = Linker::new(&self.engine);
        define_async_host_imports(&mut linker)?;

        let timeout_duration = std::time::Duration::from_millis(limits.timeout_ms);
        let started = std::time::Instant::now();

        let execute_future = async {
            let instance = linker.instantiate_async(&mut store, &driver.module).await?;
            execute_async_instance(&mut store, &instance, action.as_bytes(), payload, limits).await
        };

        let output = if limits.timeout_ms > 0 {
            tokio::time::timeout(timeout_duration, execute_future)
                .await
                .map_err(|_| ZapRuntimeError::Timeout { limit_ms: limits.timeout_ms })??
        } else {
            execute_future.await?
        };

        let elapsed_micros = started.elapsed().as_micros();
        let fuel_remaining = store.get_fuel().unwrap_or(0);
        let fuel_consumed = limits.fuel.saturating_sub(fuel_remaining);
        let host_calls = std::mem::take(&mut store.data_mut().host_calls);

        Ok(AsyncWasmExecutionResult {
            output,
            fuel_consumed,
            elapsed_micros,
            host_calls,
        })
    }

    /// Spawns driver execution onto Tokio's task runtime.
    pub fn spawn(
        &self,
        driver: AsyncWasmDriver,
        action: String,
        payload: Vec<u8>,
        limits: AsyncExecutionLimits,
    ) -> tokio::task::JoinHandle<Result<AsyncWasmExecutionResult>> {
        let executor = self.clone();
        tokio::spawn(async move {
            executor.execute_async(&driver, &action, &payload, limits).await
        })
    }
}
```

#### Async Host Bindings
The linker registers async host functions using `func_wrap_async`:
1. `rivun:emit_event(ptr: i32, len: i32) -> i32`
2. `rivun:memory_read(key_ptr: i32, key_len: i32, out_ptr: i32, out_len: i32) -> i32`
3. `rivun:memory_write(ptr: i32, len: i32) -> i32`
4. `rivun:device_call(ptr: i32, len: i32) -> i32`
5. `rivun:yield_now() -> i32`: Invokes `tokio::task::yield_now().await` to cooperatively relinquish the Tokio execution thread.
6. `rivun:ipc_send(chan: i32, ptr: i32, len: i32) -> i32`
7. `rivun:ipc_recv(chan: i32, out_ptr: i32, out_len: i32) -> i64`

---

### Module 2: `streaming.rs` — Streaming I/O Buffers & Adapters

#### Purpose
Provides lock-free, zero-copy streaming buffers and protocol adapters for high-frequency industrial streaming (TCP, Modbus RTU/TCP, circular ring-buffers).

#### Key Components

```
+-----------------------------------------------------------------------------------+
|                              StreamingRingBuffer                                  |
|   +---------------------------------------------------------------------------+   |
|   |  Atomic Read Index (head)       <---->        Atomic Write Index (tail)   |   |
|   +---------------------------------------------------------------------------+   |
|   | Ring Buffer Storage [ 64KB .. 16MB ]                                      |   |
|   | - readable_slices() -> (&[u8], &[u8])                                    |   |
|   | - writable_slices() -> (&mut [u8], &mut [u8])                             |   |
|   | - Async Backpressure via tokio::sync::Notify                              |   |
|   +---------------------------------------------------------------------------+   |
+-----------------------------------------------------------------------------------+
             ^                                                        ^
             |                                                        |
+--------------------------+                             +--------------------------+
|     TcpStreamAdapter     |                             |   ModbusStreamAdapter    |
| - Framing (Length/Delim) |                             | - RTU / TCP Frame Parser |
| - Async Socket Pump      |                             | - Virtual Register Bank  |
| - Connection Recovery    |                             | - CRC-16 Verification    |
+--------------------------+                             +--------------------------+
```

#### 1. `SpscRingBuffer` (Lock-Free Circular Ring Buffer)
- **Zero-Copy Slices**:
  ```rust
  pub struct SpscRingBuffer {
      buffer: Box<[u8]>,
      capacity: usize,
      head: std::sync::atomic::AtomicUsize, // Read pointer
      tail: std::sync::atomic::AtomicUsize, // Write pointer
      read_notify: tokio::sync::Notify,
      write_notify: tokio::sync::Notify,
  }
  ```
- **Operations**:
  - `readable_slices(&self) -> (&[u8], &[u8])`: Returns up to two slices representing contiguous readable data without wrapping or allocating.
  - `writable_slices(&mut self) -> (&mut [u8], &mut [u8])`: Returns up to two slices for direct in-place writing.
  - `advance_read(&self, count: usize)` / `advance_write(&self, count: usize)`: Atomically updates indices and notifies waiting async tasks.
  - `push_async(&self, data: &[u8]) -> impl Future<Output = Result<usize, BufferError>>`: Pushes data, suspending asynchronously if capacity is insufficient until notified.
  - `pop_async(&self, buf: &mut [u8]) -> impl Future<Output = Result<usize, BufferError>>`: Reads data, suspending asynchronously if empty.

#### 2. `TcpStreamAdapter` (Async TCP Stream Adapter)
- Wraps `tokio::net::TcpStream` with bidirectional ring buffers (`rx_ring`, `tx_ring`).
- Framing options:
  - `RawStream`: Unframed raw byte stream.
  - `LengthPrefixed`: 4-byte big-endian frame length prefix.
  - `Delimiter(u8)`: Delimited frames (e.g. `\n` or `\0`).
- Background pump tasks move bytes between OS sockets and ring buffers with backpressure.

#### 3. `ModbusStreamAdapter` & `ModbusSimulator`
- **Modbus Protocol Engine**:
  - Encodes & decodes Modbus TCP (MBAP header: 2-byte transaction ID, 2-byte protocol ID `0x0000`, 2-byte length, 1-byte unit ID, 1-byte function code) and Modbus RTU frames.
  - CRC-16 checksum calculation and verification using polynomial `0xA001`.
  - Supported Function Codes:
    - `0x01` Read Coils
    - `0x02` Read Discrete Inputs
    - `0x03` Read Holding Registers
    - `0x04` Read Input Registers
    - `0x05` Write Single Coil
    - `0x06` Write Single Register
    - `0x0F` Write Multiple Coils
    - `0x10` Write Multiple Registers
- **Modbus Simulation Server (`ModbusSimulator`)**:
  - Maintains virtual memory map:
    - 65,536 Holding Registers (`u16`)
    - 65,536 Input Registers (`u16`)
    - 65,536 Coils (`bool`)
    - 65,536 Discrete Inputs (`bool`)
  - Thread-safe atomic / RWLock state.
  - Generates standard Modbus exception frames (`0x01` Illegal Function, `0x02` Illegal Address, `0x03` Illegal Value, `0x04` Server Failure) upon out-of-bounds access.

---

### Module 3: `ipc.rs` — Deterministic Inter-Driver IPC

#### Purpose
Enables isolated WebAssembly driver instances to exchange structured messages with deterministic ordering, zero-copy host mediation, and complete memory sandboxing.

#### Memory Isolation Model
- **No Shared Guest Memory**: WebAssembly instances MUST NOT share linear memory pointers. Sharing pointers between Wasmtime instances violates isolation invariants and leads to undefined behavior.
- **Zero-Copy / Minimal-Copy Host Mediation**:
  1. Driver A writes message to its own memory and calls `rivun:ipc_send(channel, ptr, len)`.
  2. Host validates channel permissions, reads slice from Instance A's memory into a ref-counted immutable `bytes::Bytes`.
  3. Host routes `bytes::Bytes` to Driver B's inbox queue.
  4. When Driver B calls `rivun:ipc_recv(channel, out_ptr, max_len)` (or during pipeline handoff), host writes the bytes directly into Driver B's allocated memory buffer.

```
+--------------------------+                             +--------------------------+
|     WASM Instance A      |                             |     WASM Instance B      |
|  (Perception Driver)     |                             |  (Safety Policy Driver)  |
|  Linear Memory 0..16MB   |                             |  Linear Memory 0..16MB   |
+--------------------------+                             +--------------------------+
             |                                                         ^
             | rivun:ipc_send                                            | rivun:ipc_recv
             v                                                         |
+-----------------------------------------------------------------------------------+
|                                Host IPC Router                                    |
|  - Channel Table: Channel ID -> (Sender, Recipient, Queue)                        |
|  - Zero-Copy Buffer Pool (`bytes::Bytes` slice transfer)                          |
|  - Sequence Monotonicity Tracker (seq: 1, 2, 3, ...)                              |
|  - Blake3 Causal Transcript: H_n = Blake3(H_{n-1} || channel || seq || payload)  |
+-----------------------------------------------------------------------------------+
```

#### Core IPC Types

```rust
use bytes::Bytes;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IpcMessage {
    pub channel_id: u32,
    pub sender: String,
    pub recipient: String,
    pub sequence: u64,
    pub timestamp_ns: u64,
    pub payload: Bytes,
    pub causal_hash: [u8; 32],
}

pub struct IpcPipe {
    channel_id: u32,
    sender_name: String,
    recipient_name: String,
    queue: tokio::sync::mpsc::Sender<IpcMessage>,
    capacity: usize,
    sequence_counter: std::sync::atomic::AtomicU64,
    previous_hash: std::sync::Mutex<[u8; 32]>,
}

pub struct IpcRouter {
    channels: std::collections::HashMap<u32, Arc<IpcPipe>>,
}
```

---

### Module 4: `pipeline.rs` — `DriverPipeline` Multi-Stage Orchestrator

#### Purpose
Orchestrates multi-stage driver pipelines (e.g. **Perception $\rightarrow$ Safety Policy $\rightarrow$ Actuator**) with:
1. **Strict aggregate fuel budgeting** across all stages.
2. **Microsecond latency profiling** per stage and end-to-end.
3. **Blake3 rolling causal chain hashing** linking input telemetry to actuator output.
4. **Synchronous, Asynchronous, and Streaming execution modes**.

#### Pipeline Architecture

```
                                  DriverPipeline
 +-------------------------------------------------------------------------------+
 | Total Fuel Limit: 10,000,000 | Causal Chain: H_0 -> H_1 -> H_2 -> H_3         |
 +-------------------------------------------------------------------------------+
        |                               |                               |
        v Stage 1                       v Stage 2                       v Stage 3
 +--------------------+          +--------------------+          +--------------------+
 | Perception Driver  |  output  | Safety Policy      |  output  | Actuator Driver    |
 | - parse sensor telemetry   | ------>  | - check boundaries | ------>  | - encode Modbus    |
 | - extract obstacles|          | - verify invariants|          |   control packet   |
 +--------------------+          +--------------------+          +--------------------+
   Fuel: 120,400                   Fuel: 45,200                    Fuel: 88,100
   Duration: 180 µs                Duration: 65 µs                 Duration: 110 µs
        \                               |                               /
         \------------------------------+------------------------------/
                                        |
                                        v
                       +---------------------------------+
                       |    PipelineExecutionReport      |
                       | - total_fuel_consumed: 253,700  |
                       | - total_duration_micros: 355 µs |
                       | - causal_chain_hash: "blake3:.."|
                       | - final_output: [actuator cmds] |
                       +---------------------------------+
```

#### Pipeline Data Structures & Implementation

```rust
use crate::{DriverPermissions, ZapRuntimeError, WasmExecutor, AsyncWasmExecutor, AsyncExecutionLimits};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum PipelineError {
    #[error("pipeline is empty: at least one driver stage required")]
    EmptyPipeline,
    #[error("pipeline stage {stage_index} (`{driver_name}`) failed: {error}")]
    StageExecutionFailed {
        stage_index: usize,
        driver_name: String,
        error: String,
    },
    #[error("pipeline fuel limit exceeded: consumed {consumed}, limit {limit}")]
    FuelLimitExceeded { consumed: u64, limit: u64 },
    #[error("pipeline stage {stage_index} (`{driver_name}`) timed out after {timeout_ms} ms")]
    StageTimeout {
        stage_index: usize,
        driver_name: String,
        timeout_ms: u64,
    },
    #[error("channel buffer capacity overflow (max {max} bytes)")]
    BufferOverflow { max: usize },
    #[error("safety policy rejected execution at stage {stage_index}: {reason}")]
    SafetyPolicyRejected {
        stage_index: usize,
        reason: String,
    },
}

#[derive(Clone)]
pub struct PipelineStage {
    pub name: String,
    pub action: String,
    pub wasm_binary: Vec<u8>,
    pub permissions: DriverPermissions,
    pub fuel_limit: Option<u64>,
    pub timeout_ms: Option<u64>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct PipelineStageResult {
    pub stage_index: usize,
    pub stage_name: String,
    pub action: String,
    pub fuel_consumed: u64,
    pub output_len: usize,
    pub output_hash: String,
    pub duration_micros: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct PipelineExecutionReport {
    pub pipeline_id: String,
    pub stages: Vec<PipelineStageResult>,
    pub total_fuel_consumed: u64,
    pub total_duration_micros: u64,
    pub final_output: Vec<u8>,
    pub causal_chain_hash: String,
}

pub struct DriverPipeline {
    pub name: String,
    stages: Vec<PipelineStage>,
    max_total_fuel: u64,
    stage_timeout_ms: u64,
}

impl DriverPipeline {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            stages: Vec::new(),
            max_total_fuel: 10_000_000,
            stage_timeout_ms: 1_000,
        }
    }

    pub fn with_max_fuel(mut self, max_fuel: u64) -> Self {
        self.max_total_fuel = max_fuel;
        self
    }

    pub fn with_stage_timeout_ms(mut self, timeout_ms: u64) -> Self {
        self.stage_timeout_ms = timeout_ms;
        self
    }

    pub fn add_stage(
        mut self,
        name: impl Into<String>,
        action: impl Into<String>,
        wasm_binary: Vec<u8>,
        permissions: DriverPermissions,
        fuel_limit: Option<u64>,
    ) -> Self {
        self.stages.push(PipelineStage {
            name: name.into(),
            action: action.into(),
            wasm_binary,
            permissions,
            fuel_limit,
            timeout_ms: None,
        });
        self
    }

    /// Synchronous execution with accurate fuel tracking and causal hashing.
    pub fn execute(&self, initial_payload: &[u8]) -> Result<PipelineExecutionReport, PipelineError> {
        if self.stages.is_empty() {
            return Err(PipelineError::EmptyPipeline);
        }

        let executor = WasmExecutor::new().map_err(|e| PipelineError::StageExecutionFailed {
            stage_index: 0,
            driver_name: "init".into(),
            error: e.to_string(),
        })?;

        let mut current_payload = initial_payload.to_vec();
        let mut stage_results = Vec::new();
        let mut total_fuel_consumed = 0u64;
        let pipeline_start = std::time::Instant::now();

        let mut causal_hasher = blake3::Hasher::new();
        causal_hasher.update(b"rivun-PIPELINE-START-v1:");
        causal_hasher.update(self.name.as_bytes());
        causal_hasher.update(initial_payload);

        for (idx, stage) in self.stages.iter().enumerate() {
            let fuel_remaining_for_pipeline = self.max_total_fuel.saturating_sub(total_fuel_consumed);
            if fuel_remaining_for_pipeline == 0 {
                return Err(PipelineError::FuelLimitExceeded {
                    consumed: total_fuel_consumed,
                    limit: self.max_total_fuel,
                });
            }

            let stage_fuel_budget = match stage.fuel_limit {
                Some(limit) => limit.min(fuel_remaining_for_pipeline),
                None => fuel_remaining_for_pipeline,
            };

            let limits = crate::ExecutionLimits {
                fuel: stage_fuel_budget,
                timeout_ms: stage.timeout_ms.unwrap_or(self.stage_timeout_ms),
                permissions: stage.permissions,
                ..Default::default()
            };

            let stage_start = std::time::Instant::now();
            let result = executor
                .execute_bytes(&stage.wasm_binary, &stage.action, &current_payload, limits)
                .map_err(|e| PipelineError::StageExecutionFailed {
                    stage_index: idx,
                    driver_name: stage.name.clone(),
                    error: e.to_string(),
                })?;

            let stage_duration = stage_start.elapsed().as_micros() as u64;
            total_fuel_consumed = total_fuel_consumed.saturating_add(result.fuel_consumed);

            if total_fuel_consumed > self.max_total_fuel {
                return Err(PipelineError::FuelLimitExceeded {
                    consumed: total_fuel_consumed,
                    limit: self.max_total_fuel,
                });
            }

            let output_hash = blake3::hash(&result.output).to_hex().to_string();
            causal_hasher.update(stage.name.as_bytes());
            causal_hasher.update(stage.action.as_bytes());
            causal_hasher.update(output_hash.as_bytes());
            causal_hasher.update(&result.fuel_consumed.to_le_bytes());

            stage_results.push(PipelineStageResult {
                stage_index: idx,
                stage_name: stage.name.clone(),
                action: stage.action.clone(),
                fuel_consumed: result.fuel_consumed,
                output_len: result.output.len(),
                output_hash,
                duration_micros: stage_duration,
            });

            current_payload = result.output;
        }

        let causal_chain_hash = causal_hasher.finalize().to_hex().to_string();

        Ok(PipelineExecutionReport {
            pipeline_id: self.name.clone(),
            stages: stage_results,
            total_fuel_consumed,
            total_duration_micros: pipeline_start.elapsed().as_micros() as u64,
            final_output: current_payload,
            causal_chain_hash,
        })
    }

    /// Asynchronous execution on Tokio task runtime.
    pub async fn execute_async(&self, initial_payload: &[u8]) -> Result<PipelineExecutionReport, PipelineError> {
        if self.stages.is_empty() {
            return Err(PipelineError::EmptyPipeline);
        }

        let executor = AsyncWasmExecutor::new().map_err(|e| PipelineError::StageExecutionFailed {
            stage_index: 0,
            driver_name: "init".into(),
            error: e.to_string(),
        })?;

        let mut current_payload = initial_payload.to_vec();
        let mut stage_results = Vec::new();
        let mut total_fuel_consumed = 0u64;
        let pipeline_start = std::time::Instant::now();

        let mut causal_hasher = blake3::Hasher::new();
        causal_hasher.update(b"rivun-PIPELINE-START-v1:");
        causal_hasher.update(self.name.as_bytes());
        causal_hasher.update(initial_payload);

        for (idx, stage) in self.stages.iter().enumerate() {
            let fuel_remaining_for_pipeline = self.max_total_fuel.saturating_sub(total_fuel_consumed);
            if fuel_remaining_for_pipeline == 0 {
                return Err(PipelineError::FuelLimitExceeded {
                    consumed: total_fuel_consumed,
                    limit: self.max_total_fuel,
                });
            }

            let stage_fuel_budget = match stage.fuel_limit {
                Some(limit) => limit.min(fuel_remaining_for_pipeline),
                None => fuel_remaining_for_pipeline,
            };

            let limits = AsyncExecutionLimits {
                fuel: stage_fuel_budget,
                timeout_ms: stage.timeout_ms.unwrap_or(self.stage_timeout_ms),
                permissions: stage.permissions,
                ..Default::default()
            };

            let driver = executor
                .compile_and_validate_cached(&stage.wasm_binary)
                .await
                .map_err(|e| PipelineError::StageExecutionFailed {
                    stage_index: idx,
                    driver_name: stage.name.clone(),
                    error: e.to_string(),
                })?;

            let stage_start = std::time::Instant::now();
            let result = executor
                .execute_async(&driver, &stage.action, &current_payload, limits)
                .await
                .map_err(|e| PipelineError::StageExecutionFailed {
                    stage_index: idx,
                    driver_name: stage.name.clone(),
                    error: e.to_string(),
                })?;

            let stage_duration = stage_start.elapsed().as_micros() as u64;
            total_fuel_consumed = total_fuel_consumed.saturating_add(result.fuel_consumed);

            if total_fuel_consumed > self.max_total_fuel {
                return Err(PipelineError::FuelLimitExceeded {
                    consumed: total_fuel_consumed,
                    limit: self.max_total_fuel,
                });
            }

            let output_hash = blake3::hash(&result.output).to_hex().to_string();
            causal_hasher.update(stage.name.as_bytes());
            causal_hasher.update(stage.action.as_bytes());
            causal_hasher.update(output_hash.as_bytes());
            causal_hasher.update(&result.fuel_consumed.to_le_bytes());

            stage_results.push(PipelineStageResult {
                stage_index: idx,
                stage_name: stage.name.clone(),
                action: stage.action.clone(),
                fuel_consumed: result.fuel_consumed,
                output_len: result.output.len(),
                output_hash,
                duration_micros: stage_duration,
            });

            current_payload = result.output;
        }

        let causal_chain_hash = causal_hasher.finalize().to_hex().to_string();

        Ok(PipelineExecutionReport {
            pipeline_id: self.name.clone(),
            stages: stage_results,
            total_fuel_consumed,
            total_duration_micros: pipeline_start.elapsed().as_micros() as u64,
            final_output: current_payload,
            causal_chain_hash,
        })
    }
}
```

---

## 3. Dependency Updates & Workspace Configuration

To support async Wasmtime and Tokio primitives in `rivun-runtime`:

### `Cargo.toml` (Workspace Root)
Ensure `wasmtime` has `async` enabled:
```toml
wasmtime = { version = "45.0.1", default-features = false, features = ["cranelift", "runtime", "std", "wat", "async"] }
```

### `crates/rivun-runtime/Cargo.toml`
```toml
[dependencies]
blake3.workspace = true
bytes.workspace = true
serde.workspace = true
thiserror.workspace = true
tokio = { workspace = true, features = ["sync", "time", "rt", "macros", "net", "io-util"] }
tracing.workspace = true
wasmtime = { workspace = true, features = ["async"] }
rivun-capability.workspace = true

[dev-dependencies]
criterion.workspace = true
wat.workspace = true
```

---

## 4. Comprehensive Testing & Verification Plan

### Test Matrix

| Category | Test Case | Target / Invariant |
|---|---|---|
| **Async Engine** | `test_async_execute_echo_driver` | Non-blocking execution returns expected payload |
| **Async Engine** | `test_async_fuel_exhaustion` | Out-of-fuel trap cleanly halts async task |
| **Async Engine** | `test_async_timeout_cancellation` | Infinite loops abort at exact wall-clock deadline |
| **Async Engine** | `test_async_concurrent_tasks` | 100 concurrent Tokio tasks execute drivers without interference |
| **Async Engine** | `test_async_host_yield` | Guest calling `yield_now` cooperatively yields execution |
| **Streaming** | `test_spsc_ring_buffer_push_pop` | Lock-free push & pop maintains byte FIFO integrity |
| **Streaming** | `test_spsc_zero_copy_slices` | `readable_slices` and `writable_slices` wrap around ring correctly |
| **Streaming** | `test_streaming_backpressure_notify` | Consumer wakes up producer on space availability |
| **Streaming** | `test_tcp_stream_framing_round_trip` | Length-prefixed and delimiter-framed packets transfer cleanly |
| **Streaming** | `test_modbus_crc16_and_frame_parsing` | RTU/TCP frame parsing with standard polynomial 0xA001 |
| **Streaming** | `test_modbus_simulator_virtual_registers`| Read/write holding registers (0x03, 0x06, 0x10) and exceptions |
| **IPC** | `test_ipc_point_to_point_message` | Driver A sends to Driver B via host pipe with memory isolation |
| **IPC** | `test_ipc_deterministic_sequence_order` | Monotonic sequence numbering and drop detection |
| **IPC** | `test_ipc_causal_hash_transcript` | Cryptographic transcript matches hash chain |
| **Pipeline** | `test_three_stage_robotics_pipeline` | Perception $\rightarrow$ Safety Filter $\rightarrow$ Actuator execution |
| **Pipeline** | `test_pipeline_aggregate_fuel_exhaustion`| Total fuel across 3 stages capped strictly at budget |
| **Pipeline** | `test_pipeline_stage_failure_isolation` | Stage 2 failure cleanly reports error without dangling resources |
| **Pipeline** | `test_pipeline_causal_chain_verification`| Intermediate output hashes form verifiable causal proof |

---

## 5. Security, Sandboxing & Memory Isolation Guarantees

1. **Strict WebAssembly Memory Sandboxing**:
   - Each driver module is instantiated inside an isolated `wasmtime::Store` with its own linear memory address space.
   - Pointer offsets in guest linear memory cannot address host memory or peer instance memory.
2. **No Shared Guest Linear Memory**:
   - Memory is strictly mediated by the host runtime. Slices are copied safely via bounds-checked host primitives into target instances using guest-exported `@@rivun_HEADER@@alloc`.
3. **Denial-of-Service Defense**:
   - Compute loops are bounded by strict instruction-level **fuel limits** and **epoch deadline tickers**.
   - Memory growth is strictly capped by `StoreLimits::memory_size`.
   - IPC channels have bounded capacity to prevent unbounded memory consumption.
4. **Cryptographic Causal Provenance**:
   - Every intermediate stage produces a Blake3 digest of its output.
   - The aggregate causal chain hash binds the pipeline execution into an immutable, audit-verifiable artifact.

