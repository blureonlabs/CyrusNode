//! Markdown extractor adapter — implements `ExtractPort`.
//!
//! Reads every `*.html` file under `CrawlResult::output_dir`, strips noisy
//! tags (`<script>`, `<style>`), converts the surviving DOM to Markdown via
//! `html2md` (with a `scraper`-based fallback when the conversion is empty),
//! and harvests structured `SiteFacts` (phones, emails, WhatsApp links,
//! social-profile URLs, schema.org `@type` values from JSON-LD).
//!
//! Deterministic — never calls an LLM.

use std::collections::BTreeSet;
use std::path::Path;

use async_trait::async_trait;
use regex::Regex;
use scraper::{Html, Selector};
use tracing::{debug, warn};

use crate::features::research::domain::{
    CrawlResult, ExtractPort, ExtractedSite, SiteFacts, SocialLinks,
};
use crate::platform::core::CompanyId;

/// Maximum size of the concatenated Markdown body returned to downstream
/// agents. Keeps the BizIntel prompt context manageable.
const MAX_MARKDOWN_CHARS: usize = 50_000;

/// Minimum total word count below which we emit a warning (extraction
/// looks empty). We still return the partially-populated result so the
/// saga can decide whether to re-crawl with a headless browser.
const EMPTY_EXTRACTION_THRESHOLD: u32 = 200;

/// Separator inserted between per-page Markdown bodies when concatenating.
const PAGE_SEPARATOR: &str = "\n\n---\n\n";

/// Deterministic HTML → Markdown extractor.
///
/// Stateless — `MarkdownExtractorAdapter::new()` is a zero-arg constructor
/// and the adapter holds no fields. All regexes are compiled once per
/// `extract` call (cheap relative to disk + parsing).
pub struct MarkdownExtractorAdapter;

impl MarkdownExtractorAdapter {
    /// Build a new adapter. Stateless.
    pub fn new() -> Self {
        Self
    }
}

