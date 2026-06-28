//! Operator CLI.
//!
//! Subcommands:
//! - `apollo migrate`              — apply DB migrations.
//! - `apollo crawl <URL>`          — debug crawl only (no LLM, no DB).
//! - `apollo ingest <URL> --name X` — full E2E: crawl → extract → seo → bizintel → email.

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use clap::{Parser, Subcommand};

use apollo::features::drafting::application::DraftOutreach;
use apollo::features::drafting::domain::DraftInput;
use apollo::features::drafting::infra::EmailWriterAgent;
use apollo::features::research::application::ResearchCompany;
use apollo::features::research::infra::{
    BizIntelAgent, HttpCrawlerAdapter, MarkdownExtractorAdapter, StaticSeoAuditor,
};
use apollo::platform::core::CompanyId;
use apollo::platform::crawler::{CrawlerConfig, HttpCrawler};
use apollo::platform::db::{build_pool, run_migrations, DbConfig};
use apollo::platform::knowledge::PlaybookLoader;
use apollo::platform::llm::{GeminiClient, LlmClient};
use apollo::platform::prompts::PromptLoader;

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
    /// Apply database migrations against `DATABASE_URL`.
    Migrate,

    /// Crawl a single URL — no LLM, no DB. Prints CrawlSummary as JSON.
    Crawl {
        url: String,
        #[arg(long)]
        max_pages: Option<u32>,
    },

    /// Full E2E pipeline against a URL: crawl → extract → seo → bizintel → email.
    /// Skips the DB (no persistence). Requires GEMINI_API_KEY + ANTHROPIC_API_KEY.
    Ingest {
        /// The company's website URL.
        url: String,
        /// Operator's first name for the email sign-off.
        #[arg(long, default_value = "Hari")]
        signature: String,
        /// Industry key (e.g. `dental_clinic`).
        #[arg(long)]
        industry: Option<String>,
        /// Recipient's first name for salutation.
        #[arg(long)]
        contact: Option<String>,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    // Try `.env.local` first (Apollo convention), then `.env` (Node/Next.js convention).
    let _ = dotenvy::from_filename(".env.local");
    let _ = dotenvy::from_filename(".env");
    apollo::platform::observability::init()?;
    let cli = Cli::parse();

    match cli.command {
        Command::Migrate => {
            let cfg = DbConfig::from_env()?;
            let pool = build_pool(&cfg).await?;
            run_migrations(&pool).await?;
            println!("✓ migrations applied");
        }
        Command::Crawl { url, max_pages } => {
            let mut cfg = CrawlerConfig::default();
            if let Some(n) = max_pages {
                cfg.max_pages = n;
            }
            let crawler = HttpCrawler::new(cfg)?;
            let correlation_id = format!("cli-{}", chrono::Utc::now().timestamp());
            let summary = crawler.crawl(&url, &correlation_id).await?;
            println!("{}", serde_json::to_string_pretty(&summary)?);
        }
        Command::Ingest {
            url,
            signature,
            industry,
            contact,
        } => {
            run_ingest(&url, &signature, industry.as_deref(), contact.as_deref()).await?;
        }
    }
    Ok(())
}

async fn run_ingest(
    url: &str,
    signature: &str,
    industry: Option<&str>,
    contact: Option<&str>,
) -> Result<()> {
    eprintln!("==> loading prompts + industry playbooks...");
    let prompts = PromptLoader::load("apollo/04-prompts").await?;
    let playbooks = PlaybookLoader::load("apollo/05-knowledge/industries").await?;
    let known_industries = playbooks.keys().await;
    eprintln!("    industries loaded: {known_industries:?}");

    eprintln!("==> building Gemini client (single-provider per ADR-012)...");
    let gemini: Arc<dyn LlmClient> = Arc::new(GeminiClient::from_env()?);

    eprintln!("==> wiring research feature...");
    let crawler = Arc::new(HttpCrawler::new(CrawlerConfig::default())?);
    let crawl = Arc::new(HttpCrawlerAdapter::new(crawler));
    let extract = Arc::new(MarkdownExtractorAdapter);
    let seo = Arc::new(StaticSeoAuditor);
    let bizintel = Arc::new(BizIntelAgent::new(gemini.clone(), prompts.clone()));

    let research = ResearchCompany::new(crawl, extract, seo, bizintel);

    eprintln!("==> wiring drafting feature...");
    let banned_phrases_path = PathBuf::from("apollo/05-knowledge/banned-email-phrases.txt");
    let draft = Arc::new(EmailWriterAgent::new(
        gemini.clone(),
        prompts.clone(),
        playbooks.clone(),
        banned_phrases_path,
    ));
    let drafter = DraftOutreach::new(draft);

    let company_id = CompanyId(uuid::Uuid::new_v4());

    eprintln!("==> running research pipeline for {url}...");
    let dossier = research.run(company_id, url).await?;

    eprintln!("==> drafting email...");
    let anchors = anchors_from(&dossier.summary);
    let draft_input = DraftInput {
        what_they_sell: dossier.summary.what_they_sell.clone(),
        value_props: dossier.summary.value_props.clone(),
        anchors,
        industry: industry.map(|s| s.to_string()),
        contact_first_name: contact.map(|s| s.to_string()),
        operator_signature: signature.to_string(),
    };
    let email = drafter.run(company_id, &draft_input).await?;

    println!();
    println!("══════════════════════════════════════════════════════════════");
    println!("BUSINESS SUMMARY");
    println!("══════════════════════════════════════════════════════════════");
    println!("What they sell : {}", dossier.summary.what_they_sell);
    println!("Who they serve : {}", dossier.summary.who_they_serve);
    println!("Maturity       : {:?}", dossier.summary.maturity_tier);
    println!("Confidence     : {:.2}", dossier.summary.confidence);
    println!();
    println!("Value props:");
    for vp in &dossier.summary.value_props {
        println!("  • {vp}");
    }
    println!();
    println!("══════════════════════════════════════════════════════════════");
    println!("EMAIL DRAFT");
    println!("══════════════════════════════════════════════════════════════");
    println!("Subject: {}", email.subject);
    println!();
    println!("{}", email.body);
    println!();
    println!("Anchors used: {}", email.personalization_anchors.join(", "));
    Ok(())
}

fn anchors_from(summary: &apollo::features::research::domain::BusinessSummary) -> Vec<String> {
    // Pull the first two value props as anchors. Email writer is responsible
    // for using ≥ 2 of them in the body.
    summary.value_props.iter().take(3).cloned().collect()
}
