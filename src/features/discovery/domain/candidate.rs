//! Discovery domain types — `NicheQuery`, `Candidate`, and the typed errors
//! the port may return.
//!
//! Zero I/O. No `reqwest`, no `sqlx`, no platform modules beyond
//! `crate::platform::core` (and we do not need that here).

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Input to the discovery use case: a free-text niche, a city, a country, and
/// a hard cap on results.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NicheQuery {
    /// Free-text industry keyword (e.g. "dental clinic", "limousine service").
    pub niche: String,
    /// City name. Combined with country to form the Places query.
    pub city: String,
    /// ISO 3166-1 alpha-2 country code (default "AE").
    pub country: String,
    /// Max number of candidates to return (cap at 50; Google Places returns ≤ 20 per page).
    pub limit: u32,
}

/// A single discovered company. The crawler later turns this into a full
/// dossier; for V1 we only commit to the minimal surface the spec promises.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Candidate {
    /// Display name as returned by the source.
    pub name: String,
    /// May be `None` for places without a registered website.
    pub url: Option<String>,
    /// E.164-formatted phone number when available.
    pub phone: Option<String>,
    /// WhatsApp number. V1: always `None` — the crawler extracts this later.
    pub whatsapp: Option<String>,
    /// Human-readable address from the source.
    pub address: Option<String>,
    /// Source-reported rating, typically 0..5.
    pub rating: Option<f32>,
    /// Source-reported review count.
    pub review_count: Option<u32>,
    /// Source identifier — e.g. `"google_places"`.
    pub source: String,
    /// Stable per-source id (Google Places `place.id` for the gmaps adapter).
    pub source_id: String,
    /// Adapter-assigned confidence in `[0, 1]`. See adapter docs for the rubric.
    pub confidence: f32,
}

/// Failure modes the port may surface. See `apollo/03-agents/lead-discovery.md`
/// for the operator-facing semantics.
#[derive(Debug, Error)]
pub enum DiscoveryError {
    /// API key missing — terminal, surfaced to operator.
    #[error("missing api key")]
    MissingKey,
    /// Quota exhausted — transient, backoff and retry tomorrow.
    #[error("quota exhausted")]
    QuotaExhausted,
    /// Zero candidates returned — terminal so operator can refine the query.
    #[error("zero candidates")]
    Empty,
    /// Upstream returned an unexpected non-2xx response.
    #[error("upstream error: {0}")]
    Upstream(String),
    /// Transport-level failure (DNS, TCP, TLS, timeout). The infra adapter
    /// flattens its HTTP-client error into a string so domain stays free of
    /// I/O crate types (the lint forbids `reqwest` in `domain/`).
    #[error("network error: {0}")]
    Network(String),
}
