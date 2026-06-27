//! SEO/performance runner — implements `SeoPort` via Lighthouse.
//!
//! TODO(S1-T10): drive headless Chromium + Lighthouse and parse the report.

use async_trait::async_trait;

use crate::features::research::domain::{SeoPort, SeoReport};
use crate::platform::core::CompanyId;

pub struct LighthouseSeoAdapter;

#[async_trait]
impl SeoPort for LighthouseSeoAdapter {
    async fn audit(&self, _company_id: CompanyId) -> anyhow::Result<SeoReport> {
        // TODO(S1-T10): real Lighthouse run.
        Ok(SeoReport {
            performance: 0,
            seo: 0,
        })
    }
}
