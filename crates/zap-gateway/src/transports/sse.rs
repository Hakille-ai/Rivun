//! Server-Sent Events (SSE) stream broker for real-time agent telemetry.

use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::broadcast;
use zap_agent::{AgentResult, AgentStatusUpdate};

pub const DEFAULT_SSE_CHANNEL_CAPACITY: usize = 1024;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SseEvent {
    pub event_type: String,
    pub data: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retry_ms: Option<u64>,
}

impl SseEvent {
    pub fn new(event_type: impl Into<String>, data: impl Into<String>) -> Self {
        Self {
            event_type: event_type.into(),
            data: data.into(),
            id: None,
            retry_ms: None,
        }
    }

    pub fn with_id(mut self, id: impl Into<String>) -> Self {
        self.id = Some(id.into());
        self
    }

    pub fn to_sse_wire_format(&self) -> String {
        let mut out = String::new();
        if let Some(id) = &self.id {
            out.push_str(&format!("id: {id}\n"));
        }
        if let Some(retry) = self.retry_ms {
            out.push_str(&format!("retry: {retry}\n"));
        }
        out.push_str(&format!("event: {}\n", self.event_type));
        for line in self.data.lines() {
            out.push_str(&format!("data: {line}\n"));
        }
        out.push('\n');
        out
    }
}

#[derive(Clone)]
pub struct SseBroker {
    tx: Arc<broadcast::Sender<SseEvent>>,
}

impl SseBroker {
    pub fn new(capacity: usize) -> Self {
        let (tx, _) = broadcast::channel(capacity.max(16));
        Self { tx: Arc::new(tx) }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<SseEvent> {
        self.tx.subscribe()
    }

    pub fn subscriber_count(&self) -> usize {
        self.tx.receiver_count()
    }

    pub fn send(&self, event: SseEvent) {
        let _ = self.tx.send(event);
    }

    pub fn broadcast_status(&self, update: &AgentStatusUpdate) {
        if let Ok(data) = serde_json::to_string(update) {
            self.send(SseEvent::new("agent_status", data));
        }
    }

    pub fn broadcast_result(&self, result: &AgentResult) {
        if let Ok(data) = serde_json::to_string(result) {
            self.send(SseEvent::new("agent_result", data));
        }
    }

    pub fn broadcast_heartbeat(&self) {
        let now = zap_core::now_micros().unwrap_or(0);
        self.send(SseEvent::new(
            "heartbeat",
            format!(r#"{{"timestamp_micros":{now}}}"#),
        ));
    }
}

impl Default for SseBroker {
    fn default() -> Self {
        Self::new(DEFAULT_SSE_CHANNEL_CAPACITY)
    }
}
