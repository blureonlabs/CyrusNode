//! Event subscriptions for the research feature.

use crate::platform::events::EventSubscription;

/// Returns the event subscriptions this feature wants on the bus.
// TODO: subscribe to `company.discovered` once Sprint 2 wires discovery.
pub fn subscriptions() -> Vec<EventSubscription> {
    Vec::new()
}
