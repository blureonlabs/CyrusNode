//! HTTP routes for the drafting feature.

/// Returns the Axum router for drafting endpoints.
// TODO: POST /drafts, GET /drafts/{id} (later sprint).
pub fn routes() -> axum::Router<()> {
    axum::Router::new()
}
