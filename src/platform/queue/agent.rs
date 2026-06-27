//! [`Agent`] trait + supporting types.
//!
//! One Agent trait. One context. One contract. Specialization is in the
//! prompt, not the framework. See
//! `apollo/02-architecture/agent-architecture.md`.

use async_trait::async_trait;
use serde::{de::DeserializeOwned, Serialize};
use thiserror::Error;

use crate::platform::core::{CompanyId, CorrelationId, TenantId};
use crate::platform::db::PgPool;

/// The single contract every agent implements.
///
/// Implementations live next to their feature (e.g. the crawler agent in
/// `features/research/infra/crawler_adapter.rs`). The framework concerns
/// (caching, persistence, retries, events) live in [`super::process_one_job`].
///
/// `Input` / `Output` are JSON-serializable and exchanged as
/// `serde_json::Value` through [`ErasedAgent`] so a single worker loop can
/// drive heterogeneous agents.
#[async_trait]
pub trait Agent: Send + Sync + 'static {
    /// Stable, dotted name — used as the cache key alongside [`Self::VERSION`].
    const NAME: &'static str;

    /// Behavior version. Bump when the agent's output for the same input
    /// would meaningfully change, so cached artifacts are not reused
    /// across versions.
    const VERSION: u32;

    /// Typed input. Decoded from `JobRow.input` by the worker.
    type Input: DeserializeOwned + Serialize + Send + Sync;

    /// Typed output. Encoded to JSON and stored as an artifact by the worker.
    type Output: Serialize + DeserializeOwned + Send + Sync;

    /// Per-agent declarative metadata: concurrency permits, timeout, cost
    /// ceiling, backoff policy, max attempts. Override to specialize.
    fn meta() -> AgentMeta {
        AgentMeta::default()
    }

    /// Execute the agent against the given input. Pure of platform
    /// concerns — no caching, no persistence, no event emission. Those
    /// happen in the worker wrapper.
    ///
    /// # Errors
    /// Return [`AgentError::Transient`] for retriable failures (5xx,
    /// timeouts, transient DB errors), [`AgentError::Terminal`] for
    /// guaranteed-to-fail-again (404, schema mismatch, bad input), and
    /// [`AgentError::Domain`] for operator-visible domain failures
    /// (e.g. the site blocked our scraper).
    async fn run(&self, ctx: &AgentContext, input: Self::Input)
        -> Result<Self::Output, AgentError>;
}

/// Carries platform handles into agents. Built once per job by the worker;
/// agents never construct one themselves.
///
/// This is the V1 minimum surface for S1-T05. As later sprints land the
/// LLM client, prompt loader, crawler, embedder, clock, and cost tracker
/// will be added here per `agent-architecture.md` §2.
pub struct AgentContext {
    /// Tenant the job belongs to.
    pub tenant_id: TenantId,
    /// Pipeline-wide correlation id used by tracing + events.
    pub correlation_id: CorrelationId,
    /// Company under research, if any. Some jobs are tenant-scoped only.
    pub company_id: Option<CompanyId>,
    /// Pooled Postgres handle. Cheap to clone; sqlx Arc-shares internally.
    pub db: PgPool,
    /// Cooperative cancellation. Agents should check at safe points.
    pub cancel: tokio_util::sync::CancellationToken,
}

/// Per-agent declarative metadata.
///
/// Defaults are deliberately conservative; agents override `meta()` to tune
/// concurrency, timeouts, and the cost ceiling.
#[derive(Debug, Clone)]
pub struct AgentMeta {
    /// Maximum number of concurrent runs of this agent across the worker
    /// pool. Enforced by the worker via a semaphore (not in S1-T05).
    pub concurrency_permits: usize,
    /// Hard cap on `agent.run` wall time, in milliseconds.
    pub timeout_ms: u64,
    /// Cost ceiling per run, in US cents. The cost tracker (later sprint)
    /// returns `Terminal` if exceeded.
    pub cost_ceiling_cents: u32,
    /// Backoff policy for [`AgentError::Transient`] failures.
    pub backoff: BackoffPolicy,
    /// Maximum attempts before a transient failure becomes terminal.
    pub max_attempts: i32,
}

impl Default for AgentMeta {
    fn default() -> Self {
        Self {
            concurrency_permits: 4,
            timeout_ms: 60_000,
            cost_ceiling_cents: 10,
            backoff: BackoffPolicy::default(),
            max_attempts: 3,
        }
    }
}

/// Exponential backoff policy for transient retries.
#[derive(Debug, Clone)]
pub struct BackoffPolicy {
    /// First delay, in milliseconds.
    pub base_ms: u64,
    /// Hard ceiling on the per-retry delay.
    pub max_ms: u64,
    /// Multiplier per attempt. `base * multiplier^attempt`.
    pub multiplier: f64,
}

impl Default for BackoffPolicy {
    fn default() -> Self {
        Self {
            base_ms: 2_000,
            max_ms: 300_000,
            multiplier: 3.0,
        }
    }
}

/// Error classification consumed by the worker wrapper.
///
/// The worker uses the variant to decide between requeue-with-backoff,
/// terminal-fail, and domain-event emission. See
/// `agent-architecture.md` §5.
#[derive(Debug, Error)]
pub enum AgentError {
    /// Retriable: HTTP 5xx, timeouts, transient DB errors, LLM 429.
    #[error("transient: {0}")]
    Transient(#[source] anyhow::Error),

    /// Will never succeed: HTTP 404, schema mismatch after retry, bad input.
    #[error("terminal: {0}")]
    Terminal(#[source] anyhow::Error),

    /// Operator-relevant input problem (e.g. site blocks scraping).
    /// Treated as terminal but surfaces a domain-specific event.
    #[error("domain: {0}")]
    Domain(String),
}
