//! HTTP crawler adapter — implements `CrawlPort`.
//!
//! TODO(S1-T08): wire up `crate::platform::crawler` to actually fetch pages,
//! respect the crawl budget, and persist results.

use async_trait::async_trait;

use crate::features::research::domain::{CrawlError, CrawlPort, CrawlResult};
use crate::platform::core::CompanyId;

pub struct HttpCrawlerAdapter;

#[async_trait]
impl CrawlPort for HttpCrawlerAdapter {
    async fn fetch(&self, _company_id: CompanyId, _url: &str) -> Result<CrawlResult, CrawlError> {
        // TODO(S1-T08): real fetch via `crate::platform::crawler`.
        Ok(CrawlResult { pages_fetched: 0 })
    }
}
