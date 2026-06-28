//! Operator CLI.
//!
//! Subcommands:
//! - `apollo migrate`                 — apply DB migrations.
//! - `apollo crawl <URL>`             — debug crawl only (no LLM, no DB).
//! - `apollo discover --industry X --city Y` — Google Places lead discovery.
//! - `apollo ingest <URL> [...]`      — research → email → persist QueueItem.
//! - `apollo batch <csv-or-urls>`     — ingest URLs concurrently (bounded).
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
use apollo::features::research::make_industry_hint;
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
use apollo::platform::llm::{GeminiClient, LlmCallRecord, LlmClient};
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
        /// Concurrent ingest jobs. Default 4. Cap at 16.
        #[arg(long, default_value_t = 4)]
        concurrency: usize,
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

    /// Summarize LLM spend from `out/llm_calls.jsonl`.
    Cost {
        /// Window: today | week | month | all.
        #[arg(long, default_value = "today")]
        window: String,
        /// Group by: agent | model | company.
        #[arg(long, default_value = "agent")]
        group_by: String,
    },

    /// List loaded industry playbooks from `apollo/05-knowledge/industries/`.
    Industries,

    /// Remove old crawl artifacts. Defaults to crawls older than 7 days.
    Clean {
        /// Older-than threshold in days. Default 7.
        #[arg(long, default_value_t = 7)]
        older_than_days: i64,
        /// If true, print what would be deleted without removing anything.
        #[arg(long)]
        dry_run: bool,
        /// Also clean sent/rejected dossiers older than the threshold.
        #[arg(long)]
        include_processed: bool,
    },

    /// At-a-glance dashboard. No flags.
    Status,
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
            concurrency,
        } => {
            run_batch(&source, &signature, industry.as_deref(), concurrency).await?;
        }
        Command::Queue { from, auto_approve } => {
            if auto_approve {
                eprintln!("--auto-approve is not yet wired; falling back to interactive");
            }
            run_queue(&from).await?;
        }
        Command::Cost { window, group_by } => {
            run_cost(&window, &group_by).await?;
        }
        Command::Industries => {
            run_industries().await?;
        }
        Command::Clean {
            older_than_days,
            dry_run,
            include_processed,
        } => {
            run_clean(older_than_days, dry_run, include_processed).await?;
        }
        Command::Status => {
            run_status().await?;
        }
    }
    Ok(())
}

async fn run_industries() -> Result<()> {
    let playbooks = PlaybookLoader::load("apollo/05-knowledge/industries").await?;
    let keys = playbooks.keys().await;
    if keys.is_empty() {
        println!("No industry playbooks loaded.");
        println!(
            "Add a Markdown file under apollo/05-knowledge/industries/<key>.md \
             (see `_template.md`)."
        );
        return Ok(());
    }
    println!("Loaded industry playbooks:");
    for key in keys {
        if let Some(book) = playbooks.get(&key).await {
            println!("  - {key:<22} ({})", book.frontmatter.display_name.as_str());
        }
    }
    println!();
    println!("Use with: apollo ingest <URL> --industry <key>");
    Ok(())
}

/// Shared pipeline context. Prompts, playbooks, Gemini client, and all
/// adapters are built exactly once and reused across every URL the batch
/// runner processes.
struct BatchContext {
    research: ResearchCompany,
    drafter: DraftOutreach,
    /// Playbook loader, shared with drafting. Used to convert an industry key
    /// into an [`apollo::features::research::domain::IndustryHint`] per call
    /// so the bizintel agent can see typical pains + AI opportunities.
    playbooks: PlaybookLoader,
    out_dir: PathBuf,
}

impl BatchContext {
    /// Build the full pipeline once. Loads prompts + playbooks, constructs
    /// the Gemini client, wires every adapter, and resolves the dossier
    /// output directory.
    async fn build() -> Result<Self> {
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

        Ok(Self {
            research,
            drafter,
            playbooks,
            out_dir: PathBuf::from("out/dossiers"),
        })
    }

