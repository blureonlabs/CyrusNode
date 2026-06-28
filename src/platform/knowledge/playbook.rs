use serde::{Deserialize, Serialize};

/// YAML frontmatter at the top of every playbook file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaybookFrontmatter {
    /// Stable key (e.g. `dental_clinic`, `limousine_uae`). Matches the
    /// `industry` field that flows in via `DraftInput`.
    pub key: String,
    /// Human-friendly name for dashboards.
    pub display_name: String,
    /// Locale code for tone, e.g. `en_AE`. Optional — drafting falls back to
    /// the global default if unset.
    #[serde(default)]
    pub locale_tone: Option<String>,
}

/// Parsed industry playbook. Surfaces the operator-curated guidance an
/// agent needs without exposing the raw Markdown to LLM prompts in full.
#[derive(Debug, Clone)]
pub struct IndustryPlaybook {
    pub frontmatter: PlaybookFrontmatter,
    /// Body of the `## Tone` section. The email writer injects this into
    /// the `{{ tone }}` template variable.
    pub tone: String,
    /// Bullets under `## Typical pains`. Useful for the opportunity finder
    /// (Sprint 2) and the email writer's "evidence anchor" reasoning.
    pub typical_pains: Vec<String>,
    /// Bullets under `## Typical opportunities`. Each entry references an
    /// AI services catalogue id followed by " — why it usually fits".
    pub typical_opportunities: Vec<String>,
    /// Bullets under `## Banned for this industry`. Augments the global
    /// banned-phrases list at draft-time.
    pub banned_for_industry: Vec<String>,
}
