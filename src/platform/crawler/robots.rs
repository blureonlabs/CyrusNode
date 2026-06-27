//! Per-host robots.txt cache built on `texting_robots`.
//!
//! Misses (no robots, non-200, parse error) are treated as "allow all": the
//! polite-but-permissive default expected by `apollo/03-agents/crawler.md` §1.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use reqwest::Client;
use texting_robots::Robot;
use url::Url;

/// Per-host robots cache. Fetches `/robots.txt` lazily on first use.
///
/// `texting_robots::Robot` is not guaranteed `Clone` across releases, so the
/// cache stores `Arc<Robot>` and we hand back clones of the `Arc`.
pub struct RobotsCache {
    inner: Mutex<HashMap<String, Option<Arc<Robot>>>>,
}

impl RobotsCache {
    /// Build an empty cache.
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(HashMap::new()),
        }
    }

    /// Returns true if `ua` is allowed to fetch `target`.
    ///
    /// Missing or unparseable robots → allow (permissive default).
    /// A poisoned mutex is treated as an empty cache for this call: we still
    /// answer, we just don't memoize.
    pub async fn allowed(&self, http: &Client, target: &Url, ua: &str) -> bool {
        let host = match target.host_str() {
            Some(h) => h.to_string(),
            None => return false,
        };

        // Fast path: already cached.
        if let Ok(map) = self.inner.lock() {
            if let Some(entry) = map.get(&host) {
                return match entry {
                    Some(r) => r.allowed(target.as_str()),
                    None => true,
                };
            }
        }

        // Slow path: fetch + parse, then store.
        let robot = Self::fetch(http, target, ua).await.map(Arc::new);

        if let Ok(mut map) = self.inner.lock() {
            map.entry(host).or_insert_with(|| robot.clone());
        }

        match robot {
            Some(r) => r.allowed(target.as_str()),
            None => true,
        }
    }

    async fn fetch(http: &Client, target: &Url, ua: &str) -> Option<Robot> {
        let host = target.host_str()?;
        let robots_url_str = format!("{}://{}/robots.txt", target.scheme(), host);
        // SSRF guard before talking to the network.
        let robots_url = Url::parse(&robots_url_str).ok()?;
        if super::safety::ensure_public_host(&robots_url)
            .await
            .is_err()
        {
            return None;
        }
        let resp = http.get(robots_url.clone()).send().await.ok()?;
        if !resp.status().is_success() {
            return None;
        }
        let body = resp.bytes().await.ok()?;
        Robot::new(ua, &body).ok()
    }
}

impl Default for RobotsCache {
    fn default() -> Self {
        Self::new()
    }
}
