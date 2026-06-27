//! Operator CLI. One subcommand for now: `ingest <URL>`.
//!
//! Full end-to-end flow is wired in S1-T13.

use anyhow::Result;
use clap::{Parser, Subcommand};

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
    /// Submit a single URL into the research pipeline.
    Ingest {
        /// The company's website URL.
        url: String,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    apollo::platform::observability::init()?;
    let cli = Cli::parse();

    match cli.command {
        Command::Ingest { url } => {
            // TODO S1-T13: submit url.submitted event; wait for email.drafted; print to stdout.
            tracing::info!(%url, "ingest stub — full flow lands in S1-T13");
            println!("queued (stub): {url}");
        }
    }
    Ok(())
}
