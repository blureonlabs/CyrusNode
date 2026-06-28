//! Features — what Apollo *does*. One module per bounded context.
//!
//! Cross-feature `use crate::features::other::…` is forbidden. Features
//! communicate via events on the bus (`crate::platform::events`) or via shared
//! types in `crate::platform::core`.
//!
//! Sprint 1 lights up:
//! - `research`  — crawl → audit → bizintel
//! - `drafting`  — email writer (basic)
//!
//! Later sprints add: `discovery`, `synthesis`, `review`, `outreach`,
//! `conversations`, `meetings`, `intelligence`.

pub mod discovery;
pub mod drafting;
pub mod research;
pub mod review;
