# Handoff Report: Milestone 3 Exploration (Async WASM Driver Pipeline & Inter-Driver IPC)

## 1. Observation
1. **Cargo Workspace & Dependencies**:
   - `Cargo.toml`: Root defines 23 member crates and workspace dependencies including `wasmtime = { version = "45.0.1", default-features = false, features = ["cranelift", "runtime", "std", "wat"] }`, `tokio = { version = "1", features = ["macros", "net", "rt-multi-thread", "sync", "time"] }`, `bytes = "1"`, `blake3 = "1"`, `thiserror = "2"`, `serde = { version = "1", features = ["derive"] }`.
   - `crates/zap-runtime/Cargo.toml` lines 9-15:
     ```toml
     [dependencies]
     blake3.workspace = true
     serde.workspace = true
     thiserror.workspace = true
     wasmtime.workspace = true
     zap-capability.workspace = true
     ```
     Lacks `tokio.workspace = true`, `bytes.workspace = true`, and path dependency to `zap-driver-sdk`.
   - `crates/zap-driver-sdk/Cargo.toml` lines 9-10 has empty `[dependencies]`.

2. **Existing Implementation**:
   - `crates/zap-runtime/src/lib.rs`: Implements `WasmExecutor`, `WasmDriver`, `ExecutionLimits`, `WasmExecutionResult`, `HostCallRecord`, `HostCallKind`, `WasmModuleCache`, and `EngineEpochTicker`. Uses Wasmtime with `consume_fuel(true)` and `epoch_interruption(true)`.
   - `crates/zap-runtime/src/pipeline.rs` lines 6-10:
     ```rust
     use crate::{DriverPermissions, WasmActionRuntime, ZapRuntimeError};
     ```
     `cargo check --workspace --all-targets` failed with error:
     ```
     error[E0432]: unresolved import `crate::WasmActionRuntime`
      --> crates\zap-runtime\src\pipeline.rs:6:32
       |
     6 | use crate::{DriverPermissions, WasmActionRuntime, ZapRuntimeError};
       |                                ^^^^^^^^^^^^^^^^^ no `WasmActionRuntime` in the root
     ```
   - `crates/zap-driver-sdk/src/lib.rs`: Implements `ZapDriver` synchronous trait, `PackedResult`, `pack_result`, `unpack_result`, and ABI constants (`DRIVER_ABI_VERSION = 1`).
   - `crates/zap-node/src/lib.rs` line 57 & lines 2823-2848: Uses `WasmExecutor` to execute action messages synchronously via `self.runtime.execute(&driver.driver, action, &message.body, limits)`.

3. **M1 and M2 Crates**:
   - `crates/zap-net/src/gossip.rs`: Implements `GossipMesh`, `VectorClock`, `QuorumProposal` (T-of-N voting), health evaluation, and failover capability routing.
   - `crates/zap-ledger/src/mmr.rs`: Implements `MerkleMountainRange` ($O(\log N)$ append and peak bagging), `MmrInclusionProof`, and `MmrRollupCommitment`.
   - Both crates compile and test with zero errors.

## 2. Logic Chain
1. **Observation 1 & 2** show that `zap-runtime` already has a solid sandboxed execution foundation based on Wasmtime (`WasmExecutor`, `WasmDriver`, epoch ticker, fuel consumption, memory bounds, and host call auditing), but currently only exposes a synchronous host execution model and has a broken import in `pipeline.rs`.
2. **PROJECT.md (§7, §8, §9)** and **SCOPE.md** require:
   - Non-blocking async WASM driver host execution on Tokio tasks (`AsyncWasmExecutor`).
   - Streaming I/O buffers (`SpscRingBuffer`, `StreamingBufferPool`, `TcpStreamBuffer`, `ModbusStreamBuffer`) with backpressure.
   - Inter-driver IPC (`InterDriverIpcPipe`) with zero-copy data passing and memory sandboxing.
   - Multi-driver pipeline chaining (`DriverPipeline`) with aggregate fuel budgeting, per-stage latency tracking, and Blake3 causal chain hashing.
   - Async driver trait (`AsyncZapDriver`) and zero-copy pinned views (`PinnedBuffer`, `BufferSlice`) in `zap-driver-sdk`.
3. To achieve this without breaking M1 and M2 (or `zap-node`), `WasmExecutor`'s synchronous methods must remain unchanged, while `AsyncWasmExecutor`, `SpscRingBuffer`, `InterDriverIpcPipe`, and `DriverPipeline` are added or updated in `zap-runtime`, and `AsyncZapDriver` and `PinnedBuffer` are added to `zap-driver-sdk`.
4. Adding `tokio`, `bytes`, and `zap-driver-sdk` to `zap-runtime/Cargo.toml` and `bytes`, `thiserror`, `serde` to `zap-driver-sdk/Cargo.toml` resolves all dependency requirements.

## 3. Caveats
- No caveats. The requirements and boundaries are clear.

## 4. Conclusion
Milestone 3 is ready for implementation across 4 sub-milestones:
- **M3.1**: Add `AsyncZapDriver`, `PinnedBuffer`, `BufferSlice`, and IPC primitives in `zap-driver-sdk`.
- **M3.2**: Add `AsyncWasmExecutor` and `FuelMeter` in `zap-runtime/src/async_engine.rs`.
- **M3.3**: Add `SpscRingBuffer`, `StreamingBufferPool`, `TcpStreamBuffer`, `ModbusStreamBuffer`, and `StreamingSession` in `zap-runtime/src/streaming.rs`.
- **M3.4**: Add `InterDriverIpcPipe` in `zap-runtime/src/ipc.rs` and update `DriverPipeline` in `zap-runtime/src/pipeline.rs` with aggregate fuel enforcement and Blake3 causal hashing. Fix all imports and update `Cargo.toml` files.

Full design specifications and code schemas are documented in `c:\Users\Stagiaire\Documents\Amadou PGC\Prs\ZAP\.agents\sub_orch_m3_explorer_1\analysis.md`.

## 5. Verification Method
1. Inspect files:
   - `crates/zap-driver-sdk/src/lib.rs`
   - `crates/zap-runtime/src/lib.rs`
   - `crates/zap-runtime/src/async_engine.rs`
   - `crates/zap-runtime/src/streaming.rs`
   - `crates/zap-runtime/src/ipc.rs`
   - `crates/zap-runtime/src/pipeline.rs`
2. Test commands:
   - `cargo check --workspace --all-targets`
   - `cargo test -p zap-driver-sdk -p zap-runtime`
   - `cargo test --workspace --all-targets`
   - `cargo clippy --workspace --all-targets -- -D warnings`
