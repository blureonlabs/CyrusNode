//! Repositories — typed access to core Postgres tables.
//!
//! Each repo owns one table. All take a cloned `PgPool` so the same repo
//! can be moved across tasks freely. No `Arc<Self>` wrapping needed —
//! `PgPool` is internally Arc'd by SQLx.

mod artifacts;
mod companies;
mod events;
mod jobs;
mod llm_calls;

pub use artifacts::{ArtifactsRepo, NewArtifact};
pub use companies::CompaniesRepo;
pub use events::{EventsRepo, NewEvent};
pub use jobs::{JobsRepo, NewJob};
pub use llm_calls::{LlmCallsRepo, NewLlmCall};
