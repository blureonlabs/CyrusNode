//! Dossier domain types. Pure data + invariants.

use super::ports::SiteFacts;
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
///
/// `site_facts` carries the deterministic extractor output (emails, phones,
/// social links, schema.org types). Downstream consumers — the operator CLI,
/// the review queue, and drafting agents — read these values directly rather
/// than re-scraping the markdown.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DossierBase {
    /// Stable identifier for the company this dossier describes.
    pub company_id: CompanyId,
    /// LLM-produced compact business summary.
    pub summary: BusinessSummary,
    /// Deterministic facts surfaced by the extractor (emails, phones, social,
    /// schema.org types). Empty `Default` when the extractor found nothing.
    pub site_facts: SiteFacts,
}
