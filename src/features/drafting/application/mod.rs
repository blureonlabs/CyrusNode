//! Use cases — the workflows a user can trigger.
//!
//! Allowed deps: this feature's `domain`, `crate::platform::core`, `async-trait`.
//! Forbidden: `infra/`, `presentation/`, other features.

mod draft_outreach;

pub use draft_outreach::DraftOutreach;
