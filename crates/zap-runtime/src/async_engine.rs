//! Asynchronous WASM driver execution engine with Tokio task scheduling,
//! deterministic fuel tracking, streaming host bindings, and compiled async module caching.

use crate::{
    expect_func, expect_memory,
    ipc::IpcRouter,
    streaming::StreamingBufferPool,
    DriverPermissions, ExecutionLimits, HostCallKind, HostCallRecord, ZapRuntimeError, Result,
};
use std::{
    collections::{HashMap, VecDeque},
    sync::Arc,
    time::{Duration, Instant},
};
use wasmtime::{
    Caller, Config, Engine, Linker, Module, Store, StoreLimits,
    StoreLimitsBuilder, ValType,
};
use zap_capability::DEFAULT_MAX_HOST_CALL_BYTES;

const MEMORY_EXPORT: &str = "memory";
const ALLOC_EXPORT: &str = "zap_alloc";
const DEALLOC_EXPORT: &str = "zap_dealloc";
const EXECUTE_EXPORT: &str = "zap_execute";
const HOST_MODULE: &str = "zap";
const HOST_EMIT_EVENT: &str = "emit_event";
const HOST_MEMORY_READ: &str = "memory_read";
const HOST_MEMORY_WRITE: &str = "memory_write";
const HOST_DEVICE_CALL: &str = "device_call";
const HOST_ASYNC_STREAM_READ: &str = "async_stream_read";
const HOST_ASYNC_STREAM_WRITE: &str = "async_stream_write";
const HOST_ASYNC_DEVICE_CALL: &str = "async_device_call";

const HOST_DENIED: i32 = -1;
const HOST_NOT_CONFIGURED: i32 = -2;
const HOST_BAD_POINTER: i32 = -3;
const HOST_TOO_LARGE: i32 = -4;
const HOST_MEMORY_ERROR: i32 = -5;

/// Result of an asynchronous WASM driver execution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AsyncWasmExecutionResult {
    pub output: Vec<u8>,
    pub fuel_consumed: u64,
    pub elapsed_ms: u128,
    pub host_calls: Vec<HostCallRecord>,
}

/// Internal store state for asynchronous WASM driver instances.
pub struct AsyncStoreState {
    pub limits: StoreLimits,
    pub permissions: DriverPermissions,
    pub host_call_byte_limit: usize,
    pub host_calls: Vec<HostCallRecord>,
    pub stream_pool: Arc<tokio::sync::RwLock<StreamingBufferPool>>,
    pub ipc_router: Arc<tokio::sync::RwLock<IpcRouter>>,
}

impl AsyncStoreState {
    pub fn new(
        limits: StoreLimits,
        permissions: DriverPermissions,
        stream_pool: Arc<tokio::sync::RwLock<StreamingBufferPool>>,
        ipc_router: Arc<tokio::sync::RwLock<IpcRouter>>,
    ) -> Self {
        Self {
            limits,
            permissions,
            host_call_byte_limit: DEFAULT_MAX_HOST_CALL_BYTES as usize,
            host_calls: Vec::new(),
            stream_pool,
            ipc_router,
        }
    }
}

/// Compiled asynchronous WASM driver artifact with cached validation info.
#[derive(Clone, Debug)]
pub struct AsyncCompiledDriver {
    pub module: Module,
    pub digest: String,
}

impl AsyncCompiledDriver {
    pub fn validate_abi(&self) -> Result<()> {
        expect_memory(&self.module, MEMORY_EXPORT)?;
        expect_func(
            &self.module,
            ALLOC_EXPORT,
            &[ValType::I32],
            &[ValType::I32],
            "(i32) -> i32",
        )?;
        expect_func(
            &self.module,
            DEALLOC_EXPORT,
            &[ValType::I32, ValType::I32],
            &[],
            "(i32, i32) -> ()",
        )?;
        expect_func(
            &self.module,
            EXECUTE_EXPORT,
            &[ValType::I32, ValType::I32, ValType::I32, ValType::I32],
            &[ValType::I64],
            "(i32, i32, i32, i32) -> i64",
        )
    }
}

/// Thread-safe LRU/FIFO cache for compiled async WASM modules.
pub struct AsyncWasmModuleCache {
    capacity: usize,
    modules: HashMap<String, AsyncCompiledDriver>,
    order: VecDeque<String>,
}

