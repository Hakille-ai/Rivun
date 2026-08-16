//! Asynchronous driver traits, lifecycle context, and streaming I/O abstractions.

use crate::{
    buffer::{BufferSlice, PinnedBuffer},
    error::DriverError,
    ipc::IpcMessage,
    DriverInput, ZapDriver,
};
use std::{collections::HashMap, future::Future, pin::Pin};
use zap_capability::DriverPermissions;

/// Boxed send future for async trait methods without lifetime issues.
pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// Execution context and environment provided to async drivers.
#[derive(Debug, Clone)]
pub struct DriverContext {
    pub driver_id: String,
    pub stage_index: usize,
    pub permissions: DriverPermissions,
    pub fuel_limit: u64,
    pub fuel_consumed: u64,
    pub endpoints: HashMap<String, u32>,
    pub metadata: HashMap<String, String>,
}

impl DriverContext {
    /// Create a new driver context with permissions and defaults.
    pub fn new(
        driver_id: impl Into<String>,
        stage_index: usize,
        permissions: DriverPermissions,
    ) -> Self {
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

    /// Builder method to override default fuel limit.
    pub fn with_fuel_limit(mut self, limit: u64) -> Self {
        self.fuel_limit = limit;
        self
    }

    /// Calculate the remaining fuel balance.
    pub fn remaining_fuel(&self) -> u64 {
        self.fuel_limit.saturating_sub(self.fuel_consumed)
    }

    /// Explicitly consume a specified amount of fuel.
    pub fn consume_fuel(&mut self, amount: u64) -> Result<(), DriverError> {
        let new_total = self.fuel_consumed.saturating_add(amount);
        if new_total > self.fuel_limit {
            return Err(DriverError::fuel_limit_exceeded(new_total, self.fuel_limit));
        }
        self.fuel_consumed = new_total;
        Ok(())
    }

    /// Register a channel endpoint ID by name.
    pub fn register_endpoint(&mut self, name: impl Into<String>, channel_id: u32) {
        self.endpoints.insert(name.into(), channel_id);
    }

    /// Lookup a channel endpoint ID by name.
    pub fn get_endpoint(&self, name: &str) -> Option<u32> {
        self.endpoints.get(name).copied()
    }
}

/// Asynchronous stream reader trait for streaming I/O.
pub trait AsyncStreamReader: Send + Sync {
    /// Read up to `buf.len()` bytes into `buf`. Returns the number of bytes read (0 indicates EOF).
    fn read<'a>(&'a mut self, buf: &'a mut [u8]) -> BoxFuture<'a, Result<usize, DriverError>>;

    /// Read exact number of bytes to fill `buf`.
    fn read_exact<'a>(&'a mut self, buf: &'a mut [u8]) -> BoxFuture<'a, Result<(), DriverError>> {
        Box::pin(async move {
            let mut offset = 0;
            while offset < buf.len() {
                let n = self.read(&mut buf[offset..]).await?;
                if n == 0 {
                    return Err(DriverError::new("unexpected end of stream"));
                }
                offset += n;
            }
            Ok(())
        })
    }
}

/// Asynchronous stream writer trait for streaming I/O.
pub trait AsyncStreamWriter: Send + Sync {
    /// Write up to `buf.len()` bytes from `buf`. Returns the number of bytes written.
    fn write<'a>(&'a mut self, buf: &'a [u8]) -> BoxFuture<'a, Result<usize, DriverError>>;

    /// Write entire `buf` to stream.
    fn write_all<'a>(&'a mut self, buf: &'a [u8]) -> BoxFuture<'a, Result<(), DriverError>> {
        Box::pin(async move {
            let mut offset = 0;
            while offset < buf.len() {
                let n = self.write(&buf[offset..]).await?;
                if n == 0 {
                    return Err(DriverError::new("failed to write complete buffer"));
                }
                offset += n;
            }
            Ok(())
        })
    }

    /// Flush any pending buffered data.
    fn flush<'a>(&'a mut self) -> BoxFuture<'a, Result<(), DriverError>> {
        Box::pin(async move { Ok(()) })
    }
}

/// In-memory asynchronous stream reader wrapping a byte buffer.
#[derive(Debug, Clone)]
pub struct MemoryStreamReader {
    data: Vec<u8>,
    position: usize,
}

impl MemoryStreamReader {
    pub fn new(data: impl Into<Vec<u8>>) -> Self {
        Self {
            data: data.into(),
            position: 0,
        }
    }

    pub fn position(&self) -> usize {
        self.position
    }

    pub fn remaining(&self) -> usize {
        self.data.len().saturating_sub(self.position)
    }
}

