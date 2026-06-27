//! Crawler configuration, types, and the BFS driver.
//!
//! Behavior mirrors `apollo/03-agents/crawler.md`:
//! 1. Load robots (if requested). Refuse if start URL disallowed.
//! 2. Seed frontier with sitemap URLs + start URL.
//! 3. BFS within (max_pages, max_bytes) budget, sleeping `per_host_delay`
//!    before each request, picking the highest-scored URL each iteration.
//! 4. Save HTML to disk; collect same-host links; record skips with reasons.

use std::collections::{HashSet, VecDeque};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use reqwest::Client;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use url::Url;

use super::links::{extract_links, score_link};
use super::robots::RobotsCache;
use super::sitemap::try_fetch_sitemap;
use super::storage::save_html;

/// Knobs for a single crawl run.
#[derive(Debug, Clone)]
pub struct CrawlerConfig {
    /// Hard ceiling on number of pages fetched. See crawler.md §1.
    pub max_pages: u32,
    /// Hard ceiling on total bytes downloaded.
    pub max_bytes: u64,
    /// If false, robots.txt is ignored entirely (operator override).
    pub respect_robots: bool,
    /// User-Agent string sent with every request and used for robots matching.
    pub user_agent: String,
    /// Where saved HTML lives. The crawler writes to
    /// `<output_root>/<correlation_id>/<sha256-of-url>.html`.
    pub output_root: PathBuf,
    /// Minimum delay between two requests to the same host (per-host
    /// concurrency 1 + 500ms by default — crawler.md "Concurrency").
    pub per_host_delay: Duration,
    /// Per-page timeout passed to the underlying HTTP client.
    pub request_timeout: Duration,
}

impl Default for CrawlerConfig {
    fn default() -> Self {
        Self {
            max_pages: 25,
            max_bytes: 2 * 1024 * 1024,
            respect_robots: true,
            user_agent: "ApolloBot/0.1 (+https://github.com/blureonlabs/CyrusNode; contact: ops)"
                .to_string(),
            output_root: PathBuf::from("out/crawls"),
            per_host_delay: Duration::from_millis(500),
            request_timeout: Duration::from_secs(15),
        }
    }
}

/// One successfully fetched page.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrawledPage {
    /// Absolute URL of the fetched page.
    pub url: String,
    /// HTTP status code.
    pub status: u16,
    /// Content-Type header (empty string if missing).
    pub content_type: String,
    /// Body length in bytes.
    pub bytes: u64,
    /// Path on local disk where the body was saved.
    pub html_path: PathBuf,
    /// UTC timestamp when the fetch completed.
    pub fetched_at: chrono::DateTime<chrono::Utc>,
}

/// One URL the crawler chose not to fetch (or couldn't), with a reason.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkippedPage {
    /// Absolute URL of the skipped page.
    pub url: String,
    /// Why it was skipped.
    pub reason: SkipReason,
}

/// Why a particular URL was not fetched.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SkipReason {
    /// robots.txt disallowed the URL for our UA.
    RobotsDisallowed,
    /// Hit the page or byte budget.
    BudgetExceeded,
    /// Response was not HTML; the inner string is the Content-Type seen.
    NonHtml(String),
    /// Server returned a non-2xx status. 0 = network failure (no response).
    HttpError(u16),
    /// Already seen in this run.
    Duplicate,
    /// Body could not be saved or parsed.
    ParseFailure,
}

/// Result of a single crawl run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrawlSummary {
    /// The URL the crawl started from.
    pub start_url: String,
    /// Host of the start URL — crawls are scoped to this host.
    pub host: String,
    /// Pages successfully fetched and saved.
    pub fetched: Vec<CrawledPage>,
    /// URLs we considered but did not fetch.
    pub skipped: Vec<SkippedPage>,
    /// All same-host links observed across fetched pages, deduplicated.
    pub discovered_links: Vec<String>,
    /// Sum of fetched-page body sizes.
    pub total_bytes: u64,
    /// Wall-clock duration of the crawl in milliseconds.
    pub duration_ms: u32,
}