impl Default for MarkdownExtractorAdapter {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ExtractPort for MarkdownExtractorAdapter {
    /// Walk `crawl.output_dir`, convert each `*.html` file to Markdown,
    /// harvest site-wide structured facts, and return a single
    /// `ExtractedSite`.
    ///
    /// Failure modes:
    /// - The output directory cannot be read → propagates `anyhow::Error`.
    /// - Individual file read/parse errors are logged and skipped (a
    ///   single malformed page must not poison the whole site).
    /// - An empty extraction (`total_words < 200`) only logs a warning;
    ///   the partial result is still returned so the saga can decide.
    async fn extract(
        &self,
        _company_id: CompanyId,
        crawl: &CrawlResult,
    ) -> anyhow::Result<ExtractedSite> {
        let regexes = ExtractorRegexes::compile();
        let mut accumulator = SiteAccumulator::default();

        let html_paths = collect_html_paths(&crawl.output_dir)?;
        debug!(
            count = html_paths.len(),
            dir = %crawl.output_dir.display(),
            "extractor: scanning html files"
        );

        for path in html_paths {
            match std::fs::read_to_string(&path) {
                Ok(raw_html) => {
                    process_page(&raw_html, &regexes, &mut accumulator);
                }
                Err(err) => {
                    warn!(path = %path.display(), %err, "extractor: failed to read html");
                }
            }
        }

        let markdown_chunks = std::mem::take(&mut accumulator.markdown_chunks);
        let site_facts = accumulator.finalize_facts();
        let markdown = bound_markdown(markdown_chunks);
        let total_words = count_words(&markdown) as u32;

        if total_words < EMPTY_EXTRACTION_THRESHOLD {
            warn!(
                total_words,
                threshold = EMPTY_EXTRACTION_THRESHOLD,
                "extractor: empty extraction guard tripped — returning partial result"
            );
        }

        Ok(ExtractedSite {
            total_words,
            markdown,
            site_facts,
        })
    }
}

// ---------------------------------------------------------------------------
// Internals
// ---------------------------------------------------------------------------

/// Compiled regex set used by the extractor. Built once per `extract` call.
struct ExtractorRegexes {
    phone: Regex,
    email: Regex,
    whatsapp: Regex,
    cloudflare_email: Regex,
    instagram: Regex,
    facebook: Regex,
    linkedin: Regex,
    tiktok: Regex,
    youtube: Regex,
}

impl ExtractorRegexes {
    fn compile() -> Self {
        // Phone: international form with optional spaces / dashes / parens.
        // Permissive — downstream BizIntel can dedupe / canonicalize.
        let phone = Regex::new(r"\+?\d[\d\s().\-]{7,}\d").expect("static regex: phone");
        let email = Regex::new(r"(?i)\b[a-z0-9._%+\-]+@[a-z0-9.\-]+\.[a-z]{2,}\b")
            .expect("static regex: email");
        let whatsapp =
            Regex::new(r#"(?i)https?://(?:wa\.me|(?:api\.|chat\.)?whatsapp\.com)/[^\s"'<>]+"#)
                .expect("static regex: whatsapp");
        // Cloudflare email-protection link: `/cdn-cgi/l/email-protection#<hex>`.
        // The hex payload is XOR-encoded; see `decode_cloudflare_email`.
        let cloudflare_email = Regex::new(r#"(?i)/cdn-cgi/l/email-protection#([0-9a-f]+)"#)
            .expect("static regex: cloudflare_email");
        let instagram = Regex::new(r#"(?i)https?://(?:www\.)?instagram\.com/[^\s"'<>?#]+"#)
            .expect("static regex: instagram");
        let facebook = Regex::new(r#"(?i)https?://(?:www\.|m\.)?facebook\.com/[^\s"'<>?#]+"#)
            .expect("static regex: facebook");
        let linkedin = Regex::new(r#"(?i)https?://(?:www\.)?linkedin\.com/[^\s"'<>?#]+"#)
            .expect("static regex: linkedin");
        let tiktok = Regex::new(r#"(?i)https?://(?:www\.)?tiktok\.com/[^\s"'<>?#]+"#)
            .expect("static regex: tiktok");
        let youtube =
            Regex::new(r#"(?i)https?://(?:www\.)?(?:youtube\.com|youtu\.be)/[^\s"'<>?#]+"#)
                .expect("static regex: youtube");

        Self {
            phone,
            email,
            whatsapp,
            cloudflare_email,
            instagram,
            facebook,
            linkedin,
            tiktok,
            youtube,
        }
    }
}

/// Mutable site-wide harvest accumulator.
#[derive(Default)]
struct SiteAccumulator {
    markdown_chunks: Vec<String>,
    language: Option<String>,
    phones: BTreeSet<String>,
    emails: BTreeSet<String>,
    whatsapp_links: BTreeSet<String>,
    instagram: Option<String>,
    facebook: Option<String>,
    linkedin: Option<String>,
    tiktok: Option<String>,
    youtube: Option<String>,
    schema_org_types: BTreeSet<String>,
}

impl SiteAccumulator {
    fn finalize_facts(self) -> SiteFacts {
        SiteFacts {
            language: self.language,
            phones: self.phones.into_iter().collect(),
            emails: self.emails.into_iter().collect(),
            whatsapp_links: self.whatsapp_links.into_iter().collect(),
            social: SocialLinks {
                instagram: self.instagram,
                facebook: self.facebook,
                linkedin: self.linkedin,
                tiktok: self.tiktok,
                youtube: self.youtube,
            },
            schema_org_types: self.schema_org_types.into_iter().collect(),
        }
    }
}

/// Collect every `*.html` file directly under `dir` (sorted for stable
/// output across runs). Returns an empty vec if the directory does not
/// exist yet — that is not an error, it just means the crawler produced
/// no artifacts.
fn collect_html_paths(dir: &Path) -> anyhow::Result<Vec<std::path::PathBuf>> {
    if !dir.exists() {
        warn!(dir = %dir.display(), "extractor: output_dir missing — returning empty");
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_file()
            && path
                .extension()
                .and_then(|s| s.to_str())
                .map(|ext| ext.eq_ignore_ascii_case("html"))
                .unwrap_or(false)
        {
            out.push(path);
        }
    }
    out.sort();
    Ok(out)
}

/// Process one HTML page: strip script/style, convert to Markdown, and
/// harvest structured facts into the shared accumulator.
fn process_page(raw_html: &str, regexes: &ExtractorRegexes, acc: &mut SiteAccumulator) {
    let document = Html::parse_document(raw_html);

    // Capture <html lang="..."> the first time we see one.
    if acc.language.is_none() {
        if let Ok(html_sel) = Selector::parse("html[lang]") {
            if let Some(el) = document.select(&html_sel).next() {
                if let Some(lang) = el.value().attr("lang") {
                    let trimmed = lang.trim();
                    if !trimmed.is_empty() {
                        acc.language = Some(trimmed.to_string());
                    }
                }
            }
        }
    }

    // JSON-LD schema.org @type harvest BEFORE we strip <script>.
    if let Ok(ld_sel) = Selector::parse(r#"script[type="application/ld+json"]"#) {
        for script in document.select(&ld_sel) {
            let body = script.inner_html();
            harvest_schema_org_types(&body, &mut acc.schema_org_types);
        }
    }

    let cleaned_html = strip_noisy_tags(raw_html);

    // Try html2md first, fall back to a scraper heuristic if it yields
    // an empty (or whitespace-only) body.
    let mut markdown = html2md::parse_html(&cleaned_html);
    if markdown.trim().is_empty() {
        markdown = scraper_fallback_markdown(&cleaned_html);
    }
    let markdown = markdown.trim().to_string();
    if !markdown.is_empty() {
        acc.markdown_chunks.push(markdown);
    }

    // Harvest facts from the *original* HTML — links + attributes get
    // erased by the Markdown conversion.
    harvest_regex_facts(raw_html, regexes, acc);
}

/// Crude but safe `<script>` / `<style>` removal. We do this on the raw
/// HTML string before handing it to `html2md` — html2md occasionally
/// emits JS source verbatim otherwise.
fn strip_noisy_tags(raw_html: &str) -> String {
    // We use scraper to walk the parsed tree and rebuild without the
    // noisy elements. This is more robust than regex-stripping HTML.
    let document = Html::parse_document(raw_html);
    let body_sel = Selector::parse("body").expect("static selector: body");

    if let Some(body) = document.select(&body_sel).next() {
        let mut buf = String::with_capacity(raw_html.len());
        buf.push_str("<html><body>");
        push_clean_children(&body, &mut buf);
        buf.push_str("</body></html>");
        buf
    } else {
        // No <body> — just return the original; html2md handles fragments.
        raw_html.to_string()
    }
}

/// Recursively serialize an element's children to an HTML string,
/// skipping `<script>` and `<style>` subtrees.
fn push_clean_children(node: &scraper::ElementRef<'_>, buf: &mut String) {
    use scraper::node::Node;

    for child in node.children() {
        match child.value() {
            Node::Element(el) => {
                let tag = el.name();
                if tag.eq_ignore_ascii_case("script") || tag.eq_ignore_ascii_case("style") {
                    continue;
                }
                buf.push('<');
                buf.push_str(tag);
                for (name, value) in el.attrs() {
                    buf.push(' ');
                    buf.push_str(name);
                    buf.push_str("=\"");
                    buf.push_str(&html_escape_attr(value));
                    buf.push('"');
                }
                buf.push('>');
                if let Some(child_el) = scraper::ElementRef::wrap(child) {
                    push_clean_children(&child_el, buf);
                }
                buf.push_str("</");
                buf.push_str(tag);
                buf.push('>');
            }
            Node::Text(t) => {
                buf.push_str(&html_escape_text(&t.text));
            }
            _ => {}
        }
    }
}

fn html_escape_attr(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn html_escape_text(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// Fallback Markdown generator — walks `h1..h3, p, li` and emits a
/// minimal Markdown body. Used when `html2md` returns nothing useful.
fn scraper_fallback_markdown(html: &str) -> String {
    let document = Html::parse_document(html);
    let selector = Selector::parse("h1, h2, h3, p, li").expect("static selector: fallback");
    let mut out = String::new();
    for el in document.select(&selector) {
        let text = el.text().collect::<Vec<_>>().join(" ");
        let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
        if text.is_empty() {
            continue;
        }
        match el.value().name() {
            "h1" => {
                out.push_str("# ");
                out.push_str(&text);
                out.push_str("\n\n");
            }
            "h2" => {
                out.push_str("## ");
                out.push_str(&text);
                out.push_str("\n\n");
            }
            "h3" => {
                out.push_str("### ");
                out.push_str(&text);
                out.push_str("\n\n");
            }
            "li" => {
                out.push_str("- ");
                out.push_str(&text);
                out.push('\n');
            }
            _ => {
                out.push_str(&text);
                out.push_str("\n\n");
            }
        }
    }
    out
}

/// Walk a JSON-LD block and collect every `@type` (string or array)
/// found anywhere in the tree.
fn harvest_schema_org_types(body: &str, sink: &mut BTreeSet<String>) {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(body) else {
        debug!("extractor: skipping malformed JSON-LD block");
        return;
    };
    walk_json_for_types(&value, sink);
}

fn walk_json_for_types(value: &serde_json::Value, sink: &mut BTreeSet<String>) {
    match value {
        serde_json::Value::Object(map) => {
            if let Some(t) = map.get("@type") {
                match t {
                    serde_json::Value::String(s) => {
                        sink.insert(s.clone());
                    }
                    serde_json::Value::Array(items) => {
                        for item in items {
                            if let Some(s) = item.as_str() {
                                sink.insert(s.to_string());
                            }
                        }
                    }
                    _ => {}
                }
            }
            for (_k, v) in map {
                walk_json_for_types(v, sink);
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                walk_json_for_types(item, sink);
            }
        }
        _ => {}
    }
}

/// Regex-harvest contact + social facts from raw HTML. We also fold in
/// any `mailto:` / `tel:` href values that the regex might miss.
fn harvest_regex_facts(raw_html: &str, r: &ExtractorRegexes, acc: &mut SiteAccumulator) {
    for m in r.phone.find_iter(raw_html) {
        // Reject hits that are obviously not phones (too few digits).
        let digits = m.as_str().chars().filter(|c| c.is_ascii_digit()).count();
        if (8..=15).contains(&digits) {
            acc.phones.insert(normalize_phone(m.as_str()));
        }
    }

    for m in r.email.find_iter(raw_html) {
        if let Some(candidate) = clean_email_candidate(m.as_str()) {
            if is_valid_email_for_outreach(&candidate) {
                acc.emails.insert(candidate);
            }
        }
    }

    // Cloudflare email-protection: decode XOR-encoded `mailto:` payloads
    // that Cloudflare rewrites into `/cdn-cgi/l/email-protection#<hex>`.
    for cap in r.cloudflare_email.captures_iter(raw_html) {
        if let Some(hex) = cap.get(1) {
            if let Some(decoded) = decode_cloudflare_email(hex.as_str()) {
                if let Some(candidate) = clean_email_candidate(&decoded) {
                    if is_valid_email_for_outreach(&candidate) {
                        acc.emails.insert(candidate);
                    }
                }
            }
        }
    }

    for m in r.whatsapp.find_iter(raw_html) {
        let link = m.as_str();
        if is_actionable_whatsapp_link(link) {
            acc.whatsapp_links.insert(link.to_string());
        }
    }

    set_if_none(&mut acc.instagram, &r.instagram, raw_html);
    set_if_none(&mut acc.facebook, &r.facebook, raw_html);
    set_if_none(&mut acc.linkedin, &r.linkedin, raw_html);
    set_if_none(&mut acc.tiktok, &r.tiktok, raw_html);
    set_if_none(&mut acc.youtube, &r.youtube, raw_html);

    // Pick up `tel:` / `mailto:` hrefs explicitly — they're high-signal.
    let document = Html::parse_document(raw_html);
    if let Ok(a_sel) = Selector::parse("a[href]") {
        for el in document.select(&a_sel) {
            if let Some(href) = el.value().attr("href") {
                if let Some(rest) = href.strip_prefix("tel:") {
                    let decoded = decode_percent_space(rest);
                    let digits = decoded.chars().filter(|c| c.is_ascii_digit()).count();
                    if digits >= 7 {
                        acc.phones.insert(normalize_phone(&decoded));
                    }
                } else if let Some(rest) = href.strip_prefix("mailto:") {
                    let addr = rest.split('?').next().unwrap_or(rest);
                    if let Some(candidate) = clean_email_candidate(addr) {
                        if is_valid_email_for_outreach(&candidate) {
                            acc.emails.insert(candidate);
                        }
                    }
                } else if let Some(idx) = href
                    .to_ascii_lowercase()
                    .find("/cdn-cgi/l/email-protection#")
                {
                    let hex = &href[idx + "/cdn-cgi/l/email-protection#".len()..];
                    // Stop at the first non-hex char (some sites append query strings).
                    let hex_only: String =
                        hex.chars().take_while(|c| c.is_ascii_hexdigit()).collect();
                    if let Some(decoded) = decode_cloudflare_email(&hex_only) {
                        if let Some(candidate) = clean_email_candidate(&decoded) {
                            if is_valid_email_for_outreach(&candidate) {
                                acc.emails.insert(candidate);
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Decode a Cloudflare email-protection hex payload back into a plaintext
/// address.
///
/// Cloudflare's email obfuscation rewrites `<a href="mailto:foo@bar.com">`
/// into `<a href="/cdn-cgi/l/email-protection#<hex>">`, where `<hex>` is
/// the XOR-encoded address. The first byte is the XOR key; each
/// subsequent byte yields one character of the decoded email.
///
/// Returns `None` when:
/// - `hex` is shorter than 4 chars (no key + payload).
/// - `hex` has odd length or contains non-hex chars.
/// - Any decoded byte is not printable ASCII, `@`, or `.` — i.e. the
///   payload is almost certainly not an email.
pub(crate) fn decode_cloudflare_email(hex: &str) -> Option<String> {
    if hex.len() < 4 || hex.len() % 2 != 0 {
        return None;
    }
    if !hex.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let key = u8::from_str_radix(&hex[..2], 16).ok()?;
    let mut out = String::with_capacity((hex.len() - 2) / 2);
    let mut i = 2;
    while i < hex.len() {
        let byte = u8::from_str_radix(&hex[i..i + 2], 16).ok()? ^ key;
        let ch = byte as char;
        // Accept only printable ASCII + `@` + `.` (the latter two are
        // already covered by the printable range but listed explicitly
        // for intent).
        if !(ch.is_ascii_graphic() || ch == '@' || ch == '.') {
            return None;
        }
        out.push(ch);
        i += 2;
    }
    Some(out)
}

/// Trim trailing punctuation that often leaks in from prose
/// (`contact us at foo@bar.com,`) and lowercase the address so dedup
/// stays deterministic.
pub(crate) fn clean_email_candidate(raw: &str) -> Option<String> {
    let trimmed = raw.trim().trim_end_matches(|c: char| {
        matches!(c, ',' | ';' | '.' | ')' | '(' | '<' | '>' | '"' | '\'')
    });
    if trimmed.is_empty() {
        return None;
    }
    Some(trimmed.to_ascii_lowercase())
}

/// Reject emails that are structurally malformed, obviously placeholders,
/// or that point at static assets (file extensions leaking through).
///
/// Returns `true` only for addresses that are safe to feed into the
/// outreach pipeline. The check is intentionally conservative — false
/// negatives are cheap (one missed lead), false positives pollute the
/// dossier and cost downstream agent tokens.
pub(crate) fn is_valid_email_for_outreach(s: &str) -> bool {
    let len = s.len();
    if !(5..=254).contains(&len) {
        return false;
    }
    if s.contains("..") {
        return false;
    }
    if s.starts_with('.') || s.starts_with('-') {
        return false;
    }
    let mut parts = s.splitn(2, '@');
    let local = parts.next().unwrap_or("");
    let domain = parts.next().unwrap_or("");
    if local.is_empty() || domain.is_empty() {
        return false;
    }
    if local.len() > 64 {
        return false;
    }
    if !domain.contains('.') {
        return false;
    }
    // Domain ends with a static-asset extension — the regex picked up
    // something like `info@logo.png` from an image filename.
    const JUNK_TLDS: &[&str] = &[
        ".png", ".jpg", ".jpeg", ".gif", ".css", ".js", ".html", ".svg", ".webp",
    ];
    if JUNK_TLDS.iter().any(|ext| domain.ends_with(ext)) {
        return false;
    }
    // Common placeholders / template leftovers.
    const PLACEHOLDER_SUBSTRINGS: &[&str] = &[
        "example.com",
        "example.org",
        "example.net",
        "domain.com",
        "yourdomain",
        "youremail",
        "your-email",
        "your_email",
        "name@",
        "email@",
        "user@",
        "test@test",
        "sentry.io",
    ];
    if PLACEHOLDER_SUBSTRINGS.iter().any(|p| s.contains(p)) {
        return false;
    }
    true
}

/// WhatsApp share buttons (`whatsapp.com/share?text=...`) and the like
/// are not click-to-chat endpoints — they let the user share *the page*,
/// not contact the business. Keep only links that resolve to a number
/// (`wa.me/<digits>`) or an explicit send/chat surface.
pub(crate) fn is_actionable_whatsapp_link(link: &str) -> bool {
    let lower = link.to_ascii_lowercase();
    if let Some(idx) = lower.find("wa.me/") {
        let tail = &lower[idx + "wa.me/".len()..];
        let digits: String = tail.chars().take_while(|c| c.is_ascii_digit()).collect();
        return !digits.is_empty();
    }
    lower.contains("whatsapp.com/send") || lower.contains("whatsapp.com/chat")
}

/// Lightweight `%20` -> space decoder for `tel:%20+971...` style hrefs.
/// We intentionally do not pull in `urlencoding`: the only sequence we
/// see in the wild is the encoded space.
pub(crate) fn decode_percent_space(s: &str) -> String {
    s.replace("%20", " ")
}

fn set_if_none(slot: &mut Option<String>, re: &Regex, haystack: &str) {
    if slot.is_some() {
        return;
    }
    if let Some(m) = re.find(haystack) {
        *slot = Some(m.as_str().to_string());
    }
}

/// Collapse whitespace inside a phone hit so the same number written
/// `+971 4 123 4567` and `+97141234567` dedupe in the BTreeSet.
fn normalize_phone(raw: &str) -> String {
    raw.chars()
        .filter(|c| !c.is_whitespace())
        .collect::<String>()
        .trim_matches(|c: char| c == '.' || c == '-' || c == '(' || c == ')')
        .to_string()
}

/// Concatenate per-page Markdown bodies with the page separator and
/// truncate from the end if the total exceeds `MAX_MARKDOWN_CHARS`.
fn bound_markdown(chunks: Vec<String>) -> String {
    let joined = chunks.join(PAGE_SEPARATOR);
    if joined.len() <= MAX_MARKDOWN_CHARS {
        return joined;
    }
    // Truncate from the end at a char boundary.
    let mut end = MAX_MARKDOWN_CHARS;
    while end > 0 && !joined.is_char_boundary(end) {
        end -= 1;
    }
    joined[..end].to_string()
}

/// Approximate word count: whitespace-separated tokens, excluding
/// pure-punctuation tokens.
fn count_words(text: &str) -> usize {
    text.split_whitespace()
        .filter(|tok| tok.chars().any(|c| c.is_alphanumeric()))
        .count()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::core::CompanyId;
    use std::io::Write;

    fn write_fixture(dir: &Path, name: &str, html: &str) {
        let path = dir.join(name);
        let mut f = std::fs::File::create(&path).expect("create fixture");
        f.write_all(html.as_bytes()).expect("write fixture");
    }

    fn tmp_dir(label: &str) -> std::path::PathBuf {
        let base = std::env::temp_dir().join(format!(
            "apollo-extractor-test-{}-{}",
            label,
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&base).expect("mkdir tmp");
        base
    }

    #[tokio::test]
    async fn extracts_words_and_phone_from_rich_page() {
        let dir = tmp_dir("rich");
        let html = r#"<!doctype html>
<html lang="en">
  <head>
    <title>ABC Dental</title>
    <script>var x = 1;</script>
    <style>body { color: red; }</style>
    <script type="application/ld+json">
      {"@context":"https://schema.org","@type":"Dentist","name":"ABC"}
    </script>
  </head>
  <body>
    <h1>Welcome to ABC Dental</h1>
    <p>We are a family dental clinic in Dubai offering cleanings, crowns, and orthodontics.</p>
    <p>Call us at +971 4 123 4567 or email <a href="mailto:Info@ABCDental.ae">info@abcdental.ae</a>.</p>
    <a href="https://wa.me/971501234567">WhatsApp</a>
    <a href="https://instagram.com/abcdental">Insta</a>
  </body>
</html>"#;
        write_fixture(&dir, "home.html", html);

        let crawl = CrawlResult {
            pages_fetched: 1,
            output_dir: dir.clone(),
            fetched_urls: vec!["https://abcdental.ae/".to_string()],
        };

        let adapter = MarkdownExtractorAdapter::new();
        let result = adapter
            .extract(CompanyId(uuid::Uuid::new_v4()), &crawl)
            .await
            .expect("extract ok");

        assert!(
            result.total_words > 0,
            "expected non-zero word count, got {}",
            result.total_words
        );
        assert!(
            !result.site_facts.phones.is_empty(),
            "expected at least one phone, got {:?}",
            result.site_facts.phones
        );
        assert!(
            result
                .site_facts
                .emails
                .iter()
                .any(|e| e == "info@abcdental.ae"),
            "expected lowercased dedup'd email, got {:?}",
            result.site_facts.emails
        );
        assert!(
            !result.site_facts.whatsapp_links.is_empty(),
            "expected at least one whatsapp link, got {:?}",
            result.site_facts.whatsapp_links
        );
        assert_eq!(result.site_facts.language.as_deref(), Some("en"));
        assert!(
            result
                .site_facts
                .schema_org_types
                .iter()
                .any(|t| t == "Dentist"),
            "expected Dentist in schema_org_types, got {:?}",
            result.site_facts.schema_org_types
        );
        assert!(result.site_facts.social.instagram.is_some());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn empty_extraction_guard_returns_partial_result() {
        let dir = tmp_dir("empty");
        // A page with virtually no visible body text — only scripts/styles.
        let html = r#"<!doctype html>
<html><head><script>var a = 1;</script><style>p{}</style></head>
<body><script>doStuff();</script></body></html>"#;
        write_fixture(&dir, "empty.html", html);

        let crawl = CrawlResult {
            pages_fetched: 1,
            output_dir: dir.clone(),
            fetched_urls: vec!["https://example.com/".to_string()],
        };

        let adapter = MarkdownExtractorAdapter::new();
        let result = adapter
            .extract(CompanyId(uuid::Uuid::new_v4()), &crawl)
            .await
            .expect("extract still ok under empty guard");

        assert!(
            result.total_words < EMPTY_EXTRACTION_THRESHOLD,
            "fixture should trip the empty guard, got {} words",
            result.total_words
        );
        // The guard must NOT short-circuit: we still get a populated
        // (if mostly empty) ExtractedSite.
        assert!(result.site_facts.phones.is_empty());
        assert!(result.site_facts.emails.is_empty());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn bound_markdown_truncates_at_char_boundary() {
        let big = "a".repeat(MAX_MARKDOWN_CHARS + 1_000);
        let out = bound_markdown(vec![big]);
        assert!(out.len() <= MAX_MARKDOWN_CHARS);
        assert!(out.is_char_boundary(out.len()));
    }

    #[test]
    fn count_words_ignores_punctuation_tokens() {
        assert_eq!(count_words("hello, world!"), 2);
        assert_eq!(count_words("  --- "), 0);
    }

    #[test]
    fn decodes_cloudflare_email() {
        // Captured from a live Apollo run on didilimousine.com.
        let hex = "bcd5d2dad3fcd8d5d8d5d0d5d1d3c9cfd5d2d992dfd3d1";
        let decoded = decode_cloudflare_email(hex).expect("hex should decode");
        assert!(
            decoded.contains('@'),
            "decoded payload missing '@': {decoded:?}"
        );
        assert!(
            is_valid_email_for_outreach(&decoded),
            "decoded email should pass validation: {decoded:?}"
        );
    }

    #[test]
    fn rejects_obvious_placeholder_emails() {
        assert!(!is_valid_email_for_outreach("example@example.com"));
        assert!(!is_valid_email_for_outreach("your-email@domain.com"));
        assert!(!is_valid_email_for_outreach("name@example.com"));
    }

    #[test]
    fn rejects_emails_with_image_extensions() {
        assert!(!is_valid_email_for_outreach("info@logo.png"));
        assert!(!is_valid_email_for_outreach("contact@sprite.svg"));
    }

    #[test]
    fn whatsapp_filter_keeps_send_pattern() {
        assert!(is_actionable_whatsapp_link("https://wa.me/971501234567"));
        assert!(is_actionable_whatsapp_link(
            "https://api.whatsapp.com/send?phone=971501234567"
        ));
        assert!(!is_actionable_whatsapp_link(
            "https://whatsapp.com/share?text=foo"
        ));
    }

    #[test]
    fn cloudflare_decoder_rejects_garbage() {
        assert!(decode_cloudflare_email("ab").is_none(), "too short");
        assert!(decode_cloudflare_email("zzzzzz").is_none(), "non-hex");
        assert!(decode_cloudflare_email("abc").is_none(), "odd length");
    }
}
