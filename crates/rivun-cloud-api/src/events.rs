//! Server-Sent Events (SSE) Real-Time Broadcaster for Rivun Cloud API.

use axum::response::sse::{Event, KeepAlive, Sse};
use futures_util::stream::Stream;
use std::convert::Infallible;
use std::sync::Arc;
use tokio::sync::broadcast;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::StreamExt;
use uuid::Uuid;

#[derive(Debug, Clone, serde::Serialize)]
pub struct CloudEvent {
    pub org_id: Uuid,
    pub event_type: String, // "receipt_ingested", "doctor_updated", "policy_staged", "incident_fired"
    pub payload: serde_json::Value,
    pub timestamp_micros: u64,
}

#[derive(Clone)]
pub struct EventBroker {
    sender: Arc<broadcast::Sender<CloudEvent>>,
}

impl EventBroker {
    pub fn new(capacity: usize) -> Self {
        let (tx, _) = broadcast::channel(capacity);
        Self {
            sender: Arc::new(tx),
        }
    }

    pub fn publish(&self, org_id: Uuid, event_type: &str, payload: serde_json::Value) {
        let event = CloudEvent {
            org_id,
            event_type: event_type.to_string(),
            payload,
            timestamp_micros: rivun_core::now_micros().unwrap_or(0),
        };
        let _ = self.sender.send(event);
    }

    pub fn subscribe_org(
        &self,
        target_org_id: Uuid,
    ) -> Sse<impl Stream<Item = Result<Event, Infallible>> + Send + 'static> {
        let rx = self.sender.subscribe();
        let stream = BroadcastStream::new(rx).filter_map(move |item| {
            match item {
                Ok(evt) if evt.org_id == target_org_id => {
                    let json = serde_json::to_string(&evt).unwrap_or_default();
                    Some(Ok(Event::default().event(evt.event_type).data(json)))
                }
                _ => None,
            }
        });

        Sse::new(stream).keep_alive(KeepAlive::default())
    }
}
