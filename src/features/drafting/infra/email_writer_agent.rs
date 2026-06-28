//! Email writer agent — implements `DraftPort`.
//!
//! S1-T12. Renders the `email-writer` prompt (see
//! `apollo/04-prompts/email-writer.md`), calls the configured `LlmClient` in
//! JSON mode, parses the structured response, and runs three deterministic
//! post-checks before returning an [`EmailDraft`]:
//!
//! 1. **Banned phrase** — case-insensitive substring match against the file at
//!    `banned_phrases_path` (one phrase per line, `#` comments allowed).
//! 2. **Word cap** — body must be ≤ 120 words.
//! 3. **Anchors** — body must contain ≥ 2 of the caller-supplied anchor texts
//!    (case-insensitive substring match).
//!
//! Each check, if it fires, triggers exactly one targeted re-prompt. A second
//! failure is terminal and bubbles up as `anyhow::Error`.
//!
//! The banned-phrase list is loaded lazily on first use and cached in a
//! [`tokio::sync::OnceCell`] so we read disk at most once per process.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::OnceCell;

use crate::features::drafting::domain::{DraftInput, DraftPort, EmailDraft};
use crate::platform::core::CompanyId;
use crate::platform::knowledge::PlaybookLoader;
use crate::platform::llm::{LlmClient, LlmRequest};
use crate::platform::prompts::PromptLoader;

/// Hard cap on body word count (see `apollo/03-agents/email-writer.md`).
const MAX_BODY_WORDS: usize = 120;
/// Minimum number of caller-supplied anchors that must appear in the body.
const MIN_ANCHORS_IN_BODY: usize = 2;
/// Industry-default tone used when `DraftInput::industry` does not match a
/// known playbook entry. Kept verbatim from the agent spec.
const DEFAULT_TONE: &str = "direct, peer-to-peer, warm but not chatty";

/// LLM-shaped response from the email-writer prompt. Private — we project
/// down to [`EmailDraft`] before crossing the module boundary.
#[derive(Debug, serde::Deserialize)]
struct RawEmail {
    subject: Option<String>,
    body: Option<String>,
    personalization_anchors: Option<Vec<RawAnchor>>,
}

/// Anchor as returned by the LLM. Accept both shapes the model produces in
/// practice: a bare string (`"anchor text"`) or a structured object
/// (`{ "type": "...", "text": "..." }`). We project to a flat string downstream.
#[derive(Debug, serde::Deserialize)]
#[serde(untagged)]
enum RawAnchor {
    Plain(String),
    Structured {
        #[serde(rename = "type")]
        #[allow(dead_code)]
        kind: Option<String>,
        text: String,
    },
}

impl RawAnchor {
    fn into_text(self) -> String {
        match self {
            RawAnchor::Plain(s) => s,
            RawAnchor::Structured { text, .. } => text,
        }
    }
}

/// Email writer agent. Construct via [`EmailWriterAgent::new`].
///
/// Holds shared handles to the LLM client and prompt loader plus the path to
/// the banned-phrases file. The banned-phrase list is read lazily on first
/// invocation and cached for the lifetime of the agent.
pub struct EmailWriterAgent {
    llm: Arc<dyn LlmClient>,
    prompts: PromptLoader,
    playbooks: PlaybookLoader,
    banned_phrases_path: PathBuf,
    banned_phrases: OnceCell<Vec<String>>,
}

impl EmailWriterAgent {
    /// Build a new [`EmailWriterAgent`].
    ///
    /// `banned_phrases_path` is not read until the first [`Self::draft_email`]
    /// call; a missing file surfaces as an error from that call, not from
    /// construction.
    pub fn new(
        llm: Arc<dyn LlmClient>,
        prompts: PromptLoader,
        playbooks: PlaybookLoader,
        banned_phrases_path: PathBuf,
    ) -> Self {
        Self {
            llm,
            prompts,
            playbooks,
            banned_phrases_path,
            banned_phrases: OnceCell::new(),
        }
    }