impl AsyncWasmModuleCache {
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity: capacity.max(1),
            modules: HashMap::new(),
            order: VecDeque::new(),
        }
    }

    pub fn get(&self, digest: &str) -> Option<AsyncCompiledDriver> {
        self.modules.get(digest).cloned()
    }

    pub fn insert(&mut self, driver: AsyncCompiledDriver) {
        if self.modules.contains_key(&driver.digest) {
            return;
        }
        if self.modules.len() >= self.capacity {
            if let Some(oldest) = self.order.pop_front() {
                self.modules.remove(&oldest);
            }
        }
        self.order.push_back(driver.digest.clone());
        self.modules.insert(driver.digest.clone(), driver);
    }
}

/// Asynchronous, non-blocking WASM driver execution host.
#[derive(Clone)]
pub struct AsyncWasmExecutor {
    engine: Engine,
    module_cache: Arc<tokio::sync::RwLock<AsyncWasmModuleCache>>,
    stream_pool: Arc<tokio::sync::RwLock<StreamingBufferPool>>,
    ipc_router: Arc<tokio::sync::RwLock<IpcRouter>>,
}

impl AsyncWasmExecutor {
    /// Create a new asynchronous WASM executor.
    pub fn new() -> Result<Self> {
        let mut config = Config::new();
        config.consume_fuel(true);
        config.epoch_interruption(true);
        config.wasm_backtrace_details(wasmtime::WasmBacktraceDetails::Enable);

        let engine = Engine::new(&config)?;

        // Background epoch ticking for async interruption
        let engine_clone = engine.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_millis(1));
            loop {
                interval.tick().await;
                engine_clone.increment_epoch();
            }
        });

        Ok(Self {
            engine,
            module_cache: Arc::new(tokio::sync::RwLock::new(AsyncWasmModuleCache::new(64))),
            stream_pool: Arc::new(tokio::sync::RwLock::new(StreamingBufferPool::new())),
            ipc_router: Arc::new(tokio::sync::RwLock::new(IpcRouter::new())),
        })
    }

    pub fn engine(&self) -> &Engine {
        &self.engine
    }

    pub fn stream_pool(&self) -> Arc<tokio::sync::RwLock<StreamingBufferPool>> {
        self.stream_pool.clone()
    }

    pub fn ipc_router(&self) -> Arc<tokio::sync::RwLock<IpcRouter>> {
        self.ipc_router.clone()
    }

    /// Compile and validate a WASM binary for asynchronous execution.
    pub fn compile_and_validate(&self, wasm_bytes: &[u8]) -> Result<AsyncCompiledDriver> {
        let module = Module::new(&self.engine, wasm_bytes)?;
        let digest = blake3::hash(wasm_bytes).to_hex().to_string();
        let driver = AsyncCompiledDriver {
            module,
            digest,
        };
        driver.validate_abi()?;
        Ok(driver)
    }

    /// Compile and validate with module caching.
    pub async fn compile_and_validate_cached(&self, wasm_bytes: &[u8]) -> Result<AsyncCompiledDriver> {
        let digest = blake3::hash(wasm_bytes).to_hex().to_string();
        {
            let cache = self.module_cache.read().await;
            if let Some(driver) = cache.get(&digest) {
                return Ok(driver);
            }
        }

        let driver = self.compile_and_validate(wasm_bytes)?;
        {
            let mut cache = self.module_cache.write().await;
            cache.insert(driver.clone());
        }
        Ok(driver)
    }

    /// Asynchronously execute a WASM driver with fuel budgeting and memory sandboxing.
    pub async fn execute_async(
        &self,
        driver: &AsyncCompiledDriver,
        action: &str,
        payload: &[u8],
        limits: ExecutionLimits,
    ) -> Result<AsyncWasmExecutionResult> {
        let action_len: i32 = action.len().try_into().map_err(|_| ZapRuntimeError::InputTooLarge(action.len()))?;
        let payload_len: i32 = payload.len().try_into().map_err(|_| ZapRuntimeError::InputTooLarge(payload.len()))?;

        let timeout_ms = limits.timeout_ms;

        let fut = self.execute_internal(driver, action, action_len, payload, payload_len, limits);

        match tokio::time::timeout(Duration::from_millis(timeout_ms), fut).await {
            Ok(res) => res,
            Err(_) => Err(ZapRuntimeError::Timeout { limit_ms: timeout_ms }),
        }
    }

    async fn execute_internal(
        &self,
        driver: &AsyncCompiledDriver,
        action: &str,
        action_len: i32,
        payload: &[u8],
        payload_len: i32,
        limits: ExecutionLimits,
    ) -> Result<AsyncWasmExecutionResult> {
        let start = Instant::now();

        let store_limits = StoreLimitsBuilder::new()
            .memory_size(limits.max_memory_bytes)
            .instances(1)
            .memories(1)
            .tables(1)
            .build();

        let mut store = Store::new(
            &self.engine,
            AsyncStoreState::new(
                store_limits,
                limits.permissions,
                self.stream_pool.clone(),
                self.ipc_router.clone(),
            ),
        );

        store.limiter(|state| &mut state.limits);
        store.set_fuel(limits.fuel)?;
        store.set_epoch_deadline(limits.timeout_ms.max(1));

        let mut linker = Linker::new(&self.engine);
        self.bind_host_functions(&mut linker)?;

        let instance = linker.instantiate_async(&mut store, &driver.module).await?;
        let memory = instance
            .get_memory(&mut store, MEMORY_EXPORT)
            .ok_or(ZapRuntimeError::MissingExport(MEMORY_EXPORT))?;

        let alloc_fn = instance.get_typed_func::<i32, i32>(&mut store, ALLOC_EXPORT)?;
        let dealloc_fn = instance.get_typed_func::<(i32, i32), ()>(&mut store, DEALLOC_EXPORT)?;
        let execute_fn = instance.get_typed_func::<(i32, i32, i32, i32), i64>(&mut store, EXECUTE_EXPORT)?;

        let action_ptr = alloc_fn.call_async(&mut store, action_len).await?;
        let payload_ptr = alloc_fn.call_async(&mut store, payload_len).await?;

        // Write action & payload into WASM memory
        memory.write(&mut store, action_ptr as usize, action.as_bytes())
            .map_err(|e| ZapRuntimeError::MemoryAccess(e.to_string()))?;
        memory.write(&mut store, payload_ptr as usize, payload)
            .map_err(|e| ZapRuntimeError::MemoryAccess(e.to_string()))?;

        // Call execute
        let packed_res = execute_fn
            .call_async(&mut store, (action_ptr, action_len, payload_ptr, payload_len))
            .await?;

        let result_ptr = (packed_res as u64 >> 32) as u32;
        let result_len = (packed_res as u64 & 0xFFFF_FFFF) as usize;

        if result_len > limits.max_output_bytes {
            return Err(ZapRuntimeError::OutputTooLarge {
                max: limits.max_output_bytes,
                actual: result_len,
            });
        }

        let mut output = vec![0u8; result_len];
        if result_len > 0 {
            memory
                .read(&store, result_ptr as usize, &mut output)
                .map_err(|e| ZapRuntimeError::MemoryAccess(e.to_string()))?;
        }

        // Clean up allocations
        let _ = dealloc_fn.call_async(&mut store, (action_ptr, action_len)).await;
        let _ = dealloc_fn.call_async(&mut store, (payload_ptr, payload_len)).await;
        if result_len > 0 {
            let _ = dealloc_fn.call_async(&mut store, (result_ptr as i32, result_len as i32)).await;
        }

        let remaining_fuel = store.get_fuel().unwrap_or(0);
        let fuel_consumed = limits.fuel.saturating_sub(remaining_fuel);
        let elapsed_ms = start.elapsed().as_millis();
        let host_calls = store.data().host_calls.clone();

        Ok(AsyncWasmExecutionResult {
            output,
            fuel_consumed,
            elapsed_ms,
            host_calls,
        })
    }

    fn bind_host_functions(&self, linker: &mut Linker<AsyncStoreState>) -> Result<()> {
        // zap.emit_event(ptr: i32, len: i32) -> i32
        linker.func_wrap(
            HOST_MODULE,
            HOST_EMIT_EVENT,
            |mut caller: Caller<'_, AsyncStoreState>, ptr: i32, len: i32| -> i32 {
                if !caller.data().permissions.emit_event {
                    return HOST_DENIED;
                }
                if len < 0 || ptr < 0 {
                    return HOST_BAD_POINTER;
                }
                let ulen = len as usize;
                if ulen > caller.data().host_call_byte_limit {
                    return HOST_TOO_LARGE;
                }
                let Some(wasmtime::Extern::Memory(mem)) = caller.get_export(MEMORY_EXPORT) else {
                    return HOST_MEMORY_ERROR;
                };
                let mut buf = vec![0u8; ulen];
                if mem.read(&caller, ptr as usize, &mut buf).is_err() {
                    return HOST_MEMORY_ERROR;
                }
                caller.data_mut().host_calls.push(HostCallRecord {
                    kind: HostCallKind::EmitEvent,
                    payload: buf,
                });
                0
            },
        )?;

        // zap.memory_read(offset: i32, len: i32, ptr: i32) -> i32
        linker.func_wrap(
            HOST_MODULE,
            HOST_MEMORY_READ,
            |caller: Caller<'_, AsyncStoreState>, _offset: i32, _len: i32, _ptr: i32| -> i32 {
                if !caller.data().permissions.memory_read {
                    return HOST_DENIED;
                }
                0
            },
        )?;

        // zap.memory_write(offset: i32, ptr: i32, len: i32) -> i32
        linker.func_wrap(
            HOST_MODULE,
            HOST_MEMORY_WRITE,
            |caller: Caller<'_, AsyncStoreState>, _offset: i32, _ptr: i32, _len: i32| -> i32 {
                if !caller.data().permissions.memory_write {
                    return HOST_DENIED;
                }
                0
            },
        )?;

        // zap.device_call(port: i32, cmd_ptr: i32, cmd_len: i32, out_ptr: i32, max_out_len: i32) -> i32
        linker.func_wrap(
            HOST_MODULE,
            HOST_DEVICE_CALL,
            |caller: Caller<'_, AsyncStoreState>, _port: i32, _cmd_ptr: i32, _cmd_len: i32, _out_ptr: i32, _max_out_len: i32| -> i32 {
                if !caller.data().permissions.device_call {
                    return HOST_DENIED;
                }
                0
            },
        )?;

        // zap.async_stream_read(stream_id: i32, ptr: i32, max_len: i32) -> i32
        linker.func_wrap_async(
            HOST_MODULE,
            HOST_ASYNC_STREAM_READ,
            |mut caller: Caller<'_, AsyncStoreState>, (stream_id, ptr, max_len): (i32, i32, i32)| {
                Box::new(async move {
                    if stream_id < 0 || ptr < 0 || max_len < 0 {
                        return Ok(HOST_BAD_POINTER);
                    }
                    let pool = caller.data().stream_pool.clone();
                    let stream_res = {
                        let p = pool.read().await;
                        p.read_async(stream_id as u32, max_len as usize).await
                    };

                    match stream_res {
                        Ok(data) => {
                            let Some(wasmtime::Extern::Memory(mem)) = caller.get_export(MEMORY_EXPORT) else {
                                return Ok(HOST_MEMORY_ERROR);
                            };
                            if mem.write(&mut caller, ptr as usize, &data).is_err() {
                                return Ok(HOST_MEMORY_ERROR);
                            }
                            Ok(data.len() as i32)
                        }
                        Err(_) => Ok(HOST_NOT_CONFIGURED),
                    }
                })
            },
        )?;

        // zap.async_stream_write(stream_id: i32, ptr: i32, len: i32) -> i32
        linker.func_wrap_async(
            HOST_MODULE,
            HOST_ASYNC_STREAM_WRITE,
            |mut caller: Caller<'_, AsyncStoreState>, (stream_id, ptr, len): (i32, i32, i32)| {
                Box::new(async move {
                    if stream_id < 0 || ptr < 0 || len < 0 {
                        return Ok(HOST_BAD_POINTER);
                    }
                    let ulen = len as usize;
                    let Some(wasmtime::Extern::Memory(mem)) = caller.get_export(MEMORY_EXPORT) else {
                        return Ok(HOST_MEMORY_ERROR);
                    };
                    let mut buf = vec![0u8; ulen];
                    if mem.read(&caller, ptr as usize, &mut buf).is_err() {
                        return Ok(HOST_MEMORY_ERROR);
                    }

                    let pool = caller.data().stream_pool.clone();
                    let write_res = {
                        let p = pool.read().await;
                        p.write_async(stream_id as u32, &buf).await
                    };

                    match write_res {
                        Ok(n) => Ok(n as i32),
                        Err(_) => Ok(HOST_NOT_CONFIGURED),
                    }
                })
            },
        )?;

        // zap.async_device_call(port: i32, cmd_ptr: i32, cmd_len: i32, out_ptr: i32, max_out_len: i32) -> i32
        linker.func_wrap_async(
            HOST_MODULE,
            HOST_ASYNC_DEVICE_CALL,
            |caller: Caller<'_, AsyncStoreState>, (_port, _cmd_ptr, _cmd_len, _out_ptr, _max_out_len): (i32, i32, i32, i32, i32)| {
                Box::new(async move {
                    if !caller.data().permissions.device_call {
                        return Ok(HOST_DENIED);
                    }
                    Ok(0)
                })
            },
        )?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ECHO_WAT: &str = r#"
