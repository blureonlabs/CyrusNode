//! Platform — shared kernel. Capabilities the product needs from the world.
//!
//! Any feature may use any platform module. Platform modules don't know which
//! features exist; they expose generic capabilities (DB pool, LLM client,
//! event bus, prompt loader, etc.).
//!
//! Sub-modules are stubs in S1-T01 and grow through Sprint 1 stories.

pub mod core;
pub mod crawler;
pub mod db;
pub mod events;
pub mod knowledge;
pub mod llm;
pub mod observability;
pub mod prompts;
pub mod queue;
pub mod worker;
