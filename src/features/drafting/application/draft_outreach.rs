use std::sync::Arc;

use crate::features::drafting::domain::{DraftInput, DraftPort, EmailDraft};
use crate::platform::core::CompanyId;

/// Use case: produce an outreach email draft for a researched company.
///
/// Takes a `DraftInput` constructed by the caller — typically the CLI or a
/// saga that has access to the research dossier. Drafting domain stays
/// research-agnostic per the no-cross-feature-imports rule.
pub struct DraftOutreach {
    draft: Arc<dyn DraftPort>,
}

impl DraftOutreach {
    pub fn new(draft: Arc<dyn DraftPort>) -> Self {
        Self { draft }
    }

    pub async fn run(
        &self,
        company_id: CompanyId,
        input: &DraftInput,
    ) -> anyhow::Result<EmailDraft> {
        self.draft.draft_email(company_id, input).await
    }
}
