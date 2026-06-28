//! Discovery domain — types + rules. Zero I/O.
//!
//! Forbidden imports in this folder (enforced by `scripts/check-domain-imports.sh`):
//!   `sqlx`, `reqwest`, `axum`, `tokio::net`, `tokio::fs`,
//!   `crate::platform::{db,llm,crawler,events,queue,prompts}`,
//!   `crate::features::*::infra`.

mod candidate;
mod ports;

pub use candidate::{Candidate, DiscoveryError, NicheQuery};
pub use ports::DiscoveryPort;
