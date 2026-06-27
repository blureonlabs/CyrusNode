//! `HttpCrawlerAdapter` — implements `CrawlPort` via `crate::platform::crawler`.
//!
//! Domain doesn't know about HTTP, robots, sitemaps, or disk paths; the
//! adapter translates between the rich `CrawlSummary` from platform and the
//! thin `CrawlResult` the domain wants.

use std::sync::Arc;

use async_trait::async_trait;

use crate::features::research::domain::{CrawlError, CrawlPort, CrawlResult};
use crate::platform::core::CompanyId;
use crate::platform::crawler::{CrawlerError, HttpCrawler};

/// Adapter wired into `ResearchCompany` via `configure.rs`.
#[derive(Clone)]
pub struct HttpCrawlerAdapter {
    crawler: Arc<HttpCrawler>,
}

impl HttpCrawlerAdapter {
    /// Build the adapter around a pre-configured `HttpCrawler`.
    pub fn new(crawler: Arc<HttpCrawler>) -> Self {
        Self { crawler }
    }
}

#[async_trait]
impl CrawlPort for HttpCrawlerAdapter {
    async fn fetch(&self, company_id: CompanyId, url: &str) -> Result<CrawlResult, CrawlError> {
        let correlation_id = company_id.0.to_string();
        let summary = self
            .crawler
            .crawl(url, &correlation_id)
            .await
            .map_err(|e| match e {
                CrawlerError::InvalidUrl(s) => CrawlError::Unreachable(s),
                CrawlerError::Unreachable(s) => CrawlError::Unreachable(s),
                CrawlerError::RobotsBlockedStart => CrawlError::Blocked("robots".to_string()),
                CrawlerError::Blocked(s) => CrawlError::Blocked(s),
                CrawlerError::Network(e) => CrawlError::Unreachable(e.to_string()),
                CrawlerError::Io(e) => CrawlError::Unreachable(e.to_string()),
            })?;
        tracing::info!(
            host = %summary.host,
            fetched = summary.fetched.len(),
            skipped = summary.skipped.len(),
            duration_ms = summary.duration_ms,
            "crawl complete"
        );
        // Derive the on-disk dir from the first fetched page — the platform
        // crawler keys it by correlation_id which we passed above.
        let output_dir = summary
            .fetched
            .first()
            .and_then(|p| p.html_path.parent().map(|p| p.to_path_buf()))
            .unwrap_or_else(|| {
                std::path::PathBuf::from("out/crawls").join(company_id.0.to_string())
            });
        let fetched_urls = summary
            .fetched
            .iter()
            .map(|p| p.url.clone())
            .collect::<Vec<_>>();
        Ok(CrawlResult {
            pages_fetched: summary.fetched.len() as u32,
            output_dir,
            fetched_urls,
        })
    }
}
