//! Wiring file for the research feature.
//!
//! Single matchmaker for the four boxes (`domain`, `application`, `infra`,
//! `presentation`). Builds adapters from cross-cutting handles supplied by
//! the caller (typically `bootstrap.rs` or the CLI).

use std::sync::Arc;

use crate::features::research::application::ResearchCompany;
use crate::features::research::domain::{
    BizIntelPort, CrawlPort, ExtractPort, IndustryHint, SeoPort,
};
use crate::features::research::infra::{
    BizIntelAgent, HttpCrawlerAdapter, MarkdownExtractorAdapter, StaticSeoAuditor,
};
use crate::platform::crawler::{CrawlerConfig, HttpCrawler};
use crate::platform::events::EventSubscription;
use crate::platform::knowledge::PlaybookLoader;
use crate::platform::llm::LlmClient;
use crate::platform::prompts::PromptLoader;

/// Cross-cutting handles required to construct the research feature.
///
/// `playbooks` is plumbed through so the wiring layer (CLI / bootstrap) can
/// build an [`IndustryHint`] per call via [`make_industry_hint`]; the bizintel
/// agent itself does not depend on the loader.
#[derive(Clone)]
pub struct ResearchDeps {
    pub llm: Arc<dyn LlmClient>,
    pub prompts: PromptLoader,
    pub playbooks: PlaybookLoader,
}

pub struct ResearchModule {
    pub routes: axum::Router<()>,
    pub subscriptions: Vec<EventSubscription>,
    pub research_company: Arc<ResearchCompany>,
}

/// Convert an industry key into an [`IndustryHint`] by looking the playbook
/// up in `playbooks`. Returns `None` when `key` is `None`, when it is empty,
/// or when no playbook exists for that key — the bizintel agent then runs
/// with no industry context (its original behavior).
pub async fn make_industry_hint(
    playbooks: &PlaybookLoader,
    key: Option<&str>,
) -> Option<IndustryHint> {
    let key = key.map(str::trim).filter(|k| !k.is_empty())?;
    let book = playbooks.get(key).await?;
    Some(IndustryHint {
        key: book.frontmatter.key.clone(),
        display_name: book.frontmatter.display_name.clone(),
        typical_pains: book.typical_pains.clone(),
        typical_opportunities: book.typical_opportunities.clone(),
    })
}

/// Wire the research feature. Boot-time `expect` is acceptable per CLAUDE.md §3.
///
/// `deps.playbooks` is accepted for symmetry with drafting and to enable
/// `make_industry_hint` at the wiring layer; it is intentionally NOT handed to
/// the bizintel agent because the agent receives the hint per-call.
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
