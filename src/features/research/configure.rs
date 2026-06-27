//! Wiring file for the research feature.
//!
//! This is the single file that knows about all four boxes (`domain`,
//! `application`, `infra`, `presentation`) for `research`. It builds the
//! adapters, hands them to the use case, and exposes the routes + event
//! subscriptions for `bootstrap.rs` to mount.

use std::sync::Arc;

use crate::features::research::application::ResearchCompany;
use crate::features::research::domain::{BizIntelPort, CrawlPort, ExtractPort, SeoPort};
use crate::features::research::infra::{
    BizIntelAgent, HttpCrawlerAdapter, LighthouseSeoAdapter, MarkdownExtractorAdapter,
};
use crate::platform::events::EventSubscription;

pub struct ResearchModule {
    pub routes: axum::Router<()>,
    pub subscriptions: Vec<EventSubscription>,
    pub research_company: Arc<ResearchCompany>,
}

/// Wires the four boxes for the research feature. Called once from `bootstrap.rs`.
pub fn configure() -> ResearchModule {
    let crawl: Arc<dyn CrawlPort> = Arc::new(HttpCrawlerAdapter);
    let extract: Arc<dyn ExtractPort> = Arc::new(MarkdownExtractorAdapter);
    let seo: Arc<dyn SeoPort> = Arc::new(LighthouseSeoAdapter);
    let bizintel: Arc<dyn BizIntelPort> = Arc::new(BizIntelAgent);

    let research_company = Arc::new(ResearchCompany::new(crawl, extract, seo, bizintel));

    ResearchModule {
        routes: super::presentation::routes(),
        subscriptions: super::presentation::subscriptions(),
        research_company,
    }
}
