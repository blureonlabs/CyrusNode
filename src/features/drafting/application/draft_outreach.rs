use std::sync::Arc;

use crate::features::drafting::domain::{DraftPort, EmailDraft};
use crate::platform::core::CompanyId;

/// Use case: produce an outreach email draft for a researched company.
///
/// Stub. Real personalization arrives in later sprint stories; this orchestrator
/// only talks to ports.
pub struct DraftOutreach {
    draft: Arc<dyn DraftPort>,
}

impl DraftOutreach {
    pub fn new(draft: Arc<dyn DraftPort>) -> Self {
        Self { draft }
    }

    pub async fn run(&self, company_id: CompanyId) -> anyhow::Result<EmailDraft> {
        self.draft.draft_email(company_id).await
    }
}
