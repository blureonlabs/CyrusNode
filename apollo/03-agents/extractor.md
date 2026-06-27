# Extractor

> HTML → clean Markdown + structured facts. Deterministic, never calls an LLM.

## Question answered
What text and structured facts are on the pages we fetched?

## Kind
Deterministic.

## Inputs
```json
{ "company_id": "uuid", "crawl_artifact_id": "uuid" }
```

## Outputs
```json
{
  "pages": [
    {
      "url": "https://abcdental.ae/services",
      "title": "Our Services",
      "markdown": "...",
      "word_count": 412,
      "headings": ["..."],
      "meta": { "description": "...", "og:image": "..." }
    }
  ],
  "site_facts": {
    "language": "en",
    "currencies_seen": ["AED"],
    "phones": ["+9714..."],
    "emails": ["info@abcdental.ae"],
    "whatsapp_links": ["https://wa.me/9715..."],
    "social": {
      "instagram": "https://instagram.com/abcdental",
      "facebook": null,
      "linkedin": null,
      "tiktok": null
    },
    "booking_links": [],
    "schema_org_types": ["Dentist", "MedicalBusiness"]
  },
  "extraction_stats": {
    "total_words": 4810,
    "headless_used": false
  }
}
```

## Algorithm
1. **Readability-pass** with the `readability` crate to strip nav/footer noise.
2. **HTML → Markdown** with `html2md` (custom rules to preserve `<table>` and `<dl>`).
3. **Heuristic extractors**:
   - Phones via libphonenumber.
   - Emails via regex + MX validation (later).
   - WhatsApp links via `wa.me`, `whatsapp.com`, `api.whatsapp.com` patterns.
   - Social via known host prefixes.
   - Schema.org via `<script type="application/ld+json">` parsing.
4. **Empty-extraction guard**: if total words < 200 across all pages, emit `extraction.empty` so the saga can re-crawl with headless.

## Failure modes
- Malformed HTML → still attempts extraction; logs warning. Not a failure.
- Empty extraction → `extraction.empty` event, not a hard failure.

## Cost ceiling
< $0.001 per company. Zero LLM.

## Emits
- `extraction.completed`
- `extraction.empty`

## V1 acceptance
- On 50 random UAE SMB sites, ≥ 90% produce ≥ 500 total words. ≥ 80% surface at least one phone, email, or social link.

Links: [[crawler]] · [[business-intelligence]] · [[seo-auditor]]
