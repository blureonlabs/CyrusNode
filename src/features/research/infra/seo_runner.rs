//! SEO auditor adapter — implements `SeoPort`.
//!
//! S1-T10 V1: deterministic checks over crawled HTML. No Chromium, no LLM.
//! Reads every `*.html` file in `CrawlResult::output_dir`, runs the checks
//! enumerated in `apollo/03-agents/seo-auditor.md` §4, and emits a
//! [`SeoReport`] with discrete [`Finding`]s plus heuristic numeric scores.
//!
//! Lighthouse-backed `performance` and the robots.txt / sitemap.xml checks
//! land in V2 once we wire the headless Chromium runner.

use async_trait::async_trait;
use scraper::{Html, Selector};

use crate::features::research::domain::{CrawlResult, Finding, SeoPort, SeoReport, Severity};
use crate::platform::core::CompanyId;

/// Deterministic, Chromium-free SEO auditor.
///
/// V1 implementation of [`SeoPort`]. Reads the HTML pages saved by the
/// crawler under `crawl.output_dir` and produces a list of findings plus
/// rough `seo` / `performance` scores in 0..=100.
///
/// The struct is zero-sized; construct with [`StaticSeoAuditor::new`] (or
/// the unit struct literal `StaticSeoAuditor`).
pub struct StaticSeoAuditor;

impl StaticSeoAuditor {
    /// Creates a new auditor. Zero-arg, infallible — kept stable so
    /// `bootstrap.rs` can construct it without conditional plumbing.
    pub fn new() -> Self {
        Self
    }
}

impl Default for StaticSeoAuditor {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl SeoPort for StaticSeoAuditor {
    /// Audits the crawled site.
    ///
    /// Reads `*.html` files from `crawl.output_dir` (best-effort; unreadable
    /// files are skipped) and inspects `crawl.fetched_urls` for the HTTPS
    /// check. Returns an `SeoReport` whose `findings` list is empty iff every
    /// check passed; scores are derived from the finding severities.
    ///
    /// Never fails on missing files — an empty crawl yields a report full of
    /// "missing" findings, which is the right signal for downstream agents.
    async fn audit(
        &self,
        _company_id: CompanyId,
        crawl: &CrawlResult,
    ) -> anyhow::Result<SeoReport> {
        let pages = load_pages(&crawl.output_dir);
        let report = audit_pages(&pages, &crawl.fetched_urls);
        Ok(report)
    }
}

/// One parsed page's worth of state we hand to the checks.
struct Page {
    /// Raw HTML as fetched from disk. Kept for substring scans where parsing
    /// is overkill (analytics snippets, booking widget URLs, etc.).
    html: String,
}

/// Reads all `*.html` files directly inside `dir`. Subdirectories and
/// non-HTML files are ignored. Errors are swallowed per-file so a single
/// broken page does not poison the audit.
fn load_pages(dir: &std::path::Path) -> Vec<Page> {
    let mut pages = Vec::new();
    let read_dir = match std::fs::read_dir(dir) {
        Ok(rd) => rd,
        Err(_) => return pages,
    };
    for entry in read_dir.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let is_html = path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.eq_ignore_ascii_case("html") || e.eq_ignore_ascii_case("htm"))
            .unwrap_or(false);
        if !is_html {
            continue;
        }
        if let Ok(html) = std::fs::read_to_string(&path) {
            pages.push(Page { html });
        }
    }
    pages
}

