//! Hot-reloading prompt loader. Reads Markdown files under `apollo/04-prompts/`.
//!
//! Stub. Wired in S1-T06.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptFrontmatter {
    pub name: String,
    pub version: u32,
    pub model: String,
    pub temperature: f32,
    pub max_output_tokens: u32,
}
