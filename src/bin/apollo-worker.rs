//! Worker entry point. Consumes the Postgres job queue.

use anyhow::Result;
use apollo::platform::worker::{run, AgentRegistry, WorkerConfig};

#[tokio::main]
async fn main() -> Result<()> {
    let _ = dotenvy::from_filename(".env.local");
    apollo::platform::observability::init()?;

    let boot = apollo::bootstrap::build().await?;
    tracing::info!(
        subscriptions = boot.subscriptions.len(),
        "apollo-worker started"
    );

    // S1-T03: registry empty for now. Agents register in S1-T08 onwards via
    //         each feature's `configure()` returning its Arc<dyn ErasedAgent>s.
    let registry = AgentRegistry::new();

    run(boot.deps.db, registry, WorkerConfig::default()).await
}
