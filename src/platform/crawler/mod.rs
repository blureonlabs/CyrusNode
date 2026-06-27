//! Polite HTTP crawler. Used by the `feature-research` adapter.
//!
//! Behavior matches apollo/03-agents/crawler.md:
//! - Respects robots.txt unless caller opts out.
//! - Per-host concurrency 1 + 500ms minimum delay.
//! - Sitemap-first link discovery, BFS fallback with link-scoring.
//! - Stops at the page/bytes budget.
//! - Saves raw HTML to a per-correlation directory under `output_root`.

mod client;
mod links;
mod robots;
mod sitemap;
mod storage;

pub use client::{
    CrawlSummary, CrawledPage, CrawlerConfig, CrawlerError, HttpCrawler, SkipReason, SkippedPage,
};