/// Core check runner. Pure over its inputs so tests can call it directly
/// with synthetic HTML rather than touching the filesystem.
fn audit_pages(pages: &[Page], fetched_urls: &[String]) -> SeoReport {
    let mut findings: Vec<Finding> = Vec::new();

    // Parse each page once. `scraper::Html::parse_document` is infallible —
    // it returns a tree even on malformed input.
    let parsed: Vec<Html> = pages
        .iter()
        .map(|p| Html::parse_document(&p.html))
        .collect();

    // Combined lowercase haystack for cheap substring checks (analytics,
    // booking widgets, click-to-chat URLs). Lowercasing once is cheaper than
    // case-insensitive search at every call site.
    let haystack: String = pages
        .iter()
        .map(|p| p.html.to_lowercase())
        .collect::<Vec<_>>()
        .join("\n");

    // --- mobile viewport ---
    let viewport_sel = parse_selector("meta[name=\"viewport\"]");
    let has_viewport = viewport_sel
        .as_ref()
        .map(|sel| parsed.iter().any(|doc| doc.select(sel).next().is_some()))
        .unwrap_or(false);
    if !has_viewport {
        findings.push(Finding {
            category: "mobile".into(),
            severity: Severity::High,
            title: "Mobile viewport not set".into(),
            description: "No <meta name=\"viewport\"> tag found. Add \
                 <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\"> \
                 so the site renders correctly on phones."
                .into(),
        });
    }

    // --- favicon ---
    let favicon_icon = parse_selector("link[rel=\"icon\"]");
    let favicon_shortcut = parse_selector("link[rel=\"shortcut icon\"]");
    let has_favicon = parsed.iter().any(|doc| {
        let icon_hit = favicon_icon
            .as_ref()
            .map(|s| doc.select(s).next().is_some())
            .unwrap_or(false);
        let shortcut_hit = favicon_shortcut
            .as_ref()
            .map(|s| doc.select(s).next().is_some())
            .unwrap_or(false);
        icon_hit || shortcut_hit
    });
    if !has_favicon {
        findings.push(Finding {
            category: "branding".into(),
            severity: Severity::Low,
            title: "Favicon missing".into(),
            description: "No <link rel=\"icon\"> or <link rel=\"shortcut icon\"> declared. \
                 Add one so the site shows a recognisable tab icon."
                .into(),
        });
    }

    // --- og:image ---
    let og_sel = parse_selector("meta[property=\"og:image\"]");
    let has_og_image = og_sel
        .as_ref()
        .map(|sel| parsed.iter().any(|doc| doc.select(sel).next().is_some()))
        .unwrap_or(false);
    if !has_og_image {
        findings.push(Finding {
            category: "social".into(),
            severity: Severity::Medium,
            title: "Open Graph image missing".into(),
            description: "No <meta property=\"og:image\"> tag found. Without it, \
                 shared links on WhatsApp / Facebook / LinkedIn render \
                 without a preview image."
                .into(),
        });
    }

    // --- schema.org JSON-LD (loose check) ---
    let jsonld_sel = parse_selector("script[type=\"application/ld+json\"]");
    let has_schema_org = jsonld_sel
        .as_ref()
        .map(|sel| {
            parsed.iter().any(|doc| {
                doc.select(sel)
                    .any(|el| el.text().collect::<String>().contains("\"@type\""))
            })
        })
        .unwrap_or(false);
    if !has_schema_org {
        findings.push(Finding {
            category: "structured_data".into(),
            severity: Severity::Medium,
            title: "Schema.org structured data missing".into(),
            description: "No JSON-LD <script type=\"application/ld+json\"> with an \
                 \"@type\" field. Add a LocalBusiness (or industry subtype) \
                 block so Google can surface the business in rich results."
                .into(),
        });
    }

    // --- HTTPS across fetched URLs ---
    let all_https =
        !fetched_urls.is_empty() && fetched_urls.iter().all(|u| u.starts_with("https://"));
    if !all_https && !fetched_urls.is_empty() {
        findings.push(Finding {
            category: "security".into(),
            severity: Severity::High,
            title: "Site not fully served over HTTPS".into(),
            description: "One or more crawled URLs use http://. Browsers flag this \
                 as 'Not Secure' and search engines down-rank it. \
                 Provision an SSL certificate and force-redirect HTTP → HTTPS."
                .into(),
        });
    }

    // --- WhatsApp click-to-chat ---
    let has_whatsapp = haystack.contains("wa.me") || haystack.contains("whatsapp.com");
    if !has_whatsapp {
        findings.push(Finding {
            category: "conversion".into(),
            severity: Severity::Medium,
            title: "No WhatsApp click-to-chat link".into(),
            description: "No wa.me or whatsapp.com link found anywhere on the site. \
                 For SMBs in WhatsApp-heavy markets, this is the single \
                 biggest conversion lift available."
                .into(),
        });
    }

    // --- booking widget heuristic ---
    let booking_signals = ["calendly", "cal.com", "setmore", "appointment"];
    let has_booking = booking_signals.iter().any(|s| haystack.contains(s));
    if !has_booking {
        findings.push(Finding {
            category: "conversion".into(),
            severity: Severity::Medium,
            title: "No online booking widget detected".into(),
            description: "No Calendly / Cal.com / Setmore link and no 'appointment' \
                 keyword surfaced. For service businesses, embed a booking \
                 widget so visitors can self-serve outside business hours."
                .into(),
        });
    }

    // --- GA4 / analytics ---
    let has_analytics =
        haystack.contains("googletagmanager.com/gtag/js") || haystack.contains("gtag(");
    if !has_analytics {
        findings.push(Finding {
            category: "analytics".into(),
            severity: Severity::Low,
            title: "Analytics not detected".into(),
            description: "No GA4 / gtag snippet found. Without analytics there is no \
                 way to measure the impact of any future change."
                .into(),
        });
    }

    let (performance, seo) = scores(&findings);
    SeoReport {
        performance,
        seo,
        findings,
    }
}

/// Safely parses a CSS selector. Returns `None` on a malformed selector
/// rather than panicking — the selector strings here are static, so this is
/// effectively a `Some`, but the helper keeps the call sites free of
/// `unwrap()`.
fn parse_selector(s: &str) -> Option<Selector> {
    Selector::parse(s).ok()
}

