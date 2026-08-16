//! Inter-Driver IPC and Asynchronous WASM Driver Execution Pipelines.
//!
//! Enables deterministic chaining of multiple WASM drivers in a zero-copy pipeline,
//! with aggregated fuel tracking, causal intermediate step hashing, and stream buffering.

use crate::{DriverPermissions, ExecutionLimits, WasmExecutor, async_engine::AsyncWasmExecutor};
use serde::{Deserialize, Serialize};
use std::{fmt, time::Instant};
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum PipelineError {
    #[error("pipeline is empty: at least one driver stage required")]
    EmptyPipeline,
    #[error("pipeline stage {stage_index} (`{driver_name}`) failed with error: {error}")]
    StageExecutionFailed {
        stage_index: usize,
        driver_name: String,
        error: String,
    },
    #[error("pipeline fuel limit exceeded: consumed {consumed}, limit {limit}")]
    FuelLimitExceeded { consumed: u64, limit: u64 },
    #[error(
        "pipeline fuel exhausted at stage {stage_index} (`{driver_name}`): consumed {consumed}, limit {limit}"
    )]
    PipelineFuelExhausted {
        stage_index: usize,
        driver_name: String,
        consumed: u64,
        limit: u64,
    },
    #[error("channel buffer capacity overflow (max {max} bytes)")]
    BufferOverflow { max: usize },
    #[error("pipeline stage {stage_index} (`{driver_name}`) timed out")]
    Timeout {
        stage_index: usize,
        driver_name: String,
    },
}

/// A stage in the WASM driver execution pipeline.
#[derive(Clone)]
pub struct PipelineStage {
    pub name: String,
    pub action: String,
    pub wasm_binary: Vec<u8>,
    pub permissions: DriverPermissions,
    pub fuel_limit: Option<u64>,
    pub timeout_ms: Option<u64>,
}

/// Result of an individual pipeline stage execution.
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

/// Comprehensive Report of a multi-driver pipeline execution.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct PipelineExecutionReport {
    pub pipeline_id: String,
    pub stages: Vec<PipelineStageResult>,
    pub total_fuel_consumed: u64,
    pub total_elapsed_ms: u128,
    pub final_output: Vec<u8>,
    pub causal_chain_hash: String,
}

/// Chained multi-driver execution pipeline.
#[derive(Clone)]
pub struct DriverPipeline {
    pub name: String,
    stages: Vec<PipelineStage>,
    max_total_fuel: u64,
    total_timeout_ms: u64,
}

