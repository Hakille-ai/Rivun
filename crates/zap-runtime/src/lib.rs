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
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant},
};
use thiserror::Error;
use wasmtime::{
    Config, Engine, ExternType, Instance, Linker, Module, Store, StoreLimits, StoreLimitsBuilder,
    ValType,
};

const MAX_EPOCH_TICK_MS: u64 = 10;
const MEMORY_EXPORT: &str = "memory";
const ALLOC_EXPORT: &str = "zap_alloc";
const DEALLOC_EXPORT: &str = "zap_dealloc";
const EXECUTE_EXPORT: &str = "zap_execute";

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
    #[error("execution exceeded wall-clock budget of {limit_ms} ms")]
    Timeout { limit_ms: u64 },
}

pub type Result<T> = std::result::Result<T, ZapRuntimeError>;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct DriverPermissions {
    pub network: bool,
    pub filesystem: bool,
    pub clock: bool,
    pub environment: bool,
}

impl DriverPermissions {
    pub const fn none() -> Self {
        Self {
            network: false,
            filesystem: false,
            clock: false,
            environment: false,
        }
    }

    fn validate(self) -> Result<()> {
        if self.network {
            return Err(ZapRuntimeError::PermissionDenied("network"));
        }
        if self.filesystem {
            return Err(ZapRuntimeError::PermissionDenied("filesystem"));
        }
        if self.clock {
            return Err(ZapRuntimeError::PermissionDenied("clock"));
        }
        if self.environment {
            return Err(ZapRuntimeError::PermissionDenied("environment"));
        }
        Ok(())
    }
}

impl Default for DriverPermissions {
    fn default() -> Self {
        Self::none()
    }
}

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
}

#[derive(Clone)]
pub struct WasmExecutor {
    engine: Engine,
}

impl WasmExecutor {
    pub fn new() -> Result<Self> {
        let mut config = Config::new();
        config.consume_fuel(true);
        config.epoch_interruption(true);
        config.wasm_backtrace_details(wasmtime::WasmBacktraceDetails::Enable);
        Ok(Self {
            engine: Engine::new(&config)?,
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
        limits.permissions.validate()?;
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
        };
        let mut store = Store::new(&self.engine, state);
        store.limiter(|state| &mut state.limits);
        store.set_fuel(limits.fuel)?;
        let epoch_tick_ms = epoch_tick_ms(limits.timeout_ms);
        configure_epoch_deadline(&mut store, deadline_ticks(limits.timeout_ms, epoch_tick_ms));

        let linker = Linker::new(&self.engine);
        let started = Instant::now();
        let epoch_guard = EpochTicker::start(self.engine.clone(), epoch_tick_ms);
        let instance = match linker.instantiate(&mut store, &driver.module) {
            Ok(instance) => instance,
            Err(_error) if wall_clock_exceeded(started, limits.timeout_ms) => {
                drop(epoch_guard);
                return Err(ZapRuntimeError::Timeout {
                    limit_ms: limits.timeout_ms,
                });
            }
            Err(error) => return Err(error.into()),
        };
        if wall_clock_exceeded(started, limits.timeout_ms) {
            drop(epoch_guard);
            return Err(ZapRuntimeError::Timeout {
                limit_ms: limits.timeout_ms,
            });
        }
        let output =
            match execute_instance(&mut store, &instance, action.as_bytes(), payload, limits) {
                Ok(output) => output,
                Err(_error) if wall_clock_exceeded(started, limits.timeout_ms) => {
                    drop(epoch_guard);
                    return Err(ZapRuntimeError::Timeout {
                        limit_ms: limits.timeout_ms,
                    });
                }
                Err(error) => return Err(error),
            };
        let elapsed_ms = started.elapsed().as_millis();
        drop(epoch_guard);
        if elapsed_ms > u128::from(limits.timeout_ms) {
            return Err(ZapRuntimeError::Timeout {
                limit_ms: limits.timeout_ms,
            });
        }

        let fuel_remaining = store.get_fuel()?;
        Ok(WasmExecutionResult {
            output,
            fuel_consumed: limits.fuel.saturating_sub(fuel_remaining),
            elapsed_ms,
        })
    }

    pub fn execute_bytes(
        &self,
        wasm: impl AsRef<[u8]>,
        action: &str,
        payload: &[u8],
        limits: ExecutionLimits,
    ) -> Result<WasmExecutionResult> {
        let driver = self.compile_and_validate(wasm)?;
        self.execute(&driver, action, payload, limits)
    }
}

impl Default for WasmExecutor {
    fn default() -> Self {
        Self::new().expect("WasmExecutor default configuration must be valid")
    }
}

#[derive(Clone)]
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

struct StoreState {
    limits: StoreLimits,
}

struct EpochTicker {
    stop: Arc<AtomicBool>,
    handle: Option<thread::JoinHandle<()>>,
}

impl EpochTicker {
    fn start(engine: Engine, tick_ms: u64) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let thread_stop = Arc::clone(&stop);
        let interval = Duration::from_millis(tick_ms.max(1));
        let handle = thread::spawn(move || {
            while !thread_stop.load(Ordering::Relaxed) {
                thread::sleep(interval);
                engine.increment_epoch();
            }
        });
        Self {
            stop,
            handle: Some(handle),
        }
    }
}

impl Drop for EpochTicker {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(handle) = self.handle.take() {
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

fn epoch_tick_ms(timeout_ms: u64) -> u64 {
    timeout_ms.clamp(1, MAX_EPOCH_TICK_MS)
}

fn deadline_ticks(timeout_ms: u64, tick_ms: u64) -> u64 {
    timeout_ms.max(1).div_ceil(tick_ms.max(1))
}

fn wall_clock_exceeded(started: Instant, timeout_ms: u64) -> bool {
    timeout_ms > 0 && started.elapsed() >= Duration::from_millis(timeout_ms)
}

fn expect_memory(module: &Module, export: &'static str) -> Result<()> {
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

fn expect_func(
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

fn ensure_i32_len(len: usize) -> Result<()> {
    if len > i32::MAX as usize {
        return Err(ZapRuntimeError::InputTooLarge(len));
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
