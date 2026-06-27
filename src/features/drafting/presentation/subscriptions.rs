//! Event subscriptions for the drafting feature.

use crate::platform::events::EventSubscription;

/// Returns the event subscriptions this feature wants on the bus.
// TODO: subscribe to `research.dossier_base.ready` once Sprint 2 wires synthesis.
pub fn subscriptions() -> Vec<EventSubscription> {
    Vec::new()
}
