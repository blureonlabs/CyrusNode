//! Gemini 2.5 client via the public REST API.
//!
//! Endpoint: `POST /v1beta/models/{model}:generateContent`
//! Auth: `x-goog-api-key` header (keeps the key out of URLs and access logs).
//! The alternate `Authorization: Bearer` flow requires OAuth and is not used.
//!
//! Used per ADR-010 for structured-output agents (Discovery, Qualifier,
//! Snapshot, etc.). JSON mode is requested via
//! `generationConfig.responseMimeType = "application/json"` when the
//! caller sets [`LlmRequest::json_response`].

use std::time::Instant;

use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};

use super::client::{LlmClient, LlmError, LlmRequest, LlmResponse};
use super::cost::cost_usd_per_call;

/// Gemini REST client. Cheap to clone — wraps a `reqwest::Client`.
#[derive(Clone)]
pub struct GeminiClient {
    http: Client,
    api_key: String,
    base_url: String,
}

impl GeminiClient {
    /// Build a client with an explicit API key. Useful for tests that
    /// point at a mock server (override `base_url` via the helper below
    /// once we add it; not needed in S1-T07).
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            http: Client::new(),
            api_key: api_key.into(),
            base_url: "https://generativelanguage.googleapis.com/v1beta".to_string(),
        }
    }

    /// Build a client by reading `GEMINI_API_KEY` from the environment.
    /// Returns [`LlmError::MissingKey`] if the variable is unset.
    pub fn from_env() -> Result<Self, LlmError> {
        let key = std::env::var("GEMINI_API_KEY")
            .map_err(|_| LlmError::MissingKey { provider: "gemini" })?;
        Ok(Self::new(key))
    }
}

#[derive(Serialize)]
struct GeminiRequest<'a> {
    contents: Vec<GeminiContent<'a>>,
    #[serde(rename = "systemInstruction", skip_serializing_if = "Option::is_none")]
    system_instruction: Option<GeminiContent<'a>>,
    #[serde(rename = "generationConfig")]
    generation_config: GeminiGenConfig,
}

#[derive(Serialize)]
struct GeminiContent<'a> {
    role: &'a str,
    parts: Vec<GeminiPart<'a>>,
}

#[derive(Serialize)]
struct GeminiPart<'a> {
    text: &'a str,
}

#[derive(Serialize)]
struct GeminiGenConfig {
    temperature: f32,
    #[serde(rename = "maxOutputTokens")]
    max_output_tokens: u32,
    #[serde(rename = "responseMimeType", skip_serializing_if = "Option::is_none")]
    response_mime_type: Option<String>,
}

#[derive(Deserialize)]
struct GeminiResponse {
    #[serde(default)]
    candidates: Vec<GeminiCandidate>,
    #[serde(rename = "usageMetadata")]
    usage_metadata: Option<GeminiUsage>,
}

#[derive(Deserialize)]
struct GeminiCandidate {
    content: GeminiContentResp,
}

#[derive(Deserialize)]
struct GeminiContentResp {
    #[serde(default)]
    parts: Vec<GeminiPartResp>,
}

#[derive(Deserialize)]
struct GeminiPartResp {
    #[serde(default)]
    text: String,
}

#[derive(Deserialize)]
struct GeminiUsage {
    #[serde(rename = "promptTokenCount", default)]
    prompt_tokens: u32,
    #[serde(rename = "candidatesTokenCount", default)]
    candidate_tokens: u32,
}

#[async_trait]
impl LlmClient for GeminiClient {
    async fn complete(&self, req: LlmRequest) -> Result<LlmResponse, LlmError> {
        let started = Instant::now();
        // Pass API key via `x-goog-api-key` header rather than the URL query
        // string so it doesn't leak into logs, error formatting, or any
        // intermediate proxy access logs.
        let url = format!("{}/models/{}:generateContent", self.base_url, req.model);

        let body = GeminiRequest {
            contents: vec![GeminiContent {
                role: "user",
                parts: vec![GeminiPart { text: &req.user }],
            }],
            system_instruction: req.system.as_deref().map(|s| GeminiContent {
                role: "system",
                parts: vec![GeminiPart { text: s }],
            }),
            generation_config: GeminiGenConfig {
                temperature: req.temperature,
                max_output_tokens: req.max_output_tokens,
                response_mime_type: req.json_response.then(|| "application/json".to_string()),
            },
        };

        let resp = self
            .http
            .post(&url)
            .header("x-goog-api-key", &self.api_key)
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

        let parsed: GeminiResponse = resp.json().await?;
        let text = parsed
            .candidates
            .first()
            .and_then(|c| c.content.parts.first())
            .map(|p| p.text.clone())
            .unwrap_or_default();
        let (input_tokens, output_tokens) = match parsed.usage_metadata {
            Some(u) => (u.prompt_tokens, u.candidate_tokens),
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
