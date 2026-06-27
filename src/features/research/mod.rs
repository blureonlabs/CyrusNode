//! `research` — turn a company URL into a dossier base
//! (crawl → extract → SEO audit → tech detection → business intelligence).
//!
//! Four boxes. The dependency rule: presentation → application → domain ← infra.
//! `domain/` has zero I/O deps. See `apollo/02-architecture/feature-layout.md`.

pub mod application;
pub mod domain;
pub mod infra;
pub mod presentation;

mod configure;

pub use configure::{configure, ResearchDeps, ResearchModule};
