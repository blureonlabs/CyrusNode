//! Bridges the review feature's [`SendPort`] to the platform [`EmailSender`].
//!
//! Lives in review's infra layer because it implements a review-feature port.
//! Knows about `platform::email` (allowed — infra can use any platform module)
//! but the review domain has no idea this exists.

use std::sync::Arc;

use async_trait::async_trait;

use crate::features::review::domain::SendPort;
use crate::platform::email::{EmailMessage, EmailSender};

/// Real send adapter. Wraps a generic `EmailSender` (typically [`ResendClient`]
/// in production, an in-memory fake in tests).
///
/// [`ResendClient`]: crate::platform::email::ResendClient
pub struct EmailSendAdapter {
    sender: Arc<dyn EmailSender>,
}

impl EmailSendAdapter {
    pub fn new(sender: Arc<dyn EmailSender>) -> Self {
        Self { sender }
    }
}

#[async_trait]
impl SendPort for EmailSendAdapter {
    async fn send(
        &self,
        from: &str,
        to: &str,
        subject: &str,
        body: &str,
    ) -> anyhow::Result<String> {
        let msg = EmailMessage {
            from: from.to_string(),
            to: vec![to.to_string()],
            subject: subject.to_string(),
            body_text: body.to_string(),
            body_html: None,
            reply_to: None,
            in_reply_to: None,
        };
        let sent = self.sender.send(&msg).await?;
        Ok(sent.external_id)
    }
}

/// Dry-run adapter. Logs what would be sent and returns a synthetic id.
/// Used when `RESEND_API_KEY` is unset so the queue workflow still runs.
pub struct DryRunSendAdapter;

#[async_trait]
impl SendPort for DryRunSendAdapter {
    async fn send(
        &self,
        from: &str,
        to: &str,
        subject: &str,
        body: &str,
    ) -> anyhow::Result<String> {
        tracing::warn!("RESEND_API_KEY missing — dry-run send");
        println!();
        println!("┌── DRY RUN (no RESEND_API_KEY) ──────────────────────────────");
        println!("│ FROM    : {from}");
        println!("│ TO      : {to}");
        println!("│ SUBJECT : {subject}");
        println!("├────────────────────────────────────────────────────────────");
        for line in body.lines() {
            println!("│ {line}");
        }
        println!("└────────────────────────────────────────────────────────────");
        Ok(format!("dry-run-{}", chrono::Utc::now().timestamp()))
    }
}
