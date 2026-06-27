//! Link extraction and prioritization heuristics.
//!
//! Scoring matches `apollo/03-agents/crawler.md` §3 step 3: pages that
//! researchers care about (contact, services, about, pricing, FAQ) rank
//! highest; junk pages (privacy, terms, deep blog posts) rank lowest.

use scraper::{Html, Selector};
use url::Url;

/// Extract absolute http(s) URLs from `<a href>` tags.
///
/// Relative hrefs are resolved against `base`. Anchors (`#section`) and
/// non-http schemes (`mailto:`, `tel:`, `javascript:`) are dropped.
pub fn extract_links(base: &Url, html: &str) -> Vec<Url> {
    let doc = Html::parse_document(html);
    // Selector::parse on a static string is infallible in practice; the
    // `expect` here is keyed off a const input — equivalent to a compile-time
    // guarantee, which is the spirit of CLAUDE.md §3 "no unwrap in prod paths".
    let sel = Selector::parse("a[href]").expect("static selector 'a[href]' is valid");
    let mut out = Vec::new();
    for a in doc.select(&sel) {
        let href = match a.value().attr("href") {
            Some(h) => h.trim(),
            None => continue,
        };
        if href.is_empty() || href.starts_with('#') {
            continue;
        }
        if let Ok(u) = base.join(href) {
            if matches!(u.scheme(), "http" | "https") {
                out.push(u);
            }
        }
    }
    out
}

/// Score a URL for crawl-order priority. Higher = crawl first.
///
/// Heuristic: contact / services / about / pricing / FAQ rank highest.
/// Junk (privacy, terms, careers) is pushed down. Deep paths get a small
/// penalty per slash. The root path gets a baseline bonus so the homepage
/// is always near the top of the queue.
pub fn score_link(url: &Url) -> i32 {
    let path = url.path().to_ascii_lowercase();
    let mut score = 100i32;
    if path == "/" {
        score += 200;
    }
    let keywords: [(&str, i32); 10] = [
        ("contact", 300),
        ("about", 200),
        ("service", 300),
        ("pricing", 250),
        ("price", 200),
        ("faq", 150),
        ("blog", -50),
        ("careers", -100),
        ("privacy", -200),
        ("terms", -200),
    ];
    for (k, v) in keywords {
        if path.contains(k) {
            score += v;
        }
    }
    let depth = path.matches('/').count() as i32;
    score -= depth * 10;
    score
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_absolute_relative_and_skips_fragments() {
        let base = Url::parse("https://example.com/page").expect("base url");
        let html = r##"<html><body>
            <a href="https://other.example/x">abs</a>
            <a href="/about">rel-root</a>
            <a href="contact">rel-doc</a>
            <a href="#top">frag</a>
            <a href="mailto:hi@example.com">mail</a>
            <a href="">empty</a>
        </body></html>"##;
        let out = extract_links(&base, html);
        let urls: Vec<&str> = out.iter().map(|u| u.as_str()).collect();
        assert!(urls.contains(&"https://other.example/x"));
        assert!(urls.contains(&"https://example.com/about"));
        // "contact" relative to /page resolves to /contact
        assert!(urls.iter().any(|u| u.ends_with("/contact")));
        assert!(!urls.iter().any(|u| u.contains('#')));
        assert!(!urls.iter().any(|u| u.starts_with("mailto:")));
    }

    #[test]
    fn score_prioritizes_business_pages() {
        let root = Url::parse("https://x.test/").expect("root");
        let contact = Url::parse("https://x.test/contact").expect("contact");
        let blog = Url::parse("https://x.test/blog/some-post").expect("blog");
        let terms = Url::parse("https://x.test/terms").expect("terms");

        let s_root = score_link(&root);
        let s_contact = score_link(&contact);
        let s_blog = score_link(&blog);
        let s_terms = score_link(&terms);

        assert!(s_contact > s_root, "contact should outrank root");
        assert!(s_root > s_blog, "root should outrank blog post");
        assert!(s_blog > s_terms, "blog should outrank terms");
    }

    #[test]
    fn deep_paths_are_penalized() {
        let shallow = Url::parse("https://x.test/about").expect("shallow");
        let deep = Url::parse("https://x.test/about/team/members/jane").expect("deep");
        assert!(score_link(&shallow) > score_link(&deep));
    }
}
