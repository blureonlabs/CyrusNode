# ROI Estimator

> Conservative dollar estimates per opportunity, grounded in industry baselines.

## Question answered
What is each opportunity conservatively worth to this business, and what does it cost?

## Kind
LLM. Prompt: [[../04-prompts/roi-estimator]]. Batched: all opportunities for one company in one call.

## Inputs
```json
{
  "company_id": "uuid",
  "opportunities_artifact_id": "uuid",
  "business_intel_artifact_id": "uuid",
  "industry": "dental_clinic"
}
```

## Outputs
```json
{
  "estimates": [
    {
      "opportunity_id": "whatsapp-receptionist",
      "monthly_value_usd": { "low": 800, "expected": 2400, "high": 4800 },
      "one_time_setup_usd": 1500,
      "monthly_run_cost_usd": 80,
      "payback_months": 1.5,
      "assumptions": [
        "Clinic receives ~150 WhatsApp inquiries/month based on social and review volume.",
        "AI converts an additional 8–20 inquiries to booked appointments.",
        "Average appointment net revenue: $120."
      ],
      "confidence": 0.7
    }
  ]
}
```

## Schema rules
- Numbers bounded (`monthly_value_usd.expected <= 50_000` per opportunity for SMB pitches; alert above).
- `low <= expected <= high`.
- `assumptions[]` ≥ 2 and ≤ 5. Every assumption is a verifiable sentence, not "AI will be amazing."
- Estimates use **conservative** language ("based on industry baselines for X"). The prompt explicitly rewards conservatism on the goldens.

## Baselines
Industry baselines (avg appointment value, avg lead value, conversion uplift typical for each opportunity) live in `apollo/05-knowledge/industries/<industry>.md` under a `baselines` section. The prompt loader injects these.

## Failure modes
- Bounded-number violation → one retry → terminal.
- Missing baselines for industry → fallback to "global SMB" defaults with `confidence` capped at 0.4 and a warning event.

## Cost ceiling
$0.008 per company.

## Emits
- `roi.completed`

## V1 acceptance
- Estimates are never absurd (≤ $50k/month/opportunity for SMBs; flag otherwise).
- On 10 fixtures, ≥ 80% of estimates have ≥ 3 cited assumptions.

Links: [[opportunity-finder]] · [[proposal-writer]] · [[../04-prompts/roi-estimator]]