(module
  (memory (export "memory") 1)
  (func (export "zap_alloc") (param i32) (result i32) (i32.const 1024))
  (func (export "zap_dealloc") (param i32 i32))
  (func (export "zap_execute") (param i32 i32 i32 i32) (result i64)
    (local $action_ptr i32)
    (local $action_len i32)
    (local $payload_ptr i32)
    (local $payload_len i32)
    (local.set $action_ptr (local.get 0))
    (local.set $action_len (local.get 1))
    (local.set $payload_ptr (local.get 2))
    (local.set $payload_len (local.get 3))
    (i64.or
      (i64.shl (i64.extend_i32_u (local.get $payload_ptr)) (i64.const 32))
      (i64.extend_i32_u (local.get $payload_len)))))
"#;

    #[tokio::test]
    async fn test_async_wasm_executor_echo() {
        let executor = AsyncWasmExecutor::new().unwrap();
        let wasm = wat::parse_str(ECHO_WAT).unwrap();
        let driver = executor.compile_and_validate_cached(&wasm).await.unwrap();

        let payload = b"async_tokio_wasm_payload_123";
        let res = executor
            .execute_async(&driver, "echo", payload, ExecutionLimits::default())
            .await
            .unwrap();

        assert_eq!(res.output, payload);
        assert!(res.fuel_consumed > 0);
    }

    const STREAMING_WAT: &str = r#"