impl AsyncStreamReader for MemoryStreamReader {
    fn read<'a>(&'a mut self, buf: &'a mut [u8]) -> BoxFuture<'a, Result<usize, DriverError>> {
        Box::pin(async move {
            if self.position >= self.data.len() {
                return Ok(0);
            }
            let to_read = buf.len().min(self.data.len() - self.position);
            buf[..to_read].copy_from_slice(&self.data[self.position..self.position + to_read]);
            self.position += to_read;
            Ok(to_read)
        })
    }
}

/// In-memory asynchronous stream writer capturing output bytes.
#[derive(Debug, Clone, Default)]
pub struct MemoryStreamWriter {
    data: Vec<u8>,
}

impl MemoryStreamWriter {
    pub fn new() -> Self {
        Self { data: Vec::new() }
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            data: Vec::with_capacity(capacity),
        }
    }

    pub fn into_inner(self) -> Vec<u8> {
        self.data
    }

    pub fn as_slice(&self) -> &[u8] {
        &self.data
    }
}

impl AsyncStreamWriter for MemoryStreamWriter {
    fn write<'a>(&'a mut self, buf: &'a [u8]) -> BoxFuture<'a, Result<usize, DriverError>> {
        Box::pin(async move {
            self.data.extend_from_slice(buf);
            Ok(buf.len())
        })
    }

    fn flush<'a>(&'a mut self) -> BoxFuture<'a, Result<(), DriverError>> {
        Box::pin(async move { Ok(()) })
    }
}

/// Asynchronous WASM driver contract for ZAP pipelines.
///
/// Enables non-blocking streaming I/O, asynchronous event handling,
/// zero-copy memory processing, and inter-driver IPC.
pub trait AsyncZapDriver: Send + Sync + 'static {
    /// Initialize driver state, allocate pinned buffers, and register IPC endpoints.
    fn init<'a>(
        &'a mut self,
        _ctx: &'a mut DriverContext,
    ) -> BoxFuture<'a, Result<(), DriverError>> {
        Box::pin(async move { Ok(()) })
    }

    /// Asynchronously execute a single driver action on the provided input.
    fn execute_async<'a>(
        &'a mut self,
        ctx: &'a mut DriverContext,
        input: DriverInput<'a>,
    ) -> BoxFuture<'a, Result<Vec<u8>, DriverError>>;

    /// Process a continuous streaming data chunk with zero-copy input and output buffers.
    fn process_stream<'a>(
        &'a mut self,
        _ctx: &'a mut DriverContext,
        input: &'a BufferSlice<'_>,
        output: &'a mut PinnedBuffer,
    ) -> BoxFuture<'a, Result<usize, DriverError>> {
        let bytes = input.as_bytes();
        Box::pin(async move {
            output
                .write(bytes)
                .map_err(|e| DriverError::new(e.to_string()))
        })
    }

    /// Handle an asynchronous discrete event or IPC message from an upstream stage or host.
    fn handle_event<'a>(
        &'a mut self,
        _ctx: &'a mut DriverContext,
        _event: &'a IpcMessage,
    ) -> BoxFuture<'a, Result<Option<IpcMessage>, DriverError>> {
        Box::pin(async move { Ok(None) })
    }

    /// Gracefully shut down the driver, releasing resources and flushing state.
    fn shutdown<'a>(
        &'a mut self,
        _ctx: &'a mut DriverContext,
    ) -> BoxFuture<'a, Result<(), DriverError>> {
        Box::pin(async move { Ok(()) })
    }
}

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
    fn init<'a>(
        &'a mut self,
        _ctx: &'a mut DriverContext,
    ) -> BoxFuture<'a, Result<(), DriverError>> {
        Box::pin(async move { Ok(()) })
    }

    fn execute_async<'a>(
        &'a mut self,
        _ctx: &'a mut DriverContext,
        input: DriverInput<'a>,
    ) -> BoxFuture<'a, Result<Vec<u8>, DriverError>> {
        let res = self.inner.execute(input);
        Box::pin(async move { res })
    }

    fn process_stream<'a>(
        &'a mut self,
        _ctx: &'a mut DriverContext,
        input: &'a BufferSlice<'_>,
        output: &'a mut PinnedBuffer,
    ) -> BoxFuture<'a, Result<usize, DriverError>> {
        let input_bytes = input.to_vec();
        let res = self.inner.execute(DriverInput {
            action: "process_stream",
            payload: &input_bytes,
        });
        Box::pin(async move {
            let result_bytes = res?;
            output
                .write(&result_bytes)
                .map_err(|e| DriverError::new(e.to_string()))
        })
    }

    fn handle_event<'a>(
        &'a mut self,
        _ctx: &'a mut DriverContext,
        event: &'a IpcMessage,
    ) -> BoxFuture<'a, Result<Option<IpcMessage>, DriverError>> {
        let action = format!("channel_{}", event.channel_id);
        let payload = event.payload.clone();
        let channel_id = event.channel_id;
        let sequence = event.sequence;
        let res = self.inner.execute(DriverInput {
            action: &action,
            payload: &payload,
        });
        Box::pin(async move {
            let result_bytes = res?;
            Ok(Some(IpcMessage::new(
                channel_id,
                sequence + 1,
                0,
                0,
                result_bytes,
            )))
        })
    }

    fn shutdown<'a>(
        &'a mut self,
        _ctx: &'a mut DriverContext,
    ) -> BoxFuture<'a, Result<(), DriverError>> {
        Box::pin(async move { Ok(()) })
    }
}

