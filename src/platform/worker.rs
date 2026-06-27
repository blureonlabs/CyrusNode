//! Worker loop — claim jobs, dispatch to registered agents, heartbeat, reap.
//!
//! Wired in S1-T03. Uses `JobsRepo::claim_next` (SKIP LOCKED) and the
//! `process_one_job` wrapper for the actual cache → run → persist flow.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use tokio::time;

use crate::platform::db::repos::JobsRepo;
use crate::platform::db::PgPool;
use crate::platform::queue::{process_one_job, ErasedAgent};

/// Holds the agents the worker can dispatch to, keyed by `Agent::NAME`.
#[derive(Default, Clone)]
pub struct AgentRegistry {
    by_name: HashMap<&'static str, Arc<dyn ErasedAgent>>,
}

impl AgentRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register an agent under its `name()`.
    pub fn install<A: ErasedAgent + 'static>(&mut self, agent: Arc<A>) {
        self.by_name.insert(agent.name(), agent);
    }

    /// Look up an agent by name, cloning the `Arc` if present.
    pub fn get(&self, name: &str) -> Option<Arc<dyn ErasedAgent>> {
        self.by_name.get(name).cloned()
    }
}

/// Configuration for the worker poll loop.
#[derive(Debug, Clone)]
pub struct WorkerConfig {
    /// Max jobs to claim per poll tick.
    pub claim_batch_size: i64,
    /// Sleep duration when no jobs were claimed.
    pub poll_interval: Duration,
    /// How often the reaper task reclaims expired leases.
    pub reap_interval: Duration,
}

impl Default for WorkerConfig {
    fn default() -> Self {
        Self {
            claim_batch_size: 4,
            poll_interval: Duration::from_secs(5),
            reap_interval: Duration::from_secs(30),
        }
    }
}

/// Run the worker. Returns only on fatal error.
pub async fn run(pool: PgPool, registry: AgentRegistry, cfg: WorkerConfig) -> Result<()> {
    let jobs = JobsRepo::new(pool.clone());
    spawn_reaper(pool.clone(), cfg.reap_interval);

    tracing::info!(
        agents = registry.by_name.len(),
        batch = cfg.claim_batch_size,
        "worker poll loop started"
    );

    loop {
        let claimed = match jobs.claim_next(cfg.claim_batch_size).await {
            Ok(v) => v,
            Err(e) => {
                tracing::error!(error = %e, "claim_next failed");
                time::sleep(cfg.poll_interval).await;
                continue;
            }
        };

        if claimed.is_empty() {
            time::sleep(cfg.poll_interval).await;
            continue;
        }

        for job in claimed {
            let Some(agent) = registry.get(&job.agent_name) else {
                tracing::warn!(
                    job_id = %job.id,
                    agent = %job.agent_name,
                    "no registered agent for job — marking failed"
                );
                if let Err(e) = jobs.fail(job.id, "no registered agent").await {
                    tracing::error!(error = %e, job_id = %job.id, "fail() errored");
                }
                continue;
            };

            let pool_clone = pool.clone();
            tokio::spawn(async move {
                match process_one_job(pool_clone, job, agent).await {
                    Ok(outcome) => tracing::debug!(?outcome, "job done"),
                    Err(e) => tracing::error!(error = %e, "process_one_job errored"),
                }
            });
        }
    }
}

/// Background task: every `interval`, reclaim stale leases.
fn spawn_reaper(pool: PgPool, interval: Duration) {
    let jobs = JobsRepo::new(pool);
    tokio::spawn(async move {
        let mut tick = time::interval(interval);
        loop {
            tick.tick().await;
            match jobs.reap_expired().await {
                Ok(n) if n > 0 => tracing::info!(reaped = n, "expired leases reclaimed"),
                Ok(_) => {}
                Err(e) => tracing::warn!(error = %e, "reaper failed"),
            }
        }
    });
}
