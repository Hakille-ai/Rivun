//! Gateway transport implementations: HTTP REST, SSE stream, and WebSocket bridge.

pub mod http;
pub mod sse;
pub mod ws;

pub use http::HttpAgentGateway;
pub use sse::{SseBroker, SseEvent};
pub use ws::{WebSocketHandler, WsFrame, compute_ws_accept};
