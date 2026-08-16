//! Sandboxed WebAssembly execution for ZAP action drivers.
//!
//! Driver ABI v1:
//! - export `memory`
//! - export `zap_alloc(len: i32) -> i32`
//! - export `zap_dealloc(ptr: i32, len: i32)`
//! - export `zap_execute(action_ptr, action_len, payload_ptr, payload_len) -> i64`
//!
//! `zap_execute` returns `(result_ptr << 32) | result_len`.

use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, VecDeque},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant},
};
use thiserror::Error;
use wasmtime::{
    Caller, Config, Engine, ExternType, Instance, Linker, Module, Store, StoreLimits,
    StoreLimitsBuilder, ValType,
};
pub use zap_capability::{DEFAULT_MAX_HOST_CALL_BYTES, DriverPermissions};

pub mod async_engine;
pub mod ipc;
pub mod pipeline;
pub mod streaming;

pub use async_engine::{
    AsyncCompiledDriver, AsyncStoreState, AsyncWasmExecutionResult, AsyncWasmExecutor,
    AsyncWasmModuleCache,
};
pub use ipc::{IpcPipe, IpcRouter, RuntimeIpcError};
pub use pipeline::{
    DriverPipeline, PipelineError, PipelineExecutionReport, PipelineStage, PipelineStageResult,
};
pub use streaming::{
    AsyncModbusConnection, SpscRingBuffer, StreamBackpressurePolicy, StreamTransport,
    StreamingBufferPool, StreamingError,
};

pub(crate) const SHARED_EPOCH_TICK_MS: u64 = 1;
pub(crate) const MEMORY_EXPORT: &str = "memory";
pub(crate) const ALLOC_EXPORT: &str = "zap_alloc";
pub(crate) const DEALLOC_EXPORT: &str = "zap_dealloc";
pub(crate) const EXECUTE_EXPORT: &str = "zap_execute";
pub(crate) const HOST_MODULE: &str = "zap";
pub(crate) const HOST_EMIT_EVENT: &str = "emit_event";
pub(crate) const HOST_MEMORY_READ: &str = "memory_read";
pub(crate) const HOST_MEMORY_WRITE: &str = "memory_write";
pub(crate) const HOST_DEVICE_CALL: &str = "device_call";
pub(crate) const HOST_DENIED: i32 = -1;
pub(crate) const HOST_NOT_CONFIGURED: i32 = -2;
pub(crate) const HOST_BAD_POINTER: i32 = -3;
pub(crate) const HOST_TOO_LARGE: i32 = -4;
pub(crate) const HOST_MEMORY_ERROR: i32 = -5;
pub(crate) const DEFAULT_WASM_MODULE_CACHE_ENTRIES: usize = 64;