    /// Run the full pipeline for one URL and persist a `QueueItem`.
    async fn ingest_one(
        &self,
        url: &str,
        signature: &str,
        industry: Option<&str>,
        contact: Option<&str>,
        recipient: Option<&str>,
    ) -> Result<()> {
        let company_id = CompanyId(uuid::Uuid::new_v4());

        eprintln!("==> researching {url}...");
        let industry_hint = make_industry_hint(&self.playbooks, industry).await;
        let dossier = self
            .research
            .run(company_id, url, industry_hint.as_ref())
            .await?;

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
        let email = self.drafter.run(company_id, &draft_input).await?;

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
        // Prefer the CLI flag if the operator set it; else auto-pick from site_facts.
        let recipient_email = recipient
            .map(|s| s.to_string())
            .or_else(|| pick_best_email(&dossier.site_facts.emails));
        match &recipient_email {
            Some(addr) => eprintln!("==> recipient: {addr}"),
            None => eprintln!("==> recipient: (none — will go to outbox)"),
        }

        let item = QueueItem {
            company_id: company_id.0,
            url: url.to_string(),
            industry: industry.map(|s| s.to_string()),
            summary_line,
            email_subject: email.subject.clone(),
            email_body: email.body.clone(),
            email_anchors: email.personalization_anchors.clone(),
            recipient_email,
        };
        tokio::fs::create_dir_all(&self.out_dir).await?;
        let dossier_path = self.out_dir.join(format!("{}.json", company_id.0));
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
}

async fn run_ingest(
    url: &str,
    signature: &str,
    industry: Option<&str>,
    contact: Option<&str>,
    recipient: Option<&str>,
) -> Result<()> {
    let ctx = BatchContext::build().await?;
    ctx.ingest_one(url, signature, industry, contact, recipient)
        .await
}

/// Read URLs from a file path or `-` for stdin. Strips blank lines and
/// `#` comments.
async fn read_urls(source: &str) -> Result<Vec<String>> {
    if source == "-" {
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
        Ok(out)
    } else {
        let raw = tokio::fs::read_to_string(source).await?;
        Ok(raw
            .lines()
            .map(|l| l.trim().to_string())
            .filter(|l| !l.is_empty() && !l.starts_with('#'))
            .collect())
    }
}

/// Run `op` with exponential backoff on rate-limit errors only. All other
/// errors are returned immediately. Backoff: 4s, 16s, 64s, then give up.
async fn with_backoff_on_rate_limit<F, Fut, T>(mut op: F, attempts: u32) -> Result<T>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<T>>,
{
    let mut delay = 4u64;
    for attempt in 0..attempts {
        match op().await {
            Ok(v) => return Ok(v),
            Err(e) => {
                let msg = format!("{e:#}");
                let is_rate_limit = msg.contains("rate limited")
                    || msg.contains("429")
                    || msg.contains("RateLimited");
                if !is_rate_limit || attempt + 1 == attempts {
                    return Err(e);
                }
                tracing::warn!(
                    attempt = attempt + 1,
                    delay_secs = delay,
                    "rate-limit backoff"
                );
                tokio::time::sleep(std::time::Duration::from_secs(delay)).await;
                delay = (delay * 4).min(120);
            }
        }
    }
    unreachable!("loop returned in every branch")
}

async fn run_batch(
    source: &str,
    signature: &str,
    industry: Option<&str>,
    concurrency: usize,
) -> Result<()> {
    let concurrency = concurrency.clamp(1, 16);
    let urls = read_urls(source).await?;
    eprintln!(
        "==> batch: {} URLs, concurrency={}",
        urls.len(),
        concurrency
    );

    let ctx = Arc::new(BatchContext::build().await?);
    let semaphore = Arc::new(tokio::sync::Semaphore::new(concurrency));

    let mut handles = Vec::with_capacity(urls.len());
    for (idx, url) in urls.into_iter().enumerate() {
        let permit = semaphore.clone().acquire_owned().await?;
        let ctx = ctx.clone();
        let sig = signature.to_string();
        let ind = industry.map(|s| s.to_string());
        handles.push(tokio::spawn(async move {
            let _permit = permit;
            let label = format!("[{}] {}", idx + 1, url);
            eprintln!("──── {} ────", label);
            let result = with_backoff_on_rate_limit(
                || ctx.ingest_one(&url, &sig, ind.as_deref(), None, None),
                4,
            )
            .await;
            (label, result)
        }));
    }

    let mut ok = 0usize;
    let mut fail = 0usize;
    for handle in handles {
        match handle.await {
            Ok((_, Ok(_))) => ok += 1,
            Ok((label, Err(e))) => {
                eprintln!("✗ {label}: {e}");
                fail += 1;
            }
            Err(e) => {
                eprintln!("✗ join error: {e}");
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

/// Pick the most-likely-personal email from extracted addresses.
/// Prefers info@, contact@, hello@, sales@; falls back to the first valid one.
fn pick_best_email(emails: &[String]) -> Option<String> {
    if emails.is_empty() {
        return None;
    }
    let prefer = ["info@", "contact@", "hello@", "sales@", "bookings@"];
    for p in prefer {
        if let Some(m) = emails.iter().find(|e| e.to_lowercase().starts_with(p)) {
            return Some(m.clone());
        }
    }
    emails.first().cloned()
}

/// Summarize LLM spend from `out/llm_calls.jsonl`. CLAUDE.md §6 — cost is
/// observable. Reads the JSONL ledger line by line, filters by window, groups
/// by the requested dimension, and prints a fixed-width table to stdout.
///
/// Exits 0 with a friendly message if the file does not exist yet (no calls
/// recorded). Malformed lines are logged at `warn!` and skipped — one bad
/// row should not break the whole report.
async fn run_cost(window: &str, group_by: &str) -> Result<()> {
    use std::collections::BTreeMap;

    let path = std::path::Path::new("out/llm_calls.jsonl");
    if !path.exists() {
        println!("No calls recorded yet.");
        return Ok(());
    }

    let body = tokio::fs::read_to_string(path).await?;

    // Window filter — `chrono::Utc::now()` minus N days. `all` returns None
    // and disables the filter entirely.
    let now = chrono::Utc::now();
    let cutoff: Option<chrono::DateTime<chrono::Utc>> = match window {
        "today" => Some(now - chrono::Duration::days(1)),
        "week" => Some(now - chrono::Duration::days(7)),
        "month" => Some(now - chrono::Duration::days(30)),
        "all" => None,
        other => {
            anyhow::bail!(
                "unknown --window '{}'; expected today | week | month | all",
                other
            );
        }
    };

    // Read line by line, parse, filter, group. BTreeMap so the output order
    // is deterministic without an extra sort pass.
    #[derive(Default, Clone)]
    struct Row {
        calls: u64,
        input_tokens: u64,
        output_tokens: u64,
        cost_usd: f64,
    }
    let mut groups: BTreeMap<String, Row> = BTreeMap::new();
    let mut total = Row::default();

    for (idx, line) in body.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let rec: LlmCallRecord = match serde_json::from_str(line) {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!(line_number = idx + 1, error = %e, "skipping malformed ledger line");
                continue;
            }
        };
        if let Some(c) = cutoff {
            if rec.occurred_at < c {
                continue;
            }
        }

        let key = match group_by {
            "agent" => rec.agent_name.clone().unwrap_or_else(|| "<none>".into()),
            "model" => rec.model.clone(),
            "company" => rec.company_id.clone().unwrap_or_else(|| "<none>".into()),
            other => {
                anyhow::bail!(
                    "unknown --group-by '{}'; expected agent | model | company",
                    other
                );
            }
        };

        let row = groups.entry(key).or_default();
        row.calls += 1;
        row.input_tokens += rec.input_tokens as u64;
        row.output_tokens += rec.output_tokens as u64;
        row.cost_usd += rec.cost_usd;

        total.calls += 1;
        total.input_tokens += rec.input_tokens as u64;
        total.output_tokens += rec.output_tokens as u64;
        total.cost_usd += rec.cost_usd;
    }

    // Header lines describing the window and grouping dimension.
    let header_left = format!("Window: {}", window);
    let header_right = match cutoff {
        Some(c) => format!("(since {})", c.to_rfc3339()),
        None => "(all time)".to_string(),
    };
    tracing::info!(window = %window, group_by = %group_by, "apollo cost report");

    println!("{} {}", header_left, header_right);
    let dim_label = match group_by {
        "agent" => "Agent",
        "model" => "Model",
        "company" => "Company",
        _ => "Key",
    };

    let sep: String = "─".repeat(65);
    println!("{}", sep);
    println!(
        "{:<20} {:>8} {:>10} {:>10} {:>10}",
        dim_label, "Calls", "In tok", "Out tok", "Cost"
    );

    for (key, row) in &groups {
        println!(
            "{:<20} {:>8} {:>10} {:>10} {:>10}",
            truncate(key, 20),
            row.calls,
            format_tokens(row.input_tokens),
            format_tokens(row.output_tokens),
            format_cost(row.cost_usd),
        );
    }
    println!("{}", sep);
    println!(
        "{:<20} {:>8} {:>10} {:>10} {:>10}",
        "TOTAL",
        total.calls,
        format_tokens(total.input_tokens),
        format_tokens(total.output_tokens),
        format_cost(total.cost_usd),
    );

    Ok(())
}

/// Compact token count: `1234` → `1.2k`, `1_400_000` → `1.4M`, small as-is.
fn format_tokens(n: u64) -> String {
    if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.1}k", n as f64 / 1_000.0)
    } else {
        n.to_string()
    }
}

/// Format a USD value with a leading `$` and 2 decimals when small.
fn format_cost(cost: f64) -> String {
    if cost >= 100.0 {
        format!("${:.0}", cost)
    } else {
        format!("${:.2}", cost)
    }
}

/// Truncate a display string to fit a fixed-width column, eliding with `…`.
fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let mut out: String = s.chars().take(max.saturating_sub(1)).collect();
        out.push('…');
        out
    }
}

