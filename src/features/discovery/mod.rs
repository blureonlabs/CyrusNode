//! `discovery` — turn a niche + city into a list of candidate companies.
//!
//! Four boxes. The dependency rule: presentation → application → domain ← infra.
//! `domain/` has zero I/O deps. See `apollo/02-architecture/feature-layout.md`
//! and `apollo/03-agents/lead-discovery.md`.

pub mod application;
pub mod domain;
pub mod infra;
pub mod presentation;

mod configure;

pub use configure::{configure, DiscoveryDeps, DiscoveryModule};