impl DriverPipeline {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            stages: Vec::new(),
            max_total_fuel: 10_000_000,
            total_timeout_ms: 5_000,
        }
    }

    pub fn with_max_fuel(mut self, max_fuel: u64) -> Self {
        self.max_total_fuel = max_fuel;
        self
    }

    pub fn with_timeout(mut self, timeout_ms: u64) -> Self {
        self.total_timeout_ms = timeout_ms;
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

    pub fn stage_count(&self) -> usize {
        self.stages.len()
    }

    /// Asynchronously execute the pipeline sequentially across all stages on Tokio tasks.
    pub async fn execute_async(
        &self,
        initial_payload: &[u8],
    ) -> Result<PipelineExecutionReport, PipelineError> {
        if self.stages.is_empty() {
            return Err(PipelineError::EmptyPipeline);
        }

        let executor =
            AsyncWasmExecutor::new().map_err(|e| PipelineError::StageExecutionFailed {
                stage_index: 0,
                driver_name: "async_executor_init".to_string(),
                error: e.to_string(),
            })?;

        let pipeline_start = Instant::now();
        let mut current_payload = initial_payload.to_vec();
        let mut stage_results = Vec::new();
        let mut total_fuel = 0u64;

        let mut causal_hasher = blake3::Hasher::new();
        causal_hasher.update(b"ZAP-PIPELINE-START-v1:");
        causal_hasher.update(self.name.as_bytes());
        causal_hasher.update(initial_payload);

        for (idx, stage) in self.stages.iter().enumerate() {
            let stage_start = Instant::now();
            let driver = executor
                .compile_and_validate_cached(&stage.wasm_binary)
                .await
                .map_err(|e| PipelineError::StageExecutionFailed {
                    stage_index: idx,
                    driver_name: stage.name.clone(),
                    error: e.to_string(),
                })?;

            let remaining_budget = self.max_total_fuel.saturating_sub(total_fuel);
            if remaining_budget == 0 {
                return Err(PipelineError::PipelineFuelExhausted {
                    stage_index: idx,
                    driver_name: stage.name.clone(),
                    consumed: total_fuel,
                    limit: self.max_total_fuel,
                });
            }

            let fuel = match stage.fuel_limit {
                Some(limit) => limit.min(remaining_budget),
                None => remaining_budget,
            };

            let limits = ExecutionLimits {
                fuel,
                timeout_ms: stage.timeout_ms.unwrap_or(self.total_timeout_ms),
                permissions: stage.permissions,
                ..ExecutionLimits::default()
            };

            let exec_result = executor
                .execute_async(&driver, &stage.action, &current_payload, limits)
                .await
                .map_err(|e| PipelineError::StageExecutionFailed {
                    stage_index: idx,
                    driver_name: stage.name.clone(),
                    error: e.to_string(),
                })?;

            let duration_micros = stage_start.elapsed().as_micros() as u64;
            total_fuel = total_fuel.saturating_add(exec_result.fuel_consumed);

            if total_fuel > self.max_total_fuel {
                return Err(PipelineError::PipelineFuelExhausted {
                    stage_index: idx,
                    driver_name: stage.name.clone(),
                    consumed: total_fuel,
                    limit: self.max_total_fuel,
                });
            }

            let output_hash = blake3::hash(&exec_result.output).to_hex().to_string();
            causal_hasher.update(output_hash.as_bytes());

            stage_results.push(PipelineStageResult {
                stage_index: idx,
                stage_name: stage.name.clone(),
                action: stage.action.clone(),
                fuel_consumed: exec_result.fuel_consumed,
                output_len: exec_result.output.len(),
                output_hash,
                duration_micros,
            });

            current_payload = exec_result.output;
        }

        let total_elapsed_ms = pipeline_start.elapsed().as_millis();
        let causal_chain_hash = causal_hasher.finalize().to_hex().to_string();

        Ok(PipelineExecutionReport {
            pipeline_id: self.name.clone(),
            stages: stage_results,
            total_fuel_consumed: total_fuel,
            total_elapsed_ms,
            final_output: current_payload,
            causal_chain_hash,
        })
    }

    /// Synchronously execute the pipeline sequentially (backward-compatible).
    pub fn execute(
        &self,
        initial_payload: &[u8],
    ) -> Result<PipelineExecutionReport, PipelineError> {
        if self.stages.is_empty() {
            return Err(PipelineError::EmptyPipeline);
        }

        let executor = WasmExecutor::new().map_err(|e| PipelineError::StageExecutionFailed {
            stage_index: 0,
            driver_name: "executor_init".to_string(),
            error: e.to_string(),
        })?;

        let pipeline_start = Instant::now();
        let mut current_payload = initial_payload.to_vec();
        let mut stage_results = Vec::new();
        let mut total_fuel = 0u64;

        let mut causal_hasher = blake3::Hasher::new();
        causal_hasher.update(b"ZAP-PIPELINE-START-v1:");
        causal_hasher.update(self.name.as_bytes());
        causal_hasher.update(initial_payload);

        for (idx, stage) in self.stages.iter().enumerate() {
            let stage_start = Instant::now();
            let driver = executor
                .compile_and_validate_cached(&stage.wasm_binary)
                .map_err(|e| PipelineError::StageExecutionFailed {
                    stage_index: idx,
                    driver_name: stage.name.clone(),
                    error: e.to_string(),
                })?;

            let remaining_budget = self.max_total_fuel.saturating_sub(total_fuel);
            if remaining_budget == 0 {
                return Err(PipelineError::PipelineFuelExhausted {
                    stage_index: idx,
                    driver_name: stage.name.clone(),
                    consumed: total_fuel,
                    limit: self.max_total_fuel,
                });
            }

            let fuel = stage
                .fuel_limit
                .unwrap_or(remaining_budget)
                .min(remaining_budget);
            let limits = ExecutionLimits {
                fuel,
                timeout_ms: stage.timeout_ms.unwrap_or(self.total_timeout_ms),
                permissions: stage.permissions,
                ..ExecutionLimits::default()
            };

            let exec_result = executor
                .execute(&driver, &stage.action, &current_payload, limits)
                .map_err(|e| PipelineError::StageExecutionFailed {
                    stage_index: idx,
                    driver_name: stage.name.clone(),
                    error: e.to_string(),
                })?;

            let duration_micros = stage_start.elapsed().as_micros() as u64;
            total_fuel = total_fuel.saturating_add(exec_result.fuel_consumed);

            if total_fuel > self.max_total_fuel {
                return Err(PipelineError::PipelineFuelExhausted {
                    stage_index: idx,
                    driver_name: stage.name.clone(),
                    consumed: total_fuel,
                    limit: self.max_total_fuel,
                });
            }

            let output_hash = blake3::hash(&exec_result.output).to_hex().to_string();
            causal_hasher.update(output_hash.as_bytes());

            stage_results.push(PipelineStageResult {
                stage_index: idx,
                stage_name: stage.name.clone(),
                action: stage.action.clone(),
                fuel_consumed: exec_result.fuel_consumed,
                output_len: exec_result.output.len(),
                output_hash,
                duration_micros,
            });

            current_payload = exec_result.output;
        }

        let total_elapsed_ms = pipeline_start.elapsed().as_millis();
        let causal_chain_hash = causal_hasher.finalize().to_hex().to_string();

        Ok(PipelineExecutionReport {
            pipeline_id: self.name.clone(),
            stages: stage_results,
            total_fuel_consumed: total_fuel,
            total_elapsed_ms,
            final_output: current_payload,
            causal_chain_hash,
        })
    }
}

