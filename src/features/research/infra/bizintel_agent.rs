//! Business intelligence agent — implements [`BizIntelPort`].
//!
//! S1-T11: real LLM call against the `business-summary` prompt with JSON
//! schema validation. The agent:
//!
//! 1. Loads the prompt from the hot-reloading [`PromptLoader`].
//! 2. Builds a compact context from the [`ExtractedSite`] markdown + facts.
//! 3. Renders the prompt and calls [`LlmClient::complete`] in JSON mode.
//! 4. Parses the response into a tolerant [`RawBizIntel`] DTO.
//! 5. Re-prompts once with a corrective hint if JSON parsing fails.
//! 6. Maps to the strict domain [`BusinessSummary`].

use std::collections::HashMap;
use std::sync::Arc;

use anyhow::{anyhow, Context};
use async_trait::async_trait;
use tracing::Instrument;

use crate::features::research::domain::{
    BizIntelPort, BusinessSummary, Evidence, ExtractedSite, MaturityTier,
};
use crate::platform::core::CompanyId;
use crate::platform::llm::{LlmClient, LlmRequest};
use crate::platform::prompts::PromptLoader;

/// Hard cap on raw markdown bytes shipped to the LLM. Keeps the prompt under
/// the agent's per-company cost ceiling (see `apollo/03-agents/business-intelligence.md`).
const MAX_MARKDOWN_CHARS: usize = 12_000;

/// Marker appended when the markdown is truncated.
const TRUNCATION_MARKER: &str = "\n... [truncated]";

/// Prompt name to load from [`PromptLoader`]. Matches `apollo/04-prompts/business-summary.md`.
const PROMPT_NAME: &str = "business-summary";

/// Business intelligence agent — implements [`BizIntelPort`].
pub struct BizIntelAgent {
    llm: Arc<dyn LlmClient>,
    prompts: PromptLoader,
}

impl BizIntelAgent {
    /// Construct the agent with shared [`LlmClient`] and [`PromptLoader`] handles.
    pub fn new(llm: Arc<dyn LlmClient>, prompts: PromptLoader) -> Self {
        Self { llm, prompts }
    }
}

#[async_trait]
impl BizIntelPort for BizIntelAgent {
    /// Summarize the extracted site into a structured [`BusinessSummary`].
    ///
    /// Errors:
    /// - prompt `business-summary` missing from the loader
    /// - LLM transport failure
    /// - LLM returns malformed JSON twice in a row
    async fn summarize(
        &self,
        company_id: CompanyId,
        extracted: &ExtractedSite,
    ) -> anyhow::Result<BusinessSummary> {
        let span = tracing::info_span!("bizintel", company_id = %company_id.0);
        self.summarize_inner(extracted).instrument(span).await
    }
}

impl BizIntelAgent {
    async fn summarize_inner(&self, extracted: &ExtractedSite) -> anyhow::Result<BusinessSummary> {
        let prompt = self
            .prompts
            .get(PROMPT_NAME)
            .await
            .ok_or_else(|| anyhow!("prompt '{}' not loaded", PROMPT_NAME))?;

        let context = build_context(extracted)?;
        let mut vars = HashMap::new();
        vars.insert("context", context);
        let rendered = prompt.render(&vars);

        let model = prompt.frontmatter.model.clone();
        let temperature = prompt.frontmatter.temperature;
        let max_output_tokens = prompt.frontmatter.max_output_tokens;

        let first = LlmRequest {
            model: model.clone(),
            system: None,
            user: rendered.clone(),
            max_output_tokens,
            temperature,
            json_response: true,
        };

        let first_resp = self
            .llm
            .complete(first)
            .await
            .context("bizintel: first LLM call failed")?;

        let raw = match parse_raw(&first_resp.text) {
            Ok(r) => r,
            Err(err) => {
                tracing::warn!(
                    error = %err,
                    "bizintel: first JSON parse failed, retrying with corrective hint"
                );
                let corrective = format!(
                    "{}\n\nYour previous output was not valid JSON: {}. Return ONLY valid JSON matching the schema.",
                    rendered, err
                );
                let retry = LlmRequest {
                    model,
                    system: None,
                    user: corrective,
                    max_output_tokens,
                    temperature,
                    json_response: true,
                };
                let retry_resp = self
                    .llm
                    .complete(retry)
                    .await
                    .context("bizintel: retry LLM call failed")?;
                parse_raw(&retry_resp.text)
                    .map_err(|e| anyhow!("bizintel: retry also produced invalid JSON: {e}"))?
            }
        };

        Ok(map_to_domain(raw))
    }
}

/// Build the `{{ context }}` string from extracted markdown + structured site facts.
///
/// Markdown is truncated to [`MAX_MARKDOWN_CHARS`] with a visible marker.
fn build_context(extracted: &ExtractedSite) -> anyhow::Result<String> {
    let md = &extracted.markdown;
    let markdown_block = if md.chars().count() > MAX_MARKDOWN_CHARS {
        // Truncate by chars (not bytes) to keep UTF-8 boundaries safe.
        let truncated: String = md.chars().take(MAX_MARKDOWN_CHARS).collect();
        format!("{}{}", truncated, TRUNCATION_MARKER)
    } else {
        md.clone()
    };

    let facts_json = serde_json::to_string_pretty(&extracted.site_facts)
        .context("bizintel: failed to serialize site_facts")?;

    Ok(format!(
        "## Site Facts (structured)\n\n```json\n{}\n```\n\n## Page Markdown\n\n{}",
        facts_json, markdown_block
    ))
}

