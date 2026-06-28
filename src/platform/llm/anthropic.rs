//! Anthropic Messages API client.
//!
//! Endpoint: `POST https://api.anthropic.com/v1/messages`
//! Auth: `X-API-Key` header.
//! Version: pinned to `anthropic-version: 2023-06-01` (the GA messages
//! schema). When Anthropic ships a breaking change we'll bump this
//! constant deliberately, not silently.
//!
//! Used per ADR-010 for:
//! - Claude Haiku 4.5 → cold copy (email/whatsapp)
//! - Claude Sonnet 4.6 → heavy reasoning (proposal, meeting coach)

use std::time::Instant;

use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};

use super::client::{LlmClient, LlmError, LlmRequest, LlmResponse};
use super::cost::cost_usd_per_call;
use super::ledger::{LedgerWriter, LlmCallRecord};

const ANTHROPIC_VERSION: &str = "2023-06-01";

/// Default path used by [`AnthropicClient::from_env`] when no explicit
/// ledger is configured. CLAUDE.md §6 — every LLM call records its cost.
const DEFAULT_LEDGER_PATH: &str = "out/llm_calls.jsonl";

/// Anthropic Messages client. Cheap to clone.
#[derive(Clone)]
pub struct AnthropicClient {
    http: Client,
    api_key: String,
    base_url: String,
    ledger: Option<LedgerWriter>,
}

impl AnthropicClient {
    /// Build a client with an explicit API key.
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            http: Client::new(),
            api_key: api_key.into(),
            base_url: "https://api.anthropic.com".to_string(),
            ledger: None,
        }
    }

    /// Build a client by reading `ANTHROPIC_API_KEY` from the environment.
    /// Returns [`LlmError::MissingKey`] if the variable is unset.
    ///
    /// Also opens the default ledger at `out/llm_calls.jsonl`. If opening
    /// the ledger fails we log a warning and continue with no ledger —
    /// never block an LLM call on disk.
    pub fn from_env() -> Result<Self, LlmError> {
        let key = std::env::var("ANTHROPIC_API_KEY").map_err(|_| LlmError::MissingKey {
            provider: "anthropic",
        })?;
        let mut client = Self::new(key);
        match LedgerWriter::open_blocking(DEFAULT_LEDGER_PATH) {
            Ok(ledger) => client.ledger = Some(ledger),
            Err(e) => {
                tracing::warn!(
                    error = %e,
                    path = DEFAULT_LEDGER_PATH,
                    "could not open cost ledger; continuing without it"
                );
            }
        }
        Ok(client)
    }

    /// Builder — attach a custom ledger writer.
    pub fn with_ledger(mut self, ledger: LedgerWriter) -> Self {
        self.ledger = Some(ledger);
        self
    }

    /// Best-effort ledger write. Failures log a warning but never bubble.
    async fn write_ledger(&self, rec: LlmCallRecord) {
        if let Some(l) = &self.ledger {
            if let Err(e) = l.record(&rec).await {
                tracing::warn!(error = %e, "failed to write llm cost ledger row");
            }
        }
    }
}

#[derive(Serialize)]
struct AnthropicRequest<'a> {
    model: &'a str,
    max_tokens: u32,
    temperature: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    system: Option<&'a str>,
    messages: Vec<AnthropicMessage<'a>>,
}

#[derive(Serialize)]
struct AnthropicMessage<'a> {
    role: &'a str,
    content: &'a str,
}

#[derive(Deserialize)]
struct AnthropicResponse {
    #[serde(default)]
    content: Vec<AnthropicContentBlock>,
    usage: Option<AnthropicUsage>,
}

#[derive(Deserialize)]
struct AnthropicContentBlock {
    #[serde(default, rename = "type")]
    _kind: String,
    #[serde(default)]
    text: String,
}

#[derive(Deserialize)]
struct AnthropicUsage {
    #[serde(default)]
    input_tokens: u32,
    #[serde(default)]
    output_tokens: u32,
}

#[async_trait]
impl LlmClient for AnthropicClient {
    async fn complete(&self, req: LlmRequest) -> Result<LlmResponse, LlmError> {
        let started = Instant::now();
        // Capture attribution up-front because `req` is consumed below.
        let agent_name = req.agent_name.clone();
        let prompt_name = req.prompt_name.clone();
        let prompt_version = req.prompt_version;
        let company_id = req.company_id.clone();
        let model_for_ledger = req.model.clone();

        let url = format!("{}/v1/messages", self.base_url);

        let body = AnthropicRequest {
            model: &req.model,
            max_tokens: req.max_output_tokens,
            temperature: req.temperature,
            system: req.system.as_deref(),
            messages: vec![AnthropicMessage {
                role: "user",
                content: &req.user,
            }],
        };

        // Note: `json_response` is currently a no-op for Anthropic. The
        // Messages API does not (as of pinned version 2023-06-01) accept a
        // top-level `response_format`. We rely on prompt-level instruction
        // for JSON-shaped output and validate downstream.

        let result: Result<LlmResponse, LlmError> = async {
            let resp = self
                .http
                .post(&url)
                .header("x-api-key", &self.api_key)
                .header("anthropic-version", ANTHROPIC_VERSION)
                .header("content-type", "application/json")
                .json(&body)
                .send()
                .await?;

            let status = resp.status();
            if status.as_u16() == 429 {
                return Err(LlmError::RateLimited);
            }
            if status.is_server_error() {
                return Err(LlmError::Server {
                    status: status.as_u16(),
                    body: resp.text().await.unwrap_or_default(),
                });
            }
            if !status.is_success() {
                return Err(LlmError::BadRequest {
                    status: status.as_u16(),
                    body: resp.text().await.unwrap_or_default(),
                });
            }

            let parsed: AnthropicResponse = resp.json().await?;
            // Concatenate all text blocks. In practice there is one, but
            // the schema is a list so we join defensively.
            let text = parsed
                .content
                .iter()
                .map(|b| b.text.as_str())
                .collect::<Vec<_>>()
                .join("");
            let (input_tokens, output_tokens) = match parsed.usage {
                Some(u) => (u.input_tokens, u.output_tokens),
                None => (0, 0),
            };

            Ok(LlmResponse {
                text,
                input_tokens,
                output_tokens,
                cost_usd: cost_usd_per_call(&model_for_ledger, input_tokens, output_tokens),
                latency_ms: started.elapsed().as_millis() as u32,
                model: model_for_ledger.clone(),
            })
        }
        .await;

        // Record exactly one ledger row for success or failure (CLAUDE.md §6).
        let rec = match &result {
            Ok(resp) => LlmCallRecord {
                occurred_at: chrono::Utc::now(),
                model: resp.model.clone(),
                agent_name,
                prompt_name,
                prompt_version,
                company_id,
                input_tokens: resp.input_tokens,
                output_tokens: resp.output_tokens,
                cost_usd: resp.cost_usd,
                latency_ms: resp.latency_ms,
                succeeded: true,
                error: None,
            },
            Err(err) => LlmCallRecord {
                occurred_at: chrono::Utc::now(),
                model: model_for_ledger,
                agent_name,
                prompt_name,
                prompt_version,
                company_id,
                input_tokens: 0,
                output_tokens: 0,
                cost_usd: 0.0,
                latency_ms: started.elapsed().as_millis() as u32,
                succeeded: false,
                error: Some(err.to_string()),
            },
        };
        self.write_ledger(rec).await;

        result
    }
}