/// Extension trait to convert any `ZapDriver` directly into an `AsyncZapDriver`.
pub trait ZapDriverExt: ZapDriver + Sized {
    fn into_async(self) -> SyncDriverAdapter<Self> {
        SyncDriverAdapter::new(self)
    }
}

impl<D: ZapDriver> ZapDriverExt for D {}

#[cfg(test)]
mod tests {
    use super::*;

    struct AsyncEchoDriver;

    impl AsyncZapDriver for AsyncEchoDriver {
        fn execute_async<'a>(
            &'a mut self,
            _ctx: &'a mut DriverContext,
            input: DriverInput<'a>,
        ) -> BoxFuture<'a, Result<Vec<u8>, DriverError>> {
            Box::pin(async move {
                if input.action == "echo" {
                    Ok(input.payload.to_vec())
                } else {
                    Err(DriverError::new("unsupported action"))
                }
            })
        }
    }

    struct SyncEchoDriver;

    impl ZapDriver for SyncEchoDriver {
        fn execute(&self, input: DriverInput<'_>) -> Result<Vec<u8>, DriverError> {
            if input.action == "echo"
                || input.action == "process_stream"
                || input.action.starts_with("channel_")
            {
                Ok(input.payload.to_vec())
            } else {
                Err(DriverError::new("unknown sync action"))
            }
        }
    }

    #[tokio::test]
    async fn test_async_echo_driver_execute() {
        let mut driver = AsyncEchoDriver;
        let mut ctx = DriverContext::new("test_driver", 0, DriverPermissions::none());
        let input = DriverInput {
            action: "echo",
            payload: b"async_payload_test",
        };
        let res = driver.execute_async(&mut ctx, input).await.unwrap();
        assert_eq!(res, b"async_payload_test");
    }

    #[tokio::test]
    async fn test_sync_driver_adapter_lifecycle() {
        let mut adapter = SyncEchoDriver.into_async();
        let mut ctx = DriverContext::new("sync_adapted", 0, DriverPermissions::none());

        adapter.init(&mut ctx).await.unwrap();

        let input = DriverInput {
            action: "echo",
            payload: b"sync_adapted_payload",
        };
        let res = adapter.execute_async(&mut ctx, input).await.unwrap();
        assert_eq!(res, b"sync_adapted_payload");

        let slice = BufferSlice::new(b"zero_copy_chunk");
        let mut out_buf = PinnedBuffer::with_capacity(64);
        let written = adapter.process_stream(&mut ctx, &slice, &mut out_buf).await.unwrap();
        assert_eq!(written, 15);
        assert_eq!(out_buf.as_slice(), b"zero_copy_chunk");

        let event = IpcMessage::new(1, 10, 100, 0, b"ping_data".to_vec());
        let resp = adapter.handle_event(&mut ctx, &event).await.unwrap().unwrap();
        assert_eq!(resp.channel_id, 1);
        assert_eq!(resp.sequence, 11);
        assert_eq!(resp.payload, b"ping_data");

        adapter.shutdown(&mut ctx).await.unwrap();
    }

    #[tokio::test]
    async fn test_driver_context_fuel_accounting() {
        let mut ctx = DriverContext::new("test_ctx", 1, DriverPermissions::none())
            .with_fuel_limit(1000);

        assert_eq!(ctx.remaining_fuel(), 1000);
        ctx.consume_fuel(300).unwrap();
        assert_eq!(ctx.fuel_consumed, 300);
        assert_eq!(ctx.remaining_fuel(), 700);

        let err = ctx.consume_fuel(800).unwrap_err();
        assert!(err.message().contains("fuel limit exceeded"));
    }
}
