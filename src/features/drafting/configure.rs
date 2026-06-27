//! Wiring file for the drafting feature.
//!
//! This is the single file that knows about all four boxes (`domain`,
//! `application`, `infra`, `presentation`) for `drafting`. It builds the
//! adapters, hands them to the use case, and exposes the routes + event
//! subscriptions for `bootstrap.rs` to mount.

use std::sync::Arc;

use crate::features::drafting::application::DraftOutreach;
use crate::features::drafting::domain::DraftPort;
use crate::features::drafting::infra::EmailWriterAgent;
use crate::platform::events::EventSubscription;

pub struct DraftingModule {
    pub routes: axum::Router<()>,
    pub subscriptions: Vec<EventSubscription>,
    pub draft_outreach: Arc<DraftOutreach>,
}

/// Wires the four boxes for the drafting feature. Called once from `bootstrap.rs`.
pub fn configure() -> DraftingModule {
    let draft: Arc<dyn DraftPort> = Arc::new(EmailWriterAgent);

    let draft_outreach = Arc::new(DraftOutreach::new(draft));

    DraftingModule {
        routes: super::presentation::routes(),
        subscriptions: super::presentation::subscriptions(),
        draft_outreach,
    }
}
