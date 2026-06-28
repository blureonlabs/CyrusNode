//! Review feature — operator-driven queue that flips through saved dossiers
//! and approves, edits, rejects, or skips each drafted outreach email.
//!
//! V1 has no HTTP or event-bus surface; the feature is driven entirely from
//! the CLI via [`configure`] and [`application::RunReview`].

pub mod application;
pub mod domain;
pub mod infra;

mod configure;

pub use configure::{configure, ReviewDeps, ReviewModule};
