//! Worker entry point. Consumes the Postgres job queue.
//!
//! Stub. The claim → run-agent → publish-event loop is wired in S1-T03 + S1-T05.

use anyhow::Result;
use std::time::Duration;
use tokio::time;

#[tokio::main]
async fn main() -> Result<()> {
    apollo::platform::observability::init()?;

    let boot = apollo::bootstrap::build()?;
    tracing::info!(
        subscriptions = boot.subscriptions.len(),
        "apollo-worker started"
    );

    loop {
        // TODO S1-T03: claim_next from jobs table; run agent; publish events.
        tracing::trace!("worker tick");
        time::sleep(Duration::from_secs(5)).await;
    }
}
