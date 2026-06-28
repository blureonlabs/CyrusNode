//! Use cases — the workflows a user can trigger for the discovery feature.
//!
//! Allowed deps: this feature's `domain`, `crate::platform::core`, `async-trait`.
//! Forbidden: `infra/`, `presentation/`, other features.

mod discover_leads;

pub use discover_leads::DiscoverLeads;
