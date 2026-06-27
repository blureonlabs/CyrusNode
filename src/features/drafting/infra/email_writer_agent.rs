//! Email writer agent — implements `DraftPort`.
//!
//! TODO: real LLM call with the `email-writer` prompt + schema validation
//! (later sprint story).

use async_trait::async_trait;

use crate::features::drafting::domain::{DraftPort, EmailDraft};
use crate::platform::core::CompanyId;

pub struct EmailWriterAgent;

#[async_trait]
impl DraftPort for EmailWriterAgent {
    async fn draft_email(&self, _company_id: CompanyId) -> anyhow::Result<EmailDraft> {
        // TODO: real LLM call.
        Ok(EmailDraft {
            subject: String::new(),
            body: String::new(),
        })
    }
}
