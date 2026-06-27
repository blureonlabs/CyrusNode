//! Migration runner.
//!
//! Migrations live in `./migrations/` at the crate root and are compiled into
//! the binary via `sqlx::migrate!()`. The `apollo migrate` CLI subcommand calls
//! `run_migrations` explicitly; binaries may also call it on startup.

use sqlx::PgPool;

use super::DbError;

/// Apply all checked-in migrations under `./migrations/`.
///
/// Idempotent — safe to call on every startup or explicitly via the
/// `apollo migrate` CLI subcommand. Returns `DbError::Migration` if any
/// migration fails to apply; the pool is left in whatever state Postgres ended
/// the transaction in (sqlx wraps each migration in a transaction).
pub async fn run_migrations(pool: &PgPool) -> Result<(), DbError> {
    sqlx::migrate!("./migrations").run(pool).await?;
    tracing::info!("migrations applied");
    Ok(())
}
