# Meeting Coach

> Pre-call brief. Past wins, talking points, likely objections, suggested closing question.

## Question answered
How do we walk into this call already knowing what to say and what we'll hear?

## Kind
LLM with retrieval. Prompt: [[../04-prompts/meeting-coach]]. Uses Gemini 2.5 Pro.

## Inputs
```json
{
  "company_id": "uuid",
  "bizintel_artifact_id": "uuid",
  "opportunities_artifact_id": "uuid",
  "roi_artifact_id": "uuid",
  "conversation_id": "uuid",
  "meeting_at": "2026-07-02T14:00:00Z"
}
```

The agent embeds the (industry + chosen opportunities) and queries Qdrant collections `past_wins` and `objections` for top-5 each. It then constructs the brief.

## Outputs
```json
{
  "company_one_liner": "Established Dubai dental clinic, focus on cosmetic + Invisalign, strong WhatsApp presence with no automation.",
  "two_minute_warm_open": "Ahmed — appreciate the time. Saw you grew the Invisalign side over the last year; want to focus today on what happens around your WhatsApp inquiries...",
  "three_talking_points": [
    "Confirm the WhatsApp volume and the after-hours leak.",
    "Walk through the AI receptionist demo against their actual website tone.",
    "Show ROI math: if we convert 10 more inquiries/month, payback in 6 weeks."
  ],
  "likely_objections": [
    {
      "objection": "We tried a chatbot before and patients hated it.",
      "response": "Most off-the-shelf bots are linear. This is conversational, handles handoff to a human within 30 seconds if it can't answer, and is trained on your own services page. Want to see a 60-second handoff demo?"
    }
  ],
  "past_win_anchors": [
    {
      "anonymized_snippet": "Similar clinic in JLT — 18 extra appointments/month within 6 weeks.",
      "deal_id": "uuid"
    }
  ],
  "closing_question": "If we agreed on the success metric of N qualified appointments in 30 days, would that be enough to start with a 4-week pilot?"
}
```

## Retrieval
- `past_wins` — embed (industry + top opportunity ids); fetch top 5; filter by `won = true`; deduplicate by company.
- `objections` — embed (industry + opportunity ids); fetch top 5; filter by `worked = true`.

## Schema rules
- ≤ 3 talking points (hard).
- ≤ 3 likely objections (hard).
- `past_win_anchors[]` may be empty (cold start). When empty, the prompt instructs the LLM to omit that section in the rendered output rather than fabricate.

## Failure modes
- Empty Qdrant collections (cold start) → output omits anchors with `past_win_anchors: []`. Not a failure.
- LLM hallucinates a past win not in the retrieved set → schema rejects (each anchor must have a `deal_id` that exists).

## Cost ceiling
$0.03 per meeting. Gemini 2.5 Pro.

## Emits
- `meeting.brief_ready`

## V1 acceptance
- Brief generates in ≤ 10s.
- On 5 staged meetings, the operator rates ≥ 4/5 talking points as "I would have said this anyway."

Links: [[proposal-writer]] · [[../04-prompts/meeting-coach]] · [[../02-architecture/database]]
