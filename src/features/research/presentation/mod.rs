//! HTTP routes + event subscriptions for the research feature.
//!
//! Allowed deps: this feature's `application`, `domain`, `crate::platform::core`,
//! `axum`. Forbidden: `infra/`.

pub mod routes;
pub mod subscriptions;

pub use routes::routes;
pub use subscriptions::subscriptions;