    /// Resolve the `{{ tone }}` template variable. Uses the industry
    /// playbook's `## Tone` section if `input.industry` matches a loaded
    /// playbook and the section is non-empty; otherwise falls back to the
    /// crate-wide [`DEFAULT_TONE`].
    async fn resolve_tone(&self, input: &DraftInput) -> String {
        if let Some(key) = input.industry.as_deref() {
            if let Some(book) = self.playbooks.get(key).await {
                if !book.tone.is_empty() {
                    tracing::debug!(industry = key, "using playbook tone");
                    return book.tone;
                }
            }
            tracing::debug!(
                industry = key,
                "no playbook tone found — falling back to default"
            );
        }
        DEFAULT_TONE.to_string()
    }

    /// Return the cached banned-phrase list, loading it from disk on first
    /// access. Lines beginning with `#` and blank lines are skipped; the
    /// remaining entries are trimmed and lowercased so callers can do a
    /// case-insensitive `contains` check.
    async fn banned_phrases(&self) -> anyhow::Result<&Vec<String>> {
        self.banned_phrases
            .get_or_try_init(|| async {
                let raw = tokio::fs::read_to_string(&self.banned_phrases_path)
                    .await
                    .map_err(|e| {
                        anyhow::anyhow!(
                            "failed to read banned phrases file {}: {}",
                            self.banned_phrases_path.display(),
                            e
                        )
                    })?;
                Ok::<_, anyhow::Error>(parse_banned_phrases(&raw))
            })
            .await
    }
}

/// Pure parser for the banned-phrases file format. Exposed at module scope so
/// the unit test below can exercise it without touching disk.
fn parse_banned_phrases(raw: &str) -> Vec<String> {
    raw.lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(|l| l.to_lowercase())
        .collect()
}

/// First banned phrase (if any) that appears in `body`, case-insensitive.
fn first_banned_hit<'a>(body: &str, banned: &'a [String]) -> Option<&'a str> {
    let lower = body.to_lowercase();
    banned
        .iter()
        .find(|p| lower.contains(p.as_str()))
        .map(|s| s.as_str())
}

/// Whitespace-delimited word count. Good enough for the 120-word cap; a
/// pedantic Unicode count is not worth the dep here.
fn word_count(body: &str) -> usize {
    body.split_whitespace().count()
}

/// How many of `anchors` appear as a case-insensitive substring of `body`.
fn anchors_in_body(body: &str, anchors: &[String]) -> usize {
    let lower = body.to_lowercase();
    anchors
        .iter()
        .filter(|a| !a.trim().is_empty())
        .filter(|a| lower.contains(&a.to_lowercase()))
        .count()
}

/// Build the `{{ business_intel }}` block: a compact bullet list folding the
/// one-liner, value props, and anchors together.
fn build_business_intel(input: &DraftInput) -> String {
    let mut lines = Vec::with_capacity(1 + input.value_props.len() + input.anchors.len());
    lines.push(format!("- They sell: {}", input.what_they_sell));
    for vp in &input.value_props {
        lines.push(format!("- Value prop: {}", vp));
    }
    for a in &input.anchors {
        lines.push(format!("- Anchor: {}", a));
    }
    lines.join("\n")
}

/// Pick the headline opportunity to pitch. First anchor wins; otherwise the
/// first value prop; otherwise the what-they-sell line as a last resort so we
/// never feed the prompt an empty variable.
fn pick_top_opportunity(input: &DraftInput) -> String {
    input
        .anchors
        .first()
        .cloned()
        .or_else(|| input.value_props.first().cloned())
        .unwrap_or_else(|| input.what_they_sell.clone())
}

