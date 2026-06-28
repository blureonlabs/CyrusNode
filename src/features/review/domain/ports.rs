//! Port traits the application uses to talk to the outside world.
//!
//! Domain rule: this file has zero I/O dependencies. Implementations live
//! under `infra/`.

use async_trait::async_trait;

use super::queue_item::{Decision, QueueItem};

/// Persistence boundary for queue items on disk (or any other store).
#[async_trait]
pub trait QueueRepoPort: Send + Sync {
    /// Returns the items waiting for review.
    async fn pending(&self) -> anyhow::Result<Vec<QueueItem>>;

    /// Record the operator's decision. For Approve* the implementation moves
    /// the file out of `out/dossiers/` into `out/sent/` or `out/outbox/`;
    /// for Reject it moves to `out/rejected/`; Skip leaves it in place.
    async fn record_decision(&self, item: &QueueItem, decision: Decision) -> anyhow::Result<()>;

    /// Persist an edited draft back to disk (overwrites the on-disk JSON).
    async fn save_edited(&self, item: &QueueItem) -> anyhow::Result<()>;
}

/// Operator interaction surface. `StdinPrompter` is the V1 implementation;
/// future TUI / web prompters slot in here.
#[async_trait]
pub trait OperatorPrompt: Send + Sync {
    /// Display the item and return the decision.
    async fn ask(&self, item: &QueueItem) -> anyhow::Result<Decision>;
    /// Open the email body in `$EDITOR`; return the edited body.
    async fn edit_body(&self, current: &str) -> anyhow::Result<String>;
    /// Prompt for a recipient address when the item has none and the operator
    /// wants to send. Returns `Some(addr)` if the operator types one, or
    /// `None` to fall back to the outbox.
    async fn request_recipient(&self, item: &QueueItem) -> anyhow::Result<Option<String>>;
}

/// Send the approved email through whatever provider the integration layer wires.
#[async_trait]
pub trait SendPort: Send + Sync {
    /// Send `body` from `from` to `to` with `subject`. Returns the provider's
    /// external identifier so it can be persisted for tracking.
    async fn send(&self, from: &str, to: &str, subject: &str, body: &str)
        -> anyhow::Result<String>;
}
