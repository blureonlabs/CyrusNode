# Crawler

> Fetches a company's site within a budget. Polite. Robots-respecting. JS-aware on fallback.

## Question answered
What pages exist on this site that a researcher should read?

## Kind
Deterministic.

## Inputs
```json
{
  "company_id": "uuid",
  "url": "https://abcdental.ae",
  "max_pages": 25,
  "max_bytes": 2097152,
  "respect_robots": true,
  "use_headless": false
}
```

## Outputs
```json
{
  "fetched": [
    {
      "url": "https://abcdental.ae/",
      "status": 200,
      "content_type": "text/html",
      "bytes": 87231,
      "html_path": "s3://...",
      "screenshot_path": "s3://...",
      "fetched_at": "..."
    }
  ],
  "skipped": [
    { "url": "...", "reason": "robots_disallowed" }
  ],
  "discovered_links": ["..."],
  "headers": { "...": "..." },
  "performance": {
    "ttfb_ms": 412,
    "total_ms": 1820
  }
}
```

## Behavior
1. Fetch `robots.txt`. If `respect_robots` and disallowed → emit `crawl.blocked`.
2. Fetch `/sitemap.xml`. Collect URLs.
3. Fetch homepage. Parse `<a href>`. Score links by depth + path keywords (about, services, pricing, contact, FAQ rank highest).
4. BFS up to `max_pages` or `max_bytes`, whichever first.
5. For each page: save raw HTML to object storage; take a screenshot via headless Chromium (only the homepage in V1 — others on demand).
6. If `extraction.empty` is later signalled, the saga re-enqueues with `use_headless: true`.

## Concurrency
Per host: 1 in-flight at a time, 500ms delay between fetches. Per worker: 8 hosts in parallel.

## Failure modes
- DNS failure → terminal with `crawl.unreachable`.
- 4xx everywhere → terminal with `crawl.unreachable`.
- Cloudflare/Bot block (403, JS challenge) → terminal with `crawl.blocked`. Operator can request `use_headless: true` replay.
- Robots disallow → terminal with `crawl.blocked`.

## Cost ceiling
$0.005 per company (S3 storage + minor CPU). No LLM.

## Emits
- `crawl.completed`
- `crawl.blocked` / `crawl.unreachable`

## V1 acceptance
- ≥ 95% of randomly chosen UAE SMB sites complete successfully without headless.
- p95 latency < 90s for max_pages=25.

Links: [[extractor]] · [[../02-architecture/overview]]
