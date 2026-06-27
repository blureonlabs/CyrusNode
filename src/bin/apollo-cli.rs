//! Operator CLI.
//!
//! Subcommands:
//! - `apollo migrate` — apply checked-in migrations against `DATABASE_URL`.
//! - `apollo ingest <URL>` — submit a URL into the research pipeline (stub until S1-T13).

use anyhow::Result;
use clap::{Parser, Subcommand};

use apollo::platform::db::{build_pool, run_migrations, DbConfig};

#[derive(Parser)]
#[command(
    name = "apollo",
    version,
    about = "Apollo — AI consulting platform CLI"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Apply database migrations against the configured `DATABASE_URL`.
    Migrate,

    /// Submit a single URL into the research pipeline.
    Ingest {
        /// The company's website URL.
        url: String,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let _ = dotenvy::from_filename(".env.local");
    apollo::platform::observability::init()?;
    let cli = Cli::parse();

    match cli.command {
        Command::Migrate => {
            let cfg = DbConfig::from_env()?;
            let pool = build_pool(&cfg).await?;
            run_migrations(&pool).await?;
            println!("✓ migrations applied");
        }
        Command::Ingest { url } => {
            // TODO S1-T13: submit url.submitted event; wait for email.drafted; print to stdout.
            tracing::info!(%url, "ingest stub — full flow lands in S1-T13");
            println!("queued (stub): {url}");
        }
    }
    Ok(())
}
