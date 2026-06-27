//! HTTP entry point for Apollo.
//!
//! Reads `PORT` env (default 8080). Bootstraps every feature's routes via
//! `apollo::bootstrap::build()`. No business logic lives here.

use anyhow::Result;
use tokio::net::TcpListener;

#[tokio::main]
async fn main() -> Result<()> {
    apollo::platform::observability::init()?;

    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(8080);

    let boot = apollo::bootstrap::build()?;

    let addr = format!("0.0.0.0:{port}");
    let listener = TcpListener::bind(&addr).await?;
    tracing::info!(%addr, "apollo-api listening");

    axum::serve(listener, boot.router).await?;
    Ok(())
}
