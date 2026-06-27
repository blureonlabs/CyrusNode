//! Jobs repository — the queue.
//!
//! Backed by Postgres `SELECT ... FOR UPDATE SKIP LOCKED`. See
//! `apollo/02-architecture/queue-system.md` §3 for the claim semantics
//! and `database.md` §2 for the schema.

use serde_json::Value as JsonValue;
use sqlx::PgPool;
use uuid::Uuid;

use crate::platform::db::{DbError, JobRow};

/// Input for enqueueing a job. Uniqueness on
/// `(tenant_id, agent_name, agent_version, input_hash)` makes enqueue
/// idempotent.
#[derive(Debug, Clone)]
pub struct NewJob {
    pub tenant_id: Uuid,
    pub agent_name: String,
    pub agent_version: i32,
    pub company_id: Option<Uuid>,
    pub correlation_id: String,
    pub input: JsonValue,
    pub input_hash: String,
    pub priority: i32,
    pub max_attempts: i32,
}

/// Queue access for the `jobs` table. Cheap to clone.
#[derive(Debug, Clone)]
pub struct JobsRepo {
    pool: PgPool,
}

impl JobsRepo {
    /// Construct a repo backed by the given pool.
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Enqueue a job, or return the existing row if one already exists for
    /// the same `(tenant_id, agent_name, agent_version, input_hash)`.
    ///
    /// The `ON CONFLICT DO UPDATE` writes a no-op so `RETURNING *` fires
    /// for both insert and existing-row paths. The caller can compare the
    /// returned `correlation_id` to the one they supplied to detect a
    /// dedup hit.
    pub async fn enqueue(&self, job: NewJob) -> Result<JobRow, DbError> {
        let row = sqlx::query_as::<_, JobRow>(
            r#"
            INSERT INTO jobs (
                tenant_id, agent_name, agent_version, company_id,
                correlation_id, input, input_hash, priority, max_attempts
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            ON CONFLICT (tenant_id, agent_name, agent_version, input_hash) DO UPDATE
                SET correlation_id = jobs.correlation_id -- no-op to force RETURNING
            RETURNING *
            "#,
        )
        .bind(job.tenant_id)
        .bind(&job.agent_name)
        .bind(job.agent_version)
        .bind(job.company_id)
        .bind(&job.correlation_id)
        .bind(&job.input)
        .bind(&job.input_hash)
        .bind(job.priority)
        .bind(job.max_attempts)
        .fetch_one(&self.pool)
        .await?;
        Ok(row)
    }

    /// Atomically claim up to `limit` queued jobs whose `available_at` is in
    /// the past, mark them `running`, set a 5-minute lease, and bump
    /// `attempt_count`. Uses `FOR UPDATE SKIP LOCKED` so concurrent workers
    /// pick distinct rows. Returns the claimed `JobRow`s.
    pub async fn claim_next(&self, limit: i64) -> Result<Vec<JobRow>, DbError> {
        let rows = sqlx::query_as::<_, JobRow>(
            r#"
            WITH claimed AS (
                SELECT id
                FROM jobs
                WHERE status = 'queued' AND available_at <= now()
                ORDER BY priority ASC, available_at ASC
                LIMIT $1
                FOR UPDATE SKIP LOCKED
            )
            UPDATE jobs SET
                status        = 'running',
                leased_until  = now() + interval '5 minutes',
                started_at    = COALESCE(started_at, now()),
                attempt_count = attempt_count + 1
            FROM claimed
            WHERE jobs.id = claimed.id
            RETURNING jobs.*
            "#,
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Extend the lease by 5 minutes for a running job. No-op (no error)
    /// if the job is not currently `running` — the row simply isn't matched.
    pub async fn heartbeat(&self, id: Uuid) -> Result<(), DbError> {
        sqlx::query(
            r#"
            UPDATE jobs SET leased_until = now() + interval '5 minutes'
            WHERE id = $1 AND status = 'running'
            "#,
        )
        .bind(id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Mark a job as succeeded and stamp `finished_at`. Returns
    /// `DbError::NotFound` if no row matched.
    pub async fn succeed(&self, id: Uuid) -> Result<(), DbError> {
        let n =
            sqlx::query("UPDATE jobs SET status = 'succeeded', finished_at = now() WHERE id = $1")
                .bind(id)
                .execute(&self.pool)
                .await?
                .rows_affected();
        if n == 0 {
            return Err(DbError::NotFound);
        }
        Ok(())
    }

    /// Mark a job as failed (terminal), stamping `finished_at` and `last_error`.
    /// Returns `DbError::NotFound` if no row matched.
    pub async fn fail(&self, id: Uuid, error: &str) -> Result<(), DbError> {
        let n = sqlx::query(
            r#"
            UPDATE jobs SET status = 'failed', finished_at = now(), last_error = $2
            WHERE id = $1
            "#,
        )
        .bind(id)
        .bind(error)
        .execute(&self.pool)
        .await?
        .rows_affected();
        if n == 0 {
            return Err(DbError::NotFound);
        }
        Ok(())
    }

    /// Requeue a job with backoff: status back to `queued`, `available_at`
    /// pushed `backoff_secs` into the future, and `last_error` recorded.
    /// Returns `DbError::NotFound` if no row matched.
    pub async fn requeue(&self, id: Uuid, backoff_secs: i64, error: &str) -> Result<(), DbError> {
        let n = sqlx::query(
            r#"
            UPDATE jobs SET
                status       = 'queued',
                available_at = now() + make_interval(secs => $2),
                last_error   = $3
            WHERE id = $1
            "#,
        )
        .bind(id)
        .bind(backoff_secs)
        .bind(error)
        .execute(&self.pool)
        .await?
        .rows_affected();
        if n == 0 {
            return Err(DbError::NotFound);
        }
        Ok(())
    }

    /// Reclaim jobs whose lease expired. In one transaction:
    ///   1. Rows with attempts remaining go back to `queued`.
    ///   2. Rows that have exhausted `max_attempts` become `failed`.
    ///
    /// Returns the total number of reaped rows (sum of both branches).
    pub async fn reap_expired(&self) -> Result<u64, DbError> {
        let mut tx = self.pool.begin().await?;

        // 1) Retriable: attempts remaining → queue again immediately.
        let retried = sqlx::query(
            r#"
            UPDATE jobs SET
                status       = 'queued',
                leased_until = NULL,
                available_at = now()
            WHERE status IN ('leased', 'running')
              AND leased_until IS NOT NULL
              AND leased_until < now()
              AND attempt_count < max_attempts
            "#,
        )
        .execute(&mut *tx)
        .await?
        .rows_affected();

        // 2) Terminal: attempts maxed out → fail permanently.
        let failed = sqlx::query(
            r#"
            UPDATE jobs SET
                status      = 'failed',
                finished_at = now(),
                last_error  = COALESCE(last_error, 'lease expired with no attempts remaining')
            WHERE status IN ('leased', 'running')
              AND leased_until IS NOT NULL
              AND leased_until < now()
              AND attempt_count >= max_attempts
            "#,
        )
        .execute(&mut *tx)
        .await?
        .rows_affected();

        tx.commit().await?;
        Ok(retried + failed)
    }

    /// Count of jobs currently in `queued` status for the tenant.
    pub async fn count_queued(&self, tenant_id: Uuid) -> Result<i64, DbError> {
        let (n,): (i64,) = sqlx::query_as(
            "SELECT COUNT(*)::BIGINT FROM jobs WHERE tenant_id = $1 AND status = 'queued'",
        )
        .bind(tenant_id)
        .fetch_one(&self.pool)
        .await?;
        Ok(n)
    }
}
