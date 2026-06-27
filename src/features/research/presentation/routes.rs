//! HTTP routes for the research feature.

/// Returns the Axum router for research endpoints.
// TODO: POST /companies, GET /companies/{id} (S1-T13, Sprint 2)
pub fn routes() -> axum::Router<()> {
    axum::Router::new()
}
