//! In-memory representation of a loaded prompt.

use sha2::{Digest, Sha256};

use super::frontmatter::Frontmatter;

/// A loaded prompt.
///
/// `body` is the Markdown body (post-frontmatter). `body_sha256` is the
/// hex-encoded SHA-256 of `body` — used to enforce the "bump the version when
/// the body changes" rule in [`PromptLoader`](super::loader::PromptLoader).
#[derive(Debug, Clone)]
pub struct Prompt {
    /// Decoded frontmatter.
    pub frontmatter: Frontmatter,
    /// Markdown body fed to the LLM, after rendering.
    pub body: String,
    /// Hex-encoded SHA-256 of `body`.
    pub body_sha256: String,
}

impl Prompt {
    /// Construct a [`Prompt`], computing the body hash eagerly.
    pub fn new(frontmatter: Frontmatter, body: String) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(body.as_bytes());
        let body_sha256 = format!("{:x}", hasher.finalize());
        Self {
            frontmatter,
            body,
            body_sha256,
        }
    }

    /// Render with a minimal `{{ var }}` substitution.
    ///
    /// Variables are looked up in `vars`. Unknown variables are left as-is in
    /// the output so they show up loudly in logs rather than silently
    /// disappearing.
    ///
    /// S1-T06: deliberately simple. Switch to Tera if it becomes a bottleneck
    /// or the templates grow conditionals.
    pub fn render(&self, vars: &std::collections::HashMap<&str, String>) -> String {
        let mut out = self.body.clone();
        for (k, v) in vars {
            let needle = format!("{{{{ {} }}}}", k);
            out = out.replace(&needle, v);
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn fm() -> Frontmatter {
        Frontmatter {
            name: "t".into(),
            version: 1,
            model: "m".into(),
            temperature: 0.2,
            max_output_tokens: 100,
            inputs: vec![],
            outputs_schema: None,
            description: None,
        }
    }

    #[test]
    fn renders_known_variable() {
        let p = Prompt::new(fm(), "hello {{ name }}!".into());
        let mut vars = HashMap::new();
        vars.insert("name", "world".to_string());
        assert_eq!(p.render(&vars), "hello world!");
    }

    #[test]
    fn leaves_unknown_variable_intact() {
        let p = Prompt::new(fm(), "hi {{ missing }}".into());
        let out = p.render(&HashMap::new());
        assert_eq!(out, "hi {{ missing }}");
    }

    #[test]
    fn hashes_body_deterministically() {
        let a = Prompt::new(fm(), "abc".into());
        let b = Prompt::new(fm(), "abc".into());
        assert_eq!(a.body_sha256, b.body_sha256);
        assert_eq!(a.body_sha256.len(), 64);
    }
}
