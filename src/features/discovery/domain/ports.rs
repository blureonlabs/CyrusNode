//! Ports — the trait `application/` asks of the outside world.
//!
//! `infra/` implements this trait against a real source (Google Places in V1).

use async_trait::async_trait;

use super::{Candidate, DiscoveryError, NicheQuery};

/// Abstract source of company candidates. One implementation per source;
/// V1 ships with `GooglePlacesAdapter`.
#[async_trait]
pub trait DiscoveryPort: Send + Sync {
    /// Look up candidates matching `query`. Implementations must:
    ///   * truncate to at most `query.limit` results,
    ///   * never return duplicates within the same call,
    ///   * map auth failures to `DiscoveryError::MissingKey` and quota to
    ///     `DiscoveryError::QuotaExhausted`.
    async fn find(&self, query: &NicheQuery) -> Result<Vec<Candidate>, DiscoveryError>;
}