/// Human-readable byte size: 1.23 MB / 456 KB / 89 B. Uses base-1024 units.
fn human_bytes(n: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * KB;
    const GB: u64 = 1024 * MB;
    if n >= GB {
        format!("{:.2} GB", n as f64 / GB as f64)
    } else if n >= MB {
        format!("{:.2} MB", n as f64 / MB as f64)
    } else if n >= KB {
        format!("{:.2} KB", n as f64 / KB as f64)
    } else {
        format!("{} B", n)
    }
}

/// Recursively sum the size of all regular files under `path`. Symlinks and
/// errors on individual entries are silently skipped so a single unreadable
/// child does not break the whole walk.
fn dir_size(path: &std::path::Path) -> std::io::Result<u64> {
    let mut total: u64 = 0;
    let meta = std::fs::symlink_metadata(path)?;
    if meta.file_type().is_file() {
        return Ok(meta.len());
    }
    if !meta.file_type().is_dir() {
        return Ok(0);
    }
    let entries = match std::fs::read_dir(path) {
        Ok(e) => e,
        Err(_) => return Ok(0),
    };
    for entry in entries.flatten() {
        let p = entry.path();
        match std::fs::symlink_metadata(&p) {
            Ok(m) if m.file_type().is_file() => total += m.len(),
            Ok(m) if m.file_type().is_dir() => {
                total += dir_size(&p).unwrap_or(0);
            }
            _ => {}
        }
    }
    Ok(total)
}

