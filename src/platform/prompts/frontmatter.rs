//! YAML frontmatter parsing for prompt Markdown files.
//!
//! A prompt file begins with a `---` delimited YAML block, followed by the
//! Markdown body that is fed to the LLM. This module owns the split + decode.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Parsed frontmatter from a prompt Markdown file.
///
/// Required fields: `name`, `version`, `model`, `max_output_tokens`.
/// Optional fields default to sensible values:
/// - `temperature` defaults to `0.2` (Apollo's default-cool setting).
/// - `inputs` defaults to an empty list (prompt takes no template variables).
/// - `outputs_schema` and `description` default to `None`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Frontmatter {
    /// Unique prompt name (matches the `prompt_versions` table key).
    pub name: String,
    /// Monotonic version. Bump on every body or frontmatter change.
    pub version: u32,
    /// Model identifier (e.g. `gemini-2.5-flash`, `claude-haiku-4-5`).
    pub model: String,
    /// Sampling temperature. Defaults to 0.2 when omitted.
    #[serde(default = "default_temperature")]
    pub temperature: f32,
    /// Hard cap on output tokens for this prompt.
    pub max_output_tokens: u32,
    /// Names of template variables the body expects.
    #[serde(default)]
    pub inputs: Vec<String>,
    /// Relative path to the JSON schema used to validate output.
    #[serde(default)]
    pub outputs_schema: Option<String>,
    /// Human-readable description (shown in operator UI).
    #[serde(default)]
    pub description: Option<String>,
}

fn default_temperature() -> f32 {
    0.2
}

/// Errors produced while splitting or decoding a prompt's frontmatter.
#[derive(Debug, Error)]
pub enum FrontmatterError {
    /// The file did not start with a `---` delimiter.
    #[error("missing opening --- delimiter")]
    MissingOpening,
    /// The opening `---` was found but never closed by another `---`.
    #[error("missing closing --- delimiter")]
    MissingClosing,
    /// The YAML inside the frontmatter failed to decode into [`Frontmatter`].
    #[error(transparent)]
    Yaml(#[from] serde_yaml::Error),
}

/// Split a Markdown file into `(frontmatter, body)`.
///
/// # Inputs
/// - `content`: the raw file text, including the `---` fences.
///
/// # Returns
/// On success, the decoded [`Frontmatter`] and a borrowed slice into `content`
/// containing the Markdown body (whitespace at the very start trimmed once).
///
/// # Failure modes
/// - [`FrontmatterError::MissingOpening`] when the file does not start with `---`.
/// - [`FrontmatterError::MissingClosing`] when there is no terminating `---` line.
/// - [`FrontmatterError::Yaml`] when YAML decoding fails.
pub fn split(content: &str) -> Result<(Frontmatter, &str), FrontmatterError> {
    let trimmed = content.trim_start();
    if !trimmed.starts_with("---") {
        return Err(FrontmatterError::MissingOpening);
    }
    // Skip the first `---` line.
    let after_open = trimmed.strip_prefix("---").unwrap_or(trimmed);
    let after_open = after_open.strip_prefix('\n').unwrap_or(after_open);
    // Find the closing `\n---`.
    let close_idx = after_open
        .find("\n---")
        .ok_or(FrontmatterError::MissingClosing)?;
    let yaml = &after_open[..close_idx];
    // Skip past "\n---" (4 bytes) then optional newline.
    let body = &after_open[close_idx + 4..];
    let body = body.strip_prefix('\n').unwrap_or(body);
    let fm: Frontmatter = serde_yaml::from_str(yaml)?;
    Ok((fm, body))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_business_summary_fixture() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("apollo/04-prompts/business-summary.md");
        let content = std::fs::read_to_string(&path).expect("fixture exists");
        let (fm, body) = split(&content).expect("parses");
        assert_eq!(fm.name, "business-summary");
        assert_eq!(fm.version, 1);
        assert_eq!(fm.max_output_tokens, 4000);
        assert!(body.contains("senior B2B analyst"));
    }

    #[test]
    fn parses_email_writer_fixture() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("apollo/04-prompts/email-writer.md");
        let content = std::fs::read_to_string(&path).expect("fixture exists");
        let (fm, _body) = split(&content).expect("parses");
        assert_eq!(fm.name, "email-writer");
        // Gemini-only routing per ADR-012; model was claude-haiku-4-5 in ADR-010.
        assert_eq!(fm.model, "gemini-2.5-flash");
        assert_eq!(fm.max_output_tokens, 4000);
        assert!(fm.inputs.contains(&"business_intel".to_string()));
    }

    #[test]
    fn missing_opening_errors() {
        let err = split("no frontmatter here").unwrap_err();
        assert!(matches!(err, FrontmatterError::MissingOpening));
    }

    #[test]
    fn missing_closing_errors() {
        let err = split("---\nname: x\nversion: 1\n").unwrap_err();
        assert!(matches!(err, FrontmatterError::MissingClosing));
    }

    #[test]
    fn temperature_defaults_to_point_two() {
        let content = "---\nname: t\nversion: 1\nmodel: m\nmax_output_tokens: 10\n---\nbody";
        let (fm, body) = split(content).expect("parses");
        assert!((fm.temperature - 0.2).abs() < f32::EPSILON);
        assert_eq!(body, "body");
    }
}