fn parse_raw(text: &str) -> Result<RawBizIntel, serde_json::Error> {
    // Best-effort: trim whitespace and accept models that wrapped output in
    // a ```json fence even though we asked for raw JSON.
    let trimmed = text.trim();
    let cleaned = strip_code_fence(trimmed);
    serde_json::from_str::<RawBizIntel>(cleaned)
}

fn strip_code_fence(s: &str) -> &str {
    let s = s.trim();
    if let Some(rest) = s.strip_prefix("```json") {
        return rest.trim_start_matches('\n').trim_end_matches("```").trim();
    }
    if let Some(rest) = s.strip_prefix("```") {
        return rest.trim_start_matches('\n').trim_end_matches("```").trim();
    }
    s
}

fn map_to_domain(raw: RawBizIntel) -> BusinessSummary {
    let maturity_tier = match raw
        .maturity_tier
        .as_deref()
        .map(str::trim)
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("early") => MaturityTier::Early,
        Some("established") => MaturityTier::Established,
        Some("growing") => MaturityTier::Growing,
        _ => MaturityTier::Growing,
    };

    let evidence = raw
        .evidence
        .unwrap_or_default()
        .into_iter()
        .map(|e| Evidence {
            claim: e.claim,
            page_url: e.page_url,
            excerpt: e.excerpt,
        })
        .collect();

    BusinessSummary {
        what_they_sell: raw.what_they_sell.unwrap_or_default(),
        who_they_serve: raw.who_they_serve.unwrap_or_default(),
        value_props: raw.value_props.unwrap_or_default(),
        customer_segments: raw.customer_segments.unwrap_or_default(),
        maturity_tier,
        evidence,
        confidence: raw.confidence.unwrap_or(0.5),
    }
}

/// Tolerant DTO mirroring `apollo/04-prompts/schemas/business-summary.v1.json`.
/// All fields are optional so a flaky LLM payload doesn't terminate the agent
/// hard — sensible defaults are applied in [`map_to_domain`].
#[derive(serde::Deserialize)]
struct RawBizIntel {
    what_they_sell: Option<String>,
    who_they_serve: Option<String>,
    value_props: Option<Vec<String>>,
    customer_segments: Option<Vec<String>>,
    maturity_tier: Option<String>,
    evidence: Option<Vec<RawEvidence>>,
    confidence: Option<f32>,
}

#[derive(serde::Deserialize)]
struct RawEvidence {
    claim: String,
    page_url: Option<String>,
    excerpt: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_maturity_tier_strings() {
        let raw = RawBizIntel {
            what_they_sell: Some("A".into()),
            who_they_serve: Some("B".into()),
            value_props: Some(vec!["x".into()]),
            customer_segments: Some(vec!["seg".into()]),
            maturity_tier: Some("established".into()),
            evidence: Some(vec![RawEvidence {
                claim: "c".into(),
                page_url: Some("u".into()),
                excerpt: Some("e".into()),
            }]),
            confidence: Some(0.9),
        };
        let s = map_to_domain(raw);
        assert!(matches!(s.maturity_tier, MaturityTier::Established));
        assert_eq!(s.what_they_sell, "A");
        assert_eq!(s.confidence, 0.9);
        assert_eq!(s.evidence.len(), 1);

        let unknown = RawBizIntel {
            what_they_sell: None,
            who_they_serve: None,
            value_props: None,
            customer_segments: None,
            maturity_tier: Some("MASSIVE".into()),
            evidence: None,
            confidence: None,
        };
        let s2 = map_to_domain(unknown);
        assert!(matches!(s2.maturity_tier, MaturityTier::Growing));
        assert_eq!(s2.confidence, 0.5);
        assert!(s2.what_they_sell.is_empty());
        assert!(s2.evidence.is_empty());

        for (input, expected) in [
            ("early", MaturityTier::Early),
            ("EARLY", MaturityTier::Early),
            (" growing ", MaturityTier::Growing),
        ] {
            let r = RawBizIntel {
                what_they_sell: None,
                who_they_serve: None,
                value_props: None,
                customer_segments: None,
                maturity_tier: Some(input.into()),
                evidence: None,
                confidence: None,
            };
            assert!(
                std::mem::discriminant(&map_to_domain(r).maturity_tier)
                    == std::mem::discriminant(&expected),
                "input {input:?} did not map as expected",
            );
        }
    }

    #[test]
    fn parses_raw_with_code_fence() {
        let text = "```json\n{\"what_they_sell\": \"things\", \"maturity_tier\": \"early\", \"confidence\": 0.7}\n```";
        let raw = parse_raw(text).expect("parses");
        assert_eq!(raw.what_they_sell.as_deref(), Some("things"));
        assert_eq!(raw.maturity_tier.as_deref(), Some("early"));
    }
}
