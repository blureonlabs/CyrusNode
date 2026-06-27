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
    pub pages_fetched: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractedSite {
    pub total_words: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SeoReport {
    pub performance: u8,
    pub seo: u8,
}

#[async_trait]
pub trait CrawlPort: Send + Sync {
    async fn fetch(&self, company_id: CompanyId, url: &str) -> Result<CrawlResult, CrawlError>;
}

#[async_trait]
pub trait ExtractPort: Send + Sync {
    async fn extract(&self, company_id: CompanyId) -> anyhow::Result<ExtractedSite>;
}

#[async_trait]
pub trait SeoPort: Send + Sync {
    async fn audit(&self, company_id: CompanyId) -> anyhow::Result<SeoReport>;
}

#[async_trait]
pub trait BizIntelPort: Send + Sync {
    async fn summarize(&self, company_id: CompanyId) -> anyhow::Result<BusinessSummary>;
}
