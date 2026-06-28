//! Wiring file for the discovery feature.
//!
//! Single matchmaker for the four boxes (`domain`, `application`, `infra`,
//! `presentation`). The bootstrap calls `configure(DiscoveryDeps)` and gets
//! back the routes, subscriptions, and a handle to the use case.

use std::sync::Arc;

use crate::features::discovery::application::DiscoverLeads;
use crate::features::discovery::domain::DiscoveryPort;
use crate::features::discovery::infra::GooglePlacesAdapter;
use crate::platform::events::EventSubscription;

/// V1 needs no cross-cutting handles (no DB, no LLM — Google Places only).
/// The struct is kept so the public shape matches the other features and
/// future deps can land without churning callers.
pub struct DiscoveryDeps;

/// The handles `bootstrap.rs` collects from this feature.
pub struct DiscoveryModule {
    pub routes: axum::Router<()>,
    pub subscriptions: Vec<EventSubscription>,
    pub discover_leads: Arc<DiscoverLeads>,
}

/// Wire the discovery feature.
///
/// `GooglePlacesAdapter::from_env()` reads `GOOGLE_MAPS_API_KEY`. If the env
/// var is missing we still construct a `DiscoverLeads` that will return
/// `DiscoveryError::MissingKey` at runtime — boot does not fail, so the rest
/// of the system stays usable.
pub fn configure(_deps: DiscoveryDeps) -> DiscoveryModule {
    let port: Arc<dyn DiscoveryPort> = match GooglePlacesAdapter::from_env() {
        Ok(a) => Arc::new(a),
        Err(_) => Arc::new(GooglePlacesAdapter::stub()),
    };
    let discover_leads = Arc::new(DiscoverLeads::new(port));
    DiscoveryModule {
        routes: super::presentation::routes(),
        subscriptions: super::presentation::subscriptions(),
        discover_leads,
    }
}
