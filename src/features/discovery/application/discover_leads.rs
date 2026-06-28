//! `DiscoverLeads` — the V1 discovery use case.
//!
//! Calls the port, dedups by `source_id`, and turns "zero results" into the
//! terminal `DiscoveryError::Empty` so the operator can refine the query
//! (per `apollo/03-agents/lead-discovery.md`).

use std::sync::Arc;

use crate::features::discovery::domain::{Candidate, DiscoveryError, DiscoveryPort, NicheQuery};

/// Use case wired in `configure.rs`. Holds the port behind an `Arc` so the
/// owning module can hand the same instance to multiple callers (CLI, HTTP,
/// event subscriber).
pub struct DiscoverLeads {
    port: Arc<dyn DiscoveryPort>,
}

impl DiscoverLeads {
    /// Build the use case around a concrete port implementation.
    pub fn new(port: Arc<dyn DiscoveryPort>) -> Self {
        Self { port }
    }

    /// Run the discovery workflow for `query`.
    ///
    /// Returns the deduped candidate list on success. Returns
    /// `DiscoveryError::Empty` when the port returned no candidates (after
    /// dedup) — terminal per the spec. Propagates `MissingKey`,
    /// `QuotaExhausted`, `Upstream`, and `Network` unchanged.
    pub async fn run(&self, query: NicheQuery) -> Result<Vec<Candidate>, DiscoveryError> {
        let mut results = self.port.find(&query).await?;
        // Dedup by source_id. `dedup_by` requires neighbors to compare equal,
        // so sort first.
        results.sort_by(|a, b| a.source_id.cmp(&b.source_id));
        results.dedup_by(|a, b| a.source_id == b.source_id);
        if results.is_empty() {
            return Err(DiscoveryError::Empty);
        }
        Ok(results)
    }
}
