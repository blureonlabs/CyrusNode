//! Bootstrap — the only file that knows about all features.
//!
//! Builds platform handles once, calls each feature's `configure()`, returns a
//! `Bootstrap` struct the binaries use to construct their entry points.

use anyhow::Result;
use axum::Router;

use crate::features::{drafting, research};
use crate::platform::events::EventSubscription;

/// Aggregated routes + subscriptions from every feature.
pub struct Bootstrap {
    pub router: Router<()>,
    pub subscriptions: Vec<EventSubscription>,
}

/// Wire everything. Call once at startup.
pub fn build() -> Result<Bootstrap> {
    let research = research::configure();
    let drafting = drafting::configure();

    let router = Router::new().merge(research.routes).merge(drafting.routes);

    let mut subscriptions = Vec::new();
    subscriptions.extend(research.subscriptions);
    subscriptions.extend(drafting.subscriptions);

    Ok(Bootstrap {
        router,
        subscriptions,
    })
}
