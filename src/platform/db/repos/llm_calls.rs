//! LLM calls repository — append-only cost/latency ledger.
//!
//! Required by CLAUDE.md §6 ("Cost is observable"): every LLM call writes
//! a row here, no exceptions. The materialized view `cost_per_company_daily`
//! aggregates from this table.

use sqlx::PgPool;
use uuid::Uuid;

use crate::platform::db::{DbError, LlmCallRow};

/// Input for recording a single LLM call.
///
/// `cost_usd` is bound as `f64` and cast in SQL via `$N::float8::numeric` so
/// SQLx does not need the `bigdecimal` or `rust_decimal` feature to encode
/// it. The `NUMERIC(10,6)` column then enforces the 6-decimal precision.
///
/// TODO: if precision ever bites us at the float boundary, switch to an
/// `i64` micro-dollars representation and the corresponding cast.
#[derive(Debug, Clone)]
pub struct NewLlmCall {
    pub tenant_id: Uuid,
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
}

/// Append-only access to the `llm_calls` table. Cheap to clone.
#[derive(Debug, Clone)]
pub struct LlmCallsRepo {
    pool: PgPool,
}

impl LlmCallsRepo {
    /// Construct a repo backed by the given pool.
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Record a single LLM call. Returns the inserted row (with the
    /// DB-stamped `id` and `occurred_at`).
    pub async fn record(&self, call: NewLlmCall) -> Result<LlmCallRow, DbError> {
        let row = sqlx::query_as::<_, LlmCallRow>(
            r#"
            INSERT INTO llm_calls (
                tenant_id, company_id, correlation_id, agent_name,
                prompt_name, prompt_version, model,
                input_tokens, output_tokens,
                cost_usd, latency_ms,
                schema_retry, succeeded, error
            )
            VALUES (
                $1, $2, $3, $4,
                $5, $6, $7,
                $8, $9,
                $10::float8::numeric, $11,
                $12, $13, $14
            )
            RETURNING *
            "#,
        )
        .bind(call.tenant_id)
        .bind(call.company_id)
        .bind(&call.correlation_id)
        .bind(&call.agent_name)
        .bind(&call.prompt_name)
        .bind(call.prompt_version)
        .bind(&call.model)
        .bind(call.input_tokens)
        .bind(call.output_tokens)
        .bind(call.cost_usd)
        .bind(call.latency_ms)
        .bind(call.schema_retry)
        .bind(call.succeeded)
        .bind(&call.error)
        .fetch_one(&self.pool)
        .await?;
        Ok(row)
    }
}
