//! Sitemap discovery and parsing.
//!
//! Best-effort: any failure (network, non-200, malformed XML) returns an empty
//! list, never an error. The BFS still has the start URL to fall back on.

use quick_xml::events::Event;
use quick_xml::Reader;
use reqwest::Client;
use url::Url;

/// Try `<host>/sitemap.xml` then `<host>/sitemap_index.xml`. Returns discovered
/// `<loc>` URLs.
///
/// Best-effort: any parse error or non-200 → empty `Vec`, not an error.
/// Returns `Result` for forward compatibility with explicit error reporting.
pub async fn try_fetch_sitemap(http: &Client, base: &Url) -> Result<Vec<Url>, ()> {
    let host = match base.host_str() {
        Some(h) => h,
        None => return Ok(Vec::new()),
    };

    for candidate in ["sitemap.xml", "sitemap_index.xml"] {
        let url_str = format!("{}://{}/{}", base.scheme(), host, candidate);
        let url = match Url::parse(&url_str) {
            Ok(u) => u,
            Err(_) => continue,
        };
        // SSRF guard — the host comes from `base` but we still validate every
        // outbound hit so the policy lives in one place.
        if super::safety::ensure_public_host(&url).await.is_err() {
            continue;
        }
        let resp = match http.get(url.clone()).send().await {
            Ok(r) => r,
            Err(_) => continue,
        };
        if !resp.status().is_success() {
            continue;
        }
        let body = match resp.text().await {
            Ok(b) => b,
            Err(_) => continue,
        };
        let urls = parse_sitemap_xml(&body);
        if !urls.is_empty() {
            return Ok(urls);
        }
    }
    Ok(Vec::new())
}

/// Extract `<loc>` URLs from a sitemap XML body.
///
/// Handles both `urlset` (sitemap) and `sitemapindex` (sitemap index) since
/// both encode URLs in `<loc>` tags. Tolerates malformed input: a parse error
/// terminates iteration and we return whatever we collected so far.
pub fn parse_sitemap_xml(body: &str) -> Vec<Url> {
    let mut reader = Reader::from_str(body);
    reader.config_mut().trim_text(true);
    let mut buf = Vec::new();
    let mut in_loc = false;
    let mut out: Vec<Url> = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) if e.name().as_ref() == b"loc" => in_loc = true,
            Ok(Event::End(ref e)) if e.name().as_ref() == b"loc" => in_loc = false,
            Ok(Event::Text(t)) if in_loc => {
                if let Ok(s) = t.unescape() {
                    if let Ok(u) = Url::parse(s.trim()) {
                        out.push(u);
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
        buf.clear();
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_simple_urlset() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
  <url><loc>https://example.com/</loc></url>
  <url><loc>https://example.com/about</loc></url>
</urlset>"#;
        let urls = parse_sitemap_xml(xml);
        assert_eq!(urls.len(), 2);
        assert_eq!(urls[0].as_str(), "https://example.com/");
        assert_eq!(urls[1].as_str(), "https://example.com/about");
    }

    #[test]
    fn parses_sitemap_index() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<sitemapindex xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
  <sitemap><loc>https://example.com/sitemap-1.xml</loc></sitemap>
  <sitemap><loc>https://example.com/sitemap-2.xml</loc></sitemap>
</sitemapindex>"#;
        let urls = parse_sitemap_xml(xml);
        assert_eq!(urls.len(), 2);
        assert!(urls[0].as_str().ends_with("sitemap-1.xml"));
    }

    #[test]
    fn tolerates_malformed_xml() {
        let xml = "<urlset><url><loc>https://example.com/</loc";
        let urls = parse_sitemap_xml(xml);
        // Should not panic. May or may not include the partial entry depending
        // on how far the parser got; the invariant is "no panic, returns Vec".
        let _ = urls;
    }

    #[test]
    fn skips_non_url_text() {
        let xml = r#"<urlset><url><loc>not-a-url</loc></url><url><loc>https://x.test/</loc></url></urlset>"#;
        let urls = parse_sitemap_xml(xml);
        assert_eq!(urls.len(), 1);
        assert_eq!(urls[0].as_str(), "https://x.test/");
    }
}
