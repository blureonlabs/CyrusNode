//! Event bus — Postgres LISTEN/NOTIFY based.
//!
//! Writers `INSERT` into events table and `NOTIFY events` in the same
//! transaction. Listeners `LISTEN events` and fall back to polling every 5s.
//! Sagas coordinate fan-in over multiple events.

mod listener;
mod publisher;
mod saga;
mod subscription;

pub use listener::PgEventListener;
pub use publisher::{EventPublisher, PgEventPublisher, PublishPayload};
pub use saga::{SagaCoordinator, SagaDefinition, SagaState};
pub use subscription::{EventEnvelope, EventSubscription};

/// Postgres NOTIFY channel name used for the event bus.
pub const EVENTS_CHANNEL: &str = "apollo_events";