/// Terminal-only failures. Per-page issues are recorded as `SkippedPage`
/// rather than raised here — see crawler.md "Failure modes".
#[derive(Debug, Error)]
pub enum CrawlerError {
    /// The start URL wasn't a valid http(s) URL.
    #[error("invalid url: {0}")]
    InvalidUrl(String),
    /// The host could not be reached at all.
    #[error("unreachable: {0}")]
    Unreachable(String),
    /// robots.txt forbids us from fetching the start URL.
    #[error("robots blocks the start url")]
    RobotsBlockedStart,
    /// Underlying HTTP client error during setup.
    #[error("network: {0}")]
    Network(#[from] reqwest::Error),
    /// Local filesystem error (typically creating the output dir).
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
}

/// The crawler itself. Wraps a shared `reqwest::Client` and a robots cache.
/// Cheap to clone — share one across the worker pool.
#[derive(Clone)]
pub struct HttpCrawler {
    http: Client,
    robots: Arc<RobotsCache>,
    cfg: CrawlerConfig,
}

impl HttpCrawler {
    /// Build a crawler from configuration. Fails only if the underlying
    /// `reqwest::Client` cannot be constructed (TLS init, etc.).
    pub fn new(cfg: CrawlerConfig) -> Result<Self, CrawlerError> {
        let http = Client::builder()
            .user_agent(&cfg.user_agent)
            .timeout(cfg.request_timeout)
            .redirect(reqwest::redirect::Policy::limited(5))
            .build()?;
        Ok(Self {
            http,
            robots: Arc::new(RobotsCache::new()),
            cfg,
        })
    }