/// Direct children of `root` whose modification time is older than `threshold`.
/// Returns `(path, size_bytes, age)` tuples. Missing `root` yields an empty vec.
///
/// Only the top-level children are inspected — the cleanup is deliberately
/// shallow so a stray nested file does not pin an entire correlation-id dir.
async fn list_aged_entries(
    root: &std::path::Path,
    threshold: std::time::Duration,
) -> Result<Vec<(PathBuf, u64, std::time::Duration)>> {
    let mut out: Vec<(PathBuf, u64, std::time::Duration)> = Vec::new();
    if !root.exists() {
        return Ok(out);
    }
    let mut rd = tokio::fs::read_dir(root).await?;
    let now = std::time::SystemTime::now();
    while let Some(entry) = rd.next_entry().await? {
        let path = entry.path();
        let meta = match entry.metadata().await {
            Ok(m) => m,
            Err(_) => continue,
        };
        let mtime = match meta.modified() {
            Ok(t) => t,
            Err(_) => continue,
        };
        let age = match now.duration_since(mtime) {
            Ok(a) => a,
            Err(_) => continue, // mtime in the future — skip
        };
        if age < threshold {
            continue;
        }
        let size = if meta.is_dir() {
            dir_size(&path).unwrap_or(0)
        } else {
            meta.len()
        };
        out.push((path, size, age));
    }
    Ok(out)
}

/// Convert a `Duration` into a whole-day count for display purposes.
fn duration_days(d: std::time::Duration) -> u64 {
    d.as_secs() / 86_400
}

