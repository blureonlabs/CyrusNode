//! Bootstrap — the only file that knows about all features.
//!
//! Builds platform handles once, calls each feature's `configure()`, returns a
//! `Bootstrap` struct the binaries use to construct their entry points.

use anyhow::Result;
use axum::Router;

use crate::features::{drafting, research};
use crate::platform::db::{build_pool, DbConfig, PgPool};
use crate::platform::events::EventSubscription;

/// Handles to every cross-cutting capability. Built once at boot; cloned freely.
#[derive(Clone)]
pub struct PlatformDeps {
    pub db: PgPool,
}

/// Aggregated routes + subscriptions from every feature.
pub struct Bootstrap {
    pub router: Router<()>,
    pub subscriptions: Vec<EventSubscription>,
    pub deps: PlatformDeps,
}

/// Wire everything. Call once at startup.
///
/// Reads `DATABASE_URL` (and friends) from the environment via [`DbConfig::from_env`],
/// builds the connection pool, then constructs each feature module.
pub async fn build() -> Result<Bootstrap> {
    let cfg = DbConfig::from_env()?;
    let db = build_pool(&cfg).await?;
    let deps = PlatformDeps { db };

    let research = research::configure();
    let drafting = drafting::configure();

    let router = Router::new().merge(research.routes).merge(drafting.routes);

    let mut subscriptions = Vec::new();
    subscriptions.extend(research.subscriptions);
    subscriptions.extend(drafting.subscriptions);

    Ok(Bootstrap {
        router,
        subscriptions,
        deps,
    })
}
