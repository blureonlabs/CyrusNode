//! Tracing setup. `tracing` + `tracing-subscriber` with EnvFilter.
//!
//! Stub. Wired in S1-T15.

use anyhow::Result;

/// Initialize tracing. Call once from each binary's `main`.
pub fn init() -> Result<()> {
    use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
                tracing_subscriber::EnvFilter::new(
                    "apollo=debug,tower_http=info,axum::rejection=trace",
                )
            }),
        )
        .with(tracing_subscriber::fmt::layer().compact())
        .init();
    Ok(())
}
