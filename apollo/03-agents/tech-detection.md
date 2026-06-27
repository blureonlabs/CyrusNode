# Tech Detection

> Wappalyzer-style stack identification from headers, scripts, and known fingerprints.

## Question answered
What tech stack does this site use?

## Kind
Deterministic. Rule-based.

## Inputs
```json
{ "company_id": "uuid", "crawl_artifact_id": "uuid" }
```

## Outputs
```json
{
  "stack": {
    "cms": ["WordPress 6.4"],
    "ecommerce": ["WooCommerce"],
    "analytics": ["GA4", "Meta Pixel"],
    "tag_managers": ["Google Tag Manager"],
    "ad_platforms": ["Meta Ads"],
    "marketing": ["Mailchimp"],
    "chat": [],
    "booking": [],
    "crm": [],
    "frontend_js": ["jQuery 3.7.1"],
    "hosting": ["Cloudflare"]
  },
  "signals": [
    { "name": "WordPress", "confidence": 0.99, "evidence": "X-Powered-By header, /wp-content/" }
  ],
  "ai_relevant_gaps": {
    "no_live_chat": true,
    "no_crm_detected": true,
    "no_booking_widget": true,
    "no_marketing_automation": true,
    "no_review_collection": true
  }
}
```

## How it works
A YAML ruleset (`apollo/05-knowledge/tech-rules.yaml`) defines patterns by category:

```yaml
- name: WordPress
  category: cms
  signals:
    - header: x-powered-by
      regex: "PHP/"
    - html: "wp-content/"
    - meta_generator: "WordPress"
```

The agent scans the homepage HTML + headers + script srcs + JSON-LD against rules. A match contributes confidence; total confidence > 0.5 emits the signal.

`ai_relevant_gaps` is a fixed list of categories Apollo cares about for pitching: live chat, CRM, booking, marketing automation, review/UGC collection. These directly feed the Opportunity Finder.

## Failure modes
- Empty HTML → returns empty stack + ai_relevant_gaps all true (assumes gaps).

## Cost ceiling
< $0.0005 per company. No LLM.

## Emits
- `tech.completed`

## V1 acceptance
- On 5 known fixtures, ≥ 90% of expected signals detected.
- Adding a new rule to the YAML requires zero code change.

Links: [[seo-auditor]] · [[opportunity-finder]] · [[../05-knowledge/]]
