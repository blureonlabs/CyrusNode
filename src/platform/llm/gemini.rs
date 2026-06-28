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
use super::ledger::{LedgerWriter, LlmCallRecord};

/// Default path used by [`GeminiClient::from_env`] when no explicit ledger
/// is configured. CLAUDE.md §6 — every LLM call records its cost.
const DEFAULT_LEDGER_PATH: &str = "out/llm_calls.jsonl";

/// Gemini REST client. Cheap to clone — wraps a `reqwest::Client`.
#[derive(Clone)]
pub struct GeminiClient {
    http: Client,
    api_key: String,
    base_url: String,
    ledger: Option<LedgerWriter>,
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
            ledger: None,
        }
    }

    /// Build a client by reading `GEMINI_API_KEY` from the environment.
    /// Returns [`LlmError::MissingKey`] if the variable is unset.
    ///
    /// Also opens the default ledger at `out/llm_calls.jsonl`. If opening
    /// the ledger fails (read-only fs, permission, ...) we log a warning
    /// and continue with no ledger — never block an LLM call on disk.
    pub fn from_env() -> Result<Self, LlmError> {
        let key = std::env::var("GEMINI_API_KEY")
            .map_err(|_| LlmError::MissingKey { provider: "gemini" })?;
        let mut client = Self::new(key);
        // Open the default ledger via the sync helper so this constructor
        // stays non-async. If opening fails (read-only fs, perms, ...) we
        // warn and continue with no ledger — never block an LLM call on disk.
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

    /// Builder — attach a custom ledger writer. Useful for tests and for
    /// the worker pipeline that wants to centralize the ledger lifecycle.
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
        // Capture attribution up-front because `req` is consumed below.
        let agent_name = req.agent_name.clone();
        let prompt_name = req.prompt_name.clone();
        let prompt_version = req.prompt_version;
        let company_id = req.company_id.clone();
        let model_for_ledger = req.model.clone();
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

        // Inner async block — lets us record the ledger row exactly once
        // for both the success and failure paths without duplicating code.
        let result: Result<LlmResponse, LlmError> = async {
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
        .await;

        // Record exactly one ledger row, regardless of success / failure.
        // CLAUDE.md §6: every LLM call records its cost.
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
