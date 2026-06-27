//! Events repository — append-only log keyed by ULID.
//!
//! The schema column is `type`, which is a Postgres reserved word. We quote
//! it as `"type"` in every statement that references it.

use chrono::{DateTime, Utc};
use serde_json::Value as JsonValue;
use sqlx::PgPool;
use uuid::Uuid;

use crate::platform::db::{DbError, EventRow};

/// Input for appending an event. The `id` is generated inside `append`
/// as a fresh ULID for time-sortable readability (see database.md §1).
#[derive(Debug, Clone)]
pub struct NewEvent {
    pub tenant_id: Uuid,
    pub r#type: String,
    pub version: i32,
    pub correlation_id: String,
    pub causation_id: Option<String>,
    pub company_id: Option<Uuid>,
    pub payload: JsonValue,
}

/// Append-only access to the `events` table. Cheap to clone.
#[derive(Debug, Clone)]
pub struct EventsRepo {
    pool: PgPool,
}

impl EventsRepo {
    /// Construct a repo backed by the given pool.
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Append an event and return the stored row. Generates a fresh ULID
    /// for `id` and lets the database stamp `occurred_at`.
    pub async fn append(&self, event: NewEvent) -> Result<EventRow, DbError> {
        let id = ulid::Ulid::new().to_string();
        let row = sqlx::query_as::<_, EventRow>(
            r#"
            INSERT INTO events (
                id, tenant_id, "type", version, correlation_id,
                causation_id, company_id, payload
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            RETURNING *
            "#,
        )
        .bind(id)
        .bind(event.tenant_id)
        .bind(&event.r#type)
        .bind(event.version)
        .bind(&event.correlation_id)
        .bind(&event.causation_id)
        .bind(event.company_id)
        .bind(&event.payload)
        .fetch_one(&self.pool)
        .await?;
        Ok(row)
    }

    /// List all events for a correlation chain, oldest-first.
    pub async fn list_by_correlation(
        &self,
        correlation_id: &str,
        limit: i64,
    ) -> Result<Vec<EventRow>, DbError> {
        let rows = sqlx::query_as::<_, EventRow>(
            r#"
            SELECT * FROM events
            WHERE correlation_id = $1
            ORDER BY occurred_at ASC
            LIMIT $2
            "#,
        )
        .bind(correlation_id)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// List events of a given type for the tenant since `since`, newest-first.
    pub async fn list_by_type(
        &self,
        tenant_id: Uuid,
        r#type: &str,
        since: DateTime<Utc>,
        limit: i64,
    ) -> Result<Vec<EventRow>, DbError> {
        let rows = sqlx::query_as::<_, EventRow>(
            r#"
            SELECT * FROM events
            WHERE tenant_id = $1 AND "type" = $2 AND occurred_at >= $3
            ORDER BY occurred_at DESC
            LIMIT $4
            "#,
        )
        .bind(tenant_id)
        .bind(r#type)
        .bind(since)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }
}
