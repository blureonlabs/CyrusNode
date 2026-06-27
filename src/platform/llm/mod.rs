//! LLM client abstraction. Per-prompt model routing per ADR-010.
//!
//! - Structured agents → Gemini 2.5 Flash
//! - Cold copy (email/whatsapp) → Claude Haiku 4.5
//! - Heavy reasoning (proposal/meeting coach) → Claude Sonnet 4.6
//!
//! Every call records a row via [`crate::platform::db::repos::llm_calls`]
//! (see CLAUDE.md §6 "Cost is observable"). Clients here are the network
//! transport only; the recording is done by the caller because it needs
//! the tenant/company/agent context that lives outside this module.

mod anthropic;
mod client;
mod cost;
mod gemini;

pub use anthropic::AnthropicClient;
pub use client::{LlmClient, LlmError, LlmRequest, LlmResponse};
pub use cost::cost_usd_per_call;
pub use gemini::GeminiClient;
