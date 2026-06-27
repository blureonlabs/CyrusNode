use std::sync::Arc;

use crate::features::research::domain::{
    BizIntelPort, CrawlPort, DossierBase, ExtractPort, SeoPort,
};
use crate::platform::core::CompanyId;

/// Use case: take a company URL, run the research phase, produce a dossier base.
///
/// Each step calls only its port. The adapters under `infra/` do the real work;
/// the application layer just sequences. Data is threaded through:
/// crawl → extract reads the crawl output, seo reads the same; bizintel reads
/// the extracted Markdown.
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
        let crawl = self.crawl.fetch(company_id, url).await?;
        let extracted = self.extract.extract(company_id, &crawl).await?;
        // SEO audit is best-effort — log on failure but don't fail the dossier.
        if let Err(e) = self.seo.audit(company_id, &crawl).await {
            tracing::warn!(error = %e, "seo audit failed; continuing without it");
        }
        let summary = self.bizintel.summarize(company_id, &extracted).await?;
        Ok(DossierBase {
            company_id,
            summary,
        })
    }
}
