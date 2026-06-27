# Social Intelligence (V2)

> Reads public social profiles to add posting cadence, content themes, and audience signals.

## Question answered
What is this business doing on social media, and how does that change our pitch?

## Kind
Deterministic crawl + LLM summary. Off in V1, on in V2.

## Inputs
```json
{
  "company_id": "uuid",
  "instagram_handle": "abcdental",
  "linkedin_url": "https://linkedin.com/company/abc-dental",
  "tiktok_handle": null
}
```

## Outputs
```json
{
  "instagram": {
    "follower_count": 4200,
    "post_frequency_per_week": 3.2,
    "engagement_rate_pct": 1.8,
    "content_themes": ["smile_makeovers", "patient_testimonials", "promotions"],
    "recent_posts_sample": ["..."]
  },
  "linkedin": {
    "employee_count_estimate": 22,
    "post_frequency_per_month": 1.0,
    "recent_activity_score": "low"
  },
  "ai_opportunity_signals": {
    "high_post_volume_no_dm_automation": true,
    "lots_of_dm_inquiries_likely": true,
    "ugc_potential": true
  }
}
```

## Sourcing
Instagram via the public profile + a small Apify actor (rate-limited).
LinkedIn via the company page (Phantombuster / Apify), respecting ToS — minimal scrape.

## V1 status
Stubbed. The pipeline runs without it. When V2 enables it, the Opportunity Finder receives an extra input field and re-runs goldens.

## Failure modes
- Handle wrong → terminal, `social.handle_invalid`, operator surfaces.
- Rate limit → transient, backoff 24h.

## Cost ceiling
$0.02 per company.

## Emits
- `social.completed`

Links: [[opportunity-finder]] · [[../05-knowledge/]]