#[async_trait]
impl DraftPort for EmailWriterAgent {
    /// Draft a cold email for `company_id` from `input`.
    ///
    /// # Failure modes
    /// - The `email-writer` prompt is not loaded.
    /// - The banned-phrases file cannot be read.
    /// - The LLM call fails or returns non-JSON.
    /// - Two consecutive drafts violate the banned-phrase, word-cap, or
    ///   anchor-count rule.
    async fn draft_email(
        &self,
        company_id: CompanyId,
        input: &DraftInput,
    ) -> anyhow::Result<EmailDraft> {
        let span = tracing::info_span!("email_writer", company_id = %company_id.0);
        let _enter = span.enter();

        let prompt = self
            .prompts
            .get("email-writer")
            .await
            .ok_or_else(|| anyhow::anyhow!("email-writer prompt not loaded"))?;

        let banned = self.banned_phrases().await?;

        // ---- Render template variables. ----
        let business_intel = build_business_intel(input);
        let top_opportunity = pick_top_opportunity(input);
        let tone = self.resolve_tone(input).await;
        let banned_csv = banned.join(", ");

        let mut vars: HashMap<&str, String> = HashMap::new();
        vars.insert("business_intel", business_intel);
        vars.insert("top_opportunity", top_opportunity);
        vars.insert("tone", tone);
        vars.insert("operator_signature", input.operator_signature.clone());
        vars.insert("banned_phrases", banned_csv);

        let base_user = prompt.render(&vars);

        // ---- First attempt. ----
        let raw = self.call_llm(&prompt, &base_user, company_id).await?;
        let parsed = parse_raw_email(&raw.text)?;
        let (subject, body, anchor_texts) = explode(parsed)?;

        let nudge = match validate(&body, banned, &input.anchors) {
            Validation::Ok => {
                return Ok(EmailDraft {
                    subject,
                    body,
                    personalization_anchors: anchor_texts,
                });
            }
            Validation::Banned(phrase) => format!(
                "Your previous draft contained the banned phrase \"{}\". \
                 Rewrite without it. All other constraints still apply.",
                phrase
            ),
            Validation::TooLong(n) => format!(
                "Body was {} words. Cap is {}. Rewrite shorter. \
                 All other constraints still apply.",
                n, MAX_BODY_WORDS
            ),
            Validation::MissingAnchors { found, required } => format!(
                "Body referenced only {} of the required {} personalization anchors. \
                 Rewrite so at least {} of these appear verbatim in the body: {}. \
                 All other constraints still apply.",
                found,
                required,
                required,
                input.anchors.join("; ")
            ),
        };

        // ---- One targeted retry. ----
        tracing::warn!(
            company_id = %company_id.0,
            reason = %nudge,
            "email-writer first draft rejected; retrying once"
        );
        let retry_user = format!("{}\n\n## Retry instruction\n{}\n", base_user, nudge);
        let raw2 = self.call_llm(&prompt, &retry_user, company_id).await?;
        let parsed2 = parse_raw_email(&raw2.text)?;
        let (subject2, body2, anchors2) = explode(parsed2)?;

        match validate(&body2, banned, &input.anchors) {
            Validation::Ok => Ok(EmailDraft {
                subject: subject2,
                body: body2,
                personalization_anchors: anchors2,
            }),
            Validation::Banned(phrase) => Err(anyhow::anyhow!(
                "email-writer produced banned phrase \"{}\" twice in a row",
                phrase
            )),
            Validation::TooLong(n) => Err(anyhow::anyhow!(
                "email-writer body exceeded {}-word cap twice in a row (was {})",
                MAX_BODY_WORDS,
                n
            )),
            Validation::MissingAnchors { found, required } => Err(anyhow::anyhow!(
                "email-writer body missing required anchors twice in a row \
                 (found {}/{} required)",
                found,
                required
            )),
        }
    }
}

