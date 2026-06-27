# Lead Discovery Agent

> Input: a niche + location. Output: a list of candidate companies with at least a URL and a name.

## Question answered
What companies match this niche in this place?

## Kind
Deterministic (V1). LLM enrichment is a follow-up agent (Business Intelligence), not part of discovery.

## Inputs
```json
{
  "niche": "dental clinics",
  "city": "Dubai",
  "country": "AE",
  "limit": 50,
  "sources": ["gmaps", "linkedin", "directory"]  // V1: gmaps only
}
```

## Outputs
```json
{
  "candidates": [
    {
      "name": "ABC Dental Clinic",
      "url": "https://abcdental.ae",
      "phone": "+9714...",
      "whatsapp": "+9715...",
      "address": "...",
      "rating": 4.6,
      "review_count": 312,
      "source": "gmaps",
      "source_id": "ChIJ...",
      "confidence": 0.92
    }
  ]
}
```

## Source priority (V1)
1. **Google Maps Places API** (paid, low cost per call). Highest signal for SMBs in target markets.
2. **Yellow Pages UAE** scrape (deterministic).
3. **Dubai Chamber directory** scrape.

LinkedIn comes later (V4) via Apify or Phantombuster to stay within ToS.

## Failure modes
- API key missing → terminal, surfaced to operator.
- Quota exhausted → transient, backoff and retry tomorrow.
- Zero candidates → terminal with `discovery.empty` event so operator can refine the query.

## Cost ceiling
$0.10 per query (Google Places API).

## Emits
- `discovery.completed` with `candidates[]`.
- For each candidate, the saga enqueues `company.create_requested` (idempotent on host).

## V1 acceptance
- "dental clinics in Dubai", limit 50 → returns ≥ 50 candidates within 60s, ≥ 95% have a URL, ≥ 90% have a phone.
- Re-running the same query within 7 days returns the cached result with no API charge.

Links: [[../02-architecture/agent-architecture]] · [[crawler]] · [[../05-knowledge/industries/]] · [[_overview]]
