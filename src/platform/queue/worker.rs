//! The worker wrapper — the single place that runs the cache → run →
//! persist → publish loop around an [`Agent`].
//!
//! Per `apollo/02-architecture/agent-architecture.md` §4, this is the only
//! module that knows about: artifact caching, input hashing (already done
//! at enqueue), job-state transitions, and backoff. Agents do none of that.

use std::sync::Arc;

use async_trait::async_trait;
use uuid::Uuid;

use crate::platform::core::{CompanyId, CorrelationId, TenantId};
use crate::platform::db::repos::{ArtifactsRepo, JobsRepo, NewArtifact};
use crate::platform::db::{DbError, JobRow, PgPool};

use super::agent::{Agent, AgentContext, AgentError, AgentMeta};

/// Outcome of [`process_one_job`]. The caller (the worker poll loop) uses
/// this for metrics — it does not need to re-derive job state from the DB.
#[derive(Debug, Clone)]
pub enum ProcessOutcome {
    /// Cache hit. Agent did not run; the job was marked succeeded.
    Cached,
    /// Agent ran successfully; artifact upserted; job marked succeeded.
    Succeeded,
    /// Agent failed terminally; job marked failed. Carries the error string.
    Failed(String),
    /// Agent failed transiently; job requeued with backoff.
    Requeued,
}

/// Object-safe view of [`Agent`] used by the worker.
///
/// The real [`Agent`] trait has associated `Input` / `Output` types, so it
/// cannot live behind a `dyn`. The blanket impl below bridges typed agents
/// to a JSON-in / JSON-out shape so a single registry + single worker loop
/// can drive any agent.
#[async_trait]
pub trait ErasedAgent: Send + Sync {
    /// The agent's stable name (matches [`Agent::NAME`]).
    fn name(&self) -> &'static str;

    /// The agent's behavior version (matches [`Agent::VERSION`]).
    fn version(&self) -> u32;

    /// Per-agent metadata (matches [`Agent::meta`]).
    fn meta(&self) -> AgentMeta;

    /// Execute the agent with a JSON input and return a JSON output.
    /// Deserialization / serialization failures are surfaced as
    /// [`AgentError::Terminal`] — bad input never becomes a retry.
    async fn run_erased(
        &self,
        ctx: &AgentContext,
        input: serde_json::Value,
    ) -> Result<serde_json::Value, AgentError>;
}

#[async_trait]
impl<A: Agent> ErasedAgent for A {
    fn name(&self) -> &'static str {
        A::NAME
    }

    fn version(&self) -> u32 {
        A::VERSION
    }

    fn meta(&self) -> AgentMeta {
        A::meta()
    }

    async fn run_erased(
        &self,
        ctx: &AgentContext,
        input: serde_json::Value,
    ) -> Result<serde_json::Value, AgentError> {
        let typed: A::Input =
            serde_json::from_value(input).map_err(|e| AgentError::Terminal(e.into()))?;
        let out = self.run(ctx, typed).await?;
        serde_json::to_value(out).map_err(|e| AgentError::Terminal(e.into()))
    }
}

