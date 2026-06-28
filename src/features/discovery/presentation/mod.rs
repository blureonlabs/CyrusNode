//! HTTP routes + event subscriptions for the discovery feature.
//!
//! Allowed deps: this feature's `application`, `domain`, `crate::platform::core`,
//! `axum`. Forbidden: `infra/`.

mod routes;
mod subscriptions;

pub use routes::routes;
pub use subscriptions::subscriptions;
