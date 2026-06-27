//! Business intelligence agent — implements `BizIntelPort`.
//!
//! TODO(S1-T11): call the LLM with the `business-summary` prompt and validate
//! output against `apollo/04-prompts/schemas/business-summary.v1.json`.

use async_trait::async_trait;

use crate::features::research::domain::{BizIntelPort, BusinessSummary};
use crate::platform::core::CompanyId;

pub struct BizIntelAgent;

#[async_trait]
impl BizIntelPort for BizIntelAgent {
    async fn summarize(&self, _company_id: CompanyId) -> anyhow::Result<BusinessSummary> {
        // TODO(S1-T11): real LLM call + schema validation.
        Ok(BusinessSummary {
            what_they_sell: String::new(),
            who_they_serve: String::new(),
            value_props: Vec::new(),
            confidence: 0.0,
        })
    }
}
