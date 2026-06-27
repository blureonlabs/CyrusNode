//! Postgres connection pool builder.
//!
//! The pool is shared across the API server, worker, and CLI binaries. It is
//! built once at startup (see `bootstrap.rs`) and cloned cheaply (Arc inside)
//! into whatever needs DB access.

use std::time::Duration;

use sqlx::postgres::{PgPool, PgPoolOptions};

use super::DbError;

/// Configuration for the Postgres pool.
///
/// Defaults match the values used in `apollo-api` and `apollo-worker` when no
/// env override is set. `max_connections` must stay under the Supabase pooler
/// limit (see ADR-011) — bump deliberately, not by accident.
#[derive(Debug, Clone)]
pub struct DbConfig {
    pub database_url: String,
    pub max_connections: u32,
    pub acquire_timeout_secs: u64,
}

impl DbConfig {
    /// Build a `DbConfig` from process environment.
    ///
    /// Reads:
    /// - `DATABASE_URL` (required) — full Postgres URI, typically the Supabase
    ///   pooler URL.
    /// - `DATABASE_MAX_CONNECTIONS` (optional, default `10`).
    /// - `DATABASE_ACQUIRE_TIMEOUT_SECS` (optional, default `30`).
    ///
    /// Returns `Err` only when `DATABASE_URL` is missing. Malformed numbers in
    /// the optional vars silently fall back to defaults — this keeps boot
    /// resilient to a typo'd override.
    pub fn from_env() -> anyhow::Result<Self> {
        let database_url = std::env::var("DATABASE_URL")
            .map_err(|_| anyhow::anyhow!("DATABASE_URL not set (see .env.local.example)"))?;
        let max_connections = std::env::var("DATABASE_MAX_CONNECTIONS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(10);
        let acquire_timeout_secs = std::env::var("DATABASE_ACQUIRE_TIMEOUT_SECS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(30);
        Ok(Self {
            database_url,
            max_connections,
            acquire_timeout_secs,
        })
    }
}

/// Build a connection pool.
///
/// Honours Supabase pooler limits via `max_connections`. Logs a single info
/// span on success. On failure, the underlying `sqlx::Error` is wrapped in
/// `DbError::Pool` so callers can match without depending on `sqlx` directly.
pub async fn build_pool(cfg: &DbConfig) -> Result<PgPool, DbError> {
    let pool = PgPoolOptions::new()
        .max_connections(cfg.max_connections)
        .acquire_timeout(Duration::from_secs(cfg.acquire_timeout_secs))
        .connect(&cfg.database_url)
        .await?;
    tracing::info!(max_conns = cfg.max_connections, "postgres pool built");
    Ok(pool)
}