/// Derives `(performance, seo)` from the finding list.
///
/// `seo` starts at 100 and pays a penalty per finding: 15 for `High`, 8 for
/// `Medium`, 3 for `Low`, 0 for `Info`. Floor at 0.
///
/// `performance` is a placeholder until we wire Lighthouse; for V1 it is
/// `seo - 10` (saturating at 0) to encode the "we haven't measured real
/// perf" caveat the spec calls out.
fn scores(findings: &[Finding]) -> (u8, u8) {
    let mut seo: i32 = 100;
    for f in findings {
        seo -= match f.severity {
            Severity::High => 15,
            Severity::Medium => 8,
            Severity::Low => 3,
            Severity::Info => 0,
        };
    }
    let seo = seo.max(0) as u8;
    let performance = seo.saturating_sub(10);
    (performance, seo)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn report_for(html: &str, urls: &[&str]) -> SeoReport {
        let pages = vec![Page {
            html: html.to_string(),
        }];
        let urls: Vec<String> = urls.iter().map(|s| s.to_string()).collect();
        audit_pages(&pages, &urls)
    }

    /// A site that ticks every box should produce zero findings and a
    /// perfect SEO score.
    #[test]
    fn passing_site_has_no_findings() {
        let html = r#"
            <!doctype html>
            <html>
              <head>
                <meta name="viewport" content="width=device-width, initial-scale=1">
                <link rel="icon" href="/favicon.ico">
                <meta property="og:image" content="https://example.com/og.png">
                <script type="application/ld+json">
                  { "@context": "https://schema.org", "@type": "Dentist", "name": "Sample" }
                </script>
                <script src="https://www.googletagmanager.com/gtag/js?id=G-XYZ"></script>
              </head>
              <body>
                <a href="https://wa.me/15551234567">Chat on WhatsApp</a>
                <a href="https://calendly.com/sample">Book appointment</a>
              </body>
            </html>
        "#;

        let report = report_for(html, &["https://example.com/"]);

        assert!(
            report.findings.is_empty(),
            "expected zero findings, got: {:?}",
            report.findings
        );
        assert_eq!(report.seo, 100);
        assert_eq!(report.performance, 90);
    }

    /// A bare-bones page with most signals missing should flag every check
    /// and drag the score down accordingly.
    #[test]
    fn empty_page_flags_multiple_issues() {
        let html = r#"<!doctype html><html><head><title>Bare</title></head><body>hi</body></html>"#;

        let report = report_for(html, &["http://example.com/"]);

        let categories: Vec<&str> = report
            .findings
            .iter()
            .map(|f| f.category.as_str())
            .collect();
        for expected in [
            "mobile",
            "branding",
            "social",
            "structured_data",
            "security",
            "conversion",
            "analytics",
        ] {
            assert!(
                categories.contains(&expected),
                "expected finding in category {expected}, got {categories:?}"
            );
        }

        // High penalties: mobile (15) + security (15) = 30.
        // Medium: og:image (8) + schema (8) + whatsapp (8) + booking (8) = 32.
        // Low: favicon (3) + analytics (3) = 6.
        // 100 - 30 - 32 - 6 = 32.
        assert_eq!(report.seo, 32);
        assert_eq!(report.performance, 22);
    }

    /// A site that's fully https with no anchors at all should still flag
    /// the conversion checks (no WhatsApp, no booking) but NOT the security
    /// finding.
    #[test]
    fn all_https_no_anchors_skips_security_finding() {
        let html = r#"
            <!doctype html>
            <html>
              <head>
                <meta name="viewport" content="width=device-width, initial-scale=1">
                <link rel="shortcut icon" href="/favicon.ico">
                <meta property="og:image" content="https://example.com/og.png">
                <script type="application/ld+json">
                  { "@type": "LocalBusiness" }
                </script>
                <script>gtag('config', 'G-XYZ');</script>
              </head>
              <body><p>Just some text.</p></body>
            </html>
        "#;

        let report = report_for(html, &["https://example.com/", "https://example.com/about"]);

        let titles: Vec<&str> = report.findings.iter().map(|f| f.title.as_str()).collect();
        assert!(
            !titles.iter().any(|t| t.contains("HTTPS")),
            "did not expect an HTTPS finding, got {titles:?}"
        );
        assert!(
            titles.iter().any(|t| t.contains("WhatsApp")),
            "expected a WhatsApp finding, got {titles:?}"
        );
        assert!(
            titles.iter().any(|t| t.contains("booking")),
            "expected a booking finding, got {titles:?}"
        );
        // Two medium findings only: 100 - 8 - 8 = 84.
        assert_eq!(report.seo, 84);
        assert_eq!(report.performance, 74);
    }

    /// `audit_pages` with no pages and no URLs must not panic and must not
    /// raise the HTTPS finding (we can't tell when `fetched_urls` is empty).
    #[test]
    fn empty_crawl_does_not_flag_https() {
        let report = audit_pages(&[], &[]);
        assert!(report.findings.iter().all(|f| !f.title.contains("HTTPS")));
    }
}