/// Process one claimed job end-to-end.
///
/// Sequence (mirrors `agent-architecture.md` §4):
/// 1. Look up cached artifact by `(tenant_id, agent_name, version, input_hash)`.
///    On hit, mark the job succeeded and return [`ProcessOutcome::Cached`].
/// 2. Build an [`AgentContext`] from the job + pool.
/// 3. Call [`ErasedAgent::run_erased`].
/// 4. On success, upsert the artifact and mark the job succeeded.
/// 5. On [`AgentError::Transient`], requeue with exponential backoff (per
///    [`AgentMeta::backoff`]).
/// 6. On [`AgentError::Terminal`] or [`AgentError::Domain`], mark the job
///    failed.
///
/// Returns a [`DbError`] only when the database itself is unreachable —
/// agent failures are reported via [`ProcessOutcome`], not `Err`. This
/// keeps the worker poll loop's control flow about *infrastructure*
/// availability, not about *individual job* outcomes.
#[tracing::instrument(
    skip(pool, job, agent),
    fields(
        agent = %agent.name(),
        job_id = %job.id,
        correlation_id = %job.correlation_id,
    ),
)]
pub async fn process_one_job(
    pool: PgPool,
    job: JobRow,
    agent: Arc<dyn ErasedAgent>,
) -> Result<ProcessOutcome, DbError> {
    let jobs = JobsRepo::new(pool.clone());
    let artifacts = ArtifactsRepo::new(pool.clone());

    // Parse the tenant id once. The column is TEXT in V1 (see database.md
    // §2) but the artifact + agent surfaces are typed. A bad value here
    // means a corrupt row, not a bug to retry on — fail the job terminally.
    let tenant_uuid = match Uuid::parse_str(&job.tenant_id) {
        Ok(u) => u,
        Err(e) => {
            let msg = format!("invalid tenant_id on job row: {e}");
            jobs.fail(job.id, &msg).await?;
            return Ok(ProcessOutcome::Failed(msg));
        }
    };

    let agent_version_i32 = u32_to_i32_saturating(agent.version());

    // 1) Cache lookup.
    if let Some(_hit) = artifacts
        .get_cached(
            tenant_uuid,
            agent.name(),
            agent_version_i32,
            &job.input_hash,
        )
        .await?
    {
        tracing::debug!(
            agent = agent.name(),
            input_hash = %job.input_hash,
            "artifact cache hit; skipping run",
        );
        jobs.succeed(job.id).await?;
        return Ok(ProcessOutcome::Cached);
    }

    // 2) Build the context.
    let ctx = AgentContext {
        tenant_id: TenantId(tenant_uuid),
        correlation_id: CorrelationId(job.correlation_id.clone()),
        company_id: job.company_id.map(CompanyId),
        db: pool.clone(),
        cancel: tokio_util::sync::CancellationToken::new(),
    };

    // 3) Run.
    let run_result = agent.run_erased(&ctx, job.input.clone()).await;

    match run_result {
        Ok(output) => {
            // 4) Persist artifact, then mark succeeded.
            //
            // Artifacts require a `company_id` (the table column is NOT
            // NULL; see database.md §2). Jobs that do not target a company
            // skip the artifact write and only flip job state — caching
            // such jobs would also be meaningless without a stable owner.
            if let Some(company_id) = job.company_id {
                let new_artifact = NewArtifact {
                    tenant_id: tenant_uuid,
                    company_id,
                    agent_name: agent.name().to_string(),
                    agent_version: agent_version_i32,
                    input_hash: job.input_hash.clone(),
                    prompt_name: None,
                    prompt_version: None,
                    payload: output,
                    payload_schema_version: 1,
                };
                artifacts.upsert(new_artifact).await?;
            } else {
                tracing::debug!(
                    agent = agent.name(),
                    "no company_id on job; skipping artifact upsert",
                );
            }
            jobs.succeed(job.id).await?;
            Ok(ProcessOutcome::Succeeded)
        }
        Err(AgentError::Transient(err)) => {
            let attempts_so_far = job.attempt_count.max(0) as u32;
            if job.attempt_count >= job.max_attempts {
                let msg = format!("transient error after {attempts_so_far} attempts: {err:#}");
                jobs.fail(job.id, &msg).await?;
                return Ok(ProcessOutcome::Failed(msg));
            }
            // Per-agent backoff. `ErasedAgent::meta` defers to the typed
            // `Agent::meta()` so each agent gets to tune its own policy.
            let policy = agent.meta().backoff;
            let backoff_secs = compute_backoff_secs(&policy, attempts_so_far);
            let msg = format!("transient: {err:#}");
            jobs.requeue(job.id, backoff_secs, &msg).await?;
            Ok(ProcessOutcome::Requeued)
        }
        Err(AgentError::Terminal(err)) => {
            let msg = format!("terminal: {err:#}");
            jobs.fail(job.id, &msg).await?;
            Ok(ProcessOutcome::Failed(msg))
        }
        Err(AgentError::Domain(reason)) => {
            // Domain failures are terminal at the job level, but the
            // operator dashboard surfaces them as a distinct event. The
            // event emission lives with the event bus (S1-T06) — for now
            // we record the reason on the job row.
            let msg = format!("domain: {reason}");
            jobs.fail(job.id, &msg).await?;
            Ok(ProcessOutcome::Failed(msg))
        }
    }
}

/// `base * multiplier^attempt`, capped at `max_ms`, converted to whole
/// seconds for `JobsRepo::requeue`. Floors at one second so we never spin.
fn compute_backoff_secs(policy: &super::agent::BackoffPolicy, attempt: u32) -> i64 {
    let attempt_i32: i32 = attempt.min(i32::MAX as u32) as i32;
    let scaled = policy.base_ms as f64 * policy.multiplier.powi(attempt_i32);
    let capped = scaled.min(policy.max_ms as f64);
    let secs = (capped / 1000.0).round() as i64;
    secs.max(1)
}

/// `u32` → `i32` with saturation; `agent_version` columns are `INT`.
/// Versions in practice are tiny integers, but we still avoid `as`-truncation
/// to keep the rule "no surprising narrowing" honest.
fn u32_to_i32_saturating(v: u32) -> i32 {
    if v > i32::MAX as u32 {
        i32::MAX
    } else {
        v as i32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backoff_grows_then_caps() {
        let policy = super::super::agent::BackoffPolicy {
            base_ms: 1_000,
            max_ms: 10_000,
            multiplier: 2.0,
        };
        assert_eq!(compute_backoff_secs(&policy, 0), 1);
        assert_eq!(compute_backoff_secs(&policy, 1), 2);
        assert_eq!(compute_backoff_secs(&policy, 2), 4);
        // 1_000 * 2^10 = 1_024_000 → capped to 10_000ms → 10s
        assert_eq!(compute_backoff_secs(&policy, 10), 10);
    }

    #[test]
    fn backoff_floor_is_one_second() {
        let policy = super::super::agent::BackoffPolicy {
            base_ms: 10,
            max_ms: 10,
            multiplier: 2.0,
        };
        assert_eq!(compute_backoff_secs(&policy, 0), 1);
    }

    #[test]
    fn version_saturates_at_i32_max() {
        assert_eq!(u32_to_i32_saturating(0), 0);
        assert_eq!(u32_to_i32_saturating(123), 123);
        assert_eq!(u32_to_i32_saturating(u32::MAX), i32::MAX);
    }
}