/// Remove old crawl artifacts. Walks `out/crawls/` (correlation-id dirs) and,
/// when `--include-processed` is set, `out/sent/` + `out/rejected/`. Never
/// touches `out/dossiers/` (pending review) or `out/outbox/` (waiting to send).
///
/// Errors propagate from the filesystem walk or from a delete. Missing
/// directories are treated as empty — there is nothing to clean.
async fn run_clean(older_than_days: i64, dry_run: bool, include_processed: bool) -> Result<()> {
    let days = older_than_days.max(0) as u64;
    let threshold = std::time::Duration::from_secs(days * 86_400);

    let mut targets: Vec<(PathBuf, u64, std::time::Duration, bool /* is_dir */)> = Vec::new();

    // out/crawls/<correlation-id>/ — always eligible.
    let crawls_root = std::path::Path::new("out/crawls");
    for (path, size, age) in list_aged_entries(crawls_root, threshold).await? {
        let is_dir = path.is_dir();
        targets.push((path, size, age, is_dir));
    }

    if include_processed {
        for root in ["out/sent", "out/rejected"] {
            let root_path = std::path::Path::new(root);
            for (path, size, age) in list_aged_entries(root_path, threshold).await? {
                if path.is_file()
                    && path
                        .extension()
                        .and_then(|s| s.to_str())
                        .is_some_and(|s| s.eq_ignore_ascii_case("json"))
                {
                    targets.push((path, size, age, false));
                }
            }
        }
    }

    if targets.is_empty() {
        println!("Nothing to clean (threshold: {days} days).");
        return Ok(());
    }

    if dry_run {
        for (path, size, age) in targets
            .iter()
            .map(|(p, s, a, _)| (p, s, a))
            .collect::<Vec<_>>()
        {
            println!(
                "[would delete] {} ({}, {} days)",
                path.display(),
                human_bytes(*size),
                duration_days(*age),
            );
        }
        return Ok(());
    }

    let mut deleted = 0usize;
    let mut freed: u64 = 0;
    for (path, size, _age, is_dir) in targets {
        let result = if is_dir {
            tokio::fs::remove_dir_all(&path).await
        } else {
            tokio::fs::remove_file(&path).await
        };
        match result {
            Ok(()) => {
                deleted += 1;
                freed = freed.saturating_add(size);
            }
            Err(e) => {
                tracing::warn!(path = %path.display(), error = %e, "failed to delete");
            }
        }
    }
    println!("✓ deleted {} items, freed {}", deleted, human_bytes(freed));
    Ok(())
}

/// Count direct children of `path` matching `predicate`. Returns 0 if the
/// directory is missing or unreadable — best-effort by design.
async fn count_children<F>(path: &std::path::Path, predicate: F) -> usize
where
    F: Fn(&std::fs::FileType, &std::path::Path) -> bool,
{
    if !path.exists() {
        return 0;
    }
    let mut rd = match tokio::fs::read_dir(path).await {
        Ok(rd) => rd,
        Err(_) => return 0,
    };
    let mut n = 0usize;
    while let Ok(Some(entry)) = rd.next_entry().await {
        if let Ok(ft) = entry.file_type().await {
            if predicate(&ft, &entry.path()) {
                n += 1;
            }
        }
    }
    n
}

