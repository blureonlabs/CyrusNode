# Opportunity Finder

> Maps detected problems and stack gaps to a ranked list of AI services from the catalogue.

## Question answered
Which AI services would move the needle for *this* company specifically?

## Kind
LLM. Prompt: [[../04-prompts/opportunity-finder]]. Retrieval: pulls relevant entries from [[../05-knowledge/ai-services]] and the industry playbook by `industry`.

## Inputs
```json
{
  "company_id": "uuid",
  "business_intel_artifact_id": "uuid",
  "seo_artifact_id": "uuid",
  "tech_artifact_id": "uuid",
  "social_artifact_id": null,
  "industry": "dental_clinic"
}
```

The agent loads the relevant industry playbook (`apollo/05-knowledge/industries/dental_clinic.md`) and the AI services catalogue, then builds a compact context. It asks the LLM to choose only services for which there is direct evidence in the inputs.

## Outputs
```json
{
  "opportunities": [
    {
      "id": "whatsapp-receptionist",
      "name": "WhatsApp AI Receptionist",
      "headline": "Answer after-hours WhatsApp inquiries and book appointments automatically.",
      "evidence": [
        { "source": "seo", "claim": "WhatsApp click-to-chat present but no chatbot detected" },
        { "source": "bizintel", "claim": "Same-day consultations is a value prop" }
      ],
      "effort_tier": "low",          // low | medium | high
      "time_to_value_days": 14,
      "priority_score": 0.92,        // 0..1, used to sort
      "confidence": 0.88
    }
  ],
  "rejected_candidates": [
    {
      "id": "voice-receptionist",
      "reason": "No phone-volume evidence; site emphasizes WhatsApp."
    }
  ]
}
```

## Schema rules
- Between 3 and 7 opportunities.
- Each opportunity MUST cite ≥ 2 evidence items, each referencing a real input (`seo` | `tech` | `bizintel` | `social`).
- `id` must exist in the AI services catalogue.
- `priority_score` is the sort key.
- `rejected_candidates[]` is required (it forces the LLM to consider and discard, which improves selection quality on goldens).

## Failure modes
- Output cites an unknown service `id` → schema rejects; one retry; then terminal.
- No opportunity has 2 evidence items → schema rejects.

## Cost ceiling
$0.012 per company. Flash, max_output_tokens=2000.

## Emits
- `opportunities.generated`

## V1 acceptance
- On 10 fixtures, every output has ≥ 3 opportunities, ≥ 2 evidence each, and at least one service that maps to a stack gap from Tech Detection.

Links: [[business-intelligence]] · [[roi-estimator]] · [[../04-prompts/opportunity-finder]] · [[../05-knowledge/ai-services]]
