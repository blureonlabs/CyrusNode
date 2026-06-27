# SEO Auditor

> Lighthouse-style checks + a few extras tuned for SMB conversion.

## Question answered
How does this site perform technically and what's wrong with it that we could fix?

## Kind
Deterministic. The LLM never sees raw HTML — only the structured findings.

## Inputs
```json
{ "company_id": "uuid", "crawl_artifact_id": "uuid" }
```

## Outputs
```json
{
  "lighthouse": {
    "performance": 62,
    "accessibility": 71,
    "seo": 84,
    "best_practices": 78
  },
  "findings": [
    {
      "category": "mobile",
      "severity": "high",
      "title": "Mobile viewport not set",
      "evidence_url": "https://...",
      "evidence_excerpt": "<head>...</head>",
      "fix_hint": "Add <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">"
    }
  ],
  "missing_assets": {
    "favicon": false,
    "og_image": true,
    "robots_txt": false,
    "sitemap_xml": false,
    "schema_org_business": true,
    "ssl": false,
    "analytics": "none"
  }
}
```

## Checks (V1)
- Lighthouse (run via local Chromium + `chrome-launcher`-equivalent) for the four scores.
- Mobile viewport meta tag.
- Favicon present.
- Open Graph image present.
- robots.txt + sitemap.xml present.
- Schema.org `LocalBusiness` (or industry subtype) present.
- HTTPS + HSTS.
- Analytics: GA4, Plausible, GTM, etc.
- Cookie banner present (compliance flag).
- Contact page exists and has a real phone/email.
- WhatsApp click-to-chat present.
- Booking/booking-form present.

Each check writes a finding with severity (`info | low | medium | high`). The Opportunity Finder uses these as inputs.

## Failure modes
- Chromium can't start → terminal `seo.unavailable`. Operator alerted; pipeline continues to BizIntel using only `findings: []`.
- Lighthouse crash on a malformed page → returns partial findings, marks `lighthouse_partial: true`.

## Cost ceiling
< $0.002 per company (CPU + Chromium). No LLM.

## Emits
- `seo.completed`

## V1 acceptance
- Returns within 30s on warm Chromium.
- On the goldens (5 fixtures), severity classification is stable across runs.

Links: [[crawler]] · [[tech-detection]] · [[opportunity-finder]]
