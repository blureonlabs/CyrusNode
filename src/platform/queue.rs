//! Postgres-backed job queue (SKIP LOCKED + leases) and the `Agent` trait.
//!
//! Stub. `Agent` trait shape lands here in S1-T05; queue claim/lease in S1-T03.

use async_trait::async_trait;
use serde::{de::DeserializeOwned, Serialize};

/// The single contract every agent implements. Specialization lives in the prompt,
/// not the framework. See `apollo/02-architecture/agent-architecture.md`.
#[async_trait]
pub trait Agent: Send + Sync + 'static {
    const NAME: &'static str;
    const VERSION: u32;

    type Input: DeserializeOwned + Serialize + Send;
    type Output: Serialize + DeserializeOwned + Send;

    async fn run(&self, input: Self::Input) -> anyhow::Result<Self::Output>;
}