/// At-a-glance dashboard. Counts queue files, crawl runs, knowledge artifacts,
/// and today's LLM spend. Every section is best-effort: missing directories
/// print `0` (or `not yet`) rather than panicking.
async fn run_status() -> Result<()> {
    let now = chrono::Utc::now();

    // Queue counts. Dossiers/sent/rejected are *.json files; outbox is *.eml.
    let pending = count_children(std::path::Path::new("out/dossiers"), |ft, p| {
        ft.is_file()
            && p.extension()
                .and_then(|s| s.to_str())
                .is_some_and(|s| s.eq_ignore_ascii_case("json"))
    })
    .await;
    let outbox = count_children(std::path::Path::new("out/outbox"), |ft, p| {
        ft.is_file()
            && p.extension()
                .and_then(|s| s.to_str())
                .is_some_and(|s| s.eq_ignore_ascii_case("eml"))
    })
    .await;
    let sent = count_children(std::path::Path::new("out/sent"), |ft, p| {
        ft.is_file()
            && p.extension()
                .and_then(|s| s.to_str())
                .is_some_and(|s| s.eq_ignore_ascii_case("json"))
    })
    .await;
    let rejected = count_children(std::path::Path::new("out/rejected"), |ft, p| {
        ft.is_file()
            && p.extension()
                .and_then(|s| s.to_str())
                .is_some_and(|s| s.eq_ignore_ascii_case("json"))
    })
    .await;

    // Crawl runs — direct child dirs of out/crawls/.
    let crawls_root = std::path::Path::new("out/crawls");
    let crawl_runs = count_children(crawls_root, |ft, _| ft.is_dir() || ft.is_symlink()).await;
    let crawl_size = if crawls_root.exists() {
        dir_size(crawls_root).unwrap_or(0)
    } else {
        0
    };

    // Today's LLM totals (window pattern matches run_cost — 24h rolling).
    let mut llm_calls: u64 = 0;
    let mut input_tokens: u64 = 0;
    let mut output_tokens: u64 = 0;
    let mut spend_usd: f64 = 0.0;
    let mut llm_seen = false;
    let ledger_path = std::path::Path::new("out/llm_calls.jsonl");
    if ledger_path.exists() {
        llm_seen = true;
        let cutoff = now - chrono::Duration::days(1);
        let body = tokio::fs::read_to_string(ledger_path)
            .await
            .unwrap_or_default();
        for (idx, line) in body.lines().enumerate() {
            if line.trim().is_empty() {
                continue;
            }
            let rec: LlmCallRecord = match serde_json::from_str(line) {
                Ok(r) => r,
                Err(e) => {
                    tracing::warn!(
                        line_number = idx + 1,
                        error = %e,
                        "skipping malformed ledger line",
                    );
                    continue;
                }
            };
            if rec.occurred_at < cutoff {
                continue;
            }
            llm_calls += 1;
            input_tokens += rec.input_tokens as u64;
            output_tokens += rec.output_tokens as u64;
            spend_usd += rec.cost_usd;
        }
    }

    // Knowledge counts. Both loaders return empty registries on missing dir,
    // so failures here are real bugs worth surfacing rather than swallowing.
    let prompts_count = match PromptLoader::load("apollo/04-prompts").await {
        Ok(p) => p.names().await.len(),
        Err(e) => {
            tracing::warn!(error = %e, "could not load prompts");
            0
        }
    };
    let (playbook_count, playbook_keys): (usize, Vec<String>) =
        match PlaybookLoader::load("apollo/05-knowledge/industries").await {
            Ok(p) => {
                let keys = p.keys().await;
                (keys.len(), keys)
            }
            Err(e) => {
                tracing::warn!(error = %e, "could not load playbooks");
                (0, Vec::new())
            }
        };

    let sep: String = "─".repeat(60);
    println!("Apollo status — {}", now.to_rfc3339());
    println!("{}", sep);
    println!("Queue");
    println!("  Pending review  : {pending} dossiers");
    println!("  Outbox          : {outbox} .eml files");
    println!("  Sent            : {sent}");
    println!("  Rejected        : {rejected}");
    println!("Crawls");
    if crawls_root.exists() {
        println!(
            "  Saved runs      : {crawl_runs} ({} on disk)",
            human_bytes(crawl_size)
        );
    } else {
        println!("  Saved runs      : not yet");
    }
    println!("Knowledge");
    println!("  Prompts loaded  : {prompts_count}");
    if playbook_count == 0 {
        println!("  Playbooks       : 0");
    } else {
        println!(
            "  Playbooks       : {playbook_count} ({})",
            playbook_keys.join(", ")
        );
    }
    println!("Cost (today)");
    if llm_seen {
        println!("  LLM calls       : {llm_calls}");
        println!("  Input tokens    : {}", format_tokens(input_tokens));
        println!("  Output tokens   : {}", format_tokens(output_tokens));
        println!("  Spend           : {}", format_cost(spend_usd));
    } else {
        println!("  LLM calls       : not yet");
    }
    println!("{}", sep);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::pick_best_email;

    #[test]
    fn pick_best_email_prefers_info() {
        let emails = vec![
            "random@example.com".to_string(),
            "info@example.com".to_string(),
            "sales@example.com".to_string(),
        ];
        assert_eq!(
            pick_best_email(&emails).as_deref(),
            Some("info@example.com")
        );
    }

    #[test]
    fn pick_best_email_is_case_insensitive() {
        let emails = vec![
            "Random@example.com".to_string(),
            "INFO@Example.com".to_string(),
        ];
        assert_eq!(
            pick_best_email(&emails).as_deref(),
            Some("INFO@Example.com")
        );
    }

    #[test]
    fn pick_best_email_falls_back_to_first() {
        let emails = vec![
            "ceo@example.com".to_string(),
            "founder@example.com".to_string(),
        ];
        assert_eq!(pick_best_email(&emails).as_deref(), Some("ceo@example.com"));
    }

    #[test]
    fn pick_best_email_returns_none_on_empty() {
        let emails: Vec<String> = vec![];
        assert_eq!(pick_best_email(&emails), None);
    }

    #[test]
    fn pick_best_email_prefers_contact_when_no_info() {
        let emails = vec![
            "random@example.com".to_string(),
            "contact@example.com".to_string(),
            "sales@example.com".to_string(),
        ];
        assert_eq!(
            pick_best_email(&emails).as_deref(),
            Some("contact@example.com")
        );
    }
}