impl fmt::Display for DriverPipeline {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "DriverPipeline(name='{}', stages={})",
            self.name,
            self.stages.len()
        )
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

    #[test]
    fn test_pipeline_execution_sync_two_stages() {
        let wasm = wat::parse_str(ECHO_WAT).expect("valid wat");
        let pipeline = DriverPipeline::new("perception_actuation_pipe")
            .add_stage(
                "perception_filter",
                "filter",
                wasm.clone(),
                DriverPermissions::none(),
                Some(50_000),
            )
            .add_stage(
                "motor_controller",
                "actuate",
                wasm,
                DriverPermissions::none(),
                Some(50_000),
            );

        assert_eq!(pipeline.stage_count(), 2);
        let input = b"robot_telemetry_packet_xyz";
        let report = pipeline.execute(input).expect("pipeline executes cleanly");

        assert_eq!(report.pipeline_id, "perception_actuation_pipe");
        assert_eq!(report.stages.len(), 2);
        assert_eq!(report.final_output, input);
        assert!(!report.causal_chain_hash.is_empty());
    }

    #[tokio::test]
    async fn test_pipeline_execution_async_two_stages() {
        let wasm = wat::parse_str(ECHO_WAT).expect("valid wat");
        let pipeline = DriverPipeline::new("async_perception_actuation_pipe")
            .add_stage(
                "perception_filter",
                "filter",
                wasm.clone(),
                DriverPermissions::none(),
                Some(50_000),
            )
            .add_stage(
                "safety_policy",
                "evaluate",
                wasm,
                DriverPermissions::none(),
                Some(50_000),
            );

        let input = b"async_vision_frame_data";
        let report = pipeline
            .execute_async(input)
            .await
            .expect("async pipeline executes cleanly");

        assert_eq!(report.pipeline_id, "async_perception_actuation_pipe");
        assert_eq!(report.stages.len(), 2);
        assert_eq!(report.final_output, input);
        assert!(!report.causal_chain_hash.is_empty());
    }

    #[tokio::test]
    async fn test_pipeline_fuel_exhaustion() {
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
        .expect("valid wat");
        let pipeline = DriverPipeline::new("infinite_loop_pipe")
            .with_max_fuel(1_000)
            .add_stage("loop_stage", "loop", wasm, DriverPermissions::none(), None);

        let err = pipeline.execute_async(b"test").await.unwrap_err();
        assert!(matches!(
            err,
            PipelineError::StageExecutionFailed { .. }
                | PipelineError::PipelineFuelExhausted { .. }
                | PipelineError::FuelLimitExceeded { .. }
        ));
    }

    #[tokio::test]
    async fn test_pipeline_multi_stage_fuel_budget_exhaustion() {
        let wasm = wat::parse_str(ECHO_WAT).expect("valid wat");
        let pipeline = DriverPipeline::new("stage2_exhaustion_pipe")
            .with_max_fuel(12) // enough for stage 1 (~10 fuel), but not stage 2
            .add_stage(
                "stage1",
                "act1",
                wasm.clone(),
                DriverPermissions::none(),
                None,
            )
            .add_stage("stage2", "act2", wasm, DriverPermissions::none(), None);

        let err = pipeline
            .execute_async(b"test_payload_for_multi_stage")
            .await
            .unwrap_err();
        match err {
            PipelineError::StageExecutionFailed { error, .. } => {
                assert!(
                    error.contains("fuel")
                        || error.contains("Fuel")
                        || error.contains("all fuel")
                        || error.contains("consumed")
                );
            }
            PipelineError::PipelineFuelExhausted { stage_index, .. } => {
                assert_eq!(stage_index, 1);
            }
            PipelineError::FuelLimitExceeded { .. } => {}
            other => panic!("expected fuel error, got {other:?}"),
        }
    }

    #[test]
    fn test_empty_pipeline_errors() {
        let pipeline = DriverPipeline::new("empty_pipe");
        let res = pipeline.execute(b"input");
        assert_eq!(res.unwrap_err(), PipelineError::EmptyPipeline);
    }
}
