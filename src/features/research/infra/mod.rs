//! Adapters that implement the research domain's ports.
//!
//! Allowed deps: this feature's `domain`, `crate::platform::*`, third-party SDKs.
//! Forbidden: `application/`, `presentation/`, other features.

pub mod bizintel_agent;
pub mod crawler_adapter;
pub mod extractor_adapter;
pub mod seo_runner;

pub use bizintel_agent::BizIntelAgent;
pub use crawler_adapter::HttpCrawlerAdapter;
pub use extractor_adapter::MarkdownExtractorAdapter;
pub use seo_runner::StaticSeoAuditor;
