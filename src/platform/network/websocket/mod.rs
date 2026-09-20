//! Feature-gated WebSocket client scaffold (opt-in via the `websocket`
//! feature; the off build exposes a stub that reports `FeatureDisabled`).

mod client;
mod types;

pub use client::WebSocketClient;
pub use types::{ConnectionState, MessageHandler, ReconnectPolicy, WebSocketError};

#[cfg(test)]
#[path = "../websocket.test.rs"]
mod websocket_test;
