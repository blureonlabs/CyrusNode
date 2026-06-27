//! Use cases — the workflows a user can trigger.
//!
//! Allowed deps: this feature's `domain`, `crate::platform::core`, `async-trait`.
//! Forbidden: `infra/`, `presentation/`, other features.

mod research_company;

pub use research_company::ResearchCompany;
