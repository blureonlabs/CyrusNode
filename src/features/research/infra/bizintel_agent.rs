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
//! 7. If the mapped summary has empty `what_they_sell` or `who_they_serve`,
//!    re-prompts once more with a "fields required" suffix and keeps whichever
//!    of the two responses is more populated. Observed in production against
//!    didilimousine.com where Gemini occasionally omits these two keys.

use std::collections::HashMap;
use std::sync::Arc;

use anyhow::{anyhow, Context};
use async_trait::async_trait;
use tracing::Instrument;

use crate::features::research::domain::{
    BizIntelPort, BusinessSummary, Evidence, ExtractedSite, MaturityTier, SiteFacts,
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
        self.summarize_inner(company_id, extracted)
            .instrument(span)
            .await
    }
}

impl BizIntelAgent {
    async fn summarize_inner(
        &self,
        company_id: CompanyId,
        extracted: &ExtractedSite,
    ) -> anyhow::Result<BusinessSummary> {
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

        // Attribution that every LLM call in this agent shares — built once
        // and cloned per request. CLAUDE.md §6.
        let agent_name = Some("bizintel".to_string());
        let prompt_name = Some(prompt.frontmatter.name.clone());
        let prompt_version = Some(prompt.frontmatter.version);
        let company_id_str = Some(company_id.0.to_string());

        let first = LlmRequest {
            model: model.clone(),
            system: None,
            user: rendered.clone(),
            max_output_tokens,
            temperature,
            json_response: true,
            agent_name: agent_name.clone(),
            prompt_name: prompt_name.clone(),
            prompt_version,
            company_id: company_id_str.clone(),
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
                    model: model.clone(),
                    system: None,
                    user: corrective,
                    max_output_tokens,
                    temperature,
                    json_response: true,
                    agent_name: agent_name.clone(),
                    prompt_name: prompt_name.clone(),
                    prompt_version,
                    company_id: company_id_str.clone(),
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

        let summary = map_to_domain(raw);

        // Completion-retry gate (one-shot). Gemini occasionally omits these two
        // required keys despite the prompt instructing them — see module docs.
        // We only fire this *after* the JSON-parse retry path has already
        // succeeded, so the budget is at most one extra call per summarize().
        if has_required_field_gap(&summary) {
            tracing::warn!(
                "bizintel: required fields empty after first parse; triggering completion retry"
            );
            let corrective = format!("{}\n\n{}", rendered, COMPLETION_RETRY_SUFFIX);
            let retry = LlmRequest {
                model,
                system: None,
                user: corrective,
                max_output_tokens,
                temperature,
                json_response: true,
                agent_name: agent_name.clone(),
                prompt_name: prompt_name.clone(),
                prompt_version,
                company_id: company_id_str.clone(),
            };
            // Best-effort: if the retry call or its parse fails, log and keep
            // the original — we never want to fail the agent because the
            // completion retry itself errored.
            match self.llm.complete(retry).await {
                Ok(resp) => match parse_raw(&resp.text) {
                    Ok(retry_raw) => {
                        let retry_summary = map_to_domain(retry_raw);
                        return Ok(choose_more_populated(summary, retry_summary));
                    }
                    Err(err) => {
                        tracing::warn!(
                            error = %err,
                            "bizintel: completion-retry response was not valid JSON; keeping original"
                        );
                    }
                },
                Err(err) => {
                    tracing::warn!(
                        error = %err,
                        "bizintel: completion-retry LLM call failed; keeping original"
                    );
                }
            }
        }

        Ok(summary)
    }
}

/// Suffix appended to the original prompt when the first response omitted one
/// of the two required string fields. Kept as a module constant so the test
/// can assert it without re-deriving the wording.
const COMPLETION_RETRY_SUFFIX: &str = "\
Your previous output had `what_they_sell` and/or `who_they_serve` empty.
These two fields are REQUIRED — read the context again and produce a
one-sentence answer for each. Return the full JSON object with every
field populated, not a delta.";

/// Predicate: does this summary still have one of the two required string
/// fields empty (after trim)? Whitespace-only counts as empty because Gemini
/// has been seen returning `" "` instead of an actual sentence.
fn has_required_field_gap(s: &BusinessSummary) -> bool {
    s.what_they_sell.trim().is_empty() || s.who_they_serve.trim().is_empty()
}

/// Pick the response that has both required fields populated. If both still
/// have gaps we keep the retry only when it is strictly better; otherwise the
/// original wins and we log a loud warning so it shows up in the dashboard.
fn choose_more_populated(original: BusinessSummary, retry: BusinessSummary) -> BusinessSummary {
    let original_gap = has_required_field_gap(&original);
    let retry_gap = has_required_field_gap(&retry);
    match (original_gap, retry_gap) {
        // Retry filled the holes — take it.
        (true, false) => retry,
        // Original was fine; retry regressed (shouldn't happen because we
        // only enter this path when original had a gap, but defensive).
        (false, true) => original,
        // Both fine — prefer the retry as it had the "every field populated"
        // instruction explicitly attached.
        (false, false) => retry,
        // Both still have gaps. Per the task spec, return the populated one,
        // which here means whichever has more non-empty required fields.
        (true, true) => {
            tracing::warn!("bizintel: completion retry did not fill required fields");
            let original_score = required_fields_filled(&original);
            let retry_score = required_fields_filled(&retry);
            if retry_score > original_score {
                retry
            } else {
                original
            }
        }
    }
}

/// Count of the two required fields that are non-empty after trim. Used as a
/// tiebreaker when both candidates still have gaps.
fn required_fields_filled(s: &BusinessSummary) -> u8 {
    let a = u8::from(!s.what_they_sell.trim().is_empty());
    let b = u8::from(!s.who_they_serve.trim().is_empty());
    a + b
}

/// Build the `{{ context }}` string from extracted markdown + structured site facts.
///
/// Prepends a compact human-readable "SITE FACTS" block (deterministic ground
/// truth — emails, phones, schema.org types, social handles) so the LLM can
/// anchor `what_they_sell` / `who_they_serve` on signals the markdown alone
/// often buries. The block is intentionally tiny (~50–200 tokens); the bulk of
/// the context is still the markdown body, capped at [`MAX_MARKDOWN_CHARS`].
fn build_context(extracted: &ExtractedSite) -> anyhow::Result<String> {
    let md = &extracted.markdown;
    let markdown_block = if md.chars().count() > MAX_MARKDOWN_CHARS {
        // Truncate by chars (not bytes) to keep UTF-8 boundaries safe.
        let truncated: String = md.chars().take(MAX_MARKDOWN_CHARS).collect();
        format!("{}{}", truncated, TRUNCATION_MARKER)
    } else {
        md.clone()
    };

    let facts_block = render_site_facts(&extracted.site_facts);

    Ok(format!(
        "{}\n\nWEBSITE MARKDOWN:\n{}",
        facts_block, markdown_block
    ))
}

/// Render [`SiteFacts`] as a compact human-readable block. Missing values
/// collapse to the literal string "none" so the LLM never sees an ambiguous
/// blank.
fn render_site_facts(f: &SiteFacts) -> String {
    fn join_or_none(xs: &[String]) -> String {
        if xs.is_empty() {
            "none".to_string()
        } else {
            xs.join(", ")
        }
    }
    fn opt_or_none(o: &Option<String>) -> &str {
        o.as_deref().unwrap_or("none")
    }

    format!(
        "SITE FACTS (extracted deterministically — these are ground truth):\n\
         - Phones: {phones}\n\
         - Emails: {emails}\n\
         - WhatsApp links: {whatsapp}\n\
         - Schema.org types: {schema}\n\
         - Social: instagram={ig}, facebook={fb}, linkedin={li}, tiktok={tt}, youtube={yt}",
        phones = join_or_none(&f.phones),
        emails = join_or_none(&f.emails),
        whatsapp = join_or_none(&f.whatsapp_links),
        schema = join_or_none(&f.schema_org_types),
        ig = opt_or_none(&f.social.instagram),
        fb = opt_or_none(&f.social.facebook),
        li = opt_or_none(&f.social.linkedin),
        tt = opt_or_none(&f.social.tiktok),
        yt = opt_or_none(&f.social.youtube),
    )
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
    fn build_context_includes_site_facts_block() {
        use crate::features::research::domain::{SiteFacts, SocialLinks};

        let extracted = ExtractedSite {
            total_words: 12,
            markdown: "Hello from the about page.".into(),
            site_facts: SiteFacts {
                language: Some("en".into()),
                phones: vec!["+971-50-1234567".into()],
                emails: vec!["info@example.com".into(), "sales@example.com".into()],
                whatsapp_links: vec!["https://wa.me/9710501234567".into()],
                social: SocialLinks {
                    instagram: Some("https://instagram.com/example".into()),
                    facebook: None,
                    linkedin: Some("https://linkedin.com/company/example".into()),
                    tiktok: None,
                    youtube: None,
                },
                schema_org_types: vec!["LocalBusiness".into()],
            },
        };

        let ctx = build_context(&extracted).expect("builds");
        // Header marker is present.
        assert!(
            ctx.contains("SITE FACTS (extracted deterministically"),
            "expected SITE FACTS header in context, got:\n{ctx}"
        );
        // Values flow through verbatim.
        assert!(ctx.contains("+971-50-1234567"));
        assert!(ctx.contains("info@example.com, sales@example.com"));
        assert!(ctx.contains("Schema.org types: LocalBusiness"));
        assert!(ctx.contains("instagram=https://instagram.com/example"));
        // Empty optional collapses to "none".
        assert!(ctx.contains("facebook=none"));
        // Markdown body still appears after the facts block.
        assert!(ctx.contains("WEBSITE MARKDOWN:"));
        assert!(ctx.contains("Hello from the about page."));
        // SITE FACTS block precedes the markdown.
        let facts_idx = ctx.find("SITE FACTS").expect("facts header");
        let md_idx = ctx.find("WEBSITE MARKDOWN").expect("markdown header");
        assert!(facts_idx < md_idx, "facts must precede markdown body");
    }

    #[test]
    fn build_context_collapses_to_none_when_facts_empty() {
        let extracted = ExtractedSite {
            total_words: 3,
            markdown: "Just markdown.".into(),
            site_facts: crate::features::research::domain::SiteFacts::default(),
        };
        let ctx = build_context(&extracted).expect("builds");
        assert!(ctx.contains("Phones: none"));
        assert!(ctx.contains("Emails: none"));
        assert!(ctx.contains("WhatsApp links: none"));
        assert!(ctx.contains("Schema.org types: none"));
        assert!(ctx.contains("instagram=none"));
    }

    #[test]
    fn parses_raw_with_code_fence() {
        let text = "```json\n{\"what_they_sell\": \"things\", \"maturity_tier\": \"early\", \"confidence\": 0.7}\n```";
        let raw = parse_raw(text).expect("parses");
        assert_eq!(raw.what_they_sell.as_deref(), Some("things"));
        assert_eq!(raw.maturity_tier.as_deref(), Some("early"));
    }

    #[test]
    fn required_field_gap_predicate() {
        let mut s = BusinessSummary {
            what_they_sell: "cars".into(),
            who_they_serve: "people".into(),
            value_props: vec![],
            customer_segments: vec![],
            maturity_tier: MaturityTier::Growing,
            evidence: vec![],
            confidence: 0.5,
        };
        assert!(!has_required_field_gap(&s));
        s.what_they_sell = "  ".into();
        assert!(has_required_field_gap(&s));
        s.what_they_sell = "cars".into();
        s.who_they_serve = "".into();
        assert!(has_required_field_gap(&s));
    }

    #[test]
    fn choose_more_populated_prefers_filled_retry() {
        let original = BusinessSummary {
            what_they_sell: "".into(),
            who_they_serve: "".into(),
            value_props: vec![],
            customer_segments: vec![],
            maturity_tier: MaturityTier::Growing,
            evidence: vec![],
            confidence: 0.4,
        };
        let retry = BusinessSummary {
            what_they_sell: "luxury chauffeur rides".into(),
            who_they_serve: "Dubai corporate travelers".into(),
            value_props: vec![],
            customer_segments: vec![],
            maturity_tier: MaturityTier::Established,
            evidence: vec![],
            confidence: 0.8,
        };
        let chosen = choose_more_populated(original, retry);
        assert_eq!(chosen.what_they_sell, "luxury chauffeur rides");
        assert!(matches!(chosen.maturity_tier, MaturityTier::Established));

        // Both still empty: returns the one with more filled fields and logs
        // a warning. We assert the "more populated" half wins.
        let half = BusinessSummary {
            what_they_sell: "rides".into(),
            who_they_serve: "".into(),
            value_props: vec![],
            customer_segments: vec![],
            maturity_tier: MaturityTier::Growing,
            evidence: vec![],
            confidence: 0.5,
        };
        let empty = BusinessSummary {
            what_they_sell: "".into(),
            who_they_serve: "".into(),
            value_props: vec![],
            customer_segments: vec![],
            maturity_tier: MaturityTier::Growing,
            evidence: vec![],
            confidence: 0.5,
        };
        let chosen2 = choose_more_populated(empty, half);
        assert_eq!(chosen2.what_they_sell, "rides");
    }

    /// End-to-end test of the completion-retry path with a scripted fake LLM.
    ///
    /// Round 1 returns valid JSON but with `what_they_sell` and `who_they_serve`
    /// empty — the exact didilimousine.com failure mode.
    /// Round 2 (triggered by the completion-retry gate) returns a fully
    /// populated payload. We assert:
    ///   * the LLM was called exactly twice,
    ///   * the second call's prompt contains the COMPLETION_RETRY_SUFFIX,
    ///   * the returned summary uses the round-2 values.
    #[tokio::test]
    async fn completion_retry_fires_when_required_fields_empty() {
        use crate::platform::llm::{LlmError, LlmResponse};
        use async_trait::async_trait;
        use std::sync::Mutex;

        struct ScriptedLlm {
            responses: Mutex<Vec<String>>,
            captured: Mutex<Vec<String>>,
        }

        #[async_trait]
        impl LlmClient for ScriptedLlm {
            async fn complete(&self, req: LlmRequest) -> Result<LlmResponse, LlmError> {
                self.captured.lock().expect("captured lock").push(req.user);
                let mut q = self.responses.lock().expect("responses lock");
                if q.is_empty() {
                    return Err(LlmError::SchemaMismatch(
                        "ran out of scripted responses".into(),
                    ));
                }
                let text = q.remove(0);
                Ok(LlmResponse {
                    text,
                    input_tokens: 0,
                    output_tokens: 0,
                    cost_usd: 0.0,
                    latency_ms: 0,
                    model: req.model,
                })
            }
        }

        // Use the checked-in apollo/04-prompts directory so we exercise the
        // real PromptLoader -> render -> request -> parse chain without
        // pulling in a new dev-dependency. Other tests in this crate use the
        // same CARGO_MANIFEST_DIR pattern (see prompts/loader.rs tests).
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("apollo/04-prompts");
        let prompts = PromptLoader::load(&root).await.expect("loads");

        let round_one = r#"{
            "what_they_sell": "",
            "who_they_serve": "",
            "value_props": ["chauffeur fleet"],
            "customer_segments": ["luxury_travelers"],
            "maturity_tier": "growing",
            "evidence": [],
            "confidence": 0.4
        }"#
        .to_string();
        let round_two = r#"{
            "what_they_sell": "Premium chauffeur and limousine rides in Dubai.",
            "who_they_serve": "Business travelers and tourists needing reliable airport transfers.",
            "value_props": ["chauffeur fleet"],
            "customer_segments": ["luxury_travelers"],
            "maturity_tier": "established",
            "evidence": [],
            "confidence": 0.85
        }"#
        .to_string();

        let scripted = Arc::new(ScriptedLlm {
            responses: Mutex::new(vec![round_one, round_two]),
            captured: Mutex::new(vec![]),
        });

        let agent = BizIntelAgent::new(scripted.clone(), prompts);
        let extracted = ExtractedSite {
            total_words: 100,
            markdown: "About us: we drive people.".into(),
            site_facts: crate::features::research::domain::SiteFacts::default(),
        };

        let summary = agent
            .summarize(CompanyId(uuid::Uuid::nil()), &extracted)
            .await
            .expect("summarize ok");

        // The retry payload's values win.
        assert!(summary.what_they_sell.contains("Premium chauffeur"));
        assert!(summary.who_they_serve.contains("Business travelers"));
        assert!(matches!(summary.maturity_tier, MaturityTier::Established));

        // Exactly two calls fired, and the second carried the corrective suffix.
        let captured = scripted.captured.lock().expect("captured lock");
        assert_eq!(captured.len(), 2, "expected exactly two LLM calls");
        assert!(
            captured[1].contains("These two fields are REQUIRED"),
            "second call must include the completion-retry suffix"
        );
    }
}
