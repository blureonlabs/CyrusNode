//! Ports — the traits domain asks of the outside world.
//!
//! `application/` depends on these. `infra/` implements them.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::BusinessSummary;
use crate::platform::core::CompanyId;

#[derive(Debug, Error)]
pub enum CrawlError {
    #[error("unreachable: {0}")]
    Unreachable(String),
    #[error("blocked: {0}")]
    Blocked(String),
    #[error("budget exceeded")]
    BudgetExceeded,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrawlResult {
    /// Number of pages successfully fetched.
    pub pages_fetched: u32,
    /// On-disk directory containing the saved HTML files for this crawl.
    /// The `infra` extractor and SEO adapters read from here.
    pub output_dir: std::path::PathBuf,
    /// URLs that were successfully fetched, in fetch order.
    pub fetched_urls: Vec<String>,
}

/// Compact bag of structured facts the extractor surfaces alongside the raw
/// Markdown body.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SiteFacts {
    pub language: Option<String>,
    pub phones: Vec<String>,
    pub emails: Vec<String>,
    pub whatsapp_links: Vec<String>,
    pub social: SocialLinks,
    pub schema_org_types: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SocialLinks {
    pub instagram: Option<String>,
    pub facebook: Option<String>,
    pub linkedin: Option<String>,
    pub tiktok: Option<String>,
    pub youtube: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractedSite {
    /// Total visible word count across all extracted pages.
    pub total_words: u32,
    /// Concatenated Markdown body across the most signal-dense pages.
    pub markdown: String,
    /// Structured facts surfaced by deterministic regex/heuristic extractors.
    pub site_facts: SiteFacts,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Info,
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Finding {
    pub category: String,
    pub severity: Severity,
    pub title: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SeoReport {
    /// Rough performance proxy (0..100). Set to 0 when not measured —
    /// Lighthouse comes in S1-T10's optional headless path.
    pub performance: u8,
    /// Rough on-page SEO score (0..100) from deterministic checks.
    pub seo: u8,
    /// Discrete findings the operator can act on or that the
    /// Opportunity Finder can map to AI services.
    pub findings: Vec<Finding>,
}

#[async_trait]
pub trait CrawlPort: Send + Sync {
    async fn fetch(&self, company_id: CompanyId, url: &str) -> Result<CrawlResult, CrawlError>;
}

#[async_trait]
pub trait ExtractPort: Send + Sync {
    async fn extract(
        &self,
        company_id: CompanyId,
        crawl: &CrawlResult,
    ) -> anyhow::Result<ExtractedSite>;
}

#[async_trait]
pub trait SeoPort: Send + Sync {
    async fn audit(&self, company_id: CompanyId, crawl: &CrawlResult) -> anyhow::Result<SeoReport>;
}

#[async_trait]
pub trait BizIntelPort: Send + Sync {
    async fn summarize(
        &self,
        company_id: CompanyId,
        extracted: &ExtractedSite,
    ) -> anyhow::Result<BusinessSummary>;
}
