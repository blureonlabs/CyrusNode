//! Row structs that mirror the core tables.
//!
//! Specced from `apollo/02-architecture/database.md` §2. These are *plain
//! row* types — they exist to let `sqlx::query_as!` and `query_as::<_, T>`
//! decode rows into something the repositories can map onto domain types.
//!
//! Domain types live next to the features that own them. These rows are
//! deliberately permissive (lots of `Option`, `serde_json::Value` for JSONB)
//! because the database is the wire format, not the domain.

use chrono::{DateTime, Utc};
use serde_json::Value as JsonValue;
use uuid::Uuid;

/// Status values for a row in `jobs`.
///
/// Modeled as a struct with associated `&'static str` consts rather than an
/// enum so that:
/// - `sqlx` decoding stays simple (the column is `TEXT`/`VARCHAR`, not an
///   enum type in Postgres).
/// - New states can be added without a migration of the Rust type.
///
/// Callers compare with `row.status == JobStatus::QUEUED`.
pub struct JobStatus;

impl JobStatus {
    pub const QUEUED: &'static str = "queued";
    pub const LEASED: &'static str = "leased";
    pub const RUNNING: &'static str = "running";
    pub const SUCCEEDED: &'static str = "succeeded";
    pub const FAILED: &'static str = "failed";
    pub const CANCELLED: &'static str = "cancelled";
}

/// A row in `companies`.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct CompanyRow {
    pub id: Uuid,
    pub tenant_id: String,
    pub url: String,
    pub host: String,
    pub name: Option<String>,
    pub industry: Option<String>,
    pub country: Option<String>,
    pub city: Option<String>,
    pub status: String,
    pub first_seen_at: DateTime<Utc>,
    pub last_researched_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

/// A row in `jobs`.
///
/// `correlation_id` is stored as text (mirrors the `CorrelationId` wrapper in
/// `platform::core::ids`). `input` is the raw agent input as JSONB so that we
/// can rebuild the typed input downstream without losing precision.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct JobRow {
    pub id: Uuid,
    pub tenant_id: String,
    pub agent_name: String,
    pub agent_version: i32,
    pub company_id: Option<Uuid>,
    pub correlation_id: String,
    pub input: JsonValue,
    pub input_hash: String,
    pub status: String,
    pub priority: i32,
    pub available_at: DateTime<Utc>,
    pub leased_until: Option<DateTime<Utc>>,
    pub attempt_count: i32,
    pub max_attempts: i32,
    pub last_error: Option<String>,
    pub created_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
}

/// A row in `events`.
///
/// The `type` column is renamed to `type_` because `type` is a reserved Rust
/// keyword. `id` is a string ULID (see `platform::core::ids`).
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct EventRow {
    pub id: String,
    pub tenant_id: String,
    #[sqlx(rename = "type")]
    pub type_: String,
    pub version: i32,
    pub correlation_id: String,
    pub causation_id: Option<String>,
    pub company_id: Option<Uuid>,
    pub payload: JsonValue,
    pub occurred_at: DateTime<Utc>,
}

/// A row in `artifacts`.
///
/// Artifacts are the cacheable, content-addressed outputs of an agent run.
/// `input_hash` is the cache key together with `(agent_name, agent_version)`.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ArtifactRow {
    pub id: Uuid,
    pub tenant_id: String,
    pub company_id: Uuid,
    pub agent_name: String,
    pub agent_version: i32,
    pub input_hash: String,
    pub prompt_name: Option<String>,
    pub prompt_version: Option<i32>,
    pub payload: JsonValue,
    pub payload_schema_version: i32,
    pub bytes: i64,
    pub created_at: DateTime<Utc>,
}

/// A row in `llm_calls`.
///
/// One row per LLM invocation. `cost_usd` is stored as `f64` for V1 — see
/// TODO below. Every field on this row is part of the cost-discipline story
/// (CLAUDE.md §6); do not drop columns without an ADR.
///
// TODO(v2): revisit `cost_usd` precision. f64 is fine while individual call
// costs are in the $0.0001–$1 range (well within f64 precision), but if we
// start aggregating millions of rows for billing we should switch to
// `sqlx::types::BigDecimal` and add the `bigdecimal` feature to Cargo.toml.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct LlmCallRow {
    pub id: Uuid,
    pub tenant_id: String,
    pub company_id: Option<Uuid>,
    pub correlation_id: String,
    pub agent_name: String,
    pub prompt_name: String,
    pub prompt_version: i32,
    pub model: String,
    pub input_tokens: i32,
    pub output_tokens: i32,
    pub cost_usd: f64,
    pub latency_ms: i32,
    pub schema_retry: bool,
    pub succeeded: bool,
    pub error: Option<String>,
    pub occurred_at: DateTime<Utc>,
}
