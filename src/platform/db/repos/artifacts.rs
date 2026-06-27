//! Artifacts repository — content-addressed cache of agent outputs.
//!
//! The unique constraint `(tenant_id, agent_name, agent_version, input_hash)`
//! is the cache key. `upsert` does `DO UPDATE` so a replay can correct a
//! bad payload without dropping the row (and its FK references).

use serde_json::Value as JsonValue;
use sqlx::PgPool;
use uuid::Uuid;

use crate::platform::db::{ArtifactRow, DbError};

/// Input for inserting an artifact. `bytes` is computed inside `upsert`
/// from the serialized JSON length — rough but sufficient for tracking
/// cache growth (see database.md §2).
#[derive(Debug, Clone)]
pub struct NewArtifact {
    pub tenant_id: Uuid,
    pub company_id: Uuid,
    pub agent_name: String,
    pub agent_version: i32,
    pub input_hash: String,
    pub prompt_name: Option<String>,
    pub prompt_version: Option<i32>,
    pub payload: JsonValue,
    pub payload_schema_version: i32,
}

/// Read/write access to the `artifacts` table. Cheap to clone.
#[derive(Debug, Clone)]
pub struct ArtifactsRepo {
    pool: PgPool,
}

impl ArtifactsRepo {
    /// Construct a repo backed by the given pool.
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Look up a cached artifact by its content-addressed key. Returns
    /// `Ok(None)` if no row matches (cache miss).
    pub async fn get_cached(
        &self,
        tenant_id: Uuid,
        agent_name: &str,
        agent_version: i32,
        input_hash: &str,
    ) -> Result<Option<ArtifactRow>, DbError> {
        let row = sqlx::query_as::<_, ArtifactRow>(
            r#"
            SELECT * FROM artifacts
            WHERE tenant_id = $1
              AND agent_name = $2
              AND agent_version = $3
              AND input_hash = $4
            "#,
        )
        .bind(tenant_id)
        .bind(agent_name)
        .bind(agent_version)
        .bind(input_hash)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row)
    }

    /// Insert an artifact, or overwrite an existing one with the same key.
    /// Replay-friendly: re-running an agent against the same `input_hash`
    /// updates `payload`, `payload_schema_version`, `prompt_name`,
    /// `prompt_version`, and `bytes`. `bytes` is `payload.to_string().len()`.
    pub async fn upsert(&self, a: NewArtifact) -> Result<ArtifactRow, DbError> {
        let bytes: i64 = a.payload.to_string().len() as i64;
        let row = sqlx::query_as::<_, ArtifactRow>(
            r#"
            INSERT INTO artifacts (
                tenant_id, company_id, agent_name, agent_version,
                input_hash, prompt_name, prompt_version,
                payload, payload_schema_version, bytes
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            ON CONFLICT (tenant_id, agent_name, agent_version, input_hash) DO UPDATE
                SET payload                = EXCLUDED.payload,
                    payload_schema_version = EXCLUDED.payload_schema_version,
                    prompt_name            = EXCLUDED.prompt_name,
                    prompt_version         = EXCLUDED.prompt_version,
                    bytes                  = EXCLUDED.bytes
            RETURNING *
            "#,
        )
        .bind(a.tenant_id)
        .bind(a.company_id)
        .bind(&a.agent_name)
        .bind(a.agent_version)
        .bind(&a.input_hash)
        .bind(&a.prompt_name)
        .bind(a.prompt_version)
        .bind(&a.payload)
        .bind(a.payload_schema_version)
        .bind(bytes)
        .fetch_one(&self.pool)
        .await?;
        Ok(row)
    }
}
