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

const ANTHROPIC_VERSION: &str = "2023-06-01";

/// Anthropic Messages client. Cheap to clone.
#[derive(Clone)]
pub struct AnthropicClient {
    http: Client,
    api_key: String,
    base_url: String,
}

impl AnthropicClient {
    /// Build a client with an explicit API key.
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            http: Client::new(),
            api_key: api_key.into(),
            base_url: "https://api.anthropic.com".to_string(),
        }
    }

    /// Build a client by reading `ANTHROPIC_API_KEY` from the environment.
    /// Returns [`LlmError::MissingKey`] if the variable is unset.
    pub fn from_env() -> Result<Self, LlmError> {
        let key = std::env::var("ANTHROPIC_API_KEY").map_err(|_| LlmError::MissingKey {
            provider: "anthropic",
        })?;
        Ok(Self::new(key))
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
        // Concatenate all text blocks. In practice there is one, but the
        // schema is a list so we join defensively.
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
            cost_usd: cost_usd_per_call(&req.model, input_tokens, output_tokens),
            latency_ms: started.elapsed().as_millis() as u32,
            model: req.model,
        })
    }
}
