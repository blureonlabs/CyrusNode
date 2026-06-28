use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use anyhow::Result;
use tokio::sync::RwLock;

use super::playbook::{IndustryPlaybook, PlaybookFrontmatter};

/// Loads industry playbooks from a directory of Markdown files.
///
/// Cheap to clone — wraps an `Arc<RwLock<HashMap>>`. Missing playbook keys
/// return `None` so callers can fall back to defaults.
#[derive(Clone)]
pub struct PlaybookLoader {
    cache: Arc<RwLock<HashMap<String, IndustryPlaybook>>>,
}

impl PlaybookLoader {
    /// Scan `dir` for `*.md` files and parse each into an [`IndustryPlaybook`].
    /// Files starting with `_` (e.g. `_template.md`) and `README.md` are skipped.
    pub async fn load(dir: impl AsRef<Path>) -> Result<Self> {
        let loader = Self {
            cache: Arc::new(RwLock::new(HashMap::new())),
        };
        loader.scan(dir.as_ref()).await?;
        Ok(loader)
    }

    async fn scan(&self, dir: &Path) -> Result<()> {
        let mut next = HashMap::new();
        let mut entries = tokio::fs::read_dir(dir).await?;
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) != Some("md") {
                continue;
            }
            let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
            if stem.starts_with('_') || stem.eq_ignore_ascii_case("README") {
                continue;
            }
            let raw = match tokio::fs::read_to_string(&path).await {
                Ok(s) => s,
                Err(e) => {
                    tracing::warn!(path = %path.display(), error = %e, "playbook read failed");
                    continue;
                }
            };
            match parse(&raw) {
                Ok(book) => {
                    next.insert(book.frontmatter.key.clone(), book);
                }
                Err(e) => {
                    tracing::warn!(path = %path.display(), error = %e, "playbook parse failed");
                }
            }
        }
        let mut cache = self.cache.write().await;
        *cache = next;
        tracing::info!(count = cache.len(), "industry playbooks loaded");
        Ok(())
    }

    /// Get a playbook by its frontmatter `key`. Returns `None` if absent.
    pub async fn get(&self, key: &str) -> Option<IndustryPlaybook> {
        self.cache.read().await.get(key).cloned()
    }

    /// List all loaded keys. Useful for diagnostics / `apollo industries`.
    pub async fn keys(&self) -> Vec<String> {
        self.cache.read().await.keys().cloned().collect()
    }
}

fn parse(content: &str) -> Result<IndustryPlaybook> {
    let trimmed = content.trim_start();
    if !trimmed.starts_with("---") {
        anyhow::bail!("missing opening --- delimiter");
    }
    let after_open = trimmed.strip_prefix("---").unwrap_or(trimmed);
    let after_open = after_open.strip_prefix('\n').unwrap_or(after_open);
    let close_idx = after_open
        .find("\n---")
        .ok_or_else(|| anyhow::anyhow!("missing closing --- delimiter"))?;
    let yaml = &after_open[..close_idx];
    let body = &after_open[close_idx + 4..];
    let body = body.strip_prefix('\n').unwrap_or(body);

    let frontmatter: PlaybookFrontmatter = serde_yaml::from_str(yaml)?;

    Ok(IndustryPlaybook {
        frontmatter,
        tone: extract_section(body, "Tone").unwrap_or_default(),
        typical_pains: extract_bullets(body, "Typical pains"),
        typical_opportunities: extract_bullets(body, "Typical opportunities"),
        banned_for_industry: extract_bullets(body, "Banned for this industry"),
    })
}

/// Return the body of `## <header>` until the next `## ` heading.
fn extract_section(body: &str, header: &str) -> Option<String> {
    let needle_lower = format!("## {}", header.to_lowercase());
    let mut found = false;
    let mut out: Vec<&str> = Vec::new();
    for line in body.lines() {
        let line_trimmed_lower = line.trim().to_lowercase();
        if !found {
            if line_trimmed_lower.starts_with(&needle_lower) {
                found = true;
            }
            continue;
        }
        if line.starts_with("## ") {
            break;
        }
        out.push(line);
    }
    if !found {
        return None;
    }
    let joined = out.join("\n").trim().to_string();
    if joined.is_empty() {
        None
    } else {
        Some(joined)
    }
}

fn extract_bullets(body: &str, header: &str) -> Vec<String> {
    let section = match extract_section(body, header) {
        Some(s) => s,
        None => return Vec::new(),
    };
    section
        .lines()
        .filter_map(|line| {
            let t = line.trim_start();
            t.strip_prefix("- ")
                .or_else(|| t.strip_prefix("* "))
                .map(|s| s.trim().to_string())
        })
        .filter(|s| !s.is_empty())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"---
key: dental_clinic
display_name: Dental Clinic
locale_tone: en_AE
---

# Industry: Dental Clinic

## Typical pains
- After-hours WhatsApp inquiries
- Reminder follow-up workload
- Slow review collection

## Typical opportunities
- whatsapp-receptionist — most common fit
- review-automation — broadly relevant

## Tone
Direct, peer-to-peer, warm but not chatty. Reference specific services.

## Banned for this industry
- Claims about clinical outcomes
- Anything that could read as medical advice
"#;

    #[test]
    fn parses_real_playbook_shape() {
        let book = parse(SAMPLE).expect("parses");
        assert_eq!(book.frontmatter.key, "dental_clinic");
        assert_eq!(book.frontmatter.display_name, "Dental Clinic");
        assert!(book.tone.starts_with("Direct, peer-to-peer"));
        assert_eq!(book.typical_pains.len(), 3);
        assert_eq!(book.typical_opportunities.len(), 2);
        assert_eq!(book.banned_for_industry.len(), 2);
    }

    #[test]
    fn missing_section_returns_empty() {
        let no_tone = r#"---
key: x
display_name: X
---

# Foo

## Typical pains
- one
"#;
        let book = parse(no_tone).expect("parses");
        assert!(book.tone.is_empty());
        assert_eq!(book.typical_pains.len(), 1);
        assert!(book.typical_opportunities.is_empty());
    }
}
