//! Postgres connection pool, migrations, and repositories.
//!
//! Wired in S1-T02. Pool comes from Supabase (ADR-011). Repositories provide
//! typed access to core tables: companies, jobs, events, artifacts, llm_calls.

mod error;
mod migrate;
mod models;
mod pool;

pub mod repos;

pub use error::DbError;
pub use migrate::run_migrations;
pub use models::{ArtifactRow, CompanyRow, EventRow, JobRow, JobStatus, LlmCallRow};
pub use pool::{build_pool, DbConfig};

pub use sqlx::PgPool;
