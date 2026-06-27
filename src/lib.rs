//! Apollo — AI Consulting Platform.
//!
//! Single crate, organized by **feature**. Layer-by-feature, not feature-by-layer.
//!
//! ```text
//! src/
//!   platform/   shared kernel — capabilities the product needs from the world
//!   features/   what the product does — one module per bounded context
//!   bin/        thin entry points (api, worker, cli)
//! ```
//!
//! See `apollo/02-architecture/feature-layout.md` for the full rulebook.
//!
//! The one dependency rule (enforced in CI by `scripts/check-domain-imports.sh`):
//!
//! ```text
//! presentation → application → domain ← infrastructure
//! ```
//!
//! Inside a feature, `domain/` may not import anything that does I/O — no
//! `sqlx`, no `reqwest`, no `axum`, no other feature.

pub mod bootstrap;
pub mod features;
pub mod platform;
