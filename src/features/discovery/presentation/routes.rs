//! HTTP routes for discovery. Empty for V1; the CLI is the only caller.

/// Returns the Axum router for discovery endpoints.
// TODO Sprint 4: POST /discovery/runs, GET /discovery/runs/{id}
pub fn routes() -> axum::Router<()> {
    axum::Router::new()
}