    /// Run a crawl starting from `start_url`. Saves HTML under
    /// `<output_root>/<correlation_id>/`. Returns a `CrawlSummary`.
    ///
    /// Failure modes:
    /// - `InvalidUrl` if `start_url` is not http(s).
    /// - `RobotsBlockedStart` if robots disallows the start URL.
    /// - `Io` if the output dir cannot be created.
    /// - Any other per-page issue (HTTP error, non-HTML, parse failure) is
    ///   recorded in `summary.skipped` and the crawl continues.
    pub async fn crawl(
        &self,
        start_url: &str,
        correlation_id: &str,
    ) -> Result<CrawlSummary, CrawlerError> {
        let started = std::time::Instant::now();
        let span = tracing::info_span!(
            "crawl",
            start_url = %start_url,
            correlation_id = %correlation_id,
            host = tracing::field::Empty,
        );
        let _enter = span.enter();

        let url =
            Url::parse(start_url).map_err(|_| CrawlerError::InvalidUrl(start_url.to_string()))?;
        if !matches!(url.scheme(), "http" | "https") {
            return Err(CrawlerError::InvalidUrl(format!(
                "scheme: {}",
                url.scheme()
            )));
        }
        let host = url
            .host_str()
            .ok_or_else(|| CrawlerError::InvalidUrl("no host".to_string()))?
            .to_string();
        span.record("host", tracing::field::display(&host));

        let out_dir = self.cfg.output_root.join(correlation_id);
        tokio::fs::create_dir_all(&out_dir).await?;

        if self.cfg.respect_robots
            && !self
                .robots
                .allowed(&self.http, &url, &self.cfg.user_agent)
                .await
        {
            return Err(CrawlerError::RobotsBlockedStart);
        }

        // Seed frontier with sitemap URLs (best-effort) then the start URL.
        let mut frontier: VecDeque<(Url, i32)> = VecDeque::new();
        if let Ok(urls) = try_fetch_sitemap(&self.http, &url).await {
            for u in urls {
                // Sitemap entries scoped to start host only.
                if u.host_str() == Some(&host) {
                    let s = score_link(&u);
                    frontier.push_back((u, s));
                }
            }
        }
        // Start URL always wins the first pop.
        frontier.push_back((url.clone(), 1000));

        let mut visited: HashSet<String> = HashSet::new();
        let mut fetched: Vec<CrawledPage> = Vec::new();
        let mut skipped: Vec<SkippedPage> = Vec::new();
        let mut discovered: HashSet<String> = HashSet::new();
        let mut total_bytes: u64 = 0;

        while let Some((next_url, _score)) = pop_highest(&mut frontier) {
            if fetched.len() as u32 >= self.cfg.max_pages {
                skipped.push(SkippedPage {
                    url: next_url.to_string(),
                    reason: SkipReason::BudgetExceeded,
                });
                break;
            }
            if total_bytes >= self.cfg.max_bytes {
                skipped.push(SkippedPage {
                    url: next_url.to_string(),
                    reason: SkipReason::BudgetExceeded,
                });
                break;
            }
            let key = next_url.to_string();
            if !visited.insert(key.clone()) {
                continue;
            }
            if self.cfg.respect_robots
                && !self
                    .robots
                    .allowed(&self.http, &next_url, &self.cfg.user_agent)
                    .await
            {
                skipped.push(SkippedPage {
                    url: key,
                    reason: SkipReason::RobotsDisallowed,
                });
                continue;
            }

            // Per-host politeness — single concurrency + delay.
            tokio::time::sleep(self.cfg.per_host_delay).await;

            let resp = match self.http.get(next_url.clone()).send().await {
                Ok(r) => r,
                Err(e) => {
                    tracing::warn!(url = %next_url, error = %e, "fetch failed");
                    skipped.push(SkippedPage {
                        url: key,
                        reason: SkipReason::HttpError(0),
                    });
                    continue;
                }
            };
            let status = resp.status();
            if !status.is_success() {
                skipped.push(SkippedPage {
                    url: key,
                    reason: SkipReason::HttpError(status.as_u16()),
                });
                continue;
            }
            let ct = resp
                .headers()
                .get(reqwest::header::CONTENT_TYPE)
                .and_then(|h| h.to_str().ok())
                .unwrap_or("")
                .to_string();
            if !ct.starts_with("text/html") {
                skipped.push(SkippedPage {
                    url: key,
                    reason: SkipReason::NonHtml(ct),
                });
                continue;
            }
            let body_bytes = match resp.bytes().await {
                Ok(b) => b,
                Err(e) => {
                    tracing::warn!(url = %next_url, error = %e, "body read failed");
                    skipped.push(SkippedPage {
                        url: key,
                        reason: SkipReason::HttpError(0),
                    });
                    continue;
                }
            };
            let size = body_bytes.len() as u64;
            total_bytes = total_bytes.saturating_add(size);

            let html = match std::str::from_utf8(&body_bytes) {
                Ok(s) => s.to_string(),
                Err(_) => String::from_utf8_lossy(&body_bytes).into_owned(),
            };

            let html_path = match save_html(&out_dir, &next_url, &html).await {
                Ok(p) => p,
                Err(e) => {
                    tracing::warn!(url = %next_url, error = %e, "save failed");
                    skipped.push(SkippedPage {
                        url: key,
                        reason: SkipReason::ParseFailure,
                    });
                    continue;
                }
            };

            tracing::debug!(url = %next_url, bytes = size, "fetched");

            fetched.push(CrawledPage {
                url: next_url.to_string(),
                status: status.as_u16(),
                content_type: ct,
                bytes: size,
                html_path,
                fetched_at: chrono::Utc::now(),
            });

            // Discover same-host links and enqueue.
            for link in extract_links(&next_url, &html) {
                if link.host_str() == Some(&host) {
                    let s = link.to_string();
                    if visited.contains(&s) {
                        continue;
                    }
                    if discovered.insert(s.clone()) {
                        let score = score_link(&link);
                        frontier.push_back((link, score));
                    }
                }
            }
        }

        Ok(CrawlSummary {
            start_url: start_url.to_string(),
            host,
            fetched,
            skipped,
            discovered_links: discovered.into_iter().collect(),
            total_bytes,
            duration_ms: started.elapsed().as_millis() as u32,
        })
    }
}

/// Pop the highest-score entry from the frontier deque. O(n) per call, which
/// is fine for our budget (max_pages ≤ 25 in practice).
fn pop_highest(frontier: &mut VecDeque<(Url, i32)>) -> Option<(Url, i32)> {
    if frontier.is_empty() {
        return None;
    }
    let mut best_idx = 0usize;
    let mut best_score = i32::MIN;
    for (i, (_, s)) in frontier.iter().enumerate() {
        if *s > best_score {
            best_score = *s;
            best_idx = i;
        }
    }
    frontier.remove(best_idx)
}
