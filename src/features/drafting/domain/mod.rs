//! Drafting domain — types + rules. Zero I/O.
//!
//! Forbidden imports in this folder (enforced by `scripts/check-domain-imports.sh`):
//!   `sqlx`, `reqwest`, `axum`, `tokio::net`, `tokio::fs`,
//!   `crate::platform::{db,llm,crawler,events,queue,prompts}`,
//!   `crate::features::*::infra`.

mod draft;
mod ports;

pub use draft::EmailDraft;
pub use ports::DraftPort;
