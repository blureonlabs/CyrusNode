//! Ports — the traits drafting's `application` asks of the outside world.
//!
//! `infra/` implements these.

use async_trait::async_trait;

use super::EmailDraft;
use crate::platform::core::CompanyId;

#[async_trait]
pub trait DraftPort: Send + Sync {
    async fn draft_email(&self, company_id: CompanyId) -> anyhow::Result<EmailDraft>;
}