#[derive(Debug, Error)]
pub enum ZapRuntimeError {
    #[error("Wasmtime error: {0}")]
    Wasmtime(#[from] wasmtime::Error),
    #[error("missing required export `{0}`")]
    MissingExport(&'static str),
    #[error("export `{export}` has invalid type: expected {expected}, got {actual}")]
    InvalidExportType {
        export: &'static str,
        expected: &'static str,
        actual: &'static str,
    },
    #[error("function export `{export}` has invalid signature: expected {expected}, got {actual}")]
    InvalidFunctionSignature {
        export: &'static str,
        expected: &'static str,
        actual: String,
    },
    #[error("WASM memory access failed: {0}")]
    MemoryAccess(String),
    #[error("invalid pointer returned by driver: ptr={ptr}, len={len}")]
    InvalidPointer { ptr: u32, len: u32 },
    #[error("driver output length {actual} exceeds limit {max}")]
    OutputTooLarge { max: usize, actual: usize },
    #[error("driver input length {0} exceeds i32::MAX")]
    InputTooLarge(usize),
    #[error("permission `{0}` is not granted to this driver")]
    PermissionDenied(&'static str),
    #[error("host call byte limit must be greater than zero")]
    InvalidHostCallLimit,
    #[error("execution exceeded wall-clock budget of {limit_ms} ms")]
    Timeout { limit_ms: u64 },
}

pub type Result<T> = std::result::Result<T, ZapRuntimeError>;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExecutionLimits {
    pub max_memory_bytes: usize,
    pub fuel: u64,
    pub timeout_ms: u64,
    pub max_output_bytes: usize,
    pub permissions: DriverPermissions,
}

impl Default for ExecutionLimits {
    fn default() -> Self {
        Self {
            max_memory_bytes: 16 * 1024 * 1024,
            fuel: 10_000_000,
            timeout_ms: 1_000,
            max_output_bytes: 1024 * 1024,
            permissions: DriverPermissions::none(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WasmExecutionResult {
    pub output: Vec<u8>,
    pub fuel_consumed: u64,
    pub elapsed_ms: u128,
    pub host_calls: Vec<HostCallRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostCallRecord {
    pub kind: HostCallKind,
    pub payload: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostCallKind {
    EmitEvent,
    MemoryWrite,
    DeviceCall,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WasmModuleCacheConfig {
    pub enabled: bool,
    pub max_entries: usize,
}

impl Default for WasmModuleCacheConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_entries: DEFAULT_WASM_MODULE_CACHE_ENTRIES,
        }
    }
}

#[derive(Clone)]
pub struct WasmExecutor {
    engine: Engine,
    module_cache: Arc<Mutex<WasmModuleCache>>,
    _epoch_ticker: Arc<EngineEpochTicker>,
}

impl WasmExecutor {
    pub fn new() -> Result<Self> {
        Self::with_module_cache(WasmModuleCacheConfig::default())
    }

    pub fn with_module_cache(cache_config: WasmModuleCacheConfig) -> Result<Self> {
        let mut config = Config::new();
        config.consume_fuel(true);
        config.epoch_interruption(true);
        config.wasm_backtrace_details(wasmtime::WasmBacktraceDetails::Enable);
        let engine = Engine::new(&config)?;
        Ok(Self {
            module_cache: Arc::new(Mutex::new(WasmModuleCache::new(cache_config))),
            _epoch_ticker: Arc::new(EngineEpochTicker::start(engine.clone())),
            engine,
        })
    }

    pub fn compile(&self, wasm: impl AsRef<[u8]>) -> Result<WasmDriver> {
        Ok(WasmDriver {
            module: Module::new(&self.engine, wasm.as_ref())?,
        })
    }

    pub fn compile_and_validate(&self, wasm: impl AsRef<[u8]>) -> Result<WasmDriver> {
        let driver = self.compile(wasm)?;
        driver.validate_abi()?;
        Ok(driver)
    }

    pub fn compile_and_validate_cached(&self, wasm: impl AsRef<[u8]>) -> Result<WasmDriver> {
        let wasm = wasm.as_ref();
        let key = wasm_module_cache_key(wasm);
        if let Some(driver) = self
            .module_cache
            .lock()
            .expect("WASM module cache mutex must not be poisoned")
            .get(&key)
        {
            return Ok(driver);
        }

        let driver = self.compile_and_validate(wasm)?;
        self.module_cache
            .lock()
            .expect("WASM module cache mutex must not be poisoned")
            .insert(key, driver.clone());
        Ok(driver)
    }

    pub fn validate_driver_abi(&self, driver: &WasmDriver) -> Result<()> {
        driver.validate_abi()
    }

    pub fn execute(
        &self,
        driver: &WasmDriver,
        action: &str,
        payload: &[u8],
        limits: ExecutionLimits,
    ) -> Result<WasmExecutionResult> {
        validate_permissions(limits.permissions)?;
        ensure_i32_len(action.len())?;
        ensure_i32_len(payload.len())?;
        driver.validate_abi()?;

        let state = StoreState {
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
        store.limiter(|state| &mut state.limits);
        store.set_fuel(limits.fuel)?;
        configure_epoch_deadline(
            &mut store,
            deadline_ticks(limits.timeout_ms, SHARED_EPOCH_TICK_MS),
        );

        let mut linker = Linker::new(&self.engine);
        define_host_imports(&mut linker)?;
        let started = Instant::now();
        let instance = match linker.instantiate(&mut store, &driver.module) {
            Ok(instance) => instance,
            Err(_error) if wall_clock_exceeded(started, limits.timeout_ms) => {
                return Err(ZapRuntimeError::Timeout {
                    limit_ms: limits.timeout_ms,
                });
            }
            Err(error) => return Err(error.into()),
        };
        if wall_clock_exceeded(started, limits.timeout_ms) {
            return Err(ZapRuntimeError::Timeout {
                limit_ms: limits.timeout_ms,
            });
        }
        let output =
            match execute_instance(&mut store, &instance, action.as_bytes(), payload, limits) {
                Ok(output) => output,
                Err(_error) if wall_clock_exceeded(started, limits.timeout_ms) => {
                    return Err(ZapRuntimeError::Timeout {
                        limit_ms: limits.timeout_ms,
                    });
                }
                Err(error) => return Err(error),
            };
        let elapsed_ms = started.elapsed().as_millis();
        if elapsed_ms > u128::from(limits.timeout_ms) {
            return Err(ZapRuntimeError::Timeout {
                limit_ms: limits.timeout_ms,
            });
        }

        let fuel_remaining = store.get_fuel()?;
        let host_calls = std::mem::take(&mut store.data_mut().host_calls);
        Ok(WasmExecutionResult {
            output,
            fuel_consumed: limits.fuel.saturating_sub(fuel_remaining),
            elapsed_ms,
            host_calls,
        })
    }

    pub fn execute_bytes(
        &self,
        wasm: impl AsRef<[u8]>,
        action: &str,
        payload: &[u8],
        limits: ExecutionLimits,
    ) -> Result<WasmExecutionResult> {
        let driver = self.compile_and_validate_cached(wasm)?;
        self.execute(&driver, action, payload, limits)
    }
}

impl Default for WasmExecutor {
    fn default() -> Self {
        Self::new().expect("WasmExecutor default configuration must be valid")
    }
}

#[derive(Clone, Debug)]
pub struct WasmDriver {
    module: Module,
}

impl WasmDriver {
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

type WasmModuleCacheKey = [u8; 32];

struct WasmModuleCache {
    config: WasmModuleCacheConfig,
    entries: HashMap<WasmModuleCacheKey, WasmDriver>,
    order: VecDeque<WasmModuleCacheKey>,
}

impl WasmModuleCache {
    fn new(config: WasmModuleCacheConfig) -> Self {
        Self {
            config,
            entries: HashMap::new(),
            order: VecDeque::new(),
        }
    }

    fn get(&self, key: &WasmModuleCacheKey) -> Option<WasmDriver> {
        if !self.config.enabled {
            return None;
        }
        self.entries.get(key).cloned()
    }

    fn insert(&mut self, key: WasmModuleCacheKey, driver: WasmDriver) {
        if !self.config.enabled || self.config.max_entries == 0 || self.entries.contains_key(&key) {
            return;
        }
        while self.entries.len() >= self.config.max_entries {
            let Some(evicted) = self.order.pop_front() else {
                break;
            };
            self.entries.remove(&evicted);
        }
        self.order.push_back(key);
        self.entries.insert(key, driver);
    }
}

fn wasm_module_cache_key(wasm: &[u8]) -> WasmModuleCacheKey {
    *blake3::hash(wasm).as_bytes()
}

struct StoreState {
    limits: StoreLimits,
    permissions: DriverPermissions,
    host_calls: Vec<HostCallRecord>,
}

struct EngineEpochTicker {
    stop: Arc<AtomicBool>,
    handle: Mutex<Option<thread::JoinHandle<()>>>,
}

impl EngineEpochTicker {
    fn start(engine: Engine) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let thread_stop = Arc::clone(&stop);
        let interval = Duration::from_millis(SHARED_EPOCH_TICK_MS);
        let handle = thread::spawn(move || {
            while !thread_stop.load(Ordering::Relaxed) {
                thread::park_timeout(interval);
                engine.increment_epoch();
            }
        });
        Self {
            stop,
            handle: Mutex::new(Some(handle)),
        }
    }
}

impl Drop for EngineEpochTicker {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        let handle = self
            .handle
            .get_mut()
            .expect("epoch ticker mutex must not be poisoned")
            .take();
        if let Some(handle) = handle {
            handle.thread().unpark();
            let _ = handle.join();
        }
    }
}

#[cfg(target_has_atomic = "64")]
fn configure_epoch_deadline(store: &mut Store<StoreState>, ticks: u64) {
    store.set_epoch_deadline(ticks.max(1));
    store.epoch_deadline_trap();
}

#[cfg(not(target_has_atomic = "64"))]
fn configure_epoch_deadline(_store: &mut Store<StoreState>, _ticks: u64) {}

fn deadline_ticks(timeout_ms: u64, tick_ms: u64) -> u64 {
    timeout_ms.max(1).div_ceil(tick_ms.max(1))
}

fn wall_clock_exceeded(started: Instant, timeout_ms: u64) -> bool {
    timeout_ms > 0 && started.elapsed() >= Duration::from_millis(timeout_ms)
}

pub(crate) fn expect_memory(module: &Module, export: &'static str) -> Result<()> {
    match module
        .get_export(export)
        .ok_or(ZapRuntimeError::MissingExport(export))?
    {
        ExternType::Memory(_) => Ok(()),
        actual => Err(ZapRuntimeError::InvalidExportType {
            export,
            expected: "memory",
            actual: export_kind(&actual),
        }),
    }
}

pub(crate) fn expect_func(
    module: &Module,
    export: &'static str,
    expected_params: &[ValType],
    expected_results: &[ValType],
    expected: &'static str,
) -> Result<()> {
    match module
        .get_export(export)
        .ok_or(ZapRuntimeError::MissingExport(export))?
    {
        ExternType::Func(func) => {
            let params = func.params().collect::<Vec<_>>();
            let results = func.results().collect::<Vec<_>>();
            if val_types_equal(&params, expected_params)
                && val_types_equal(&results, expected_results)
            {
                Ok(())
            } else {
                Err(ZapRuntimeError::InvalidFunctionSignature {
                    export,
                    expected,
                    actual: format_signature(&params, &results),
                })
            }
        }
        actual => Err(ZapRuntimeError::InvalidExportType {
            export,
            expected: "function",
            actual: export_kind(&actual),
        }),
    }
}

fn export_kind(export: &ExternType) -> &'static str {
    match export {
        ExternType::Func(_) => "function",
        ExternType::Global(_) => "global",
        ExternType::Table(_) => "table",
        ExternType::Memory(_) => "memory",
        ExternType::Tag(_) => "tag",
    }
}

fn val_types_equal(actual: &[ValType], expected: &[ValType]) -> bool {
    actual.len() == expected.len()
        && actual
            .iter()
            .zip(expected.iter())
            .all(|(actual, expected)| ValType::eq(actual, expected))
}

fn format_signature(params: &[ValType], results: &[ValType]) -> String {
    format!(
        "({}) -> {}",
        params
            .iter()
            .map(format_val_type)
            .collect::<Vec<_>>()
            .join(", "),
        match results {
            [] => "()".to_string(),
            [single] => format_val_type(single),
            many => format!(
                "({})",
                many.iter()
                    .map(format_val_type)
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        }
    )
}

fn format_val_type(value: &ValType) -> String {
    match value {
        ValType::I32 => "i32".to_string(),
        ValType::I64 => "i64".to_string(),
        ValType::F32 => "f32".to_string(),
        ValType::F64 => "f64".to_string(),
        ValType::V128 => "v128".to_string(),
        ValType::Ref(reference) => format!("{reference:?}"),
    }
}

fn define_host_imports(linker: &mut Linker<StoreState>) -> Result<()> {
    linker.func_wrap(
        HOST_MODULE,
        HOST_EMIT_EVENT,
        |mut caller: Caller<'_, StoreState>, ptr: i32, len: i32| -> i32 {
            host_capture_call(
                &mut caller,
                HostCallKind::EmitEvent,
                ptr,
                len,
                |permissions| permissions.emit_event,
            )
        },
    )?;
    linker.func_wrap(
        HOST_MODULE,
        HOST_MEMORY_WRITE,
        |mut caller: Caller<'_, StoreState>, ptr: i32, len: i32| -> i32 {
            host_capture_call(
                &mut caller,
                HostCallKind::MemoryWrite,
                ptr,
                len,
                |permissions| permissions.memory_write,
            )
        },
    )?;
    linker.func_wrap(
        HOST_MODULE,
        HOST_DEVICE_CALL,
        |mut caller: Caller<'_, StoreState>, ptr: i32, len: i32| -> i32 {
            if !caller.data().permissions.device_call {
                return HOST_DENIED;
            }
            match read_import_bytes(&mut caller, ptr, len) {
                Ok(payload) => {
                    caller.data_mut().host_calls.push(HostCallRecord {
                        kind: HostCallKind::DeviceCall,
                        payload,
                    });
                    HOST_NOT_CONFIGURED
                }
                Err(status) => status,
            }
        },
    )?;
    linker.func_wrap(
        HOST_MODULE,
        HOST_MEMORY_READ,
        |mut caller: Caller<'_, StoreState>,
         key_ptr: i32,
         key_len: i32,
         out_ptr: i32,
         out_len: i32|
         -> i32 {
            if !caller.data().permissions.memory_read {
                return HOST_DENIED;
            }
            if out_ptr < 0 || out_len < 0 {
                return HOST_BAD_POINTER;
            }
            match read_import_bytes(&mut caller, key_ptr, key_len) {
                Ok(_) => 0,
                Err(status) => status,
            }
        },
    )?;
    Ok(())
}

fn host_capture_call(
    caller: &mut Caller<'_, StoreState>,
    kind: HostCallKind,
    ptr: i32,
    len: i32,
    allowed: fn(DriverPermissions) -> bool,
) -> i32 {
    if !allowed(caller.data().permissions) {
        return HOST_DENIED;
    }
    match read_import_bytes(caller, ptr, len) {
        Ok(payload) => {
            caller
                .data_mut()
                .host_calls
                .push(HostCallRecord { kind, payload });
            0
        }
        Err(status) => status,
    }
}

fn read_import_bytes(
    caller: &mut Caller<'_, StoreState>,
    ptr: i32,
    len: i32,
) -> std::result::Result<Vec<u8>, i32> {
    if ptr < 0 || len < 0 {
        return Err(HOST_BAD_POINTER);
    }
    let max = caller.data().permissions.max_host_call_bytes as usize;
    let len = len as usize;
    if len > max {
        return Err(HOST_TOO_LARGE);
    }
    let memory = caller
        .get_export(MEMORY_EXPORT)
        .and_then(|export| export.into_memory())
        .ok_or(HOST_MEMORY_ERROR)?;
    let start = ptr as usize;
    let mut out = vec![0_u8; len];
    memory
        .read(caller, start, &mut out)
        .map_err(|_| HOST_MEMORY_ERROR)?;
    Ok(out)
}

fn execute_instance(
    store: &mut Store<StoreState>,
    instance: &Instance,
    action: &[u8],
    payload: &[u8],
    limits: ExecutionLimits,
) -> Result<Vec<u8>> {
    let memory = instance
        .get_memory(&mut *store, MEMORY_EXPORT)
        .ok_or(ZapRuntimeError::MissingExport(MEMORY_EXPORT))?;
    let alloc = instance
        .get_typed_func::<i32, i32>(&mut *store, ALLOC_EXPORT)
        .map_err(|_| ZapRuntimeError::MissingExport(ALLOC_EXPORT))?;
    let dealloc = instance
        .get_typed_func::<(i32, i32), ()>(&mut *store, DEALLOC_EXPORT)
        .map_err(|_| ZapRuntimeError::MissingExport(DEALLOC_EXPORT))?;
    let execute = instance
        .get_typed_func::<(i32, i32, i32, i32), i64>(&mut *store, EXECUTE_EXPORT)
        .map_err(|_| ZapRuntimeError::MissingExport(EXECUTE_EXPORT))?;

    let action_ptr = alloc.call(&mut *store, action.len() as i32)?;
    let payload_ptr = alloc.call(&mut *store, payload.len() as i32)?;
    write_memory(store, &memory, action_ptr, action)?;
    write_memory(store, &memory, payload_ptr, payload)?;

    let packed = execute.call(
        &mut *store,
        (
            action_ptr,
            action.len() as i32,
            payload_ptr,
            payload.len() as i32,
        ),
    )? as u64;

    dealloc.call(&mut *store, (action_ptr, action.len() as i32))?;
    dealloc.call(&mut *store, (payload_ptr, payload.len() as i32))?;

    let result_ptr = (packed >> 32) as u32;
    let result_len = (packed & 0xFFFF_FFFF) as u32;
    let result_len_usize = result_len as usize;
    if result_len_usize > limits.max_output_bytes {
        return Err(ZapRuntimeError::OutputTooLarge {
            max: limits.max_output_bytes,
            actual: result_len_usize,
        });
    }

    read_memory(store, &memory, result_ptr, result_len)
}

fn write_memory(
    store: &mut Store<StoreState>,
    memory: &wasmtime::Memory,
    ptr: i32,
    bytes: &[u8],
) -> Result<()> {
    if ptr < 0 {
        return Err(ZapRuntimeError::InvalidPointer {
            ptr: ptr as u32,
            len: bytes.len() as u32,
        });
    }
    memory
        .write(store, ptr as usize, bytes)
        .map_err(|err| ZapRuntimeError::MemoryAccess(err.to_string()))
}

fn read_memory(
    store: &mut Store<StoreState>,
    memory: &wasmtime::Memory,
    ptr: u32,
    len: u32,
) -> Result<Vec<u8>> {
    let start = ptr as usize;
    let end = start
        .checked_add(len as usize)
        .ok_or(ZapRuntimeError::InvalidPointer { ptr, len })?;
    if end > memory.data_size(&mut *store) {
        return Err(ZapRuntimeError::InvalidPointer { ptr, len });
    }

    let mut out = vec![0_u8; len as usize];
    memory
        .read(store, start, &mut out)
        .map_err(|err| ZapRuntimeError::MemoryAccess(err.to_string()))?;
    Ok(out)
}

pub(crate) fn ensure_i32_len(len: usize) -> Result<()> {
    if len > i32::MAX as usize {
        return Err(ZapRuntimeError::InputTooLarge(len));
    }
    Ok(())
}

pub(crate) fn validate_permissions(permissions: DriverPermissions) -> Result<()> {
    if permissions.max_host_call_bytes == 0 {
        return Err(ZapRuntimeError::InvalidHostCallLimit);
    }
    if permissions.network {
        return Err(ZapRuntimeError::PermissionDenied("network"));
    }
    if permissions.filesystem {
        return Err(ZapRuntimeError::PermissionDenied("filesystem"));
    }
    if permissions.clock {
        return Err(ZapRuntimeError::PermissionDenied("clock"));
    }
    if permissions.environment {
        return Err(ZapRuntimeError::PermissionDenied("environment"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

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

    fn echo_driver_with_heap_start(heap_start: i32) -> Vec<u8> {
        wat::parse_str(format!(
            r#"
            (module
              (memory (export "memory") 1)
              (global $heap (mut i32) (i32.const {heap_start}))
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
        ))
        .unwrap()
    }

    fn module_cache_len(executor: &WasmExecutor) -> usize {
        executor
            .module_cache
            .lock()
            .expect("WASM module cache mutex must not be poisoned")
            .entries
            .len()
    }

    #[test]
    fn executes_echo_driver() {
        let executor = WasmExecutor::new().unwrap();
        let result = executor
            .execute_bytes(
                echo_driver(),
                "echo",
                b"hello",
                ExecutionLimits {
                    fuel: 100_000,
                    ..ExecutionLimits::default()
                },
            )
            .unwrap();

        assert_eq!(result.output, b"hello");
        assert!(result.fuel_consumed > 0);
    }

    #[test]
    fn validates_echo_driver_abi() {
        let executor = WasmExecutor::new().unwrap();
        let driver = executor.compile_and_validate(echo_driver()).unwrap();

        executor.validate_driver_abi(&driver).unwrap();
    }

    #[test]
    fn cached_compile_reuses_same_module_for_same_wasm() {
        let wasm = echo_driver();
        let executor = WasmExecutor::new().unwrap();

        let first = executor.compile_and_validate_cached(&wasm).unwrap();
        let second = executor.compile_and_validate_cached(&wasm).unwrap();

        assert!(wasmtime::Module::same(&first.module, &second.module));
        assert_eq!(module_cache_len(&executor), 1);
    }

    #[test]
    fn invalid_abi_is_not_cached() {
        let wasm = wat::parse_str(
            r#"
            (module
              (memory (export "memory") 1)
              (func (export "zap_alloc") (param i32) (result i32) i32.const 0)
              (func (export "zap_dealloc") (param i32 i32)))
            "#,
        )
        .unwrap();
        let executor = WasmExecutor::new().unwrap();

        assert!(matches!(
            executor.compile_and_validate_cached(&wasm),
            Err(ZapRuntimeError::MissingExport("zap_execute"))
        ));
        assert_eq!(module_cache_len(&executor), 0);
    }

    #[test]
    fn module_cache_evicts_fifo_entries() {
        let first_wasm = echo_driver();
        let second_wasm = echo_driver_with_heap_start(2048);
        let executor = WasmExecutor::with_module_cache(WasmModuleCacheConfig {
            enabled: true,
            max_entries: 1,
        })
        .unwrap();

        let first = executor.compile_and_validate_cached(&first_wasm).unwrap();
        let first_cached = executor.compile_and_validate_cached(&first_wasm).unwrap();
        assert!(wasmtime::Module::same(&first.module, &first_cached.module));

        executor.compile_and_validate_cached(&second_wasm).unwrap();
        let first_after_eviction = executor.compile_and_validate_cached(&first_wasm).unwrap();

        assert!(!wasmtime::Module::same(
            &first.module,
            &first_after_eviction.module
        ));
        assert_eq!(module_cache_len(&executor), 1);
    }

    #[test]
    fn rejects_missing_execute_export() {
        let wasm = wat::parse_str(
            r#"
            (module
              (memory (export "memory") 1)
              (func (export "zap_alloc") (param i32) (result i32) i32.const 0)
              (func (export "zap_dealloc") (param i32 i32)))
            "#,
        )
        .unwrap();
        let executor = WasmExecutor::new().unwrap();
        let error = match executor.compile_and_validate(wasm) {
            Ok(_) => panic!("expected missing export error"),
            Err(error) => error,
        };

        assert!(matches!(
            error,
            ZapRuntimeError::MissingExport("zap_execute")
        ));
    }

    #[test]
    fn rejects_bad_execute_signature() {
        let wasm = wat::parse_str(
            r#"
            (module
              (memory (export "memory") 1)
              (func (export "zap_alloc") (param i32) (result i32) i32.const 0)
              (func (export "zap_dealloc") (param i32 i32))
              (func (export "zap_execute") (param i32 i32 i32 i32) (result i32) i32.const 0))
            "#,
        )
        .unwrap();
        let executor = WasmExecutor::new().unwrap();
        let error = match executor.compile_and_validate(wasm) {
            Ok(_) => panic!("expected invalid signature error"),
            Err(error) => error,
        };

        assert!(matches!(
            error,
            ZapRuntimeError::InvalidFunctionSignature {
                export: "zap_execute",
                ..
            }
        ));
        assert!(format!("{error}").contains("expected (i32, i32, i32, i32) -> i64"));
    }

    #[test]
    fn rejects_ungranted_permissions() {
        let executor = WasmExecutor::new().unwrap();
        let mut limits = ExecutionLimits::default();
        limits.permissions.network = true;

        assert!(matches!(
            executor.execute_bytes(echo_driver(), "echo", b"hello", limits),
            Err(ZapRuntimeError::PermissionDenied("network"))
        ));
    }

    #[test]
    fn host_emit_event_records_auditable_call_when_granted() {
        let wasm = wat::parse_str(
            r#"
            (module
              (import "zap" "emit_event" (func $emit_event (param i32 i32) (result i32)))
              (memory (export "memory") 1)
              (data (i32.const 1024) "machine-ready")
              (data (i32.const 2048) "ok")
              (func (export "zap_alloc") (param i32) (result i32) i32.const 3072)
              (func (export "zap_dealloc") (param i32 i32))
              (func (export "zap_execute") (param i32 i32 i32 i32) (result i64)
                i32.const 1024
                i32.const 13
                call $emit_event
                drop
                i64.const 2048
                i64.const 32
                i64.shl
                i64.const 2
                i64.or))
            "#,
        )
        .unwrap();
        let executor = WasmExecutor::new().unwrap();
        let mut permissions = DriverPermissions::none();
        permissions.emit_event = true;
        let result = executor
            .execute_bytes(
                wasm,
                "machine.status",
                b"",
                ExecutionLimits {
                    permissions,
                    ..ExecutionLimits::default()
                },
            )
            .unwrap();

        assert_eq!(result.output, b"ok");
        assert_eq!(
            result.host_calls,
            vec![HostCallRecord {
                kind: HostCallKind::EmitEvent,
                payload: b"machine-ready".to_vec()
            }]
        );
    }

    #[test]
    fn host_emit_event_denies_without_permission() {
        let wasm = wat::parse_str(
            r#"
            (module
              (import "zap" "emit_event" (func $emit_event (param i32 i32) (result i32)))
              (memory (export "memory") 1)
              (data (i32.const 1024) "machine-ready")
              (data (i32.const 2048) "denied")
              (func (export "zap_alloc") (param i32) (result i32) i32.const 3072)
              (func (export "zap_dealloc") (param i32 i32))
              (func (export "zap_execute") (param i32 i32 i32 i32) (result i64)
                i32.const 1024
                i32.const 13
                call $emit_event
                i32.const -1
                i32.eq
                if
                  i64.const 2048
                  i64.const 32
                  i64.shl
                  i64.const 6
                  i64.or
                  return
                end
                i64.const 0))
            "#,
        )
        .unwrap();
        let executor = WasmExecutor::new().unwrap();
        let result = executor
            .execute_bytes(wasm, "machine.status", b"", ExecutionLimits::default())
            .unwrap();

        assert_eq!(result.output, b"denied");
        assert!(result.host_calls.is_empty());
    }

    #[test]
    fn host_call_byte_limit_is_enforced() {
        let wasm = wat::parse_str(
            r#"
            (module
              (import "zap" "device_call" (func $device_call (param i32 i32) (result i32)))
              (memory (export "memory") 1)
              (data (i32.const 1024) "0123456789")
              (data (i32.const 2048) "too-large")
              (func (export "zap_alloc") (param i32) (result i32) i32.const 3072)
              (func (export "zap_dealloc") (param i32 i32))
              (func (export "zap_execute") (param i32 i32 i32 i32) (result i64)
                i32.const 1024
                i32.const 10
                call $device_call
                i32.const -4
                i32.eq
                if
                  i64.const 2048
                  i64.const 32
                  i64.shl
                  i64.const 9
                  i64.or
                  return
                end
                i64.const 0))
            "#,
        )
        .unwrap();
        let executor = WasmExecutor::new().unwrap();
        let mut permissions = DriverPermissions::none();
        permissions.device_call = true;
        permissions.max_host_call_bytes = 4;
        let result = executor
            .execute_bytes(
                wasm,
                "machine.call",
                b"",
                ExecutionLimits {
                    permissions,
                    ..ExecutionLimits::default()
                },
            )
            .unwrap();

        assert_eq!(result.output, b"too-large");
        assert!(result.host_calls.is_empty());
    }

    #[test]
    fn fuel_limits_infinite_loop() {
        let wasm = wat::parse_str(
            r#"
            (module
              (memory (export "memory") 1)
              (func (export "zap_alloc") (param i32) (result i32) i32.const 0)
              (func (export "zap_dealloc") (param i32 i32))
              (func (export "zap_execute")
                (param i32 i32 i32 i32)
                (result i64)
                (loop br 0)
                i64.const 0))
            "#,
        )
        .unwrap();
        let executor = WasmExecutor::new().unwrap();
        let result = executor.execute_bytes(
            wasm,
            "loop",
            b"",
            ExecutionLimits {
                fuel: 1_000,
                ..ExecutionLimits::default()
            },
        );

        assert!(result.is_err());
    }

    #[test]
    fn memory_limit_is_enforced() {
        let wasm = wat::parse_str(
            r#"
            (module
              (memory (export "memory") 2)
              (func (export "zap_alloc") (param i32) (result i32) i32.const 0)
              (func (export "zap_dealloc") (param i32 i32))
              (func (export "zap_execute") (param i32 i32 i32 i32) (result i64) i64.const 0))
            "#,
        )
        .unwrap();
        let executor = WasmExecutor::new().unwrap();
        let result = executor.execute_bytes(
            wasm,
            "x",
            b"",
            ExecutionLimits {
                max_memory_bytes: 64 * 1024,
                ..ExecutionLimits::default()
            },
        );

        assert!(result.is_err());
    }

    #[test]
    fn executes_wat_text_driver() {
        let wat = r#"
            (module
              (memory (export "memory") 1)
              (data (i32.const 1024) "wat-ok")
              (func (export "zap_alloc") (param i32) (result i32) i32.const 2048)
              (func (export "zap_dealloc") (param i32 i32))
              (func (export "zap_execute") (param i32 i32 i32 i32) (result i64)
                i64.const 1024
                i64.const 32
                i64.shl
                i64.const 6
                i64.or))
        "#;
        let executor = WasmExecutor::new().unwrap();
        let result = executor
            .execute_bytes(wat.as_bytes(), "wat", b"", ExecutionLimits::default())
            .unwrap();

        assert_eq!(result.output, b"wat-ok");
    }

    #[test]
    fn wall_clock_timeout_interrupts_long_running_driver() {
        let wasm = wat::parse_str(
            r#"
            (module
              (memory (export "memory") 1)
              (func (export "zap_alloc") (param i32) (result i32) i32.const 0)
              (func (export "zap_dealloc") (param i32 i32))
              (func (export "zap_execute")
                (param i32 i32 i32 i32)
                (result i64)
                (loop br 0)
                i64.const 0))
            "#,
        )
        .unwrap();
        let executor = WasmExecutor::new().unwrap();
        let started = Instant::now();
        let result = executor.execute_bytes(
            wasm,
            "loop",
            b"",
            ExecutionLimits {
                fuel: u64::MAX / 2,
                timeout_ms: 20,
                ..ExecutionLimits::default()
            },
        );

        assert!(matches!(result, Err(ZapRuntimeError::Timeout { .. })));
        assert!(started.elapsed() < Duration::from_secs(2));
    }

    #[test]
    fn wall_clock_timeout_interrupts_start_function() {
        let wasm = wat::parse_str(
            r#"
            (module
              (memory (export "memory") 1)
              (func $start
                (loop br 0))
              (start $start)
              (func (export "zap_alloc") (param i32) (result i32) i32.const 0)
              (func (export "zap_dealloc") (param i32 i32))
              (func (export "zap_execute")
                (param i32 i32 i32 i32)
                (result i64)
                i64.const 0))
            "#,
        )
        .unwrap();
        let executor = WasmExecutor::new().unwrap();
        let started = Instant::now();
        let result = executor.execute_bytes(
            wasm,
            "start-loop",
            b"",
            ExecutionLimits {
                fuel: u64::MAX / 2,
                timeout_ms: 20,
                ..ExecutionLimits::default()
            },
        );

        assert!(matches!(result, Err(ZapRuntimeError::Timeout { .. })));
        assert!(started.elapsed() < Duration::from_secs(2));
    }
}
