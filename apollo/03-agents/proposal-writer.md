# Proposal Writer

> One-page proposal. Looks like a consulting deliverable, not chatbot output.

## Question answered
How do we present this to the prospect as a one-page, branded proposal?

## Kind
LLM. Prompt: [[../04-prompts/proposal-writer]]. Uses Gemini 2.5 Pro (only agent + Meeting Coach that does).

## Inputs
```json
{
  "company_id": "uuid",
  "bizintel_artifact_id": "uuid",
  "opportunities_artifact_id": "uuid",
  "roi_artifact_id": "uuid",
  "focus_opportunity_ids": ["whatsapp-receptionist", "review-automation"]
}
```

`focus_opportunity_ids` defaults to the top 2 by `priority_score`. Operator can override.

## Outputs
```json
{
  "title": "AI Receptionist for ABC Dental Clinic",
  "executive_summary": "...",        // ≤ 80 words
  "the_problem": "...",              // ≤ 120 words, names specific friction
  "what_we_propose": "...",          // ≤ 160 words, concrete deliverables
  "expected_outcome": "...",         // ≤ 120 words, references ROI estimate
  "timeline": [
    { "week": 1, "milestone": "Discovery + content audit" },
    { "week": 2, "milestone": "WhatsApp agent v1 in test number" },
    { "week": 3, "milestone": "Launch to live number + monitoring" }
  ],
  "investment": {
    "one_time_usd": 1500,
    "monthly_usd": 80,
    "note": "Stripe / bank transfer in AED at prevailing rate."
  },
  "next_step": "30-minute discovery call this week."
}
```

## Rendering
Output is structured JSON. A separate (deterministic) renderer turns it into a branded PDF using a Tera/Handlebars template under `apollo/05-knowledge/proposal-template/`. The PDF is stored as an artifact; the JSON is the canonical proposal.

## Schema rules
- Word caps enforced.
- `the_problem` MUST reference at least one piece of evidence by quoting a phrase from BizIntel or SEO findings.
- `expected_outcome` MUST reference the ROI estimate's `expected` value range.

## Failure modes
- Word cap violation → one retry → terminal.
- Citation missing → schema rejects, one retry → terminal.

## Cost ceiling
$0.04 per company. Gemini 2.5 Pro, max_output_tokens=3000.

## Emits
- `proposal.drafted`

## V1 acceptance
- PDF renders in < 5s.
- On 10 fixtures, blind reviewers (operator) rank ≥ 7/10 as "looks like a consulting proposal."

Links: [[opportunity-finder]] · [[roi-estimator]] · [[../04-prompts/proposal-writer]]
