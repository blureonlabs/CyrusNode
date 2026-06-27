# Business Intelligence

> The first LLM agent in the pipeline. Turns extracted Markdown into a structured business summary.

## Question answered
What does this company actually sell, to whom, and how mature is the operation?

## Kind
LLM. Single prompt: [[../04-prompts/business-summary]].

## Inputs
```json
{
  "company_id": "uuid",
  "extraction_artifact_id": "uuid",
  "industry_hint": "dental_clinic" // optional, from discovery
}
```

The agent reads the extraction artifact and constructs a compact context (≤ 8k tokens) of: site_facts + top 8 pages by signal score + headings index. Pages are scored by keyword (services, about, pricing, contact).

## Outputs
```json
{
  "what_they_sell": "Dental services for adults and children, with emphasis on cosmetic dentistry (veneers, whitening) and Invisalign.",
  "who_they_serve": "Expat residents of Dubai aged 25–55, mid-to-upper income.",
  "value_props": [
    "Same-day consultations",
    "Bilingual (English / Arabic) staff",
    "Insurance accepted"
  ],
  "customer_segments": ["expat_adults", "family_pediatric", "cosmetic_clients"],
  "maturity_tier": "established",  // early | growing | established
  "headcount_estimate": "10-30",
  "online_presence": {
    "instagram_active": true,
    "google_reviews": "positive",
    "review_count_seen": 312
  },
  "evidence": [
    { "claim": "Cosmetic dentistry focus", "page_url": "https://...", "excerpt": "..." },
    { "claim": "Bilingual staff", "page_url": "https://...", "excerpt": "..." }
  ],
  "confidence": 0.86
}
```

## Schema rules
- `evidence[]` must have ≥ 3 items, each citing a page URL + excerpt. Output without evidence is rejected.
- `confidence` ∈ [0, 1]. If < 0.5 the saga marks the company `low_confidence` and the Opportunity Finder is more conservative.
- `value_props` ≤ 5 items, each ≤ 100 chars.

## Failure modes
- LLM returns invalid JSON → one retry with corrective system message.
- Extraction too sparse (< 200 words) → terminal `bizintel.insufficient_context`. Operator sees in dashboard.

## Cost ceiling
$0.01 per company. Gemini 2.5 Flash, max_output_tokens=1500.

## Emits
- `bizintel.completed`
- `bizintel.insufficient_context`

## V1 acceptance
- On 10 fixtures across 4 industries, every output has ≥ 3 evidence items and a non-null `what_they_sell`.

Links: [[extractor]] · [[opportunity-finder]] · [[../04-prompts/business-summary]]
