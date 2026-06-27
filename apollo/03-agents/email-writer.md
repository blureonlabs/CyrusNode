# Email Writer

> Cold email. ≤ 120 words. Specific. No clichés. Earns the reply or it doesn't ship.

## Question answered
What is the most likely-to-reply cold email for this prospect?

## Kind
LLM. Prompt: [[../04-prompts/email-writer]]. Gemini 2.5 Flash.

## Inputs
```json
{
  "company_id": "uuid",
  "bizintel_artifact_id": "uuid",
  "opportunities_artifact_id": "uuid",
  "roi_artifact_id": "uuid",
  "contact_name": "Ahmed",
  "industry_tone": "dental_clinic",  // selects tone subsection in playbook
  "operator_signature": "Hari"
}
```

## Outputs
```json
{
  "subject": "A quick note on your WhatsApp inquiries",
  "body": "Hi Ahmed,\n\nI was reading your services page while looking at Dubai dental clinics that emphasize same-day consults. Noticed your WhatsApp number sits on every page but there's no automation behind it — for clinics your size, after-hours messages from prospective patients are usually the leakiest funnel.\n\nWe built an AI receptionist that handles WhatsApp 24/7, qualifies the patient, and books into your calendar. Setup takes two weeks.\n\nWorth a 15-minute call this week to see if the numbers make sense for ABC Dental?\n\n— Hari",
  "personalization_anchors": [
    { "type": "service", "text": "same-day consults" },
    { "type": "gap",     "text": "WhatsApp without automation" }
  ],
  "compliance_check": {
    "length_ok": true,
    "no_banned_phrases": true,
    "has_anchors": true
  }
}
```

## Schema rules
- `body` word count ≤ 120 (hard).
- `body` MUST contain ≥ 2 personalization anchors. The output lists them explicitly; a deterministic checker verifies they appear in the body.
- **Banned phrases** (regex list in `apollo/05-knowledge/banned-email-phrases.txt`): "I hope this email finds you well", "10x your", "growth hack", "synergy", "I came across your business", "circle back". The list grows as we identify them.
- `subject` ≤ 7 words. No emojis (V1). No `[Action Required]`-style nonsense.

## Tone
The industry playbook injects a tone subsection ("formal but warm for dental", "punchy and metric-heavy for e-commerce", etc.). The prompt loader merges.

## Failure modes
- Banned phrase detected → one retry with the offending phrase named → terminal.
- Word count > 120 → one retry → terminal.
- Missing anchors → schema rejects.

## Cost ceiling
$0.006 per company.

## Emits
- `email.drafted`

## V1 acceptance
- Goldens (10 fixtures across 4 industries) all pass compliance.
- Operator approves ≥ 70% without edits on the first 30 real drafts.

Links: [[whatsapp-writer]] · [[proposal-writer]] · [[../04-prompts/email-writer]] · [[../05-knowledge/]]
