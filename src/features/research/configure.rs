//! Wiring file for the research feature.
//!
//! Single matchmaker for the four boxes (`domain`, `application`, `infra`,
//! `presentation`). Builds adapters from cross-cutting handles supplied by
//! the caller (typically `bootstrap.rs` or the CLI).

use std::sync::Arc;

use crate::features::research::application::ResearchCompany;
use crate::features::research::domain::{BizIntelPort, CrawlPort, ExtractPort, SeoPort};
use crate::features::research::infra::{
    BizIntelAgent, HttpCrawlerAdapter, MarkdownExtractorAdapter, StaticSeoAuditor,
};
use crate::platform::crawler::{CrawlerConfig, HttpCrawler};
use crate::platform::events::EventSubscription;
use crate::platform::llm::LlmClient;
use crate::platform::prompts::PromptLoader;

/// Cross-cutting handles required to construct the research feature.
#[derive(Clone)]
pub struct ResearchDeps {
    pub llm: Arc<dyn LlmClient>,
    pub prompts: PromptLoader,
}

pub struct ResearchModule {
    pub routes: axum::Router<()>,
    pub subscriptions: Vec<EventSubscription>,
    pub research_company: Arc<ResearchCompany>,
}

/// Wire the research feature. Boot-time `expect` is acceptable per CLAUDE.md §3.
pub fn configure(deps: ResearchDeps) -> ResearchModule {
    let crawler = Arc::new(HttpCrawler::new(CrawlerConfig::default()).expect("crawler init"));
    let crawl: Arc<dyn CrawlPort> = Arc::new(HttpCrawlerAdapter::new(crawler));
    let extract: Arc<dyn ExtractPort> = Arc::new(MarkdownExtractorAdapter);
    let seo: Arc<dyn SeoPort> = Arc::new(StaticSeoAuditor);
    let bizintel: Arc<dyn BizIntelPort> =
        Arc::new(BizIntelAgent::new(deps.llm.clone(), deps.prompts.clone()));

    let research_company = Arc::new(ResearchCompany::new(crawl, extract, seo, bizintel));

    ResearchModule {
        routes: super::presentation::routes(),
        subscriptions: super::presentation::subscriptions(),
        research_company,
    }
}
