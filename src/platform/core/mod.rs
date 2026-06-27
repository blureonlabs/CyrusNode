//! Shared base types used across every feature.
//!
//! Allowed deps inside `domain/` modules: only this module and pure-data crates
//! (`serde`, `chrono`, `uuid`, `ulid`, `thiserror`). No I/O.

mod clock;
mod ids;

pub use clock::{Clock, SystemClock};
pub use ids::{CompanyId, CorrelationId, TenantId};
