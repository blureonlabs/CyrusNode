//! Research domain — types + rules. Zero I/O.
//!
//! Forbidden imports in this folder (enforced by `scripts/check-domain-imports.sh`):
//!   `sqlx`, `reqwest`, `axum`, `tokio::net`, `tokio::fs`,
//!   `crate::platform::{db,llm,crawler,events,queue,prompts}`,
//!   `crate::features::*::infra`.

mod dossier;
mod ports;

pub use dossier::{BusinessSummary, DossierBase, Evidence, MaturityTier};
pub use ports::{
    BizIntelPort, CrawlError, CrawlPort, CrawlResult, ExtractPort, ExtractedSite, Finding, SeoPort,
    SeoReport, Severity, SiteFacts, SocialLinks,
};
