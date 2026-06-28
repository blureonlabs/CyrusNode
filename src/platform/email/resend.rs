//! Resend HTTP client implementing [`EmailSender`].
//!
//! Endpoint: `POST https://api.resend.com/emails`
//! Auth: `Authorization: Bearer <api_key>`
//!
//! Mirrors the reqwest pattern in `platform::llm::gemini`: a thin `Client`
//! wrapper with explicit-key and `from_env` constructors, and a single
//! request/response pair of typed structs.

use std::time::Duration;

use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};

use super::sender::{EmailError, EmailMessage, EmailSender, SentEmail};

/// Resend REST client. Cheap to clone — wraps a `reqwest::Client`.
#[derive(Clone)]
pub struct ResendClient {
    http: Client,
    api_key: String,
}

impl ResendClient {
    /// Build a client with an explicit API key.
    pub fn new(api_key: impl Into<String>) -> Self {
        let http = Client::builder()
            .timeout(Duration::from_secs(15))
            .user_agent("ApolloBot/0.1")
            .build()
            .expect("reqwest client");
        Self {
            http,
            api_key: api_key.into(),
        }
    }

    /// Build a client by reading `RESEND_API_KEY` from the environment.
    /// Returns [`EmailError::MissingKey`] if the variable is unset.
    pub fn from_env() -> Result<Self, EmailError> {
        let key = std::env::var("RESEND_API_KEY").map_err(|_| EmailError::MissingKey("resend"))?;
        Ok(Self::new(key))
    }
}

#[derive(Serialize)]
struct ResendRequest<'a> {
    from: &'a str,
    to: &'a [String],
    subject: &'a str,
    text: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    html: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    reply_to: Option<&'a str>,
}

#[derive(Deserialize)]
struct ResendResponse {
    id: String,
}

#[async_trait]
impl EmailSender for ResendClient {
    async fn send(&self, msg: &EmailMessage) -> Result<SentEmail, EmailError> {
        let body = ResendRequest {
            from: &msg.from,
            to: &msg.to,
            subject: &msg.subject,
            text: &msg.body_text,
            html: msg.body_html.as_deref(),
            reply_to: msg.reply_to.as_deref(),
        };

        let resp = self
            .http
            .post("https://api.resend.com/emails")
            .bearer_auth(&self.api_key)
            .json(&body)
            .send()
            .await?;

        let status = resp.status();
        if status.as_u16() == 429 {
            return Err(EmailError::RateLimited);
        }
        if !status.is_success() {
            return Err(EmailError::Upstream {
                status: status.as_u16(),
                body: resp.text().await.unwrap_or_default(),
            });
        }

        let parsed: ResendResponse = resp.json().await?;
        Ok(SentEmail {
            external_id: parsed.id,
        })
    }
}
