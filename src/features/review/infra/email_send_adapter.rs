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

/// Dry-run adapter. Saves what would be sent as an RFC 822 `.eml` file under
/// `out/outbox/dry-run-<timestamp>.eml` so the operator can drag-and-drop it
/// into Gmail / Mail.app later. Also logs to stderr for visibility.
///
/// Used when `RESEND_API_KEY` is unset so the queue workflow still runs in
/// dev. CLI integrators can swap this for [`EmailSendAdapter`] when ready.
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
        let ts = chrono::Utc::now().format("%Y%m%dT%H%M%S").to_string();
        let id = format!("dry-run-{ts}");
        let out_dir = std::path::PathBuf::from("out/outbox");
        tokio::fs::create_dir_all(&out_dir).await?;
        let path = out_dir.join(format!("{id}.eml"));
        // Minimal RFC 822 envelope. Good enough for Gmail's "Forward as attachment"
        // and for `mail` / `mutt` to read.
        let eml = format!(
            "From: {from}\r\nTo: {to}\r\nSubject: {subject}\r\nDate: {date}\r\nContent-Type: text/plain; charset=utf-8\r\n\r\n{body}\r\n",
            date = chrono::Utc::now().to_rfc2822(),
        );
        tokio::fs::write(&path, eml).await?;
        eprintln!(
            "(dry-run) wrote {} — drag into Gmail or paste body",
            path.display()
        );
        Ok(id)
    }
}
