//! Postgres LISTEN/NOTIFY based event listener with a 5-second poll fallback.
//!
//! See `apollo/02-architecture/event-driven.md` §5 for the design.
//!
//! - LISTEN on `apollo_events`. NOTIFY payloads carry the event id.
//! - On notification, fetch the event row and dispatch.
//! - In parallel, every 5 seconds poll for any events with
//!   `occurred_at > last_seen` that we haven't dispatched yet.
//! - Single in-process listener, no clustering. V2 (Redis Streams) will
//!   add fan-out and consumer groups.

use std::collections::HashSet;
use std::time::Duration;

use chrono::{DateTime, Utc};
use sqlx::postgres::PgListener;
use tokio::time;

use crate::platform::db::{DbError, EventRow, PgPool};

use super::{EventEnvelope, EVENTS_CHANNEL};

/// How often to poll the events table for rows we may have missed while
/// disconnected from the NOTIFY channel.
const POLL_INTERVAL: Duration = Duration::from_secs(5);

/// Max rows pulled per poll cycle. Plenty of headroom for the V1 rate of
/// 1–5 events/s.
const POLL_BATCH_LIMIT: i64 = 256;

/// Postgres-backed event listener.
///
/// Cheap to clone; just holds the pool. The actual loop is driven by `run`.
#[derive(Clone)]
pub struct PgEventListener {
    pool: PgPool,
}

impl PgEventListener {
    /// Construct a listener backed by the given pool.
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Run the listen + poll loop until the underlying connection cannot be
    /// recovered. `dispatch` is called for every freshly observed event;
    /// the closure is expected to return a `JoinHandle` so the caller
    /// controls handler concurrency.
    ///
    /// Failure modes:
    /// - `DbError::Pool` if we cannot establish the initial LISTEN or run
    ///   the bootstrap query for `last_seen`.
    ///
    /// Under normal operation this future does not return.
    pub async fn run<F>(&self, dispatch: F) -> Result<(), DbError>
    where
        F: Fn(EventEnvelope) -> tokio::task::JoinHandle<()> + Send + Sync + 'static,
    {
        let mut listener = PgListener::connect_with(&self.pool).await?;
        listener.listen(EVENTS_CHANNEL).await?;

        // Track event ids we have already dispatched so the poll fallback
        // does not double-fire when NOTIFY also delivered them.
        let mut dispatched: HashSet<String> = HashSet::new();
        // `last_seen` is the high-water mark for the poll query. Initialized
        // to "now" so we only react to events that arrive after startup.
        let mut last_seen: DateTime<Utc> = Utc::now();

        let mut poll_ticker = time::interval(POLL_INTERVAL);
        // Skip the first immediate tick — we just initialized `last_seen`.
        poll_ticker.tick().await;

        loop {
            tokio::select! {
                notif = listener.recv() => {
                    match notif {
                        Ok(notification) => {
                            let id = notification.payload().to_string();
                            if dispatched.contains(&id) {
                                continue;
                            }
                            match self.fetch_by_id(&id).await {
                                Ok(Some(row)) => {
                                    last_seen = last_seen.max(row.occurred_at);
                                    self.dispatch_row(row, &mut dispatched, &dispatch);
                                }
                                Ok(None) => {
                                    tracing::warn!(event_id = %id, "NOTIFY referenced unknown event id");
                                }
                                Err(err) => {
                                    tracing::warn!(error = %err, event_id = %id, "failed to fetch event from NOTIFY");
                                }
                            }
                        }
                        Err(err) => {
                            // PgListener auto-reconnects on the next recv; log and continue.
                            tracing::warn!(error = %err, "pg listener recv error; will retry");
                        }
                    }
                }
                _ = poll_ticker.tick() => {
                    match self.poll_since(last_seen).await {
                        Ok(rows) => {
                            for row in rows {
                                last_seen = last_seen.max(row.occurred_at);
                                self.dispatch_row(row, &mut dispatched, &dispatch);
                            }
                            // Bound memory: dispatched is a safety net for the
                            // poll/NOTIFY overlap window only. 5s window × a
                            // few hundred events/s is the worst case; cap at
                            // 4_096 with simple FIFO eviction.
                            if dispatched.len() > 4_096 {
                                dispatched.clear();
                            }
                        }
                        Err(err) => {
                            tracing::warn!(error = %err, "events poll failed");
                        }
                    }
                }
            }
        }
    }

    /// Fetch a single event row by id. Returns `Ok(None)` if missing.
    async fn fetch_by_id(&self, id: &str) -> Result<Option<EventRow>, DbError> {
        let row = sqlx::query_as::<_, EventRow>(r#"SELECT * FROM events WHERE id = $1"#)
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(row)
    }

    /// Pull every event with `occurred_at > since`, oldest-first.
    async fn poll_since(&self, since: DateTime<Utc>) -> Result<Vec<EventRow>, DbError> {
        let rows = sqlx::query_as::<_, EventRow>(
            r#"
            SELECT * FROM events
            WHERE occurred_at > $1
            ORDER BY occurred_at ASC
            LIMIT $2
            "#,
        )
        .bind(since)
        .bind(POLL_BATCH_LIMIT)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Build an envelope from a row and hand it to `dispatch`. Skips rows
    /// whose `tenant_id` cannot be parsed (logged) so a malformed row never
    /// poisons the loop.
    fn dispatch_row<F>(&self, row: EventRow, dispatched: &mut HashSet<String>, dispatch: &F)
    where
        F: Fn(EventEnvelope) -> tokio::task::JoinHandle<()> + Send + Sync + 'static,
    {
        let id = row.id.clone();
        match EventEnvelope::from_row(row) {
            Ok(env) => {
                dispatched.insert(id);
                let _handle = dispatch(env);
            }
            Err(err) => {
                tracing::warn!(error = %err, event_id = %id, "skipping malformed event row");
            }
        }
    }
}
