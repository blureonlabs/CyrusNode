//! Dossier domain types. Pure data + invariants.

use crate::platform::core::CompanyId;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MaturityTier {
    Early,
    Growing,
    Established,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Evidence {
    /// Short claim the LLM is asserting.
    pub claim: String,
    /// URL of the page the evidence came from.
    pub page_url: Option<String>,
    /// Verbatim excerpt that supports the claim.
    pub excerpt: Option<String>,
}

/// The compact business summary produced by the BizIntel agent.
/// Shape mirrors `apollo/04-prompts/schemas/business-summary.v1.json` (S1-T11).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BusinessSummary {
    pub what_they_sell: String,
    pub who_they_serve: String,
    pub value_props: Vec<String>,
    pub customer_segments: Vec<String>,
    pub maturity_tier: MaturityTier,
    pub evidence: Vec<Evidence>,
    pub confidence: f32,
}

/// Everything we know about a company at the end of the research phase,
/// before opportunities and ROI are layered on.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DossierBase {
    pub company_id: CompanyId,
    pub summary: BusinessSummary,
}
