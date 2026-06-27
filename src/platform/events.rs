//! Event bus (Postgres LISTEN/NOTIFY V1) + saga runtime.
//!
//! Stub. Wired in S1-T04.

use async_trait::async_trait;
use serde_json::Value;

#[async_trait]
pub trait EventPublisher: Send + Sync {
    async fn publish(&self, event_type: &str, payload: Value) -> anyhow::Result<()>;
}

/// Subscription contract — features describe what they listen for.
pub struct EventSubscription {
    pub event_type: &'static str,
}
