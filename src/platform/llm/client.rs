//! Provider-agnostic LLM request/response types and the [`LlmClient`] trait.
//!
//! Concrete providers live in sibling modules (`gemini`, `anthropic`). The
//! trait is intentionally minimal — single-shot completion only. Streaming,
//! tools, and multi-turn chat are deliberately out of scope for Sprint 1;
//! Apollo's agents are batch jobs that produce one structured artifact per
//! invocation.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// A single completion request. `model` is the provider-specific model
/// identifier (e.g. `gemini-2.5-flash`, `claude-haiku-4-5`). The caller is
/// responsible for picking it per ADR-010.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmRequest {
    /// Provider-specific model identifier.
    pub model: String,
    /// Optional system prompt. Anthropic uses `system`; Gemini maps this
    /// to `systemInstruction`.
    pub system: Option<String>,
    /// User message. We do not support multi-turn here.
    pub user: String,
    /// Hard cap on output tokens. Providers that allow `0` to mean "unset"
    /// still receive this value verbatim — pick something sensible.
    pub max_output_tokens: u32,
    /// Sampling temperature.
    pub temperature: f32,
    /// Optional response-format hint. Providers that support JSON mode
    /// (Gemini's `responseMimeType`) will honour this; others ignore it.
    #[serde(default)]
    pub json_response: bool,
}

/// A completion response. Token counts and cost are populated from the
/// provider's usage metadata; `cost_usd` is computed via
/// [`super::cost::cost_usd_per_call`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmResponse {
    /// The generated text (concatenated content parts).
    pub text: String,
    /// Prompt token count reported by the provider, or `0` if missing.
    pub input_tokens: u32,
    /// Completion token count reported by the provider, or `0` if missing.
    pub output_tokens: u32,
    /// Computed USD cost for this call.
    pub cost_usd: f64,
    /// Wall-clock latency measured client-side, in milliseconds.
    pub latency_ms: u32,
    /// Echo of the model id used. Useful when the request used an alias.
    pub model: String,
}

/// Errors returned by [`LlmClient::complete`].
///
/// Variants are intentionally coarse: the calling code only needs to know
/// "retryable rate limit", "retryable server error", or "fatal". The raw
/// status + body is preserved in [`LlmError::Server`] / [`LlmError::BadRequest`]
/// for the cost ledger's `error` column.
#[derive(Debug, Error)]
pub enum LlmError {
    /// Transport-level failure (DNS, connection, body read, ...).
    #[error("http: {0}")]
    Http(#[from] reqwest::Error),
    /// HTTP 429. Caller should back off and retry.
    #[error("rate limited (429)")]
    RateLimited,
    /// HTTP 5xx. Caller may retry once.
    #[error("server error ({status}): {body}")]
    Server {
        /// HTTP status code.
        status: u16,
        /// Raw response body (truncated by the server in practice).
        body: String,
    },
    /// HTTP 4xx other than 429 — usually a prompt/quota bug. Do not retry.
    #[error("bad request ({status}): {body}")]
    BadRequest {
        /// HTTP status code.
        status: u16,
        /// Raw response body.
        body: String,
    },
    /// Response decoded but did not match the structured schema the caller
    /// expected. Not produced by the transport itself — kept here so all
    /// LLM-related errors live in one enum.
    #[error("schema mismatch: {0}")]
    SchemaMismatch(String),
    /// Constructor was asked to read an env var that wasn't set.
    #[error("missing api key for {provider}")]
    MissingKey {
        /// Provider name, e.g. `"gemini"` or `"anthropic"`.
        provider: &'static str,
    },
}

/// A single-shot LLM completion client. Implementations are `Send + Sync`
/// so they can be stored in `Arc<dyn LlmClient>` and shared across tasks.
#[async_trait]
pub trait LlmClient: Send + Sync {
    /// Issue one completion. Implementations measure latency client-side
    /// and populate `cost_usd` via [`super::cost::cost_usd_per_call`].
    async fn complete(&self, req: LlmRequest) -> Result<LlmResponse, LlmError>;
}
