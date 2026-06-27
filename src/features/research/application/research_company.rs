use std::sync::Arc;

use crate::features::research::domain::{
    BizIntelPort, CrawlPort, DossierBase, ExtractPort, SeoPort,
};
use crate::platform::core::CompanyId;

/// Use case: take a company URL, run the research phase, produce a dossier base.
///
/// Stub. Logic lands across S1-T08..T11. Wiring is in `infra/` adapters; this
/// orchestrator only talks to ports.
pub struct ResearchCompany {
    crawl: Arc<dyn CrawlPort>,
    extract: Arc<dyn ExtractPort>,
    seo: Arc<dyn SeoPort>,
    bizintel: Arc<dyn BizIntelPort>,
}

impl ResearchCompany {
    pub fn new(
        crawl: Arc<dyn CrawlPort>,
        extract: Arc<dyn ExtractPort>,
        seo: Arc<dyn SeoPort>,
        bizintel: Arc<dyn BizIntelPort>,
    ) -> Self {
        Self {
            crawl,
            extract,
            seo,
            bizintel,
        }
    }

    pub async fn run(&self, company_id: CompanyId, url: &str) -> anyhow::Result<DossierBase> {
        let _ = self.crawl.fetch(company_id, url).await?;
        let _ = self.extract.extract(company_id).await?;
        let _ = self.seo.audit(company_id).await?;
        let summary = self.bizintel.summarize(company_id).await?;
        Ok(DossierBase {
            company_id,
            summary,
        })
    }
}
