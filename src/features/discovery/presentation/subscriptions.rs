//! Event subscriptions for the discovery feature.

use crate::platform::events::EventSubscription;

/// Returns the event subscriptions this feature wants on the bus.
// TODO Sprint 4: subscribe to `discovery.run_requested`, emit `discovery.completed`.
pub fn subscriptions() -> Vec<EventSubscription> {
    Vec::new()
}
