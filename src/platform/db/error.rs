//! Typed errors for the `platform::db` module.
//!
//! Library boundary uses `DbError` (per CLAUDE.md §2 "Errors"). Application
//! glue may convert into `anyhow::Error` at the edges.

use thiserror::Error;

/// Errors produced by the database layer.
///
/// `Pool` wraps any `sqlx::Error` from queries or connection acquisition.
/// `Migration` wraps failures from `sqlx::migrate!()`.
/// `NotFound` is returned by repositories when a lookup by id misses.
/// `Conflict` is returned by repositories when a uniqueness invariant is hit
/// (for example duplicate idempotency key on `jobs`).
#[derive(Debug, Error)]
pub enum DbError {
    #[error(transparent)]
    Pool(#[from] sqlx::Error),
    #[error(transparent)]
    Migration(#[from] sqlx::migrate::MigrateError),
    #[error("not found")]
    NotFound,
    #[error("conflict: {0}")]
    Conflict(String),
}
