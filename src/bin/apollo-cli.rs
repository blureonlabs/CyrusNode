//! Operator CLI.
//!
//! Subcommands:
//! - `apollo migrate`                 — apply DB migrations.
//! - `apollo crawl <URL>`             — debug crawl only (no LLM, no DB).
//! - `apollo discover --industry X --city Y` — Google Places lead discovery.
//! - `apollo ingest <URL> [...]`      — research → email → persist QueueItem.
//! - `apollo batch <csv-or-urls>`     — ingest a list of URLs in sequence.
//! - `apollo queue`                   — interactive review of saved dossiers.

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use clap::{Parser, Subcommand};

use apollo::features::discovery::application::DiscoverLeads;
use apollo::features::discovery::domain::{DiscoveryPort, NicheQuery};
use apollo::features::discovery::infra::GooglePlacesAdapter;
use apollo::features::drafting::application::DraftOutreach;
use apollo::features::drafting::domain::DraftInput;
use apollo::features::drafting::infra::EmailWriterAgent;
use apollo::features::research::application::ResearchCompany;
use apollo::features::research::infra::{
    BizIntelAgent, HttpCrawlerAdapter, MarkdownExtractorAdapter, StaticSeoAuditor,
};
use apollo::features::review::application::RunReview;
use apollo::features::review::domain::{QueueItem, SendPort};
use apollo::features::review::infra::{
    DryRunSendAdapter, EmailSendAdapter, FilesystemQueueRepo, StdinPrompter,
};
use apollo::platform::core::CompanyId;
use apollo::platform::crawler::{CrawlerConfig, HttpCrawler};
use apollo::platform::db::{build_pool, run_migrations, DbConfig};
use apollo::platform::email::{EmailSender, ResendClient};
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

    /// Find candidate companies via Google Maps Places. Prints JSON.
    /// Requires `GOOGLE_MAPS_API_KEY`.
    Discover {
        /// Free-text niche, e.g. "limousine service".
        #[arg(long)]
        industry_query: String,
        /// City name.
        #[arg(long, default_value = "Dubai")]
        city: String,
        /// ISO 3166-1 alpha-2 country code.
        #[arg(long, default_value = "AE")]
        country: String,
        /// Max candidates (Places hard cap is 20 per page).
        #[arg(long, default_value_t = 20)]
        limit: u32,
    },

    /// Full E2E pipeline against a URL: crawl → extract → seo → bizintel → email.
    /// Persists the result as a `QueueItem` JSON for `apollo queue`.
    Ingest {
        /// The company's website URL.
        url: String,
        /// Operator's first name for the email sign-off.
        #[arg(long, default_value = "Hari")]
        signature: String,
        /// Industry playbook key (e.g. `dental_clinic`, `limousine_uae`).
        #[arg(long)]
        industry: Option<String>,
        /// Recipient's first name for salutation.
        #[arg(long)]
        contact: Option<String>,
        /// Recipient email address. If set, `apollo queue` can send via Resend.
        #[arg(long)]
        recipient: Option<String>,
    },

    /// Ingest a batch — one URL per line from stdin or a file.
    Batch {
        /// File containing one URL per line. Use `-` for stdin.
        #[arg(default_value = "-")]
        source: String,
        #[arg(long, default_value = "Hari")]
        signature: String,
        #[arg(long)]
        industry: Option<String>,
    },

    /// Interactive review of saved dossiers under `out/dossiers/`.
    /// Approve → Resend (if `RESEND_API_KEY` set) or outbox.
    Queue {
        #[arg(long, default_value = "Hari <hari@apollo.dev>")]
        from: String,
        /// Skip the prompt and approve everything (smoke test only).
        #[arg(long)]
        auto_approve: bool,
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
        Command::Discover {
            industry_query,
            city,
            country,
            limit,
        } => {
            let port: Arc<dyn DiscoveryPort> = Arc::new(GooglePlacesAdapter::from_env()?);
            let use_case = DiscoverLeads::new(port);
            let query = NicheQuery {
                niche: industry_query,
                city,
                country,
                limit,
            };
            let candidates = use_case.run(query).await?;
            println!("{}", serde_json::to_string_pretty(&candidates)?);
            eprintln!("✓ {} candidates", candidates.len());
        }
        Command::Ingest {
            url,
            signature,
            industry,
            contact,
            recipient,
        } => {
            run_ingest(
                &url,
                &signature,
                industry.as_deref(),
                contact.as_deref(),
                recipient.as_deref(),
            )
            .await?;
        }
        Command::Batch {
            source,
            signature,
            industry,
        } => {
            run_batch(&source, &signature, industry.as_deref()).await?;
        }
        Command::Queue { from, auto_approve } => {
            if auto_approve {
                eprintln!("--auto-approve is not yet wired; falling back to interactive");
            }
            run_queue(&from).await?;
        }
    }
    Ok(())
}

