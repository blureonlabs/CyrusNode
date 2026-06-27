//! Event publisher — appends to the events table and fires `NOTIFY`.

use async_trait::async_trait;
use serde_json::Value;

use crate::platform::db::{
    repos::{EventsRepo, NewEvent},
    DbError, PgPool,
};

use super::EVENTS_CHANNEL;

/// Publishes events onto the bus.
///
/// Trait-based so handlers and tests can be wired against a fake. The real
/// implementation is `PgEventPublisher`; tests typically use a hand-rolled
/// in-memory recorder.
#[async_trait]
pub trait EventPublisher: Send + Sync {
    /// Append an event of `event_type` with the given metadata + payload.
    /// Returns the new event id (ULID) on success.
    async fn publish(&self, event_type: &str, payload: PublishPayload) -> Result<String, DbError>;
}

/// Metadata + payload bundle the publisher needs.
///
/// The publisher fills in `version` (callers pass 1 by default) and the row's
/// `occurred_at` is stamped by Postgres; everything else is supplied by the
/// caller.
#[derive(Debug, Clone)]
pub struct PublishPayload {
    pub tenant_id: uuid::Uuid,
    pub version: i32,
    pub correlation_id: String,
    pub causation_id: Option<String>,
    pub company_id: Option<uuid::Uuid>,
    pub payload: Value,
}

/// Postgres-backed publisher.
///
/// Cheap to clone; internally just holds the pool.
#[derive(Clone)]
pub struct PgEventPublisher {
    pool: PgPool,
}

impl PgEventPublisher {
    /// Construct a publisher backed by the given pool.
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl EventPublisher for PgEventPublisher {
    /// Append the event, then issue a best-effort `pg_notify` on the
    /// `apollo_events` channel carrying the new event id.
    ///
    /// The NOTIFY is best-effort: if it fails, listeners still pick up the
    /// row via their 5-second poll fallback. We log at WARN so the failure
    /// is visible but does not bubble up as an error to the caller.
    async fn publish(&self, event_type: &str, p: PublishPayload) -> Result<String, DbError> {
        let repo = EventsRepo::new(self.pool.clone());
        let row = repo
            .append(NewEvent {
                tenant_id: p.tenant_id,
                r#type: event_type.to_string(),
                version: p.version,
                correlation_id: p.correlation_id,
                causation_id: p.causation_id,
                company_id: p.company_id,
                payload: p.payload,
            })
            .await?;

        // Best-effort NOTIFY. Failure here is non-fatal — listeners poll as
        // a fallback every 5 seconds. We do not want a transient connection
        // hiccup on NOTIFY to roll the caller back.
        if let Err(err) = sqlx::query("SELECT pg_notify($1, $2)")
            .bind(EVENTS_CHANNEL)
            .bind(&row.id)
            .execute(&self.pool)
            .await
        {
            tracing::warn!(
                error = %err,
                event_id = %row.id,
                event_type = %event_type,
                "pg_notify failed; listeners will pick up via poll fallback",
            );
        }

        Ok(row.id)
    }
}
