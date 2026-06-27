//! Job queue + Agent trait + worker wrapper.
//!
//! The queue itself (claim/lease/heartbeat) lives in
//! `crate::platform::db::repos::jobs`. This module owns the *programming
//! model* a job runs under: the [`Agent`] trait, the [`AgentContext`] the
//! worker injects, and [`process_one_job`] — the single place that does
//! cache lookup, run, persist, publish, and error classification.
//!
//! Per `apollo/02-architecture/agent-architecture.md` §4 these concerns
//! live here and nowhere else.

mod agent;
mod worker;

pub use agent::{Agent, AgentContext, AgentError, AgentMeta, BackoffPolicy};
pub use worker::{process_one_job, ErasedAgent, ProcessOutcome};
