//! Provider-agnostic email sending trait and value types.
//!
//! Features depend on [`EmailSender`] rather than a concrete client so the
//! transport (Resend today, possibly SES or SMTP later) can be swapped without
//! touching feature code. Mirrors the shape of `platform::llm::client`.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// A single outbound email.
///
/// Constructed by the caller (drafting feature, outreach feature, etc.) and
/// handed to an [`EmailSender`]. The struct is provider-agnostic; each
/// implementation maps it to its own wire format.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmailMessage {
    /// RFC-5322 `From` value, e.g. `"Hari <hari@apollo.dev>"`.
    pub from: String,
    /// One or more recipient addresses.
    pub to: Vec<String>,
    /// Subject line.
    pub subject: String,
    /// Plaintext body. Always sent so clients without HTML support degrade
    /// gracefully.
    pub body_text: String,
    /// Optional HTML body. When present, providers send it as the preferred
    /// alternative alongside [`Self::body_text`].
    pub body_html: Option<String>,
    /// Optional `Reply-To` address.
    pub reply_to: Option<String>,
    /// Optional `Message-ID` we want set on the outbound mail. Resend ignores
    /// it for new threads but threading replies later may use it.
    pub in_reply_to: Option<String>,
}

/// Result of a successful send.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SentEmail {
    /// Provider message id (Resend returns a uuid).
    pub external_id: String,
}

/// Errors a send may produce.
#[derive(Debug, Error)]
pub enum EmailError {
    /// API credentials for the named provider are not configured.
    #[error("missing api key for {0}")]
    MissingKey(&'static str),
    /// Provider returned HTTP 429. Caller should back off.
    #[error("rate limited (429)")]
    RateLimited,
    /// Any other non-2xx response from the provider.
    #[error("upstream {status}: {body}")]
    Upstream {
        /// HTTP status code.
        status: u16,
        /// Raw response body, useful for diagnostics and logging.
        body: String,
    },
    /// Transport-level failure (DNS, TLS, timeout, malformed JSON, etc).
    #[error(transparent)]
    Network(#[from] reqwest::Error),
}

/// Sends an [`EmailMessage`] via some upstream provider.
#[async_trait]
pub trait EmailSender: Send + Sync {
    /// Deliver `msg`. On success returns the provider's message id wrapped in
    /// [`SentEmail`].
    async fn send(&self, msg: &EmailMessage) -> Result<SentEmail, EmailError>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn email_message_serializes_to_expected_shape() {
        let msg = EmailMessage {
            from: "Hari <hari@apollo.dev>".to_string(),
            to: vec!["a@example.com".to_string(), "b@example.com".to_string()],
            subject: "hello".to_string(),
            body_text: "plain body".to_string(),
            body_html: Some("<p>plain body</p>".to_string()),
            reply_to: Some("hari@apollo.dev".to_string()),
            in_reply_to: Some("<abc@apollo.dev>".to_string()),
        };

        let value = serde_json::to_value(&msg).expect("serialize");
        assert_eq!(
            value,
            json!({
                "from": "Hari <hari@apollo.dev>",
                "to": ["a@example.com", "b@example.com"],
                "subject": "hello",
                "body_text": "plain body",
                "body_html": "<p>plain body</p>",
                "reply_to": "hari@apollo.dev",
                "in_reply_to": "<abc@apollo.dev>",
            })
        );
    }

    #[test]
    fn email_message_optional_fields_are_null_when_absent() {
        let msg = EmailMessage {
            from: "Hari <hari@apollo.dev>".to_string(),
            to: vec!["a@example.com".to_string()],
            subject: "hello".to_string(),
            body_text: "plain body".to_string(),
            body_html: None,
            reply_to: None,
            in_reply_to: None,
        };

        let value = serde_json::to_value(&msg).expect("serialize");
        assert_eq!(value["body_html"], serde_json::Value::Null);
        assert_eq!(value["reply_to"], serde_json::Value::Null);
        assert_eq!(value["in_reply_to"], serde_json::Value::Null);
    }
}
