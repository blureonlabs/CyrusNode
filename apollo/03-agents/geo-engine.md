# GEO Engine (V2)

> Generative Engine Optimization audit — how do AI engines describe this business, who do they cite, who outranks them?

## Question answered
For each top engine, when a prospect asks "best X in Y", what does the engine say — is the brand mentioned, in what context, with what sources cited, and how does it stack up against named competitors?

## Kind
Mixed. Deterministic query templates + LLM calls + answer parsing.

## Status
Spec only. Lives in `apollo/05-knowledge/ai-services.md` as `geo-audit`. The feature module lands in **Sprint 2** under `src/features/geo/` once the Opportunity Finder is in place (GEO is itself an opportunity it surfaces).

## Inputs
```json
{
  "company_id": "uuid",
  "business_name": "ABC Dental Clinic",
  "industry": "dental_clinic",
  "city": "Dubai",
  "competitors": ["..."],
  "query_count": 10
}
```

## Outputs
```json
{
  "engine_verdicts": [
    {
      "engine": "openai-gpt-4o",
      "query": "best dental clinic in Dubai for Invisalign",
      "mention": "listed_3rd",          // listed_1st | listed_2nd | listed_3rd | listed_other | not_mentioned
      "sentiment": "positive",
      "citations": [
        { "url": "https://...", "domain": "..." }
      ],
      "competitors_mentioned": ["..."]
    }
  ],
  "visibility_score": 0.42,              // 0..1
  "top_cited_sources": [{ "domain": "...", "count": 5 }],
  "competitor_co_occurrence": [{ "name": "...", "shared_queries": 7 }],
  "recommended_fixes": [
    "Add Dental Clinic schema.org markup",
    "Get listed on healthhub.ae",
    "Wikipedia page for the clinic group"
  ]
}
```

## Engines (V1 module)
- **OpenAI** (gpt-4o or gpt-4o-mini) — Chat Completions API.
- **Anthropic** (Haiku 4.5) — Messages API.
- **Perplexity** (sonar-pro) — has explicit citations, the easiest engine to mine.

## Engines deferred to V2
- **Google AI Overviews** — no public API; would require scraping Google SERP via a headless browser + SerpAPI. Fragile.
- **Gemini grounding** — Google has a grounded answers API; cost analysis needed before wiring.

## Query templates per industry
A `apollo/05-knowledge/geo-queries/<industry>.yaml` file defines 10 query templates with `{city}` and `{service}` placeholders. Example for `dental_clinic`:

```yaml
- "best dental clinic in {city} for Invisalign"
- "{city} dentist that accepts insurance"
- "cheapest cosmetic dentistry {city}"
- "dental clinic {city} open weekends"
- "top rated dentists in {city}"
- ...
```

## Cost target
≤ $0.50 per audit at V1 query count. Drops as cheaper engines come online.

## Failure modes
- Engine API key missing → skip that engine, note in report.
- Rate limit → backoff; partial report acceptable.
- Engine refuses to answer ("I can't recommend specific businesses") → record as `engine_refused`, surface to operator.

## Acceptance (V1)
- On a known clinic with strong Google reviews, GPT-4o mentions it in ≥ 3 of 10 queries.
- Operator can read the report in < 30 seconds and act on the top 3 fixes.

Links: [[../05-knowledge/ai-services]] · [[opportunity-finder]] · [[_overview]]