(module
  (import "zap" "async_stream_read" (func $stream_read (param i32 i32 i32) (result i32)))
  (import "zap" "async_stream_write" (func $stream_write (param i32 i32 i32) (result i32)))
  (memory (export "memory") 1)
  (func (export "zap_alloc") (param i32) (result i32) (i32.const 2048))
  (func (export "zap_dealloc") (param i32 i32))
  (func (export "zap_execute") (param i32 i32 i32 i32) (result i64)
    (local $read_bytes i32)
    ;; Read up to 32 bytes from stream ID 1 into memory at offset 1024
    (local.set $read_bytes (call $stream_read (i32.const 1) (i32.const 1024) (i32.const 32)))
    ;; Write those bytes into stream ID 2
    (drop (call $stream_write (i32.const 2) (i32.const 1024) (local.get $read_bytes)))
    ;; Return 1024 ptr and read_bytes len
    (i64.or
      (i64.shl (i64.extend_i32_u (i32.const 1024)) (i64.const 32))
      (i64.extend_i32_u (local.get $read_bytes)))))
"#;

    #[tokio::test]
    async fn test_async_wasm_streaming_host_calls() {
        use crate::streaming::{SpscRingBuffer, StreamBackpressurePolicy, StreamTransport};

        let executor = AsyncWasmExecutor::new().unwrap();
        let wasm = wat::parse_str(STREAMING_WAT).unwrap();
        let driver = executor.compile_and_validate_cached(&wasm).await.unwrap();

        // Register stream 1 (input) and stream 2 (output) in the executor's stream pool
        let ring_in = Arc::new(SpscRingBuffer::new(64, StreamBackpressurePolicy::DropOldest));
        ring_in.write(b"sensor_telemetry_stream_chunk_42").unwrap();

        let ring_out = Arc::new(SpscRingBuffer::new(64, StreamBackpressurePolicy::DropOldest));

        {
            let mut pool = executor.stream_pool().write().await;
            pool.register_stream(1, StreamTransport::SharedRingBuffer(ring_in));
            pool.register_stream(2, StreamTransport::SharedRingBuffer(ring_out.clone()));
        }

        let res = executor
            .execute_async(&driver, "stream_process", b"", ExecutionLimits::default())
            .await
            .unwrap();

        assert_eq!(res.output, b"sensor_telemetry_stream_chunk_42");
        assert_eq!(ring_out.read_all(), b"sensor_telemetry_stream_chunk_42");
    }

    #[tokio::test]
    async fn test_async_wasm_timeout() {
        let wasm = wat::parse_str(
            r#"
            (module
              (memory (export "memory") 1)
              (func (export "zap_alloc") (param i32) (result i32) i32.const 0)
              (func (export "zap_dealloc") (param i32 i32))
              (func (export "zap_execute") (param i32 i32 i32 i32) (result i64)
                (loop br 0)
                i64.const 0))
            "#,
        )
        .unwrap();

        let executor = AsyncWasmExecutor::new().unwrap();
        let driver = executor.compile_and_validate(&wasm).unwrap();

        let limits = ExecutionLimits {
            timeout_ms: 10,
            fuel: 1_000_000_000,
            ..ExecutionLimits::default()
        };

        let err = executor.execute_async(&driver, "hang", b"", limits).await.unwrap_err();
        assert!(matches!(
            err,
            ZapRuntimeError::Timeout { .. } | ZapRuntimeError::Wasmtime(_)
        ));
    }
}
}
