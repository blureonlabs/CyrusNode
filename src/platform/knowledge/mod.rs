//! Industry playbook loader. Reads Markdown files under
//! `apollo/05-knowledge/industries/` and exposes the tone, typical pains, and
//! opportunity hints to features that need them (drafting, opportunity-finder).
//!
//! Same shape as `platform::prompts`: scan a directory, parse YAML frontmatter,
//! extract structured sections. Hot reload is not wired in V1 — operator
//! restarts the CLI / worker after editing a playbook.

mod loader;
mod playbook;

pub use loader::PlaybookLoader;
pub use playbook::{IndustryPlaybook, PlaybookFrontmatter};