impl EmailWriterAgent {
    /// Issue one LLM call using this prompt's frontmatter for model /
    /// temperature / max-tokens. JSON response mode is on so the response is
    /// directly deserializable.
    async fn call_llm(
        &self,
        prompt: &crate::platform::prompts::Prompt,
        user: &str,
        company_id: CompanyId,
    ) -> anyhow::Result<crate::platform::llm::LlmResponse> {
        let req = LlmRequest {
            model: prompt.frontmatter.model.clone(),
            system: None,
            user: user.to_string(),
            max_output_tokens: prompt.frontmatter.max_output_tokens,
            temperature: prompt.frontmatter.temperature,
            json_response: true,
            // Attribution for the cost ledger — CLAUDE.md §6.
            agent_name: Some("email-writer".to_string()),
            prompt_name: Some(prompt.frontmatter.name.clone()),
            prompt_version: Some(prompt.frontmatter.version),
            company_id: Some(company_id.0.to_string()),
        };
        self.llm
            .complete(req)
            .await
            .map_err(|e| anyhow::anyhow!("email-writer LLM call failed: {}", e))
    }
}

/// Deserialize the LLM response body into [`RawEmail`].
fn parse_raw_email(text: &str) -> anyhow::Result<RawEmail> {
    serde_json::from_str::<RawEmail>(text)
        .map_err(|e| anyhow::anyhow!("email-writer response was not valid JSON: {}", e))
}

/// Validate that the raw fields the model gave us are non-empty, and split
/// out the anchor texts.
fn explode(raw: RawEmail) -> anyhow::Result<(String, String, Vec<String>)> {
    let subject = raw
        .subject
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(|| anyhow::anyhow!("email-writer response missing `subject`"))?;
    let body = raw
        .body
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(|| anyhow::anyhow!("email-writer response missing `body`"))?;
    let anchors = raw
        .personalization_anchors
        .unwrap_or_default()
        .into_iter()
        .map(RawAnchor::into_text)
        .collect();
    Ok((subject, body, anchors))
}

/// Result of the post-LLM deterministic checks.
enum Validation {
    Ok,
    Banned(String),
    TooLong(usize),
    MissingAnchors { found: usize, required: usize },
}

fn validate(body: &str, banned: &[String], anchors: &[String]) -> Validation {
    if let Some(hit) = first_banned_hit(body, banned) {
        return Validation::Banned(hit.to_string());
    }
    let wc = word_count(body);
    if wc > MAX_BODY_WORDS {
        return Validation::TooLong(wc);
    }
    let found = anchors_in_body(body, anchors);
    // If the caller supplied fewer than the minimum anchors, only require
    // what's available — but at minimum 1 if any anchor exists.
    let required = MIN_ANCHORS_IN_BODY.min(anchors.len().max(1));
    if !anchors.is_empty() && found < required {
        return Validation::MissingAnchors { found, required };
    }
    Validation::Ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_banned_phrases_strips_comments_blanks_and_lowercases() {
        // Each line is trimmed first, then `#`-prefixed and empty lines are
        // dropped. Surviving lines are lowercased.
        let raw = "\
# header comment
I hope this email finds you well

  Circle Back
# another comment
SYNERGY
\t
   # indented comment skipped after trim
Growth Hack
";
        let out = parse_banned_phrases(raw);
        assert_eq!(
            out,
            vec![
                "i hope this email finds you well".to_string(),
                "circle back".to_string(),
                "synergy".to_string(),
                "growth hack".to_string(),
            ]
        );
    }

    #[test]
    fn first_banned_hit_is_case_insensitive() {
        let banned = vec!["synergy".to_string(), "circle back".to_string()];
        let body = "Let's Circle Back on this.";
        assert_eq!(first_banned_hit(body, &banned), Some("circle back"));
        assert_eq!(first_banned_hit("clean copy", &banned), None);
    }

    #[test]
    fn anchors_in_body_counts_unique_matches() {
        let anchors = vec![
            "same-day consults".to_string(),
            "WhatsApp without automation".to_string(),
            "missing-anchor".to_string(),
        ];
        let body = "We noticed your same-day consults and WhatsApp without automation.";
        assert_eq!(anchors_in_body(body, &anchors), 2);
    }

    #[test]
    fn word_count_handles_newlines_and_punctuation() {
        let body = "Hi Ahmed,\n\nA quick three line\nemail body here.";
        // "Hi", "Ahmed,", "A", "quick", "three", "line", "email", "body", "here."
        assert_eq!(word_count(body), 9);
    }
}
