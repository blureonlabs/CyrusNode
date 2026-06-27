//! Hot-reloading prompt loader. Reads Markdown files under `apollo/04-prompts/`.
//!
//! Prompts are data, not code (see `CLAUDE.md` §5). This module owns the
//! filesystem-facing pieces:
//!
//! - [`Frontmatter`] / [`split`] — YAML frontmatter parsing.
//! - [`Prompt`] — an in-memory prompt with body and SHA-256 of the body.
//! - [`PromptLoader`] — directory scan + `notify`-based hot reload.

mod frontmatter;
mod loader;
mod prompt;

pub use frontmatter::{split, Frontmatter, FrontmatterError};
pub use loader::PromptLoader;
pub use prompt::Prompt;
