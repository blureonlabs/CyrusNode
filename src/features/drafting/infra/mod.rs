//! Adapters that implement the drafting domain's ports.
//!
//! Allowed deps: this feature's `domain`, `crate::platform::*`, third-party SDKs.
//! Forbidden: `application/`, `presentation/`, other features.

pub mod email_writer_agent;

pub use email_writer_agent::EmailWriterAgent;
