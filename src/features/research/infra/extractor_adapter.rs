//! Markdown extractor adapter — implements `ExtractPort`.
//!
//! TODO(S1-T09): convert crawled HTML into Markdown and emit per-page word
//! counts.

use async_trait::async_trait;

use crate::features::research::domain::{ExtractPort, ExtractedSite};
use crate::platform::core::CompanyId;

pub struct MarkdownExtractorAdapter;

#[async_trait]
impl ExtractPort for MarkdownExtractorAdapter {
    async fn extract(&self, _company_id: CompanyId) -> anyhow::Result<ExtractedSite> {
        // TODO(S1-T09): real HTML→Markdown extraction.
        Ok(ExtractedSite { total_words: 0 })
    }
}