async fn run_ingest(
    url: &str,
    signature: &str,
    industry: Option<&str>,
    contact: Option<&str>,
    recipient: Option<&str>,
) -> Result<()> {
    eprintln!("==> loading prompts + industry playbooks...");
    let prompts = PromptLoader::load("apollo/04-prompts").await?;
    let playbooks = PlaybookLoader::load("apollo/05-knowledge/industries").await?;

    eprintln!("==> building Gemini client (ADR-012)...");
    let gemini: Arc<dyn LlmClient> = Arc::new(GeminiClient::from_env()?);

    let crawler = Arc::new(HttpCrawler::new(CrawlerConfig::default())?);
    let crawl = Arc::new(HttpCrawlerAdapter::new(crawler));
    let extract = Arc::new(MarkdownExtractorAdapter);
    let seo = Arc::new(StaticSeoAuditor);
    let bizintel = Arc::new(BizIntelAgent::new(gemini.clone(), prompts.clone()));
    let research = ResearchCompany::new(crawl, extract, seo, bizintel);

    let banned_phrases_path = PathBuf::from("apollo/05-knowledge/banned-email-phrases.txt");
    let draft = Arc::new(EmailWriterAgent::new(
        gemini.clone(),
        prompts.clone(),
        playbooks.clone(),
        banned_phrases_path,
    ));
    let drafter = DraftOutreach::new(draft);

    let company_id = CompanyId(uuid::Uuid::new_v4());

    eprintln!("==> researching {url}...");
    let dossier = research.run(company_id, url).await?;

    eprintln!("==> drafting email...");
    let anchors = dossier
        .summary
        .value_props
        .iter()
        .take(3)
        .cloned()
        .collect();
    let draft_input = DraftInput {
        what_they_sell: dossier.summary.what_they_sell.clone(),
        value_props: dossier.summary.value_props.clone(),
        anchors,
        industry: industry.map(|s| s.to_string()),
        contact_first_name: contact.map(|s| s.to_string()),
        operator_signature: signature.to_string(),
    };
    let email = drafter.run(company_id, &draft_input).await?;

    // Persist as a QueueItem so `apollo queue` can pick it up.
    let summary_line = if !dossier.summary.what_they_sell.is_empty() {
        dossier.summary.what_they_sell.clone()
    } else {
        dossier
            .summary
            .value_props
            .first()
            .cloned()
            .unwrap_or_default()
    };
    let item = QueueItem {
        company_id: company_id.0,
        url: url.to_string(),
        industry: industry.map(|s| s.to_string()),
        summary_line,
        email_subject: email.subject.clone(),
        email_body: email.body.clone(),
        email_anchors: email.personalization_anchors.clone(),
        recipient_email: recipient.map(|s| s.to_string()),
    };
    let dossiers_dir = PathBuf::from("out/dossiers");
    tokio::fs::create_dir_all(&dossiers_dir).await?;
    let dossier_path = dossiers_dir.join(format!("{}.json", company_id.0));
    tokio::fs::write(&dossier_path, serde_json::to_string_pretty(&item)?).await?;
    eprintln!("==> dossier saved: {}", dossier_path.display());

    println!();
    println!("══════════════════════════════════════════════════════════════");
    println!("Subject: {}", email.subject);
    println!();
    println!("{}", email.body);
    println!();
    println!("Anchors used: {}", email.personalization_anchors.join(", "));
    Ok(())
}

async fn run_batch(source: &str, signature: &str, industry: Option<&str>) -> Result<()> {
    let urls: Vec<String> = if source == "-" {
        use tokio::io::AsyncBufReadExt;
        let stdin = tokio::io::BufReader::new(tokio::io::stdin());
        let mut lines = stdin.lines();
        let mut out = Vec::new();
        while let Some(line) = lines.next_line().await? {
            let trimmed = line.trim().to_string();
            if !trimmed.is_empty() && !trimmed.starts_with('#') {
                out.push(trimmed);
            }
        }
        out
    } else {
        let raw = tokio::fs::read_to_string(source).await?;
        raw.lines()
            .map(|l| l.trim().to_string())
            .filter(|l| !l.is_empty() && !l.starts_with('#'))
            .collect()
    };
    eprintln!("==> batch: {} URLs", urls.len());
    let total = urls.len();
    let mut ok = 0usize;
    let mut fail = 0usize;
    for (idx, url) in urls.iter().enumerate() {
        eprintln!();
        eprintln!("──── [{}/{}] {} ────", idx + 1, total, url);
        match run_ingest(url, signature, industry, None, None).await {
            Ok(_) => ok += 1,
            Err(e) => {
                eprintln!("✗ {url}: {e}");
                fail += 1;
            }
        }
    }
    eprintln!();
    eprintln!("==> batch done: {ok} ok, {fail} failed");
    Ok(())
}

async fn run_queue(from_address: &str) -> Result<()> {
    eprintln!("==> wiring review feature...");
    let queue = Arc::new(FilesystemQueueRepo::new(PathBuf::from("out")));
    let prompter = Arc::new(StdinPrompter);

    // Send port: real Resend if RESEND_API_KEY is set, dry-run otherwise.
    let sender: Arc<dyn SendPort> = match ResendClient::from_env() {
        Ok(client) => {
            eprintln!("    Resend client ready");
            let email_sender: Arc<dyn EmailSender> = Arc::new(client);
            Arc::new(EmailSendAdapter::new(email_sender))
        }
        Err(_) => {
            eprintln!("    RESEND_API_KEY not set — using dry-run sender");
            Arc::new(DryRunSendAdapter)
        }
    };

    let review = RunReview::new(queue, prompter, sender, from_address.to_string());
    let summary = review.run().await?;

    println!();
    println!("══════════════════════════════════════════════════════════════");
    println!("REVIEW SESSION SUMMARY");
    println!("──────────────────────────────────────────────────────────────");
    println!("Reviewed         : {}", summary.reviewed);
    println!("Approved & sent  : {}", summary.approved_sent);
    println!("Approved → outbox: {}", summary.approved_outbox);
    println!("Rejected         : {}", summary.rejected);
    println!("Skipped          : {}", summary.skipped);
    Ok(())
}
