//! Subscription contract + envelope type seen by event handlers.

use serde::{Deserialize, Serialize};

use crate::platform::db::{DbError, EventRow};

/// Subscription contract — features describe what event types they listen for.
///
/// `EventSubscription` is a static declaration: features expose a
/// `subscriptions()` function returning `Vec<EventSubscription>`, which
/// `bootstrap.rs` walks at startup to wire dispatch.
#[derive(Debug, Clone)]
pub struct EventSubscription {
    /// Dotted, lowercase event type (e.g. `crawl.completed`).
    pub event_type: &'static str,
}

/// What a handler receives. Includes envelope metadata + the JSON payload.
///
/// `tenant_id` is parsed from the `events.tenant_id` column (stored as TEXT
/// in V1 — see `EventRow`). If parsing fails we surface a `DbError` rather
/// than panicking; that should be impossible in practice because writers
/// always insert a `Uuid`, but we don't want a single corrupt row to take
/// down the listener.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventEnvelope {
    pub id: String,
    pub tenant_id: uuid::Uuid,
    pub event_type: String,
    pub version: i32,
    pub correlation_id: String,
    pub causation_id: Option<String>,
    pub company_id: Option<uuid::Uuid>,
    pub payload: serde_json::Value,
    pub occurred_at: chrono::DateTime<chrono::Utc>,
}

impl EventEnvelope {
    /// Convert a raw `EventRow` from the DB into a dispatch envelope.
    ///
    /// Returns `DbError::NotFound` if `tenant_id` is not a valid UUID — that
    /// signals "this row is malformed, skip it" upstream rather than poisoning
    /// the listener loop.
    pub fn from_row(row: EventRow) -> Result<Self, DbError> {
        let tenant_id = uuid::Uuid::parse_str(&row.tenant_id).map_err(|_| DbError::NotFound)?;
        Ok(Self {
            id: row.id,
            tenant_id,
            event_type: row.type_,
            version: row.version,
            correlation_id: row.correlation_id,
            causation_id: row.causation_id,
            company_id: row.company_id,
            payload: row.payload,
            occurred_at: row.occurred_at,
        })
    }
}
